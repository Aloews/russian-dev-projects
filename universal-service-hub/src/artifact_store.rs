use std::{env, time::Duration};

use anyhow::{Context, bail};
use aws_credential_types::Credentials;
use aws_sdk_s3::{config::{BehaviorVersion, Builder, Region}, primitives::ByteStream, Client};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use sqlx::PgPool;
use tokio::io::AsyncReadExt;

const DEFAULT_MAX_ARTIFACT_BYTES: usize = 20 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: usize = 100 * 1024 * 1024;

#[derive(Clone)]
pub struct ArtifactStore {
    client: Client,
    bucket: String,
    prefix: String,
    max_bytes: usize,
}

pub struct ArtifactRegistration<'a> {
    pub project_id: &'a str,
    pub job_id: Uuid,
    pub task_id: Option<Uuid>,
    pub artifact_type: &'a str,
    pub content_type: &'a str,
    pub bytes: &'a [u8],
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

pub fn valid_artifact_type(value: &str) -> bool {
    !value.is_empty() && value.len() <= 48
        && value.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c))
}

pub fn valid_content_type(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 || !value.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        return false;
    }
    let media = value.split(';').next().unwrap_or("").trim();
    let Some((kind, subtype)) = media.split_once('/') else { return false; };
    let valid_token = |part: &str| {
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
    };
    valid_token(kind) && valid_token(subtype)
}

fn valid_bucket(value: &str) -> bool {
    (3..=63).contains(&value.len())
        && value.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && !value.starts_with('.') && !value.ends_with('.')
        && !value.starts_with('-') && !value.ends_with('-')
        && !value.contains("..")
}

