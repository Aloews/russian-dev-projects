use std::{env, time::Duration};

use anyhow::{Context, bail};
use aws_credential_types::Credentials;
use aws_sdk_s3::{config::{Builder, Region}, primitives::ByteStream, Client};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use sqlx::PgPool;

const DEFAULT_MAX_ARTIFACT_BYTES: usize = 20 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: usize = 100 * 1024 * 1024;

#[derive(Clone)]
pub struct ArtifactStore {
    client: Client,
    bucket: String,
    prefix: String,
    max_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct StoredArtifact {
    pub artifact_id: Uuid,
    pub object_key: String,
    pub sha256: String,
    pub size_bytes: usize,
    pub content_type: String,
}

fn bounded_size(raw: Option<String>) -> anyhow::Result<usize> {
    let value = raw.map(|v| v.parse::<usize>()).transpose()?.unwrap_or(DEFAULT_MAX_ARTIFACT_BYTES);
    if value == 0 || value > MAX_ARTIFACT_BYTES {
        bail!("USH_ARTIFACT_MAX_BYTES must be between 1 and {MAX_ARTIFACT_BYTES}");
    }
    Ok(value)
}

fn safe_component(value: &str) -> bool {
    !value.is_empty() && value.len() <= 80
        && value.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
}

fn artifact_key(prefix: &str, project_id: &str, job_id: Uuid, artifact_id: Uuid) -> anyhow::Result<String> {
    if !safe_component(project_id) || !safe_component(prefix) {
        bail!("invalid artifact key component");
    }
    Ok(format!("{}/{}/jobs/{}/{}", prefix, project_id, job_id, artifact_id))
}

impl ArtifactStore {
    pub fn from_env() -> anyhow::Result<Self> {
        let bucket = env::var("USH_ARTIFACT_BUCKET").context("USH_ARTIFACT_BUCKET is required")?;
        if bucket.is_empty() || bucket.len() > 255 || bucket.contains('/') || bucket.contains('\\') {
            bail!("invalid USH_ARTIFACT_BUCKET");
        }
        let region = env::var("USH_ARTIFACT_REGION").context("USH_ARTIFACT_REGION is required")?;
        let access_key = env::var("USH_ARTIFACT_ACCESS_KEY_ID").context("USH_ARTIFACT_ACCESS_KEY_ID is required")?;
        let secret_key = env::var("USH_ARTIFACT_SECRET_ACCESS_KEY").context("USH_ARTIFACT_SECRET_ACCESS_KEY is required")?;
        if access_key.is_empty() || secret_key.is_empty() {
            bail!("artifact storage credentials must not be empty");
        }
        let endpoint = env::var("USH_ARTIFACT_ENDPOINT").ok().filter(|v| !v.trim().is_empty());
        let prefix = env::var("USH_ARTIFACT_PREFIX").unwrap_or_else(|_| "ush-artifacts-v1".into());
        if !safe_component(&prefix) { bail!("USH_ARTIFACT_PREFIX must be a safe component"); }
        let max_bytes = bounded_size(env::var("USH_ARTIFACT_MAX_BYTES").ok())?;

        let credentials = Credentials::new(access_key, secret_key, None, None, "ush-artifact-store");
        let mut builder = Builder::new()
            .region(Region::new(region))
            .credentials_provider(credentials)
            .timeout_config(aws_sdk_s3::config::TimeoutConfig::builder()
                .operation_timeout(Duration::from_secs(30))
                .build())
            .force_path_style(true);
        if let Some(endpoint) = endpoint {
            if !endpoint.starts_with("https://") && env::var("USH_ENVIRONMENT").unwrap_or_default() == "production" {
                bail!("artifact endpoint must use HTTPS in production");
            }
            builder = builder.endpoint_url(endpoint);
        }
        let client = Client::from_conf(builder.build());
        Ok(Self { client, bucket, prefix, max_bytes })
    }

