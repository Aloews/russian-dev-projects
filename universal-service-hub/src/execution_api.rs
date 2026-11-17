use super::{can_access_project, can_manage_delivery_schedule, authenticate, ApiError, ApiResult, AppState};
use super::execution_queue::{self, EnqueueRequest, QueueError};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

fn queue_error(error: QueueError) -> ApiError {
    match error {
        QueueError::InvalidInput => ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_queue_request"),
        QueueError::IdempotencyConflict => ApiError(StatusCode::CONFLICT, "idempotency_key_conflict"),
        QueueError::NotFoundOrLeaseLost => ApiError(StatusCode::CONFLICT, "job_not_found_or_lease_lost"),
        QueueError::Database => ApiError(StatusCode::INTERNAL_SERVER_ERROR, "queue_store_error"),
    }
}

async fn project_operator(state: &AppState, headers: &HeaderMap, project_id: &str) -> ApiResult<String> {
    let principal = authenticate(state, headers).await?;
    if !can_manage_delivery_schedule(&principal, project_id) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_operator_required"));
    }
    Ok(principal.actor)
}

async fn project_reader(state: &AppState, headers: &HeaderMap, project_id: &str) -> ApiResult<()> {
    let principal = authenticate(state, headers).await?;
    if !can_access_project(&principal)
        || (!principal.admin && principal.project_id.as_deref() != Some(project_id))
    {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_access_required"));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct JobListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}
fn default_limit() -> i64 { 50 }

pub async fn enqueue_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(body): Json<EnqueueRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let actor = project_operator(&state, &headers, &project_id).await?;
    let (job, created) = execution_queue::enqueue(&state.db, &project_id, &body)
        .await.map_err(queue_error)?;
    Ok((if created { StatusCode::CREATED } else { StatusCode::OK },
        Json(json!({"job": job, "created": created, "actor": actor}))))
}

pub async fn list_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Query(query): Query<JobListQuery>,
) -> ApiResult<Json<Value>> {
    project_reader(&state, &headers, &project_id).await?;
    let jobs = execution_queue::list_project_jobs(&state.db, &project_id, query.limit)
        .await.map_err(queue_error)?;
    Ok(Json(json!({"jobs": jobs})))
}

#[derive(Deserialize)]
pub struct ClaimRequest {
    #[serde(default = "default_lease_seconds")]
    lease_seconds: i64,
}
fn default_lease_seconds() -> i64 { 60 }

pub async fn claim_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(body): Json<ClaimRequest>,
) -> ApiResult<Json<Value>> {
    let worker_id = project_operator(&state, &headers, &project_id).await?;
    let lease = execution_queue::claim(&state.db, &project_id, &worker_id, body.lease_seconds)
        .await.map_err(queue_error)?;
    Ok(Json(match lease {
        Some(lease) => json!({"lease": {
            "job": lease.job,
            "payload": lease.payload,
            "checkpoint": lease.checkpoint,
            "worker_id": lease.worker_id,
            "lease_token": lease.lease_token,
            "lease_expires_at": lease.lease_expires_at
        }}),
        None => json!({"lease": null}),
    }))
}

#[derive(Deserialize)]
pub struct LeaseRequest {
    lease_token: Uuid,
    #[serde(default = "default_lease_seconds")]
    lease_seconds: i64,
}
pub async fn heartbeat_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, job_id)): Path<(String, Uuid)>,
    Json(body): Json<LeaseRequest>,
) -> ApiResult<Json<Value>> {
    project_operator(&state, &headers, &project_id).await?;
    let expires = execution_queue::heartbeat(&state.db, &project_id, job_id, body.lease_token, body.lease_seconds)
        .await.map_err(queue_error)?;
    Ok(Json(json!({"lease_expires_at": expires})))
}

#[derive(Deserialize)]
pub struct CheckpointRequest {
    lease_token: Uuid,
    checkpoint: Value,
}
pub async fn checkpoint_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, job_id)): Path<(String, Uuid)>,
    Json(body): Json<CheckpointRequest>,
) -> ApiResult<Json<Value>> {
    project_operator(&state, &headers, &project_id).await?;
    execution_queue::save_checkpoint(&state.db, &project_id, job_id, body.lease_token, &body.checkpoint)
        .await.map_err(queue_error)?;
    Ok(Json(json!({"saved": true})))
}

#[derive(Deserialize)]
pub struct CompleteRequest {
    lease_token: Uuid,
    #[serde(default = "empty_object")]
    result_metadata: Value,
}
fn empty_object() -> Value { json!({}) }
pub async fn complete_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, job_id)): Path<(String, Uuid)>,
    Json(body): Json<CompleteRequest>,
) -> ApiResult<Json<Value>> {
    project_operator(&state, &headers, &project_id).await?;
    execution_queue::complete(&state.db, &project_id, job_id, body.lease_token, &body.result_metadata)
        .await.map_err(queue_error)?;
    Ok(Json(json!({"completed": true})))
}

#[derive(Deserialize)]
pub struct FailRequest {
    lease_token: Uuid,
    error_code: String,
    #[serde(default)]
    retryable: bool,
}
pub async fn fail_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, job_id)): Path<(String, Uuid)>,
    Json(body): Json<FailRequest>,
) -> ApiResult<Json<Value>> {
    project_operator(&state, &headers, &project_id).await?;
    execution_queue::fail(&state.db, &project_id, job_id, body.lease_token, &body.error_code, body.retryable)
        .await.map_err(queue_error)?;
    Ok(Json(json!({"failed": true})))
}
