use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct EnqueueRequest {
    pub job_type: String,
    pub idempotency_key: String,
    #[serde(default)]
    pub task_id: Option<Uuid>,
    #[serde(default)]
    pub priority: i16,
    #[serde(default = "default_attempts")]
    pub max_attempts: i32,
    #[serde(default)]
    pub payload: Value,
}

fn default_attempts() -> i32 { 3 }


fn safe_metadata(value: &Value, max_bytes: usize) -> bool {
    fn inspect(value: &Value) -> bool {
        match value {
            Value::Object(map) => map.iter().all(|(key, child)| {
                let key = key.to_ascii_lowercase();
                !["prompt", "transcript", "secret", "password", "credential", "api_key",
                  "access_token", "refresh_token", "raw_response", "generated_text",
                  "customer_data", "messages", "system_prompt", "user_input"]
                    .iter().any(|blocked| key.contains(blocked))
                    && inspect(child)
            }),
            Value::Array(items) => items.iter().all(inspect),
            Value::String(text) => text.len() <= 2048,
            _ => true,
        }
    }
    value.is_object()
        && serde_json::to_vec(value).is_ok_and(|bytes| bytes.len() <= max_bytes)
        && inspect(value)
}


#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub project_id: String,
    pub task_id: Option<Uuid>,
    pub job_type: String,
    pub status: String,
    pub priority: i16,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub available_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Lease {
    pub job: Job,
    pub payload: Value,
    pub checkpoint: Value,
    pub worker_id: String,
    pub lease_token: Uuid,
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    InvalidInput,
    IdempotencyConflict,
    NotFoundOrLeaseLost,
    Database,
}

fn row_to_job(row: &sqlx::postgres::PgRow) -> Result<Job, QueueError> {
    Ok(Job {
        id: row.try_get("id").map_err(|_| QueueError::Database)?,
        project_id: row.try_get("project_id").map_err(|_| QueueError::Database)?,
        task_id: row.try_get("task_id").map_err(|_| QueueError::Database)?,
        job_type: row.try_get("job_type").map_err(|_| QueueError::Database)?,
        status: row.try_get("status").map_err(|_| QueueError::Database)?,
        priority: row.try_get("priority").map_err(|_| QueueError::Database)?,
        attempt_count: row.try_get("attempt_count").map_err(|_| QueueError::Database)?,
        max_attempts: row.try_get("max_attempts").map_err(|_| QueueError::Database)?,
        available_at: row.try_get("available_at").map_err(|_| QueueError::Database)?,
        created_at: row.try_get("created_at").map_err(|_| QueueError::Database)?,
    })
}

pub async fn enqueue(pool: &PgPool, project_id: &str, request: &EnqueueRequest) -> Result<(Job, bool), QueueError> {
    if project_id.is_empty() || request.job_type.is_empty() || request.job_type.len() > 64
        || request.idempotency_key.is_empty() || request.idempotency_key.len() > 160
        || !request.job_type.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        || request.priority < 0 || request.priority > 1000
        || !(1..=20).contains(&request.max_attempts)
        || !safe_metadata(&request.payload, 32768)
    {
        return Err(QueueError::InvalidInput);
    }

    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let inserted = sqlx::query(
        "INSERT INTO execution_jobs (project_id, task_id, job_type, status, priority, idempotency_key, payload, max_attempts) \
         VALUES ($1,$2,$3,'queued',$4,$5,$6,$7) \
         ON CONFLICT (project_id, idempotency_key) DO NOTHING RETURNING *"
    )
    .bind(project_id).bind(request.task_id).bind(&request.job_type).bind(request.priority)
    .bind(&request.idempotency_key).bind(&request.payload).bind(request.max_attempts)
    .fetch_optional(&mut *tx).await.map_err(|_| QueueError::Database)?;

    let (row, created) = if let Some(row) = inserted {
        (row, true)
    } else {
        let row = sqlx::query(
            "SELECT * FROM execution_jobs WHERE project_id=$1 AND idempotency_key=$2 FOR SHARE"
        ).bind(project_id).bind(&request.idempotency_key)
         .fetch_optional(&mut *tx).await.map_err(|_| QueueError::Database)?
         .ok_or(QueueError::Database)?;
        let same = row.try_get::<String, _>("job_type").map_err(|_| QueueError::Database)? == request.job_type
            && row.try_get::<Option<Uuid>, _>("task_id").map_err(|_| QueueError::Database)? == request.task_id
            && row.try_get::<Value, _>("payload").map_err(|_| QueueError::Database)? == request.payload
            && row.try_get::<i16, _>("priority").map_err(|_| QueueError::Database)? == request.priority
            && row.try_get::<i32, _>("max_attempts").map_err(|_| QueueError::Database)? == request.max_attempts;
        if !same { return Err(QueueError::IdempotencyConflict); }
        (row, false)
    };
    let job = row_to_job(&row)?;
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok((job, created))
}

