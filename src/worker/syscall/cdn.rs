use axum::body::Bytes;
use khronos_runtime::{primitives::blob::Blob, rt::mluau::prelude::*};
use reqwest::Url;

use crate::worker::{limits::MAX_ATTACHMENT_SIZE, syscall::SyscallHandler, workervmmanager::Id};

/// The core underlying syscall
#[derive(Debug)]
pub enum CdnCall {
    DownloadFile {
        url: String,
        as_buffer: bool
    },
}

impl FromLua for CdnCall {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "CdnCall".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: String = tab.get("op")?;
        match typ.as_str() {
            "DownloadFile" => {
                let url = tab.get("url")?;
                let as_buffer = tab.get("as_buffer")?;
                Ok(CdnCall::DownloadFile { url, as_buffer })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "CdnCall".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

pub enum CdnResult {
    Blob {
        data: Bytes
    },
    Buffer {
        data: Bytes
    }
}

impl IntoLua for CdnResult {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        match self {
            Self::Blob { data } => {
                table.set("op", "Blob")?;
                table.set("data", Blob { data: data.to_vec() })?;
            },
            Self::Buffer { data } => {
                table.set("op", "Buffer")?;
                table.set("data", lua.create_buffer(data)?)?;
            }
        }
        table.set_readonly(true); // We want StateExecResult's to be immutable
        Ok(LuaValue::Table(table))
    }
}

impl CdnCall {
    pub(super) async fn exec(self, _id: Id, handler: &SyscallHandler) -> Result<CdnResult, crate::Error> {
        handler.ratelimits.http.check("syscall")?;
        match self {
            Self::DownloadFile { url, as_buffer } => {
                if !url.is_ascii() {
                    return Err("Url must be ascii-only".into());
                }

                let parsed_url = Url::parse(&url)?;
                
                if parsed_url.scheme() != "https" {
                    return Err("HTTPS required".into());
                }

                match parsed_url.domain() {
                    Some("cdn.discordapp.com") => {}
                    _ => return Err("Invalid Discord CDN domain".into()),
                }

                if parsed_url.path().contains("..") {
                    return Err("Path traversal denied".into());
                }

                if parsed_url.host_str().is_none() {
                    return Err("URL does not have a valid host".into());
                }

                if parsed_url.port().is_some() {
                    return Err("URL cannot have a port".into());
                }

                let resp = handler.state.reqwest.get(parsed_url).send().await?;
                
                let Some(content_length) = get_content_length_from_headers(&resp) else {
                    return Err("No content length set".into());
                };

                if content_length > MAX_ATTACHMENT_SIZE {
                    return Err(format!("Max attachment size of {MAX_ATTACHMENT_SIZE} reached").into());
                }

                let bytes = resp.bytes().await?;

                if bytes.len() > MAX_ATTACHMENT_SIZE {
                    return Err(format!("Max attachment size of {MAX_ATTACHMENT_SIZE} reached").into());
                }

                if as_buffer {
                    Ok(CdnResult::Buffer { data: bytes })
                } else {
                    Ok(CdnResult::Blob { data: bytes })
                }
            }
        }
    }
}

fn get_content_length_from_headers(resp: &reqwest::Response) -> Option<usize> {
    let content_length_header = resp.headers().get(reqwest::header::CONTENT_LENGTH);
    if content_length_header.is_none() {
        return None;
    }

    let content_length = content_length_header.unwrap();
    if content_length.is_empty() {
        return None;
    }

    let content_length = content_length
        .to_str()
        .ok()?
        .parse::<usize>()
        .ok()?;

    Some(content_length)
}
