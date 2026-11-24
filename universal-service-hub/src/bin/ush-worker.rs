#[path = "../artifact_store.rs"]
#[allow(dead_code)]
mod artifact_store;
#[path = "../execution_queue.rs"]
#[allow(dead_code)]
mod execution_queue;

use std::{env, str::FromStr, time::Duration};

use anyhow::{Context, bail};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use tokio::{task::JoinSet, time::sleep, signal, sync::watch};
use tracing::{error, info, warn};
use uuid::Uuid;

const WORKER_VERSION: &str = "0.1.0";

#[derive(Clone)]
struct ArtifactClient {
    base_url: url::Url,
    http: reqwest::Client,
}

struct ArtifactDownloadRequest<'a> {
    project_id: &'a str,
    job_id: Uuid,
    artifact_id: Uuid,
    worker_id: &'a str,
    lease_token: Uuid,
    expected_sha256: &'a str,
    expected_size: usize,
}

impl ArtifactClient {
    fn from_env() -> anyhow::Result<Option<Self>> {
        let Ok(raw) = env::var("USH_WORKER_API_BASE_URL") else { return Ok(None); };
        let base_url = url::Url::parse(raw.trim()).context("USH_WORKER_API_BASE_URL must be an absolute URL")?;
        let host = base_url.host_str().unwrap_or_default();
        let local_http = base_url.scheme() == "http"
            && matches!(host, "localhost" | "127.0.0.1" | "::1");
        if (base_url.scheme() != "https" && !local_http)
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            bail!("USH_WORKER_API_BASE_URL must be HTTPS (HTTP is allowed only for loopback development)");
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("worker artifact HTTP client initialization failed")?;
        Ok(Some(Self { base_url, http }))
    }

    async fn upload(
        &self,
        project_id: &str,
        job_id: Uuid,
        worker_id: &str,
        lease_token: Uuid,
        bytes: Vec<u8>,
    ) -> anyhow::Result<Uuid> {
        let mut url = self.base_url.clone();
        {
            let mut segments = url.path_segments_mut()
                .map_err(|_| anyhow::anyhow!("worker API URL cannot accept path segments"))?;
            segments.pop_if_empty()
                .push("v1").push("worker").push("projects").push(project_id)
                .push("jobs").push(&job_id.to_string()).push("artifacts");
        }
        let response = self.http.post(url)
            .header("x-ush-worker-id", worker_id)
            .header("x-ush-lease-token", lease_token.to_string())
            .header("content-type", "application/json")
            .header("x-artifact-type", "worker_result")
            .header("x-artifact-idempotency-key", format!("{job_id}-result-v1"))
            .body(bytes)
            .send().await.context("worker artifact upload request failed")?;
        if !response.status().is_success() {
            bail!("worker artifact upload rejected with HTTP {}", response.status());
        }
        let payload: serde_json::Value = response.json().await
            .context("worker artifact upload returned invalid JSON")?;
        let artifact_id = payload.get("artifact_id").and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("worker artifact response omitted artifact_id"))?;
        Uuid::parse_str(artifact_id).context("worker artifact response contained invalid artifact_id")
    }
    #[allow(dead_code)]
    async fn download(&self, request: ArtifactDownloadRequest<'_>) -> anyhow::Result<Vec<u8>> {
        let ArtifactDownloadRequest {
            project_id,
            job_id,
            artifact_id,
            worker_id,
            lease_token,
            expected_sha256,
            expected_size,
        } = request;
        const MAX_DOWNLOAD_BYTES: usize = 2 * 1024 * 1024;
        if expected_size == 0 || expected_size > MAX_DOWNLOAD_BYTES {
            bail!("worker artifact download exceeds the size bound");
        }
        if expected_sha256.len() != 64
            || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            bail!("worker artifact expected SHA-256 is invalid");
        }
        let mut url = self.base_url.clone();
        {
            let mut segments = url.path_segments_mut()
                .map_err(|_| anyhow::anyhow!("worker API URL cannot accept path segments"))?;
            segments.pop_if_empty()
                .push("v1").push("worker").push("projects").push(project_id)
                .push("jobs").push(&job_id.to_string()).push("artifacts").push(&artifact_id.to_string());
        }
        let response = self.http.get(url)
            .header("x-ush-worker-id", worker_id)
            .header("x-ush-lease-token", lease_token.to_string())
            .send().await.context("worker artifact download request failed")?;
        if !response.status().is_success() {
            bail!("worker artifact download rejected with HTTP {}", response.status());
        }
        if response.content_length().is_some_and(|size| size != expected_size as u64 || size > MAX_DOWNLOAD_BYTES as u64) {
            bail!("worker artifact download content length mismatch");
        }
        let response_sha = response.headers().get("x-artifact-sha256")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("worker artifact download omitted SHA-256 header"))?;
        if response_sha != expected_sha256 {
            bail!("worker artifact download SHA-256 header mismatch");
        }
        let content_type = response.headers().get("content-type")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("worker artifact download omitted content type"))?;
        if !artifact_store::valid_content_type(content_type) {
            bail!("worker artifact download content type is invalid");
        }
        let bytes = response.bytes().await.context("worker artifact download body failed")?;
        if bytes.len() != expected_size || bytes.len() > MAX_DOWNLOAD_BYTES {
            bail!("worker artifact download body size mismatch");
        }
        let actual_sha = hex::encode(Sha256::digest(&bytes));
        if actual_sha != expected_sha256 {
            bail!("worker artifact download content SHA-256 mismatch");
        }
        Ok(bytes.to_vec())
    }

}