/// Atomically leases one due job and records its attempt. The lease token fences stale workers.
pub async fn claim(pool: &PgPool, project_id: &str, worker_id: &str, lease_seconds: i64) -> Result<Option<Lease>, QueueError> {
    if project_id.is_empty() || worker_id.is_empty() || worker_id.len() > 160 || !(5..=3600).contains(&lease_seconds) {
        return Err(QueueError::InvalidInput);
    }
    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let token = Uuid::new_v4();
    let row = sqlx::query(
        "WITH candidate AS ( \
            SELECT id FROM execution_jobs \
            WHERE project_id=$1 AND status IN ('queued','retry_wait') AND available_at <= NOW() AND attempt_count < max_attempts \
            ORDER BY priority DESC, available_at, created_at \
            FOR UPDATE SKIP LOCKED LIMIT 1 \
         ) \
         UPDATE execution_jobs j SET status='leased', lease_owner=$2, lease_token=$3, \
            lease_expires_at=NOW() + ($4 * INTERVAL '1 second'), heartbeat_at=NOW(), \
            attempt_count=j.attempt_count+1, updated_at=NOW() \
         FROM candidate c WHERE j.id=c.id RETURNING j.*"
    ).bind(project_id).bind(worker_id).bind(token).bind(lease_seconds)
     .fetch_optional(&mut *tx).await.map_err(|_| QueueError::Database)?;

    let Some(row) = row else { tx.commit().await.map_err(|_| QueueError::Database)?; return Ok(None); };
    let job = row_to_job(&row)?;
    let payload: Value = row.try_get("payload").map_err(|_| QueueError::Database)?;
    let checkpoint: Value = row.try_get("checkpoint").map_err(|_| QueueError::Database)?;
    sqlx::query(
        "INSERT INTO execution_attempts (project_id,job_id,attempt_number,worker_id,lease_token,status) \
         VALUES ($1,$2,$3,$4,$5,'running')"
    ).bind(&job.project_id).bind(job.id).bind(job.attempt_count).bind(worker_id).bind(token)
     .execute(&mut *tx).await.map_err(|_| QueueError::Database)?;
    let lease_expires_at = row.try_get("lease_expires_at").map_err(|_| QueueError::Database)?;
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok(Some(Lease { job, payload, checkpoint, worker_id: worker_id.to_owned(), lease_token: token, lease_expires_at }))
}

pub async fn heartbeat(pool: &PgPool, project_id: &str, job_id: Uuid, token: Uuid, lease_seconds: i64) -> Result<DateTime<Utc>, QueueError> {
    if !(5..=3600).contains(&lease_seconds) { return Err(QueueError::InvalidInput); }
    let row = sqlx::query(
        "UPDATE execution_jobs SET heartbeat_at=NOW(), lease_expires_at=NOW()+($4 * INTERVAL '1 second'), updated_at=NOW() \
         WHERE project_id=$1 AND id=$2 AND lease_token=$3 AND status='leased' AND lease_expires_at > NOW() RETURNING lease_expires_at"
    ).bind(project_id).bind(job_id).bind(token).bind(lease_seconds).fetch_optional(pool).await.map_err(|_| QueueError::Database)?
     .ok_or(QueueError::NotFoundOrLeaseLost)?;
    row.try_get("lease_expires_at").map_err(|_| QueueError::Database)
}

