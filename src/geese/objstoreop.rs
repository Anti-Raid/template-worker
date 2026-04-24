use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use khronos_runtime::{primitives::blob::{BlobTaker, Blob}, core::datetime::{DateTime as LuaDateTime, TimeDelta as LuaTimeDelta}, rt::mluau::prelude::*};
use serde::{Deserialize, Serialize};
use crate::{geese::objectstore::{Bucket, BucketWithKey, BucketWithPrefix, ObjectStore}, worker::{limits::{MAX_OBJ_STORAGE_BYTES, MAX_OBJ_STORAGE_PATH_LENGTH}, workervmmanager::Id}};


/// The core underlying syscall
#[derive(Debug, Serialize, Deserialize)]
pub enum ObjectStorageCall {
    ListFileMetas {
        prefix: Option<String>
    },
    GetFileMeta {
        key: String
    },
    GetFileUrl {
        key: String,
        expiry: Duration
    },
    DownloadFile {
        key: String
    },
    UploadFile {
        key: String,
        data: serde_bytes::ByteBuf
    },
    DeleteFile {
        key: String
    }
}

impl FromLua for ObjectStorageCall {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "ObjectStorageCall".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"ListFileMetas" => {
                let prefix = tab.get("prefix")?;
                Ok(ObjectStorageCall::ListFileMetas { prefix })
            },
            b"GetFileMeta" => {
                let key = tab.get("key")?;
                Ok(ObjectStorageCall::GetFileMeta { key })
            },
            b"GetFileUrl" => {
                let key = tab.get("key")?;
                let expiry = tab.get::<LuaUserDataRef<LuaTimeDelta>>("expiry")?;
                Ok(ObjectStorageCall::GetFileUrl { key, expiry: expiry.timedelta.to_std().map_err(LuaError::external)? })
            },
            b"DownloadFile" => {
                let key = tab.get("key")?;
                Ok(ObjectStorageCall::DownloadFile { key })
            }
            b"UploadFile" => {
                let key = tab.get("key")?;
                let data = tab.get::<BlobTaker>("data")?;
                Ok(ObjectStorageCall::UploadFile { key, data: data.0.into() })
            }
            b"DeleteFile" => {
                let key = tab.get("key")?;
                Ok(ObjectStorageCall::DeleteFile { key })
            }
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "ObjectStorageCall".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum ObjectStorageResult {
    ObjectMetadata {
        objs: Vec<ObjectMetadata>
    },
    FileUrl {
        url: String
    },
    Blob {
        data: Vec<u8>
    },
    Ack,
}

impl IntoLua for ObjectStorageResult {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        match self {
            Self::ObjectMetadata { objs } => {
                table.set("op", "ObjectMetadata")?;
                table.set("objs", objs)?;
            }
            Self::FileUrl { url } => {
                table.set("op", "FileUrl")?;
                table.set("url", url)?;
            }
            Self::Blob { data } => {
                table.set("op", "Blob")?;
                table.set("data", Blob { data })?;
            }
            Self::Ack => {
                table.set("op", "Ack")?;
            }
        }
        table.set_readonly(true); // We want StateExecResult's to be immutable
        Ok(LuaValue::Table(table))
    }
}

#[derive(Serialize, Deserialize)]
pub struct ObjectMetadata {
    pub key: String,
    pub last_modified: Option<DateTime<Utc>>,
    pub size: i64,
    pub etag: Option<String>,
}

impl IntoLua for ObjectMetadata {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        table.set("key", self.key)?;
        table.set("last_modified", self.last_modified.map(|dt| LuaDateTime::from_utc(dt)))?;
        table.set("size", self.size)?;
        table.set("etag", self.etag)?;
        table.set_readonly(true);
        Ok(LuaValue::Table(table))
    }
}

#[derive(Clone)]
/// A simple wrapper around the object storage that provides luau object storage manipulation functionality
pub struct ObjStorageOp {
    client: Arc<ObjectStore>
}

impl ObjStorageOp {
    pub fn new(client: Arc<ObjectStore>) -> Self {
        Self { client }
    }

    pub async fn do_op(&self, id: Id, op: ObjectStorageCall) -> Result<ObjectStorageResult, crate::Error> {
        let bucket = Bucket::from_id(id);
        match op {
            ObjectStorageCall::ListFileMetas { prefix } => {
                let objs = self.client.list_files(BucketWithPrefix::new(bucket, prefix.as_deref()))
                .await?
                .into_iter()
                .map(|x| ObjectMetadata {
                    key: x.key,
                    last_modified: x.last_modified,
                    size: x.size,
                    etag: x.etag,
                })
                .collect::<Vec<_>>();

                Ok(ObjectStorageResult::ObjectMetadata { objs })
            }
            ObjectStorageCall::GetFileMeta { key } => {
                let mut objs = vec![];
                let obj = self.client.get_object_metadata(BucketWithKey::new(bucket, &key)).await?;
                if let Some(obj) = obj {
                    objs.push(ObjectMetadata {
                        key: obj.key,
                        last_modified: obj.last_modified,
                        size: obj.size,
                        etag: obj.etag,
                    });
                }

                Ok(ObjectStorageResult::ObjectMetadata { objs })
            }
            ObjectStorageCall::GetFileUrl { key, expiry } => {
                let url = self.client.get_url(BucketWithKey::new(bucket, &key), expiry).await?;
                Ok(ObjectStorageResult::FileUrl { url })
            }
            ObjectStorageCall::DownloadFile { key } => {
                let data = self.client.download_file(BucketWithKey::new(bucket, &key)).await?;
                Ok(ObjectStorageResult::Blob { data })
            }
            ObjectStorageCall::UploadFile { key, data } => {
                if key.len() > MAX_OBJ_STORAGE_PATH_LENGTH {
                    return Err("Path length too long".into());
                }

                if data.len() > MAX_OBJ_STORAGE_BYTES {
                    return Err("Data too large".into());
                }

                self.client.upload_file(BucketWithKey::new(bucket, &key), data.into_vec()).await?;

                Ok(ObjectStorageResult::Ack)
            }
            ObjectStorageCall::DeleteFile { key } => {
                self.client.delete(BucketWithKey::new(bucket, &key)).await?;
                Ok(ObjectStorageResult::Ack)
            }
        }
    }
}