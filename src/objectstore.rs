use dashmap::DashMap;

const CHUNK_SIZE: usize = 5 * 1024 * 1024;
const MULTIPART_MIN_SIZE: usize = 50 * 1024 * 1024;

/// Simple abstraction around object storages
pub enum ObjectStore {
    S3 {
        client: aws_sdk_s3::Client,
        cdn_client: aws_sdk_s3::Client,
        cdn_endpoint: String,
        created_buckets: DashMap<String, ()>,
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
            created_buckets: DashMap::new(),
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

impl ObjectStore {
    /// Create a bucket with the given name
    pub async fn create_bucket(&self, name: &str) -> Result<(), crate::Error> {
        match self {
            ObjectStore::S3 {
                client,
                created_buckets,
                ..
            } => {
                client.create_bucket().bucket(name).send().await?;
                created_buckets.insert(name.to_string(), ());
                Ok(())
            }
            ObjectStore::Local { dir } => {
                // Make directory <prefix>
                std::fs::create_dir_all(format!("{}/{}", dir, name))
                    .map_err(|e| format!("Failed to create directory: {}", e))?;

                Ok(())
            }
        }
    }

    /// Creates the bucket if it does not already exist
    pub async fn create_bucket_if_not_exists(&self, name: &str) -> Result<(), crate::Error> {
        match self {
            ObjectStore::S3 {
                client,
                created_buckets,
                ..
            } => {
                if created_buckets.contains_key(name) {
                    return Ok(());
                }

                let action = client.head_bucket().bucket(name);

                let must_create_bucket = match action.send().await {
                    Ok(_) => false,
                    Err(e) => {
                        let Some(e) = e.as_service_error() else {
                            return Err(format!("Failed to list objects: {}", e).into());
                        };

                        e.is_not_found()
                    }
                };

                if must_create_bucket {
                    self.create_bucket(name).await?;
                }

                Ok(())
            }
            ObjectStore::Local { dir } => {
                // Make directory <prefix>
                std::fs::create_dir_all(format!("{}/{}", dir, name))
                    .map_err(|e| format!("Failed to create directory: {}", e))?;

                Ok(())
            }
        }
    }

    /// Returns if a file exists in the object store
    pub async fn exists(&self, bucket: &str, key: &str) -> Result<bool, crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                let action = client.head_object().bucket(bucket).key(key);

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
                let path = std::path::Path::new(dir).join(bucket).join(key);
                Ok(path.exists())
            }
        }
    }

    /// Note that duration is only supported for S3
    ///
    /// On S3, this returns a presigned URL, on local, it returns a file:// url
    pub async fn get_url(
        &self,
        bucket: &str,
        key: &str,
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
                    .bucket(bucket)
                    .key(key)
                    .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
                        duration,
                    )?)
                    .await?;

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
            ObjectStore::Local { dir } => Ok(format!("file://{}/{}/{}", dir, bucket, key)),
        }
    }

    /// Lists all files in the object store with a given prefix
    pub async fn list_files(
        &self,
        bucket: &str,
        key: Option<&str>,
    ) -> Result<Vec<ListObjectsResponse>, crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                let mut continuation_token = None;
                let mut have_created_bucket = false;
                let mut resp = vec![];

                loop {
                    let mut action = client.list_objects_v2().bucket(bucket);

                    if let Some(key) = key {
                        action = action.prefix(key);
                    }

                    if let Some(continuation_token) = &continuation_token {
                        action = action.continuation_token(continuation_token);
                    }

                    let response = match action.send().await {
                        Ok(response) => response,
                        Err(e) => {
                            let Some(e) = e.as_service_error() else {
                                return Err(format!("Failed to list objects: {}", e).into());
                            };

                            if e.is_no_such_bucket() && !have_created_bucket {
                                // Try creating a new bucket
                                self.create_bucket(bucket).await?;
                                have_created_bucket = true;
                                continue;
                            } else {
                                return Err(format!("Failed to list objects: {}", e).into());
                            }
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
                let mut path = std::path::Path::new(dir).join(bucket).to_path_buf();

                if let Some(key) = key {
                    path = path.join(key);
                }

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
    pub async fn download_file(&self, bucket: &str, key: &str) -> Result<Vec<u8>, crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                let resp = client.get_object().bucket(bucket).key(key).send().await?;

                let body = resp.body.collect().await?;

                Ok(body.into_bytes().to_vec())
            }
            ObjectStore::Local { dir } => {
                let path = std::path::Path::new(dir).join(bucket).join(key);
                Ok(std::fs::read(path).map_err(|e| format!("Failed to read object: {}", e))?)
            }
        }
    }

    /// Uploads a file to the object store with a given key
    pub async fn upload_file(
        &self,
        bucket: &str,
        key: &str,
        data: Vec<u8>,
    ) -> Result<(), crate::Error> {
        self.create_bucket_if_not_exists(bucket).await?;

        match self {
            ObjectStore::S3 { client, .. } => {
                if data.len() > MULTIPART_MIN_SIZE {
                    let cmuo = client
                        .create_multipart_upload()
                        .bucket(bucket)
                        .key(key)
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
                            .bucket(bucket)
                            .upload_id(upload_id.clone())
                            .key(key)
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
                            .bucket(bucket)
                            .key(key)
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
                        .bucket(bucket)
                        .key(key)
                        .upload_id(upload_id)
                        .multipart_upload(completed_multipart_upload)
                        .send()
                        .await?;

                    Ok(())
                } else {
                    client
                        .put_object()
                        .bucket(bucket)
                        .key(key)
                        .body(aws_smithy_types::byte_stream::ByteStream::from(data))
                        .send()
                        .await?;

                    Ok(())
                }
            }
            ObjectStore::Local { dir } => {
                let path = std::path::Path::new(dir).join(bucket).join(key);
                std::fs::write(path, data).map_err(|e| format!("Failed to write object: {}", e))?;

                Ok(())
            }
        }
    }

    pub async fn delete(&self, bucket: &str, key: &str) -> Result<(), crate::Error> {
        match self {
            ObjectStore::S3 { client, .. } => {
                client
                    .delete_object()
                    .bucket(bucket)
                    .key(key)
                    .send()
                    .await?;

                Ok(())
            }
            ObjectStore::Local { dir } => {
                let path = std::path::Path::new(dir).join(bucket).join(key);
                std::fs::remove_file(path)
                    .map_err(|e| format!("Failed to delete object: {}", e))?;

                Ok(())
            }
        }
    }
}

/// Returns the name of the bucket for the given guild
pub fn guild_bucket(guild_id: serenity::all::GuildId) -> String {
    format!("antiraid.guild.{}", guild_id)
}
