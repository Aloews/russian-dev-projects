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

fn db_error<T>(_: sqlx::Error) -> Result<T, QueueError> { Err(QueueError::Database) }

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
        || !request.payload.is_object()
        || serde_json::to_vec(&request.payload).map_err(|_| QueueError::InvalidInput)?.len() > 32768
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
            && row.try_get::<Value, _>("payload").map_err(|_| QueueError::Database)? == request.payload;
        if !same { return Err(QueueError::IdempotencyConflict); }
        (row, false)
    };
    let job = row_to_job(&row)?;
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok((job, created))
}

/// Reconciles expired leases transactionally before new work is claimed.
pub async fn recover_expired_leases(pool: &PgPool) -> Result<u64, QueueError> {
    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let rows = sqlx::query(
        "SELECT project_id, id, attempt_count, lease_token
         FROM execution_jobs
         WHERE status = 'leased' AND lease_expires_at <= NOW()
         ORDER BY lease_expires_at
         FOR UPDATE SKIP LOCKED"
    ).fetch_all(&mut *tx).await.map_err(|_| QueueError::Database)?;
    for row in &rows {
        let project_id: String = row.try_get("project_id").map_err(|_| QueueError::Database)?;
        let job_id: Uuid = row.try_get("id").map_err(|_| QueueError::Database)?;
        let attempt: i32 = row.try_get("attempt_count").map_err(|_| QueueError::Database)?;
        let token: Uuid = row.try_get("lease_token").map_err(|_| QueueError::Database)?;
        let updated_job = sqlx::query(
            "UPDATE execution_jobs SET
                status = CASE WHEN attempt_count < max_attempts THEN 'retry_wait' ELSE 'dead_letter' END,
                available_at = NOW(),
                lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL,
                last_error_code = 'worker_lease_expired',
                finished_at = CASE WHEN attempt_count < max_attempts THEN NULL ELSE NOW() END,
                updated_at = NOW()
             WHERE project_id=$1 AND id=$2 AND status='leased' AND lease_token=$3"
        ).bind(&project_id).bind(job_id).bind(token)
         .execute(&mut *tx).await.map_err(|_| QueueError::Database)?;
        if updated_job.rows_affected() != 1 { return Err(QueueError::Database); }
        let updated_attempt = sqlx::query(
            "UPDATE execution_attempts SET status='lease_expired', finished_at=NOW(),
             duration_ms=GREATEST(0,(EXTRACT(EPOCH FROM (NOW()-started_at))*1000)::bigint),
             error_code='worker_lease_expired'
             WHERE project_id=$1 AND job_id=$2 AND attempt_number=$3 AND lease_token=$4 AND status='running'"
        ).bind(&project_id).bind(job_id).bind(attempt).bind(token)
         .execute(&mut *tx).await.map_err(|_| QueueError::Database)?;
        if updated_attempt.rows_affected() != 1 { return Err(QueueError::Database); }
    }
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok(rows.len() as u64)
}

/// Atomically leases one due job and records its attempt. The lease token fences stale workers.
pub async fn claim(pool: &PgPool, worker_id: &str, lease_seconds: i64) -> Result<Option<Lease>, QueueError> {
    claim_for_types(pool, worker_id, lease_seconds, &[]).await
}