fn parse_job_types(raw: &str) -> anyhow::Result<Vec<String>> {
    let mut types = raw.split(',').map(str::trim).filter(|v| !v.is_empty())
        .map(str::to_owned).collect::<Vec<_>>();
    types.sort();
    types.dedup();
    if types.is_empty() || types.iter().any(|value| value != "system_noop") {
        bail!("USH_WORKER_JOB_TYPES currently permits only system_noop");
    }
    Ok(types)
}

fn bounded_usize(name: &str, default: usize, min: usize, max: usize) -> anyhow::Result<usize> {
    let value = env::var(name).ok().map(|raw| raw.parse::<usize>()).transpose()?.unwrap_or(default);
    if !(min..=max).contains(&value) {
        bail!("{name} must be between {min} and {max}");
    }
    Ok(value)
}

async fn shutdown_signal() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = signal::ctrl_c() => result?,
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    signal::ctrl_c().await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn worker_loop(
    pool: sqlx::PgPool,
    worker_id: String,
    job_types: Vec<String>,
    poll_interval: Duration,
    lease_seconds: i64,
    test_pause: Duration,
    mut shutdown: watch::Receiver<bool>,
    artifact_client: Option<ArtifactClient>,
) {
    loop {
        if *shutdown.borrow() { break; }

        match execution_queue::recover_expired_leases(&pool, &job_types).await {
            Ok(recovered) if recovered > 0 => info!(worker_id, recovered, "expired execution leases recovered"),
            Ok(_) => {}
            Err(error) => {
                error!(worker_id, error = ?error, "lease recovery failed");
                tokio::select! {
                    _ = shutdown.changed() => break,
                    _ = sleep(poll_interval) => continue,
                }
            }
        }

        match execution_queue::claim_for_types(&pool, &worker_id, lease_seconds, &job_types).await {
            Ok(Some(lease)) => {
                let job_id = lease.job.id;
                if !test_pause.is_zero() {
                    sleep(test_pause).await;
                }
                match lease.job.job_type.as_str() {
                    "system_noop" => {
                        let result = json!({
                            "executor": "system_noop",
                            "worker_version": WORKER_VERSION
                        });
                        let mut completion = result.clone();
                        if let Some(client) = &artifact_client {
                            let artifact_bytes = match serde_json::to_vec(&result) {
                                Ok(bytes) => bytes,
                                Err(error) => {
                                    let _ = execution_queue::fail(&pool, job_id, lease.lease_token, "artifact_serialize_failed", false).await;
                                    error!(worker_id, job_id = %job_id, error = ?error, "worker result serialization failed");
                                    continue;
                                }
                            };
                            match client.upload(&lease.job.project_id, job_id, &worker_id, lease.lease_token, artifact_bytes).await {
                                Ok(artifact_id) => completion["artifact_id"] = json!(artifact_id),
                                Err(error) => {
                                    warn!(worker_id, job_id = %job_id, error = ?error, "artifact upload failed; job will retry");
                                    let _ = execution_queue::fail(&pool, job_id, lease.lease_token, "artifact_upload_failed", true).await;
                                    continue;
                                }
                            }
                        }
                        match execution_queue::complete(&pool, job_id, lease.lease_token, &completion).await {
                            Ok(()) => info!(worker_id, job_id = %job_id, "non-billable worker job completed"),
                            Err(error) => warn!(worker_id, job_id = %job_id, error = ?error, "job completion rejected; lease recovery will reconcile"),
                        }
                    }
                    _ => {
                        // The SQL claim filter must make this branch unreachable.
                        let _ = execution_queue::fail(&pool, job_id, lease.lease_token, "unsupported_job_type", false).await;
                        error!(worker_id, job_id = %job_id, "worker claimed a job outside its capability allowlist");
                    }
                }
            }
            Ok(None) => {
                tokio::select! {
                    _ = shutdown.changed() => break,
                    _ = sleep(poll_interval) => {}
                }
            }
            Err(error) => {
                error!(worker_id, error = ?error, "job claim failed");
                tokio::select! {
                    _ = shutdown.changed() => break,
                    _ = sleep(poll_interval) => {}
                }
            }
        }
    }
    info!(worker_id, "worker stopped gracefully");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url = env::var("USH_WORKER_DATABASE_URL")
        .context("USH_WORKER_DATABASE_URL must be a dedicated, least-privilege worker database role")?;
    if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
        bail!("USH_WORKER_DATABASE_URL must use PostgreSQL");
    }

    let concurrency = bounded_usize("USH_WORKER_CONCURRENCY", 2, 1, 32)?;
    let poll_ms = bounded_usize("USH_WORKER_POLL_MS", 1000, 100, 30_000)?;
    let lease_seconds = bounded_usize("USH_WORKER_LEASE_SECONDS", 60, 10, 300)? as i64;
    let test_pause_ms = bounded_usize("USH_WORKER_TEST_PAUSE_AFTER_CLAIM_MS", 0, 0, 120_000)?;
    let job_types = parse_job_types(&env::var("USH_WORKER_JOB_TYPES").unwrap_or_else(|_| "system_noop".into()))?;
    let artifact_client = ArtifactClient::from_env()?;
    let worker_prefix = env::var("USH_WORKER_ID").unwrap_or_else(|_| "ush-worker".into());
    if worker_prefix.is_empty() || worker_prefix.len() > 100
        || !worker_prefix.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        bail!("USH_WORKER_ID must be a bounded identifier");
    }

    let production = env::var("USH_ENVIRONMENT").unwrap_or_else(|_| "development".into()) == "production";
    if production && test_pause_ms > 0 {
        bail!("USH_WORKER_TEST_PAUSE_AFTER_CLAIM_MS is forbidden in production");
    }
    let mut connect_options = PgConnectOptions::from_str(&database_url)
        .context("invalid worker database URL")?;
    connect_options = connect_options.ssl_mode(if production { PgSslMode::VerifyFull } else { PgSslMode::Prefer });
    let pool = PgPoolOptions::new()
        .max_connections((concurrency + 2) as u32)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(connect_options).await
        .context("worker database connection failed")?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut workers = JoinSet::new();
    let poll_interval = Duration::from_millis(poll_ms as u64);
    for index in 0..concurrency {
        let pool = pool.clone();
        let worker_id = format!("{worker_prefix}-{}-{index}", Uuid::new_v4());
        let job_types = job_types.clone();
        let shutdown = shutdown_rx.clone();
        workers.spawn(worker_loop(pool, worker_id, job_types, poll_interval, lease_seconds, Duration::from_millis(test_pause_ms as u64), shutdown, artifact_client.clone()));
    }

    info!(concurrency, poll_ms, lease_seconds, job_types = ?job_types, artifact_upload_enabled = artifact_client.is_some(), "USH worker supervisor started; provider execution is disabled");
    shutdown_signal().await.context("failed to listen for shutdown signal")?;
    let _ = shutdown_tx.send(true);
    while workers.join_next().await.is_some() {}
    pool.close().await;
    info!("USH worker supervisor shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn worker_artifact_download_checks_lease_and_hash() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let project_id = "project-a";
        let job_id = Uuid::from_u128(1);
        let artifact_id = Uuid::from_u128(2);
        let worker_id = "worker-a";
        let lease_token = Uuid::from_u128(3);
        let payload = b"verified observation bytes".to_vec();
        let expected_sha = hex::encode(Sha256::digest(&payload));
        let expected_path = format!(
            "/v1/worker/projects/{project_id}/jobs/{job_id}/artifacts/{artifact_id}"
        );
        Mock::given(method("GET"))
            .and(path(&expected_path))
            .and(header("x-ush-worker-id", worker_id))
            .and(header("x-ush-lease-token", lease_token.to_string()))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/octet-stream")
                    .insert_header("x-artifact-sha256", expected_sha.clone())
                    .set_body_bytes(payload.clone()),
            )
            .mount(&server)
            .await;
        let client = ArtifactClient {
            base_url: url::Url::parse(&server.uri()).unwrap(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        };
        let bytes = client.download(ArtifactDownloadRequest {
            project_id,
            job_id,
            artifact_id,
            worker_id,
            lease_token,
            expected_sha256: &expected_sha,
            expected_size: payload.len(),
        }).await.unwrap();
        assert_eq!(bytes, payload);
    }

    #[tokio::test]
    async fn worker_artifact_download_rejects_expected_hash_mismatch() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let project_id = "project-a";
        let job_id = Uuid::from_u128(4);
        let artifact_id = Uuid::from_u128(5);
        let worker_id = "worker-a";
        let lease_token = Uuid::from_u128(6);
        let payload = b"verified observation bytes".to_vec();
        let actual_sha = hex::encode(Sha256::digest(&payload));
        let wrong_sha = "0".repeat(64);
        let expected_path = format!(
            "/v1/worker/projects/{project_id}/jobs/{job_id}/artifacts/{artifact_id}"
        );
        Mock::given(method("GET"))
            .and(path(&expected_path))
            .and(header("x-ush-worker-id", worker_id))
            .and(header("x-ush-lease-token", lease_token.to_string()))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/octet-stream")
                    .insert_header("x-artifact-sha256", actual_sha)
                    .set_body_bytes(payload.clone()),
            )
            .mount(&server)
            .await;
        let client = ArtifactClient {
            base_url: url::Url::parse(&server.uri()).unwrap(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        };
        let result = client.download(ArtifactDownloadRequest {
            project_id,
            job_id,
            artifact_id,
            worker_id,
            lease_token,
            expected_sha256: &wrong_sha,
            expected_size: payload.len(),
        }).await;
        assert!(result.is_err());
    }

    #[test]
    fn worker_capability_allowlist_is_nonempty_and_non_billable() {
        assert_eq!(parse_job_types("system_noop").unwrap(), vec!["system_noop"]);
        assert_eq!(parse_job_types(" system_noop,system_noop ").unwrap(), vec!["system_noop"]);
        assert!(parse_job_types("").is_err());
        assert!(parse_job_types("agent.develop").is_err());
        assert!(parse_job_types("system_noop,agent.architect").is_err());
    }
}