pub async fn save_checkpoint(pool: &PgPool, project_id: &str, job_id: Uuid, token: Uuid, checkpoint: &Value) -> Result<(), QueueError> {
    if !safe_metadata(checkpoint, 32768) {
        return Err(QueueError::InvalidInput);
    }
    let result = sqlx::query(
        "UPDATE execution_jobs SET checkpoint=$4, heartbeat_at=NOW(), updated_at=NOW() \
         WHERE project_id=$1 AND id=$2 AND lease_token=$3 AND status='leased' AND lease_expires_at > NOW()"
    ).bind(project_id).bind(job_id).bind(token).bind(checkpoint).execute(pool).await.map_err(|_| QueueError::Database)?;
    if result.rows_affected() != 1 { return Err(QueueError::NotFoundOrLeaseLost); }
    Ok(())
}

pub async fn complete(pool: &PgPool, project_id: &str, job_id: Uuid, token: Uuid, result_metadata: &Value) -> Result<(), QueueError> {
    if !safe_metadata(result_metadata, 16384) {
        return Err(QueueError::InvalidInput);
    }
    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let row = sqlx::query(
        "UPDATE execution_jobs SET status='succeeded', lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, \
         heartbeat_at=NULL, finished_at=NOW(), updated_at=NOW() \
         WHERE project_id=$1 AND id=$2 AND lease_token=$3 AND status='leased' AND lease_expires_at > NOW() RETURNING project_id, attempt_count"
    ).bind(project_id).bind(job_id).bind(token).fetch_optional(&mut *tx).await.map_err(|_| QueueError::Database)?
     .ok_or(QueueError::NotFoundOrLeaseLost)?;
    let project_id: String = row.try_get("project_id").map_err(|_| QueueError::Database)?;
    let attempt: i32 = row.try_get("attempt_count").map_err(|_| QueueError::Database)?;
    let attempt_update = sqlx::query(
        "UPDATE execution_attempts SET status='succeeded', finished_at=NOW(), \
         duration_ms=(EXTRACT(EPOCH FROM (NOW()-started_at))*1000)::bigint, result_metadata=$4 \
         WHERE project_id=$1 AND job_id=$2 AND attempt_number=$3 AND lease_token=$5 AND status='running'"
    ).bind(&project_id).bind(job_id).bind(attempt).bind(result_metadata).bind(token)
     .execute(&mut *tx).await.map_err(|_| QueueError::Database)?;
    if attempt_update.rows_affected() != 1 { return Err(QueueError::Database); }
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok(())
}

/// Failure handling uses bounded exponential backoff and moves exhausted work to dead_letter.
pub async fn fail(pool: &PgPool, project_id: &str, job_id: Uuid, token: Uuid, error_code: &str, retryable: bool) -> Result<(), QueueError> {
    if error_code.is_empty() || error_code.len() > 100
        || !error_code.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        return Err(QueueError::InvalidInput);
    }
    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let row = sqlx::query(
        "UPDATE execution_jobs SET \
           status=CASE WHEN $4 AND attempt_count < max_attempts THEN 'retry_wait' ELSE 'dead_letter' END, \
           available_at=CASE WHEN $4 AND attempt_count < max_attempts \
             THEN NOW() + (LEAST(300, (5 * power(2, GREATEST(0, attempt_count-1)))::integer) * INTERVAL '1 second') \
             ELSE available_at END, \
           lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, heartbeat_at=NULL, \
           last_error_code=$5, finished_at=CASE WHEN NOT ($4 AND attempt_count < max_attempts) THEN NOW() ELSE NULL END, updated_at=NOW() \
         WHERE project_id=$1 AND id=$2 AND lease_token=$3 AND status='leased' AND lease_expires_at > NOW() \
         RETURNING project_id, attempt_count, status"
    ).bind(project_id).bind(job_id).bind(token).bind(retryable).bind(error_code)
     .fetch_optional(&mut *tx).await.map_err(|_| QueueError::Database)?
     .ok_or(QueueError::NotFoundOrLeaseLost)?;
    let project_id: String = row.try_get("project_id").map_err(|_| QueueError::Database)?;
    let attempt: i32 = row.try_get("attempt_count").map_err(|_| QueueError::Database)?;
    let status: String = row.try_get("status").map_err(|_| QueueError::Database)?;
    let attempt_status = if status == "retry_wait" { "retryable_failure" } else { "permanent_failure" };
    let attempt_update = sqlx::query(
        "UPDATE execution_attempts SET status=$4, finished_at=NOW(), \
         duration_ms=(EXTRACT(EPOCH FROM (NOW()-started_at))*1000)::bigint, error_code=$5 \
         WHERE project_id=$1 AND job_id=$2 AND attempt_number=$3 AND lease_token=$6 AND status='running'"
    ).bind(&project_id).bind(job_id).bind(attempt).bind(attempt_status).bind(error_code).bind(token)
     .execute(&mut *tx).await.map_err(|_| QueueError::Database)?;
    if attempt_update.rows_affected() != 1 { return Err(QueueError::Database); }
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok(())
}

