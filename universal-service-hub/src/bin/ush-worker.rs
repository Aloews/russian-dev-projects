#[path = "../artifact_store.rs"]
#[allow(dead_code)]
mod artifact_store;
#[path = "../execution_queue.rs"]
#[allow(dead_code)]
mod execution_queue;

use std::{env, str::FromStr, time::Duration};

use anyhow::{Context, bail};
use serde_json::json;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use tokio::{task::JoinSet, time::sleep, signal, sync::watch};
use tracing::{error, info, warn};
use uuid::Uuid;

const WORKER_VERSION: &str = "0.1.0";

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

async fn worker_loop(
    pool: sqlx::PgPool,
    worker_id: String,
    job_types: Vec<String>,
    poll_interval: Duration,
    lease_seconds: i64,
    test_pause: Duration,
    mut shutdown: watch::Receiver<bool>,
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
                            "worker_version": WORKER_VERSION,
                            "attempt": lease.job.attempt_count
                        });
                        match execution_queue::complete(&pool, job_id, lease.lease_token, &result).await {
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
        workers.spawn(worker_loop(pool, worker_id, job_types, poll_interval, lease_seconds, Duration::from_millis(test_pause_ms as u64), shutdown));
    }

    info!(concurrency, poll_ms, lease_seconds, job_types = ?job_types, "USH worker supervisor started; provider execution is disabled");
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

    #[test]
    fn worker_capability_allowlist_is_nonempty_and_non_billable() {
        assert_eq!(parse_job_types("system_noop").unwrap(), vec!["system_noop"]);
        assert_eq!(parse_job_types(" system_noop,system_noop ").unwrap(), vec!["system_noop"]);
        assert!(parse_job_types("").is_err());
        assert!(parse_job_types("agent.develop").is_err());
        assert!(parse_job_types("system_noop,agent.architect").is_err());
    }
}