/// Claims only explicitly enabled job types. An empty allowlist preserves the legacy all-types behavior.
pub async fn claim_for_types(
    pool: &PgPool,
    worker_id: &str,
    lease_seconds: i64,
    allowed_job_types: &[String],
) -> Result<Option<Lease>, QueueError> {
    if worker_id.is_empty() || worker_id.len() > 160 || !(5..=3600).contains(&lease_seconds) {
        return Err(QueueError::InvalidInput);
    }
    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let token = Uuid::new_v4();
    let row = sqlx::query(
        "WITH candidate AS ( \
            SELECT id FROM execution_jobs \
            WHERE status IN ('queued','retry_wait') AND available_at <= NOW() AND attempt_count < max_attempts \
              AND ($4::text[] IS NULL OR job_type = ANY($4)) \
            ORDER BY priority DESC, available_at, created_at \
            FOR UPDATE SKIP LOCKED LIMIT 1 \
         ) \
         UPDATE execution_jobs j SET status='leased', lease_owner=$1, lease_token=$2, \
            lease_expires_at=NOW() + ($3 * INTERVAL '1 second'), heartbeat_at=NOW(), \
            attempt_count=j.attempt_count+1, updated_at=NOW() \
         FROM candidate c WHERE j.id=c.id RETURNING j.id,j.project_id,j.task_id,j.job_type,j.status,j.priority,j.attempt_count,j.max_attempts,j.available_at,j.created_at,j.lease_expires_at"
    ).bind(worker_id).bind(token).bind(lease_seconds)
     .bind(if allowed_job_types.is_empty() { None::<Vec<String>> } else { Some(allowed_job_types.to_vec()) })
     .fetch_optional(&mut *tx).await.map_err(|_| QueueError::Database)?;

    let Some(row) = row else { tx.commit().await.map_err(|_| QueueError::Database)?; return Ok(None); };
    let job = row_to_job(&row)?;
    sqlx::query(
        "INSERT INTO execution_attempts (project_id,job_id,attempt_number,worker_id,lease_token,status) \
         VALUES ($1,$2,$3,$4,$5,'running')"
    ).bind(&job.project_id).bind(job.id).bind(job.attempt_count).bind(worker_id).bind(token)
     .execute(&mut *tx).await.map_err(|_| QueueError::Database)?;
    let lease_expires_at = row.try_get("lease_expires_at").map_err(|_| QueueError::Database)?;
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok(Some(Lease { job, worker_id: worker_id.to_owned(), lease_token: token, lease_expires_at }))
}

pub async fn heartbeat(pool: &PgPool, job_id: Uuid, token: Uuid, lease_seconds: i64) -> Result<DateTime<Utc>, QueueError> {
    if !(5..=3600).contains(&lease_seconds) { return Err(QueueError::InvalidInput); }
    let row = sqlx::query(
        "UPDATE execution_jobs SET heartbeat_at=NOW(), lease_expires_at=NOW()+($3 * INTERVAL '1 second'), updated_at=NOW() \
         WHERE id=$1 AND lease_token=$2 AND status='leased' AND lease_expires_at > NOW() RETURNING lease_expires_at"
    ).bind(job_id).bind(token).bind(lease_seconds).fetch_optional(pool).await.map_err(|_| QueueError::Database)?
     .ok_or(QueueError::NotFoundOrLeaseLost)?;
    row.try_get("lease_expires_at").map_err(|_| QueueError::Database)
}

pub async fn save_checkpoint(pool: &PgPool, job_id: Uuid, token: Uuid, checkpoint: &Value) -> Result<(), QueueError> {
    if !checkpoint.is_object() || serde_json::to_vec(checkpoint).map_err(|_| QueueError::InvalidInput)?.len() > 32768 {
        return Err(QueueError::InvalidInput);
    }
    let result = sqlx::query(
        "UPDATE execution_jobs SET checkpoint=$3, heartbeat_at=NOW(), updated_at=NOW() \
         WHERE id=$1 AND lease_token=$2 AND status='leased' AND lease_expires_at > NOW()"
    ).bind(job_id).bind(token).bind(checkpoint).execute(pool).await.map_err(|_| QueueError::Database)?;
    if result.rows_affected() != 1 { return Err(QueueError::NotFoundOrLeaseLost); }
    Ok(())
}

