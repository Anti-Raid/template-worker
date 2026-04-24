use aws_sdk_s3::primitives::ByteStream;
use serenity::all::{GuildId, UserId};

use crate::worker::workervmmanager::Id;

// 10mb max upload size (hard cap)
//
// in practice, workers have a 512kb upload limit (much smaller than hard cap)
const UPLOAD_MAX_SUPPORTED_SIZE: usize = 10 * 1024 * 1024; 

/// Simple abstraction around s3-compatible object storages
pub struct ObjectStore {
    client: aws_sdk_s3::Client,
    cdn_client: aws_sdk_s3::Client,
    cdn_endpoint: String,
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

        Ok(ObjectStore {
            client,
            cdn_client,
            cdn_endpoint,
        })
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
    pub async fn get_object_metadata(&self, b: BucketWithKey<'_>) -> Result<Option<ListObjectsResponse>, crate::Error> {
        let key = b.key()?;

        let action = self.client.head_object().bucket(b.bucket()).key(&key);

        match action.send().await {
            Ok(r) => Ok(Some(ListObjectsResponse {
                key,
                last_modified: match r.last_modified {
                    Some(last_modified) => {
                        chrono::DateTime::from_timestamp(last_modified.secs(), 0)
                    }
                    None => None,
                },
                size: r.content_length().unwrap_or(0),
                etag: r.e_tag().map(|etag| etag.to_string()),
            })),
            Err(e) => {
                let Some(e) = e.as_service_error() else {
                    return Err(format!("Failed to list objects: {}", e).into());
                };

                if e.is_not_found() {
                    Ok(None)
                } else {
                    Err(format!("Failed to get object metadata: {}", e).into())
                }
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
        let url = self.cdn_client
            .get_object()
            .bucket(b.bucket())
            .key(b.key()?)
            .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
                duration,
            )?)
            .await
            .map_err(|e| format!("failed to get presigned url: {e:?}"))?;

        let url = url.uri();

        let url = if self.cdn_endpoint.starts_with("$DOCKER:") {
            let mut parsed_url = reqwest::Url::parse(url)?;
            parsed_url.set_host(Some(self.cdn_endpoint.trim_start_matches("$DOCKER:")))?;
            parsed_url
                .set_scheme("http")
                .map_err(|_| "Failed to set new scheme")?;
            parsed_url.to_string()
        } else {
            url.to_string()
        };

        Ok(url)
    }

    /// Lists all files in the object store with a given prefix
    pub async fn list_files(
        &self,
        b: BucketWithPrefix<'_>
    ) -> Result<Vec<ListObjectsResponse>, crate::Error> {
        let mut continuation_token = None;
        let mut resp = vec![];

        loop {
            let mut action = self.client.list_objects_v2().bucket(b.bucket()).prefix(b.prefix()?);

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

    /// Downloads a file from the object store with a given key
    pub async fn download_file(&self, b: BucketWithKey<'_>) -> Result<Vec<u8>, crate::Error> {
        let resp = self.client.get_object().bucket(b.bucket()).key(b.key()?).send().await?;

        let body = resp.body.collect().await?;

        Ok(body.to_vec())
    }

    /// Uploads a file to the object store with a given key
    pub async fn upload_file(
        &self,
        b: BucketWithKey<'_>,
        data: Vec<u8>,
    ) -> Result<(), crate::Error> {
        if data.len() > UPLOAD_MAX_SUPPORTED_SIZE {
            return Err("internal error: data size > UPLOAD_MAX_SUPPORTED_SIZE (hard cap)".into())
        }
        let bucket = b.bucket();
        let key = b.key()?;
        self.client.put_object().bucket(bucket).key(&key).body(ByteStream::from(data)).send().await?;
        return Ok(());
    }

    pub async fn delete(&self, b: BucketWithKey<'_>) -> Result<(), crate::Error> {
        self.client
        .delete_object()
        .bucket(b.bucket())
        .key(b.key()?)
        .send()
        .await?;

        Ok(())
    }
}