fn validate_endpoint(value: &str, production: bool) -> anyhow::Result<String> {
    let parsed = url::Url::parse(value).context("invalid artifact endpoint URL")?;
    if parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some()
        || parsed.query().is_some() || parsed.fragment().is_some()
        || (production && parsed.scheme() != "https")
        || (!production && !["http", "https"].contains(&parsed.scheme()))
    {
        bail!("artifact endpoint must use HTTPS in production and contain no credentials, query or fragment");
    }
    Ok(parsed.to_string().trim_end_matches('/').to_owned())
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
        if !valid_bucket(&bucket) {
            bail!("invalid USH_ARTIFACT_BUCKET");
        }
        let region = env::var("USH_ARTIFACT_REGION").context("USH_ARTIFACT_REGION is required")?;
        if region.trim().is_empty() { bail!("USH_ARTIFACT_REGION must not be empty"); }
        let access_key = env::var("USH_ARTIFACT_ACCESS_KEY_ID").context("USH_ARTIFACT_ACCESS_KEY_ID is required")?;
        let secret_key = env::var("USH_ARTIFACT_SECRET_ACCESS_KEY").context("USH_ARTIFACT_SECRET_ACCESS_KEY is required")?;
        if access_key.is_empty() || secret_key.is_empty() {
            bail!("artifact storage credentials must not be empty");
        }
        let production = env::var("USH_ENVIRONMENT").unwrap_or_default() == "production";
        let endpoint = env::var("USH_ARTIFACT_ENDPOINT").ok().filter(|v| !v.trim().is_empty())
            .map(|value| validate_endpoint(&value, production)).transpose()?;
        let prefix = env::var("USH_ARTIFACT_PREFIX").unwrap_or_else(|_| "ush-artifacts-v1".into());
        if !safe_component(&prefix) { bail!("USH_ARTIFACT_PREFIX must be a safe component"); }
        let max_bytes = bounded_size(env::var("USH_ARTIFACT_MAX_BYTES").ok())?;

        let credentials = Credentials::new(access_key, secret_key, None, None, "ush-artifact-store");
        let mut builder = Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(region))
            .credentials_provider(credentials)
            .timeout_config(aws_sdk_s3::config::timeout::TimeoutConfig::builder()
                .operation_timeout(Duration::from_secs(30))
                .build())
            .force_path_style(true);
        if let Some(endpoint) = endpoint {
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
        if !valid_content_type(content_type) {
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
        registration: ArtifactRegistration<'_>,
    ) -> anyhow::Result<StoredArtifact> {
        if !valid_artifact_type(registration.artifact_type) { bail!("invalid artifact type"); }
        let stored = self.put(registration.project_id, registration.job_id, registration.content_type, registration.bytes).await?;
        let result = sqlx::query(
            "INSERT INTO execution_artifacts (id, project_id, job_id, task_id, artifact_type, storage_backend, storage_key, content_sha256, size_bytes, content_type) VALUES ($1, $2, $3, $4, $5, 's3', $6, $7, $8, $9)"
        )
            .bind(stored.artifact_id)
            .bind(registration.project_id)
            .bind(registration.job_id)
            .bind(registration.task_id)
            .bind(registration.artifact_type)
            .bind(&stored.object_key)
            .bind(&stored.sha256)
            .bind(stored.size_bytes as i64)
            .bind(&stored.content_type)
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
        let initial_capacity = response.content_length().unwrap_or(0).max(0).min(self.max_bytes as i64) as usize;
        let mut bytes = Vec::with_capacity(initial_capacity);
        let mut limited_body = response.body.into_async_read().take((self.max_bytes + 1) as u64);
        limited_body.read_to_end(&mut bytes).await.context("artifact body read failed")?;
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
    fn bucket_and_endpoint_validation_reject_unsafe_values() {
        assert!(valid_bucket("private-artifacts-123"));
        assert!(!valid_bucket("UPPER"));
        assert!(!valid_bucket("ab"));
        assert!(!valid_bucket("../bucket"));
        assert!(validate_endpoint("https://s3.example.test", true).is_ok());
        assert!(validate_endpoint("http://s3.example.test", true).is_err());
        assert!(validate_endpoint("https://user:pass@s3.example.test", false).is_err());
        assert!(validate_endpoint("http://minio.local:9000", false).is_ok());
    }

    #[test]
    fn artifact_type_and_content_type_are_validated() {
        assert!(valid_artifact_type("source_code"));
        assert!(valid_artifact_type("test-logs"));
        assert!(!valid_artifact_type("UPPER"));
        assert!(!valid_artifact_type("../path"));
        assert!(valid_content_type("application/json"));
        assert!(valid_content_type("text/plain; charset=utf-8"));
        assert!(!valid_content_type("invalid"));
        assert!(!valid_content_type("application/\u{7f}bad"));
        assert!(!valid_content_type("application/"));
    }

    #[test]
    fn rejects_empty_or_unsafe_components() {
        assert!(!safe_component(""));
        assert!(!safe_component("../x"));
        assert!(!safe_component("project with spaces"));
        assert!(safe_component("project-123"));
        assert!(safe_component("project_123"));
    }

    async fn test_store(server: &wiremock::MockServer, max_bytes: usize) -> ArtifactStore {
        let credentials = Credentials::new("test-access-key", "test-secret-key", None, None, "ush-test");
        let config = Builder::new()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .credentials_provider(credentials)
            .endpoint_url(server.uri())
            .force_path_style(true)
            .build();
        ArtifactStore {
            client: Client::from_conf(config),
            bucket: "ush-test".into(),
            prefix: "ush-artifacts-v1".into(),
            max_bytes,
        }
    }

    #[tokio::test]
    async fn s3_put_get_and_delete_use_scoped_keys_and_integrity_metadata() {
        use wiremock::{matchers::{header, method, path_regex}, Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let store = test_store(&server, 64).await;
        let job_id = Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap();

        Mock::given(method("PUT"))
            .and(path_regex(r"^/ush-test/ush-artifacts-v1/project-a/jobs/[^/]+$"))
            .and(header("x-amz-server-side-encryption", "AES256"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let stored = store.put("project-a", job_id, "text/plain", b"artifact-data").await.unwrap();
        assert!(stored.object_key.starts_with("ush-artifacts-v1/project-a/jobs/"));
        assert_eq!(stored.size_bytes, 13);
        assert_eq!(stored.sha256, hex::encode(Sha256::digest(b"artifact-data")));

        Mock::given(method("GET"))
            .and(path_regex(r"^/ush-test/ush-artifacts-v1/project-a/jobs/[^/]+$"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/plain")
                    .set_body_bytes(b"artifact-data".to_vec()),
            )
            .expect(1)
            .mount(&server)
            .await;

        let (downloaded, content_type) = store.get(&stored.object_key).await.unwrap();
        assert_eq!(downloaded, b"artifact-data");
        assert_eq!(content_type.as_deref(), Some("text/plain"));

        Mock::given(method("DELETE"))
            .and(path_regex(r"^/ush-test/ush-artifacts-v1/project-a/jobs/[^/]+$"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        store.delete(&stored.object_key).await.unwrap();
    }

    #[tokio::test]
    async fn s3_download_enforces_streaming_limit_even_without_trusting_metadata() {
        use wiremock::{matchers::{method, path_regex}, Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let store = test_store(&server, 8).await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/ush-test/ush-artifacts-v1/project-a/jobs/oversized$"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"123456789".to_vec()))
            .expect(1)
            .mount(&server)
            .await;

        let result = store.get("ush-artifacts-v1/project-a/jobs/oversized").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn s3_put_rejects_cross_project_path_injection_before_network_io() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let store = test_store(&server, 64).await;
        let job_id = Uuid::new_v4();
        assert!(store.put("../other-project", job_id, "text/plain", b"x").await.is_err());
        assert!(server.received_requests().await.unwrap().is_empty());
    }

}