    pub async fn put(
        &self,
        project_id: &str,
        job_id: Uuid,
        content_type: &str,
        bytes: &[u8],
    ) -> anyhow::Result<StoredArtifact> {
        if !safe_component(project_id) {
            bail!("invalid project id for artifact storage");
        }
        if bytes.is_empty() || bytes.len() > self.max_bytes {
            bail!("artifact size must be between 1 and {} bytes", self.max_bytes);
        }
        if content_type.is_empty() || content_type.len() > 128 || !content_type.bytes().all(|b| b >= 0x20 && b != 0x7f) {
            bail!("invalid artifact content type");
        }
        let artifact_id = Uuid::new_v4();
        let object_key = artifact_key(&self.prefix, project_id, job_id, artifact_id)?;
        let sha256 = hex::encode(Sha256::digest(bytes));
        self.client.put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .content_type(content_type)
            .metadata("sha256", sha256.clone())
            .metadata("project_id", project_id.to_owned())
            .metadata("job_id", job_id.to_string())
            .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::Aes256)
            .body(ByteStream::from(bytes.to_vec()))
            .send().await.context("artifact upload failed")?;
        Ok(StoredArtifact {
            artifact_id,
            object_key,
            sha256,
            size_bytes: bytes.len(),
            content_type: content_type.to_owned(),
        })
    }

    pub async fn put_and_register(
        &self,
        pool: &PgPool,
        project_id: &str,
        job_id: Uuid,
        task_id: Option<Uuid>,
        artifact_type: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> anyhow::Result<StoredArtifact> {
        if artifact_type.is_empty() || artifact_type.len() > 48
            || !artifact_type.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c)) {
            bail!("invalid artifact type");
        }
        let stored = self.put(project_id, job_id, content_type, bytes).await?;
        let result = sqlx::query(
            "INSERT INTO execution_artifacts (id, project_id, job_id, task_id, artifact_type, storage_backend, storage_key, content_sha256, size_bytes) VALUES ($1, $2, $3, $4, $5, 's3', $6, $7, $8)"
        )
            .bind(stored.artifact_id)
            .bind(project_id)
            .bind(job_id)
            .bind(task_id)
            .bind(artifact_type)
            .bind(&stored.object_key)
            .bind(&stored.sha256)
            .bind(stored.size_bytes as i64)
            .execute(pool).await;
        if let Err(error) = result {
            if let Err(cleanup_error) = self.delete(&stored.object_key).await {
                tracing::error!(artifact_id = %stored.artifact_id, error = ?cleanup_error, "artifact cleanup after metadata failure failed");
            }
            return Err(error).context("artifact metadata registration failed");
        }
        Ok(stored)
    }

    pub async fn get(&self, object_key: &str) -> anyhow::Result<(Vec<u8>, Option<String>)> {
        if !object_key.starts_with(&format!("{}/", self.prefix)) || object_key.contains("..") {
            bail!("artifact key is outside the configured prefix");
        }
        let response = self.client.get_object().bucket(&self.bucket).key(object_key)
            .send().await.context("artifact download failed")?;
        if response.content_length().is_some_and(|size| size < 0 || size as usize > self.max_bytes) {
            bail!("stored artifact exceeds configured size limit");
        }
        let content_type = response.content_type().map(str::to_owned);
        let bytes = response.body.collect().await.context("artifact body read failed")?.into_bytes().to_vec();
        if bytes.len() > self.max_bytes {
            bail!("stored artifact exceeds configured size limit");
        }
        Ok((bytes, content_type))
    }

    pub async fn delete(&self, object_key: &str) -> anyhow::Result<()> {
        if !object_key.starts_with(&format!("{}/", self.prefix)) || object_key.contains("..") {
            bail!("artifact key is outside the configured prefix");
        }
        self.client.delete_object().bucket(&self.bucket).key(object_key)
            .send().await.context("artifact deletion failed")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_generated_from_validated_components_and_ids() {
        let project = "project-123";
        let job = Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap();
        let artifact = Uuid::parse_str("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb").unwrap();
        assert_eq!(
            artifact_key("ush-artifacts-v1", project, job, artifact).unwrap(),
            "ush-artifacts-v1/project-123/jobs/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa/bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"
        );
        assert!(artifact_key("../unsafe", project, job, artifact).is_err());
        assert!(artifact_key("prefix", "../project", job, artifact).is_err());
    }

    #[test]
    fn size_limits_are_bounded() {
        assert_eq!(bounded_size(None).unwrap(), DEFAULT_MAX_ARTIFACT_BYTES);
        assert_eq!(bounded_size(Some("1".into())).unwrap(), 1);
        assert!(bounded_size(Some("0".into())).is_err());
        assert!(bounded_size(Some(MAX_ARTIFACT_BYTES.to_string())).is_ok());
        assert!(bounded_size(Some((MAX_ARTIFACT_BYTES + 1).to_string())).is_err());
    }

    #[test]
    fn rejects_empty_or_unsafe_components() {
        assert!(!safe_component(""));
        assert!(!safe_component("../x"));
        assert!(!safe_component("project with spaces"));
        assert!(safe_component("project-123"));
        assert!(safe_component("project_123"));
    }
}