pub async fn list_project_jobs(pool: &PgPool, project_id: &str, limit: i64) -> Result<Vec<Job>, QueueError> {
    if !(1..=100).contains(&limit) { return Err(QueueError::InvalidInput); }
    let rows = sqlx::query(
        "SELECT * FROM execution_jobs WHERE project_id=$1 ORDER BY created_at DESC LIMIT $2"
    ).bind(project_id).bind(limit).fetch_all(pool).await.map_err(|_| QueueError::Database)?;
    rows.iter().map(row_to_job).collect()
}


/// Requeues expired leases in bounded batches and closes the corresponding attempt.
pub async fn recover_expired(pool: &PgPool, limit: i64) -> Result<u64, QueueError> {
    if !(1..=500).contains(&limit) { return Err(QueueError::InvalidInput); }
    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let rows = sqlx::query(
        "WITH expired AS (
            SELECT id, project_id, attempt_count, lease_token
            FROM execution_jobs
            WHERE status='leased' AND lease_expires_at <= NOW()
            ORDER BY lease_expires_at
            FOR UPDATE SKIP LOCKED
            LIMIT $1
         ),
         recovered AS (
            UPDATE execution_jobs j SET
                status=CASE WHEN j.attempt_count < j.max_attempts THEN 'retry_wait' ELSE 'dead_letter' END,
                available_at=CASE WHEN j.attempt_count < j.max_attempts
                    THEN NOW() + (LEAST(300, (5 * power(2, GREATEST(0, j.attempt_count-1)))::integer) * INTERVAL '1 second')
                    ELSE j.available_at END,
                lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, heartbeat_at=NULL,
                last_error_code='lease_expired',
                finished_at=CASE WHEN j.attempt_count >= j.max_attempts THEN NOW() ELSE NULL END,
                updated_at=NOW()
            FROM expired e
            WHERE j.id=e.id AND j.project_id=e.project_id AND j.lease_token=e.lease_token
            RETURNING j.project_id, j.id, j.attempt_count, e.lease_token
         )
         UPDATE execution_attempts a SET
            status='lease_expired', finished_at=NOW(),
            duration_ms=(EXTRACT(EPOCH FROM (NOW()-a.started_at))*1000)::bigint,
            error_code='lease_expired'
         FROM recovered r
         WHERE a.project_id=r.project_id AND a.job_id=r.id
           AND a.attempt_number=r.attempt_count AND a.lease_token=r.lease_token
           AND a.status='running'
         RETURNING a.id"
    ).bind(limit).fetch_all(&mut *tx).await.map_err(|_| QueueError::Database)?;
    let count = rows.len() as u64;
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok(count)
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    #[test]
    fn metadata_rejects_non_objects_oversize_and_sensitive_keys() {
        assert!(safe_metadata(&serde_json::json!({"stage":"build","progress":2}), 1024));
        assert!(!safe_metadata(&serde_json::json!(["not-an-object"]), 1024));
        assert!(!safe_metadata(&serde_json::json!({"prompt":"private text"}), 1024));
        assert!(!safe_metadata(&serde_json::json!({"nested":{"api_key":"secret"}}), 1024));
        assert!(!safe_metadata(&serde_json::json!({"stage":"x".repeat(3000)}), 4096));
    }
}