pub async fn complete(pool: &PgPool, job_id: Uuid, token: Uuid, result_metadata: &Value) -> Result<(), QueueError> {
    if !result_metadata.is_object() || serde_json::to_vec(result_metadata).map_err(|_| QueueError::InvalidInput)?.len() > 16384 {
        return Err(QueueError::InvalidInput);
    }
    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let row = sqlx::query(
        "UPDATE execution_jobs SET status='succeeded', lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, \
         heartbeat_at=NULL, finished_at=NOW(), updated_at=NOW() \
         WHERE id=$1 AND lease_token=$2 AND status='leased' AND lease_expires_at > NOW() RETURNING project_id, attempt_count"
    ).bind(job_id).bind(token).fetch_optional(&mut *tx).await.map_err(|_| QueueError::Database)?
     .ok_or(QueueError::NotFoundOrLeaseLost)?;
    let project_id: String = row.try_get("project_id").map_err(|_| QueueError::Database)?;
    let attempt: i32 = row.try_get("attempt_count").map_err(|_| QueueError::Database)?;
    sqlx::query(
        "UPDATE execution_attempts SET status='succeeded', finished_at=NOW(), \
         duration_ms=(EXTRACT(EPOCH FROM (NOW()-started_at))*1000)::bigint, result_metadata=$4 \
         WHERE project_id=$1 AND job_id=$2 AND attempt_number=$3 AND lease_token=$5 AND status='running'"
    ).bind(&project_id).bind(job_id).bind(attempt).bind(result_metadata).bind(token)
     .execute(&mut *tx).await.map_err(|_| QueueError::Database)?;
    tx.commit().await.map_err(|_| QueueError::Database)?;
    Ok(())
}

/// Failure handling uses bounded exponential backoff and moves exhausted work to dead_letter.
pub async fn fail(pool: &PgPool, job_id: Uuid, token: Uuid, error_code: &str, retryable: bool) -> Result<(), QueueError> {
    if error_code.is_empty() || error_code.len() > 100
        || !error_code.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        return Err(QueueError::InvalidInput);
    }
    let mut tx = pool.begin().await.map_err(|_| QueueError::Database)?;
    let row = sqlx::query(
        "UPDATE execution_jobs SET \
           status=CASE WHEN $3 AND attempt_count < max_attempts THEN 'retry_wait' ELSE 'dead_letter' END, \
           available_at=CASE WHEN $3 AND attempt_count < max_attempts \
             THEN NOW() + (LEAST(300, (5 * power(2, GREATEST(0, attempt_count-1)))::integer) * INTERVAL '1 second') \
             ELSE available_at END, \
           lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, heartbeat_at=NULL, \
           last_error_code=$4, finished_at=CASE WHEN NOT ($3 AND attempt_count < max_attempts) THEN NOW() ELSE NULL END, updated_at=NOW() \
         WHERE id=$1 AND lease_token=$2 AND status='leased' AND lease_expires_at > NOW() \
         RETURNING project_id, attempt_count, status"
    ).bind(job_id).bind(token).bind(retryable).bind(error_code)
     .fetch_optional(&mut *tx).await.map_err(|_| QueueError::Database)?
     .ok_or(QueueError::NotFoundOrLeaseLost)?;
    let project_id: String = row.try_get("project_id").map_err(|_| QueueError::Database)?;
    let attempt: i32 = row.try_get("attempt_count").map_err(|_| QueueError::Database)?;
    let status: String = row.try_get("status").map_err(|_| QueueError::Database)?;
    let attempt_status = if status == "retry_wait" { "retryable_failure" } else { "permanent_failure" };
    sqlx::query(
        "UPDATE execution_attempts SET status=$4, finished_at=NOW(), \
         duration_ms=(EXTRACT(EPOCH FROM (NOW()-started_at))*1000)::bigint, error_code=$5 \
         WHERE project_id=$1 AND job_id=$2 AND attempt_number=$3 AND lease_token=$6 AND status='running'"
    ).bind(&project_id).bind(job_id).bind(attempt).bind(attempt_status).bind(error_code).bind(token)
     .execute(&mut *tx).await.map_err(|_| QueueError::Database)?;
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
