use serenity::all::{GuildId, UserId};

use crate::worker::workervmmanager::Id;

const CHUNK_SIZE: usize = 5 * 1024 * 1024;
const MULTIPART_MIN_SIZE: usize = 50 * 1024 * 1024;

/// Simple abstraction around object storages
pub enum ObjectStore {
    S3 {
        client: aws_sdk_s3::Client,
        cdn_client: aws_sdk_s3::Client,
        cdn_endpoint: String,
    },
    Local {
        dir: String,
    },
}

impl ObjectStore {
    pub fn new_s3(
        app_name: String,
        endpoint: String,
        cdn_endpoint: String,
        key: String,
        secret: String,
    ) -> Result<Self, crate::Error> {
        let client = aws_sdk_s3::Client::from_conf(
            aws_sdk_s3::Config::builder()
                .credentials_provider(aws_sdk_s3::config::Credentials::new(
                    key.clone(),
                    secret.clone(),
                    None,
                    None,
                    "s3",
                ))
                .app_name(aws_sdk_s3::config::AppName::new(app_name.clone())?)
                .region(aws_sdk_s3::config::Region::new("us-east-1"))
                .endpoint_url(endpoint.clone())
                .force_path_style(true)
                .behavior_version_latest()
                .build(),
        );

        let cdn_client = if cdn_endpoint.starts_with("$DOCKER:") {
            aws_sdk_s3::Client::from_conf(
                aws_sdk_s3::Config::builder()
                    .credentials_provider(aws_sdk_s3::config::Credentials::new(
                        key.clone(),
                        secret.clone(),
                        None,
                        None,
                        "s3",
                    ))
                    .app_name(aws_sdk_s3::config::AppName::new(app_name)?)
                    .region(aws_sdk_s3::config::Region::new("us-east-1"))
                    .endpoint_url(endpoint.clone())
                    .force_path_style(true)
                    .behavior_version_latest()
                    .build(),
            )
        } else {
            aws_sdk_s3::Client::from_conf(
                aws_sdk_s3::Config::builder()
                    .credentials_provider(aws_sdk_s3::config::Credentials::new(
                        key.clone(),
                        secret.clone(),
                        None,
                        None,
                        "s3",
                    ))
                    .app_name(aws_sdk_s3::config::AppName::new(app_name)?)
                    .region(aws_sdk_s3::config::Region::new("us-east-1"))
                    .endpoint_url(cdn_endpoint.clone())
                    .force_path_style(true)
                    .behavior_version_latest()
                    .build(),
            )
        };

        Ok(ObjectStore::S3 {
            client,
            cdn_client,
            cdn_endpoint,
        })
    }

    pub fn new_local(dir: String) -> Self {
        ObjectStore::Local { dir }
    }
}

pub struct ListObjectsResponse {
    pub key: String,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    pub size: i64,
    pub etag: Option<String>,
}

/// Represents a bucket in the object store
#[derive(Clone, Copy, Debug)]
pub enum Bucket {
    Guild(GuildId),
    User(UserId),
}

impl Bucket {
    pub fn from_id(id: Id) -> Self {
        match id {
            Id::Guild(guild_id) => Bucket::Guild(guild_id),
            Id::User(user_id) => Bucket::User(user_id),
        }
    }

    pub fn bucket(&self) -> &'static str {
        match self {
            Bucket::Guild(_) => "antiraid.guilds",
            Bucket::User(_) => "antiraid.users",
        }
    }

    pub fn prefix(&self) -> String {
        match self {
            Bucket::Guild(guild_id) => format!("{}", guild_id),
            Bucket::User(user_id) => format!("{}", user_id),
        }
    }

    /// Returns the full key for the given object in the bucket
    /// 
    /// Errors if the key is invalid (contains "../")
    pub fn key(&self, key: &str) -> Result<String, crate::Error> {
        for segment in key.split('/') {
            match segment {
                ".." => return Err("Invalid key: potential path traversal attempt".into()),
                "." => return Err("Invalid key: current directory reference".into()),
                "" => return Err("Invalid key: empty path segment".into()),
                _ => {}
            }
        }
        Ok(format!("{}/{}", self.prefix(), key))
    }
}

pub struct BucketWithKey<'a> {
    bucket: Bucket,
    key: &'a str,
}

impl<'a> std::ops::Deref for BucketWithKey<'a> {
    type Target = Bucket;

    fn deref(&self) -> &Self::Target {
        &self.bucket
    }
}

impl<'a> BucketWithKey<'a> {
    pub fn new(bucket: Bucket, key: &'a str) -> Self {
        Self { bucket, key }
    }

    pub fn key(&self) -> Result<String, crate::Error> {
        self.bucket.key(self.key)
    }
}

pub struct BucketWithPrefix<'a> {
    bucket: Bucket,
    key: Option<&'a str>,
}

impl<'a> std::ops::Deref for BucketWithPrefix<'a> {
    type Target = Bucket;

    fn deref(&self) -> &Self::Target {
        &self.bucket
    }
}

impl<'a> BucketWithPrefix<'a> {
    pub fn new(bucket: Bucket, key: Option<&'a str>) -> Self {
        Self { bucket, key }
    }

    pub fn prefix(&self) -> Result<String, crate::Error> {
        match self.key {
            Some(key) => Ok(self.bucket.key(key)?),
            None => Ok(self.bucket.prefix()),
        }
    }
}

impl ObjectStore {
    /// Returns if a file exists in the object store
    pub async fn exists(&self, b: BucketWithKey<'_>) -> Result<bool, crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                let action = client.head_object().bucket(b.bucket()).key(b.key()?);

                match action.send().await {
                    Ok(_) => Ok(true),
                    Err(e) => {
                        let Some(e) = e.as_service_error() else {
                            return Err(format!("Failed to list objects: {}", e).into());
                        };

                        if e.is_not_found() {
                            Ok(false)
                        } else {
                            Err(format!("Failed to list objects: {}", e).into())
                        }
                    }
                }
            }
            ObjectStore::Local { dir } => {
                let path = std::path::Path::new(dir).join(b.bucket()).join(b.key()?);
                Ok(path.exists())
            }
        }
    }

    /// Note that duration is only supported for S3
    ///
    /// On S3, this returns a presigned URL, on local, it returns a file:// url
    pub async fn get_url(
        &self,
        b: BucketWithKey<'_>,
        duration: std::time::Duration,
    ) -> Result<String, crate::Error> {
        match self {
            ObjectStore::S3 {
                cdn_client,
                cdn_endpoint,
                ..
            } => {
                let url = cdn_client
                    .get_object()
                    .bucket(b.bucket())
                    .key(b.key()?)
                    .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
                        duration,
                    )?)
                    .await
                    .map_err(|e| format!("failed to get presigned url: {e:?}"))?;

                let url = url.uri();

                /*
                    if strings.HasPrefix(o.c.CdnEndpoint, "$DOCKER:") {
                        p.Scheme = "http"
                        p.Host = strings.TrimPrefix(o.c.CdnEndpoint, "$DOCKER:")
                    }
                */
                let url = if cdn_endpoint.starts_with("$DOCKER:") {
                    let mut parsed_url = reqwest::Url::parse(url)?;
                    parsed_url.set_host(Some(cdn_endpoint.trim_start_matches("$DOCKER:")))?;
                    parsed_url
                        .set_scheme("http")
                        .map_err(|_| "Failed to set new scheme")?;
                    parsed_url.to_string()
                } else {
                    url.to_string()
                };

                Ok(url)
            }
            ObjectStore::Local { dir } => Ok(format!("file://{}/{}/{}", dir, b.bucket(), b.key()?)),
        }
    }

    /// Lists all files in the object store with a given prefix
    pub async fn list_files(
        &self,
        b: BucketWithPrefix<'_>
    ) -> Result<Vec<ListObjectsResponse>, crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                let mut continuation_token = None;
                let mut resp = vec![];

                loop {
                    let mut action = client.list_objects_v2().bucket(b.bucket()).prefix(b.prefix()?);

                    if let Some(continuation_token) = &continuation_token {
                        action = action.continuation_token(continuation_token);
                    }

                    let response = match action.send().await {
                        Ok(response) => response,
                        Err(e) => {
                            let Some(e) = e.as_service_error() else {
                                return Err(format!("Failed to list objects: {}", e).into());
                            };

                            return Err(format!("Failed to list objects: {}", e).into());
                        }
                    };

                    if let Some(contents) = response.contents {
                        for object in contents {
                            let Some(ref key) = object.key else {
                                continue;
                            };

                            resp.push(ListObjectsResponse {
                                key: key.to_string(),
                                last_modified: match object.last_modified {
                                    Some(last_modified) => {
                                        chrono::DateTime::from_timestamp(last_modified.secs(), 0)
                                    }
                                    None => None,
                                },
                                size: object.size.unwrap_or(0),
                                etag: object.e_tag().map(|etag| etag.to_string()),
                            });
                        }
                    }

                    if response.next_continuation_token.is_none() {
                        break;
                    }

                    continuation_token = response.next_continuation_token;
                }

                Ok(resp)
            }
            ObjectStore::Local { dir } => {
                let path = std::path::Path::new(dir).join(b.bucket()).join(b.prefix()?).to_path_buf();

                let mut files = vec![];
                for entry in std::fs::read_dir(path)
                    .map_err(|e| format!("Failed to read directory: {}", e))?
                {
                    let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
                    let path = entry.path();
                    if path.is_file() {
                        files.push(ListObjectsResponse {
                            key: path
                                .file_name()
                                .ok_or("Failed to get file name")?
                                .to_string_lossy()
                                .to_string(),
                            last_modified: Some(
                                entry
                                    .metadata()
                                    .map_err(|e| format!("Failed to get metadata: {}", e))?
                                    .modified()
                                    .map_err(|e| format!("Failed to get modified time: {}", e))?
                                    .into(),
                            ),
                            size: entry
                                .metadata()
                                .map_err(|e| format!("Failed to get metadata: {}", e))?
                                .len()
                                .try_into()
                                .unwrap_or(0),
                            etag: None,
                        });
                    }
                }

                Ok(files)
            }
        }
    }

    /// Downloads a file from the object store with a given key
    pub async fn download_file(&self, b: BucketWithKey<'_>) -> Result<Vec<u8>, crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                let resp = client.get_object().bucket(b.bucket()).key(b.key()?).send().await?;

                let body = resp.body.collect().await?;

                Ok(body.into_bytes().to_vec())
            }
            ObjectStore::Local { dir } => {
                let path = std::path::Path::new(dir).join(b.bucket()).join(b.key()?);
                Ok(std::fs::read(path).map_err(|e| format!("Failed to read object: {}", e))?)
            }
        }
    }

    /// Uploads a file to the object store with a given key
    pub async fn upload_file(
        &self,
        b: BucketWithKey<'_>,
        data: Vec<u8>,
    ) -> Result<(), crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                if data.len() > MULTIPART_MIN_SIZE {
                    let cmuo = client
                        .create_multipart_upload()
                        .bucket(b.bucket())
                        .key(b.key()?)
                        .send()
                        .await?;

                    let Some(upload_id) = cmuo.upload_id else {
                        return Err("Failed to get upload id".into());
                    };

                    // Upload parts
                    let mut error: Option<crate::Error> = None;
                    let mut parts = vec![];
                    loop {
                        let mut action = client
                            .upload_part()
                            .bucket(b.bucket())
                            .upload_id(upload_id.clone())
                            .key(b.key()?)
                            .part_number(match parts.len().try_into() {
                                Ok(part_number) => part_number,
                                Err(_) => {
                                    error = Some("Failed to convert part number".into());
                                    break;
                                }
                            });

                        // Split into 5 mb parts
                        let range = std::ops::Range {
                            start: parts.len() * CHUNK_SIZE,
                            end: std::cmp::min(data.len(), (parts.len() + 1) * CHUNK_SIZE),
                        };

                        let send_data = &data[range.start..range.end];

                        action = action.body(aws_smithy_types::byte_stream::ByteStream::from(
                            send_data.to_vec(),
                        ));

                        let resp = match action.send().await {
                            Ok(resp) => resp,
                            Err(e) => {
                                error = Some(format!("Failed to upload part: {}", e).into());
                                break;
                            }
                        };

                        let Some(e_tag) = resp.e_tag else {
                            error = Some("Failed to get e_tag".into());
                            break;
                        };

                        parts.push(
                            aws_sdk_s3::types::CompletedPart::builder()
                                .e_tag(e_tag)
                                .part_number(parts.len().try_into()?)
                                .build(),
                        );

                        if range.end == data.len() {
                            break;
                        }
                    }

                    if let Some(error) = error {
                        client
                            .abort_multipart_upload()
                            .bucket(b.bucket())
                            .key(b.key()?)
                            .upload_id(upload_id)
                            .send()
                            .await?;

                        return Err(error);
                    }

                    let completed_multipart_upload =
                        aws_sdk_s3::types::CompletedMultipartUpload::builder()
                            .set_parts(Some(parts))
                            .build();

                    client
                        .complete_multipart_upload()
                        .bucket(b.bucket())
                        .key(b.key()?)
                        .upload_id(upload_id)
                        .multipart_upload(completed_multipart_upload)
                        .send()
                        .await?;

                    Ok(())
                } else {
                    client
                        .put_object()
                        .bucket(b.bucket())
                        .key(b.key()?)
                        .body(aws_smithy_types::byte_stream::ByteStream::from(data))
                        .send()
                        .await?;

                    Ok(())
                }
            }
            ObjectStore::Local { dir } => {
                let path = std::path::Path::new(dir).join(b.bucket()).join(b.key()?);
                std::fs::write(path, data).map_err(|e| format!("Failed to write object: {}", e))?;

                Ok(())
            }
        }
    }

    pub async fn delete(&self, b: BucketWithKey<'_>) -> Result<(), crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                client
                    .delete_object()
                    .bucket(b.bucket())
                    .key(b.key()?)
                    .send()
                    .await?;

                Ok(())
            }
            ObjectStore::Local { dir } => {
                let path = std::path::Path::new(dir).join(b.bucket()).join(b.key()?);
                std::fs::remove_file(path)
                    .map_err(|e| format!("Failed to delete object: {}", e))?;

                Ok(())
            }
        }
    }
}