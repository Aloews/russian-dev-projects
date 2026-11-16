use std::{env, net::{IpAddr, SocketAddr}, str::FromStr, sync::Arc, time::{Duration, Instant}};

use axum::{
    extract::{Path, Query, State},
    http::{header::{AUTHORIZATION, HeaderValue}, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use rand::{distr::Alphanumeric, Rng};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::{PgConnectOptions, PgPoolOptions, PgSslMode}, PgPool, Row};
use subtle::ConstantTimeEq;
use tokio::sync::RwLock;
use tower_http::{limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing::{error, info};
use uuid::Uuid;

mod task_state;

#[derive(Clone)]
struct AppState {
    db: PgPool,
    admin_token: Arc<String>,
    http: reqwest::Client,
    infisical: Option<InfisicalClient>,
    rate_limit_per_minute: i64,
}

#[derive(Clone)]
struct InfisicalClient {
    http: reqwest::Client,
    base_url: String,
    client_id: String,
    client_secret: String,
    project_id: String,
    environment: String,
    path_prefix: String,
    token_cache: Arc<RwLock<Option<(String, Instant)>>>,
}

impl InfisicalClient {
    async fn access_token(&self) -> ApiResult<String> {
        {
            let cached = self.token_cache.read().await;
            if let Some((token, expires_at)) = cached.as_ref()
                && *expires_at > Instant::now() + Duration::from_secs(30)
            {
                return Ok(token.clone());
            }
        }
        let mut cached = self.token_cache.write().await;
        if let Some((token, expires_at)) = cached.as_ref()
            && *expires_at > Instant::now() + Duration::from_secs(30)
        {
            return Ok(token.clone());
        }
        let response = self.http.post(format!("{}/api/v1/auth/universal-auth/login", self.base_url.trim_end_matches('/')))
            .form(&[("clientId", self.client_id.as_str()), ("clientSecret", self.client_secret.as_str())])
            .send().await.map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "secret_manager_auth_failed"))?;
        if !response.status().is_success() {
            return Err(ApiError(StatusCode::SERVICE_UNAVAILABLE, "secret_manager_auth_failed"));
        }
        let data: Value = response.json().await.map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "secret_manager_invalid_response"))?;
        let token = data["accessToken"].as_str().filter(|v| !v.is_empty())
            .ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "secret_manager_invalid_response"))?.to_owned();
        let ttl = data["expiresIn"].as_u64().unwrap_or(300).clamp(60, 86_400);
        *cached = Some((token.clone(), Instant::now() + Duration::from_secs(ttl.saturating_sub(30))));
        Ok(token)
    }

    async fn get_secret(&self, name: &str, secret_path: &str) -> ApiResult<Option<String>> {
        if name.is_empty() || name.len() > 120 || !name.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c)) {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_secret_reference"));
        }
        let token = self.access_token().await?;
        let url = format!("{}/api/v4/secrets/{}", self.base_url.trim_end_matches('/'), name);
        let response = self.http.get(url).bearer_auth(token)
            .query(&[
                ("projectId", self.project_id.as_str()),
                ("environment", self.environment.as_str()),
                ("secretPath", secret_path),
            ]).send().await.map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "secret_manager_request_failed"))?;
        if response.status() == StatusCode::NOT_FOUND { return Ok(None); }
        if !response.status().is_success() {
            return Err(ApiError(StatusCode::SERVICE_UNAVAILABLE, "secret_manager_request_failed"));
        }
        let data: Value = response.json().await.map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "secret_manager_invalid_response"))?;
        let value = data["secret"]["secretValue"].as_str()
            .or_else(|| data["secretValue"].as_str())
            .filter(|v| !v.is_empty())
            .map(str::to_owned);
        Ok(value)
    }

    fn provider_path(&self, project_id: Option<&str>) -> String {
        let prefix = self.path_prefix.trim_end_matches('/');
        match project_id {
            Some(id) => format!("{prefix}/{id}/providers"),
            None => format!("{prefix}/shared/providers"),
        }
    }
}


#[derive(Debug)]
struct ApiError(StatusCode, &'static str);

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.0, self.1)
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.0, Json(json!({"error": self.1}))).into_response();
        if self.0 == StatusCode::TOO_MANY_REQUESTS {
            response.headers_mut().insert("retry-after", HeaderValue::from_static("60"));
        }
        response
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Clone)]
struct Principal {
    actor: String,
    role: String,
    tenant_id: Option<String>,
    project_id: Option<String>,
    worker_id: Option<String>,
    worker_role: Option<String>,
    queue_name: Option<String>,
    admin: bool,
}

async fn begin_platform_transaction(
    state: &AppState,
) -> ApiResult<sqlx::Transaction<'static, sqlx::Postgres>> {
    let mut tx=state.db.begin().await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_failed"))?;
    sqlx::query("SELECT set_config('app.is_admin','true',true), set_config('app.tenant_id','',true)")
        .execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_tenant_context_failed"))?;
    Ok(tx)
}

async fn begin_principal_transaction(
    state: &AppState,
    principal: &Principal,
) -> ApiResult<sqlx::Transaction<'static, sqlx::Postgres>> {
    let mut tx=state.db.begin().await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_failed"))?;
    if principal.admin {
        sqlx::query("SELECT set_config('app.is_admin','true',true), set_config('app.tenant_id','',true)")
            .execute(&mut *tx).await
            .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_tenant_context_failed"))?;
    } else {
        let tenant_id=principal.tenant_id.as_deref()
            .ok_or(ApiError(StatusCode::FORBIDDEN,"tenant_context_required"))?;
        sqlx::query("SELECT set_config('app.is_admin','false',true), set_config('app.tenant_id',$1,true)")
            .bind(tenant_id).execute(&mut *tx).await
            .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_tenant_context_failed"))?;
    }
    Ok(tx)
}

fn bearer(headers: &HeaderMap) -> ApiResult<&str> {
    let value = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok())
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "bearer_token_required"))?;
    let (scheme, token) = value.split_once(' ')
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "bearer_token_required"))?;
    if !scheme.eq_ignore_ascii_case("Bearer") || token.contains(' ') {
        return Err(ApiError(StatusCode::UNAUTHORIZED, "bearer_token_required"));
    }
    if token.len() < 32 {
        return Err(ApiError(StatusCode::UNAUTHORIZED, "invalid_bearer_token"));
    }
    Ok(token)
}

async fn prune_rate_limit_rows(db: &PgPool) -> Result<u64, sqlx::Error> {
    let mut tx=db.begin().await?;
    sqlx::query("SELECT set_config('app.is_admin','true',true),set_config('app.tenant_id','',true)")
        .execute(&mut *tx).await?;
    let result = sqlx::query(
        "DELETE FROM api_rate_limits WHERE updated_at < NOW() - INTERVAL '24 hours'"
    ).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(result.rows_affected())
}

async fn enforce_rate_limit(
    state: &AppState, token_hash: &str, tenant_id: Option<&str>, admin: bool
) -> ApiResult<()> {
    let principal=Principal {
        actor:"rate-limit".into(),role:if admin {"admin"} else {"tenant_service"}.into(),
        tenant_id:tenant_id.map(str::to_owned),project_id:None,worker_id:None,
        worker_role:None,queue_name:None,admin
    };
    let mut tx=begin_principal_transaction(state,&principal).await?;
    let row = sqlx::query(
        "INSERT INTO api_rate_limits (token_hash,tenant_id,window_started_at,request_count,updated_at) VALUES ($1,$2,date_trunc('minute',NOW()),1,NOW()) ON CONFLICT (token_hash) DO UPDATE SET tenant_id=EXCLUDED.tenant_id,request_count=CASE WHEN api_rate_limits.window_started_at<date_trunc('minute',NOW()) THEN 1 ELSE api_rate_limits.request_count+1 END,window_started_at=CASE WHEN api_rate_limits.window_started_at<date_trunc('minute',NOW()) THEN date_trunc('minute',NOW()) ELSE api_rate_limits.window_started_at END,updated_at=NOW() RETURNING request_count"
    ).bind(token_hash).bind(tenant_id).fetch_one(&mut *tx).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "rate_limit_store_error"))?;
    let count: i32 = row.try_get("request_count")
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "rate_limit_store_error"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if i64::from(count) > state.rate_limit_per_minute {
        return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "rate_limit_exceeded"));
    }
    Ok(())
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let token = bearer(headers)?;
    let digest = hex::encode(Sha256::digest(token.as_bytes()));
    if token.as_bytes().ct_eq(state.admin_token.as_bytes()).into() {
        enforce_rate_limit(state, &digest, None, true).await?;
        return Ok(Principal {
            actor: "service-token".into(), role: "admin".into(), tenant_id: None,
            project_id: None, worker_id: None, worker_role: None, queue_name: None, admin: true
        });
    }

    let mut worker_auth_tx=begin_platform_transaction(state).await?;
    let worker_row = sqlx::query(
        "SELECT id,tenant_id,project_id,worker_id,worker_role,queue_name FROM engineering_worker_tokens WHERE token_hash=$1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>NOW())"
    ).bind(&digest).fetch_optional(&mut *worker_auth_tx).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    if let Some(row) = worker_row {
        let id: String = row.try_get("id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let tenant_id: String = row.try_get("tenant_id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let project_id: String = row.try_get("project_id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let worker_id: String = row.try_get("worker_id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let worker_role: String = row.try_get("worker_role").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let queue_name: String = row.try_get("queue_name").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let _ = sqlx::query("UPDATE engineering_worker_tokens SET last_used_at=NOW() WHERE id=$1")
            .bind(&id).execute(&mut *worker_auth_tx).await;
        worker_auth_tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
        enforce_rate_limit(state, &digest, Some(&tenant_id), false).await?;
        return Ok(Principal {
            actor: format!("worker:{tenant_id}:{project_id}:{worker_id}:{id}"),
            role: "worker".into(), tenant_id: Some(tenant_id), project_id: Some(project_id),
            worker_id: Some(worker_id), worker_role: Some(worker_role), queue_name: Some(queue_name), admin: false
        });
    }
    worker_auth_tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;

    let mut tenant_auth_tx=begin_platform_transaction(state).await?;
    let tenant_row = sqlx::query(
        "SELECT id,tenant_id,role FROM tenant_tokens WHERE token_hash=$1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>NOW())"
    ).bind(&digest).fetch_optional(&mut *tenant_auth_tx).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    if let Some(row) = tenant_row {
        let id: String = row.try_get("id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let tenant_id: String = row.try_get("tenant_id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let role: String = row.try_get("role").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
        let _ = sqlx::query("UPDATE tenant_tokens SET last_used_at=NOW() WHERE id=$1")
            .bind(&id).execute(&mut *tenant_auth_tx).await;
        tenant_auth_tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
        enforce_rate_limit(state, &digest, Some(&tenant_id), false).await?;
        return Ok(Principal {
            actor: format!("tenant:{tenant_id}:token:{id}"), role, tenant_id: Some(tenant_id),
            project_id: None, worker_id: None, worker_role: None, queue_name: None, admin: false
        });
    }
    tenant_auth_tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;

    let mut project_auth_tx=begin_platform_transaction(state).await?;
    let row = sqlx::query(
        "SELECT id,tenant_id,project_id,role FROM project_tokens WHERE token_hash=$1 AND revoked_at IS NULL"
    ).bind(&digest).fetch_optional(&mut *project_auth_tx).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?
      .ok_or(ApiError(StatusCode::UNAUTHORIZED, "invalid_bearer_token"))?;
    let id: String = row.try_get("id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    let tenant_id: String = row.try_get("tenant_id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    let project_id: String = row.try_get("project_id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    let role: String = row.try_get("role").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    let _ = sqlx::query("UPDATE project_tokens SET last_used_at=NOW() WHERE id=$1")
        .bind(&id).execute(&mut *project_auth_tx).await;
    project_auth_tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    enforce_rate_limit(state, &digest, Some(&tenant_id), false).await?;
    Ok(Principal {
        actor: format!("project:{project_id}:token:{id}"), role, tenant_id: Some(tenant_id),
        project_id: Some(project_id), worker_id: None, worker_role: None, queue_name: None, admin: false
    })
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if !principal.admin { return Err(ApiError(StatusCode::FORBIDDEN, "administrator_required")); }
    Ok(principal)
}

fn can_invoke_ai(principal: &Principal) -> bool {
    principal.admin || principal.role == "owner" || principal.role == "operator"
}

fn worker_queue_for_role(role: &str) -> Option<&'static str> {
    match role {
        "architect" => Some("planning"),
        "implementer" => Some("implementation"),
        "builder" => Some("build"),
        "tester" => Some("test"),
        "reviewer" => Some("review"),
        "recovery" => Some("recovery"),
        _ => None,
    }
}

async fn require_worker_identity(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if principal.worker_id.is_none() || principal.worker_role.is_none() || principal.queue_name.is_none() {
        return Err(ApiError(StatusCode::FORBIDDEN, "engineering_worker_token_required"));
    }
    Ok(principal)
}

async fn require_worker_queue(state: &AppState, headers: &HeaderMap, queue: &str) -> ApiResult<Principal> {
    let principal = require_worker_identity(state, headers).await?;
    if principal.queue_name.as_deref() != Some(queue) {
        return Err(ApiError(StatusCode::FORBIDDEN, "worker_queue_scope_forbidden"));
    }
    Ok(principal)
}

async fn require_operator(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if !can_invoke_ai(&principal) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_operator_required"));
    }
    Ok(principal)
}

fn can_access_project(principal: &Principal) -> bool {
    principal.admin || [
        "owner", "operator", "viewer",
        "tenant_owner", "tenant_admin", "workspace_admin", "project_maintainer",
        "developer", "reviewer", "auditor",
    ].contains(&principal.role.as_str())
}

fn can_manage_project_tokens(principal: &Principal, project_id: &str) -> bool {
    principal.admin
        || (principal.role == "owner" && principal.project_id.as_deref() == Some(project_id))
}

async fn require_project_access(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if !can_access_project(&principal) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_access_required"));
    }
    Ok(principal)
}

async fn require_project_owner_or_admin(
    state: &AppState,
    headers: &HeaderMap,
    project_id: &str,
) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if can_manage_project_tokens(&principal, project_id) {
        return Ok(principal);
    }
    let tenant_admin = ["tenant_owner", "tenant_admin"].contains(&principal.role.as_str());
    if tenant_admin && let Some(tenant_id) = principal.tenant_id.as_deref() {
        let mut tx=begin_principal_transaction(state,&principal).await?;
        let owns_project = sqlx::query("SELECT id FROM projects WHERE id=$1 AND tenant_id=$2")
            .bind(project_id).bind(tenant_id).fetch_optional(&mut *tx).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_scope_check_failed"))?
            .is_some();
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
        if owns_project { return Ok(principal); }
    }
    Err(ApiError(StatusCode::FORBIDDEN, "project_owner_required"))
}

async fn audit(state: &AppState, actor: &str, action: &str, resource_type: &str, resource_id: &str, details: Value) {
    let result = async {
        let mut tx=state.db.begin().await?;
        sqlx::query("SELECT set_config('app.is_admin','true',true),set_config('app.tenant_id','',true)")
            .execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO audit_events (id, occurred_at, actor, action, resource_type, resource_id, details) VALUES ($1,$2,$3,$4,$5,$6,$7)"
        ).bind(Uuid::new_v4().to_string()).bind(Utc::now()).bind(actor).bind(action)
         .bind(resource_type).bind(resource_id).bind(details).execute(&mut *tx).await?;
        tx.commit().await
    }.await;
    if let Err(err) = result {
        error!(error = %err, action, "audit persistence failed");
    }
}


#[derive(Deserialize)]
struct TaskCreateBody {
    idempotency_key: String,
    title: String,
    objective: String,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default = "default_task_priority")]
    priority: i32,
    #[serde(default = "default_task_metadata")]
    metadata: Value,
}
fn default_task_priority() -> i32 { 100 }
fn default_task_metadata() -> Value { json!({}) }

fn metadata_contains_sensitive_key(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key,value)| {
            let key=key.to_ascii_lowercase();
            ["token","secret","password","api_key","credential","authorization"]
                .iter().any(|sensitive|key.contains(sensitive))
                || metadata_contains_sensitive_key(value)
        }),
        Value::Array(values) => values.iter().any(metadata_contains_sensitive_key),
        _ => false,
    }
}

async fn task_project_scope(
    state: &AppState, principal: &Principal, project_id: &str, write: bool
) -> ApiResult<()> {
    if principal.admin { return Ok(()); }

    let project_role_allowed = if write {
        ["owner", "operator"].contains(&principal.role.as_str())
    } else {
        ["owner", "operator", "viewer"].contains(&principal.role.as_str())
    };
    if principal.project_id.as_deref() == Some(project_id) {
        return if project_role_allowed { Ok(()) }
            else { Err(ApiError(StatusCode::FORBIDDEN, "project_role_forbidden")) };
    }

    let tenant_role_allowed = if write {
        ["tenant_owner", "tenant_admin", "workspace_admin", "project_maintainer", "developer", "operator"]
            .contains(&principal.role.as_str())
    } else {
        ["tenant_owner", "tenant_admin", "workspace_admin", "project_maintainer", "developer", "reviewer", "operator", "auditor", "viewer"]
            .contains(&principal.role.as_str())
    };
    let Some(tenant_id) = principal.tenant_id.as_deref() else {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_forbidden"));
    };
    if !tenant_role_allowed {
        return Err(ApiError(StatusCode::FORBIDDEN, "tenant_role_forbidden"));
    }
    let mut tx=begin_principal_transaction(state,principal).await?;
    let belongs_to_tenant = sqlx::query("SELECT id FROM projects WHERE id=$1 AND tenant_id=$2")
        .bind(project_id).bind(tenant_id).fetch_optional(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_scope_check_failed"))?
        .is_some();
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if !belongs_to_tenant {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_forbidden"));
    }
    Ok(())
}

async fn create_engineering_task(
    State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>,
    Json(body): Json<TaskCreateBody>
) -> ApiResult<(StatusCode, Json<Value>)> {
    let principal = authenticate(&state, &headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    task_project_scope(&state, &principal, &project_id, true).await?;
    if body.idempotency_key.trim().is_empty() || body.idempotency_key.len() > 160
        || body.title.trim().is_empty() || body.title.len() > 200
        || body.objective.trim().is_empty() || body.objective.len() > 20_000
        || body.acceptance_criteria.len() > 100 || body.dependencies.len() > 100
        || !(0..=1000).contains(&body.priority)
        || body.acceptance_criteria.iter().any(|v| v.trim().is_empty() || v.len() > 2000)
        || body.dependencies.iter().any(|v| Uuid::parse_str(v).is_err())
        || !body.metadata.is_object() || body.metadata.to_string().len()>16_384
        || metadata_contains_sensitive_key(&body.metadata)
    {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_task"));
    }
    let mut unique_dependencies = std::collections::HashSet::new();
    if body.dependencies.iter().any(|dependency| !unique_dependencies.insert(dependency)) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "duplicate_task_dependency"));
    }
    if !body.dependencies.is_empty() {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM engineering_tasks WHERE id=ANY($1::varchar[]) AND project_id=$2")
            .bind(&body.dependencies).bind(&project_id).fetch_one(&mut *tx).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_dependency_check_failed"))?;
        let count: i64 = row.try_get("count").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_dependency_check_failed"))?;
        if count != body.dependencies.len() as i64 {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "task_dependencies_must_be_in_same_project"));
        }
    }
    let id = Uuid::new_v4().to_string();
    let inserted = sqlx::query(
        "INSERT INTO engineering_tasks (id,project_id,idempotency_key,title,objective,acceptance_criteria,dependencies,priority,created_by,metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (project_id,idempotency_key) DO NOTHING"
    ).bind(&id).bind(&project_id).bind(&body.idempotency_key).bind(&body.title)
     .bind(&body.objective).bind(json!(body.acceptance_criteria)).bind(json!(body.dependencies))
     .bind(body.priority).bind(&principal.actor).bind(&body.metadata).execute(&mut *tx).await
     .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_create_failed"))?;
    let created = inserted.rows_affected() == 1;
    let row = sqlx::query("SELECT id,title,objective,dependencies,metadata,candidate_sha,status,queue_name,priority,version,created_at FROM engineering_tasks WHERE project_id=$1 AND idempotency_key=$2")
        .bind(&project_id).bind(&body.idempotency_key).fetch_one(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_read_after_create_failed"))?;
    let task_id: String = row.try_get("id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_read_failed"))?;
    let stored_dependencies: Value = row.try_get("dependencies").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_read_failed"))?;
    if stored_dependencies != json!(body.dependencies) {
        return Err(ApiError(StatusCode::CONFLICT, "idempotency_key_payload_mismatch"));
    }
    let stored_metadata: Value = row.try_get("metadata").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_read_failed"))?;
    if stored_metadata != body.metadata {
        return Err(ApiError(StatusCode::CONFLICT, "idempotency_key_payload_mismatch"));
    }
    for dependency_id in &body.dependencies {
        sqlx::query("INSERT INTO engineering_task_dependencies (tenant_id,project_id,parent_task_id,child_task_id) SELECT tenant_id,project_id,$1,$2 FROM engineering_tasks WHERE id=$1 AND project_id=$3 ON CONFLICT DO NOTHING")
            .bind(dependency_id).bind(&task_id).bind(&project_id).execute(&mut *tx).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_dependency_write_failed"))?;
    }
    if created {
        sqlx::query("INSERT INTO engineering_task_events (id,task_id,project_id,actor,event_type,payload) VALUES ($1,$2,$3,$4,'task.received',$5)")
            .bind(Uuid::new_v4().to_string()).bind(&task_id).bind(&project_id).bind(&principal.actor)
            .bind(json!({"source":"api"})).execute(&mut *tx).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_event_write_failed"))?;
    } else {
        let existing_title: String = row.try_get("title").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_read_failed"))?;
        let existing_objective: String = row.try_get("objective").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_read_failed"))?;
        if existing_title != body.title || existing_objective != body.objective {
            return Err(ApiError(StatusCode::CONFLICT, "idempotency_key_payload_mismatch"));
        }
    }
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if created { audit(&state, &principal.actor, "engineering_task.create", "engineering_task", &task_id, json!({"project_id":project_id})).await; }
    Ok((if inserted.rows_affected() == 1 { StatusCode::CREATED } else { StatusCode::OK }, Json(json!({
        "id":task_id, "title":row.try_get::<String,_>("title").unwrap_or_default(),
        "objective":row.try_get::<String,_>("objective").unwrap_or_default(),
        "metadata":row.try_get::<Value,_>("metadata").unwrap_or(json!({})),
        "candidate_sha":row.try_get::<Option<String>,_>("candidate_sha").unwrap_or(None),
        "status":row.try_get::<String,_>("status").unwrap_or_default(),
        "queue":row.try_get::<String,_>("queue_name").unwrap_or_default(),
        "priority":row.try_get::<i32,_>("priority").unwrap_or(100),
        "version":row.try_get::<i64,_>("version").unwrap_or(1),
        "created_at":row.try_get::<DateTime<Utc>,_>("created_at").ok()
    }))))
}

async fn list_engineering_tasks(
    State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>
) -> ApiResult<Json<Value>> {
    let principal = require_project_access(&state, &headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    task_project_scope(&state, &principal, &project_id, false).await?;
    let rows = sqlx::query("SELECT id,title,status,queue_name,priority,version,attempt_count,metadata,candidate_sha,created_at,updated_at FROM engineering_tasks WHERE project_id=$1 ORDER BY priority ASC,created_at ASC LIMIT 200")
        .bind(&project_id).fetch_all(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_list_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let values: Vec<Value> = rows.into_iter().map(|r| json!({
        "id":r.get::<String,_>("id"), "title":r.get::<String,_>("title"),
        "status":r.get::<String,_>("status"), "queue":r.get::<String,_>("queue_name"),
        "priority":r.get::<i32,_>("priority"), "version":r.get::<i64,_>("version"),
        "attempt_count":r.get::<i32,_>("attempt_count"),
        "metadata":r.get::<Value,_>("metadata"),
        "candidate_sha":r.get::<Option<String>,_>("candidate_sha"),
        "created_at":r.get::<DateTime<Utc>,_>("created_at"),
        "updated_at":r.get::<DateTime<Utc>,_>("updated_at")
    })).collect();
    Ok(Json(json!(values)))
}

#[derive(Deserialize)]
struct TaskEventBody {
    event_type: String,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default)]
    payload: Value,
}

async fn append_engineering_task_event(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id, task_id)): Path<(String,String)>,
    Json(body): Json<TaskEventBody>
) -> ApiResult<Json<Value>> {
    let principal = authenticate(&state, &headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    task_project_scope(&state, &principal, &project_id, true).await?;
    if body.event_type != "comment" && body.event_type != "clarification" && body.event_type != "requirement.added" {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unsupported_task_event"));
    }
    if body.payload.to_string().len() > 16_384 || body.idempotency_key.as_ref().is_some_and(|v| v.is_empty() || v.len() > 160) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_task_event"));
    }
    let exists = sqlx::query("SELECT id FROM engineering_tasks WHERE id=$1 AND project_id=$2")
        .bind(&task_id).bind(&project_id).fetch_optional(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_lookup_failed"))?;
    if exists.is_none() { return Err(ApiError(StatusCode::NOT_FOUND, "task_not_found")); }
    let event_id = Uuid::new_v4().to_string();
    let result = sqlx::query("INSERT INTO engineering_task_events (id,task_id,project_id,actor,event_type,idempotency_key,payload) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (task_id,idempotency_key) DO NOTHING")
        .bind(&event_id).bind(&task_id).bind(&project_id).bind(&principal.actor)
        .bind(&body.event_type).bind(&body.idempotency_key).bind(&body.payload)
        .execute(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_event_write_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state, &principal.actor, "engineering_task.event", "engineering_task", &task_id, json!({"event_type":body.event_type})).await;
    Ok(Json(json!({"accepted":true,"duplicate":result.rows_affected()==0,"event_id":if result.rows_affected()==1 {Some(event_id)} else {None}})))
}

#[derive(Deserialize)]
struct TaskTransitionBody { next_status: String, expected_version: i64, #[serde(default)] queue: Option<String> }

async fn transition_engineering_task(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id, task_id)): Path<(String,String)>,
    Json(body): Json<TaskTransitionBody>
) -> ApiResult<Json<Value>> {
    let principal = authenticate(&state, &headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    task_project_scope(&state, &principal, &project_id, true).await?;
    let next = task_state::TaskStatus::parse(&body.next_status)
        .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unknown_task_status"))?;
    let row = sqlx::query("SELECT status,version FROM engineering_tasks WHERE id=$1 AND project_id=$2")
        .bind(&task_id).bind(&project_id).fetch_optional(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_lookup_failed"))?;
    let Some(row) = row else { return Err(ApiError(StatusCode::NOT_FOUND, "task_not_found")); };
    let current_text: String = row.try_get("status").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_read_failed"))?;
    let current_version: i64 = row.try_get("version").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_read_failed"))?;
    if current_version != body.expected_version { return Err(ApiError(StatusCode::CONFLICT, "task_version_conflict")); }
    let current = task_state::TaskStatus::parse(&current_text)
        .ok_or(ApiError(StatusCode::INTERNAL_SERVER_ERROR, "stored_task_status_invalid"))?;
    if !current.can_transition_to(next) { return Err(ApiError(StatusCode::CONFLICT, "illegal_task_transition")); }
    let queue = body.queue.unwrap_or_else(|| match next {
        task_state::TaskStatus::Queued => "implementation".into(),
        task_state::TaskStatus::QueuedBuild => "build".into(),
        task_state::TaskStatus::QueuedTest => "test".into(),
        task_state::TaskStatus::Review => "review".into(),
        task_state::TaskStatus::Recovery => "recovery".into(),
        _ => "implementation".into(),
    });
    if !["planning","implementation","build","test","review","recovery"].contains(&queue.as_str()) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_task_queue"));
    }
    let updated = sqlx::query("UPDATE engineering_tasks SET status=$1,queue_name=$2,version=version+1,updated_at=NOW(),worker_id=NULL,lease_token_hash=NULL,lease_expires_at=NULL WHERE id=$3 AND project_id=$4 AND version=$5 RETURNING version")
        .bind(next.as_str()).bind(&queue).bind(&task_id).bind(&project_id).bind(body.expected_version)
        .fetch_optional(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_transition_failed"))?;
    let Some(updated) = updated else { return Err(ApiError(StatusCode::CONFLICT, "task_version_conflict")); };
    sqlx::query("INSERT INTO engineering_task_events (id,task_id,project_id,actor,event_type,payload) VALUES ($1,$2,$3,$4,'task.transition',$5)")
        .bind(Uuid::new_v4().to_string()).bind(&task_id).bind(&project_id).bind(&principal.actor)
        .bind(json!({"from":current_text,"to":next.as_str(),"queue":queue}))
        .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_event_write_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state, &principal.actor, "engineering_task.transition", "engineering_task", &task_id, json!({"from":current_text,"to":next.as_str()})).await;
    Ok(Json(json!({"task_id":task_id,"status":next.as_str(),"queue":queue,"version":updated.try_get::<i64,_>("version").unwrap_or(body.expected_version+1)})))
}

#[derive(Deserialize)]
struct TaskClaimBody { queue: String, worker_id: String, #[serde(default = "default_lease_seconds")] lease_seconds: i64 }
fn default_lease_seconds() -> i64 { 120 }

async fn claim_engineering_task(
    State(state): State<AppState>, headers: HeaderMap, Json(body): Json<TaskClaimBody>
) -> ApiResult<Json<Value>> {
    let principal = require_worker_queue(&state, &headers, &body.queue).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    if body.worker_id.trim().is_empty() || body.worker_id.len() > 120
        || principal.worker_id.as_deref() != Some(body.worker_id.as_str()) {
        return Err(ApiError(StatusCode::FORBIDDEN, "worker_identity_mismatch"));
    }
    let project_id = principal.project_id.as_deref()
        .ok_or(ApiError(StatusCode::FORBIDDEN, "worker_project_scope_required"))?;
    let tenant_id = principal.tenant_id.as_deref()
        .ok_or(ApiError(StatusCode::FORBIDDEN, "worker_tenant_scope_required"))?;
    let lease_seconds = body.lease_seconds.clamp(15, 900);
    let raw_lease: String = rand::rng().sample_iter(&Alphanumeric).take(48).map(char::from).collect();
    let lease_hash = hex::encode(Sha256::digest(raw_lease.as_bytes()));
    let row = sqlx::query(
        "WITH candidate AS (
            SELECT t.id FROM engineering_tasks t
            WHERE t.queue_name=$1
              AND (($1='recovery' AND t.status IN ('recovery','blocked','failed')) OR ($1<>'recovery' AND t.status=CASE $1 WHEN 'build' THEN 'queued_build' WHEN 'test' THEN 'queued_test' WHEN 'review' THEN 'review' ELSE 'queued' END))
              AND (t.attempt_count < t.max_attempts OR $1='recovery')
              AND t.available_at <= NOW()
              AND t.project_id=$2 AND t.tenant_id=$6
              AND NOT EXISTS (
                  SELECT 1
                  FROM engineering_task_dependencies d
                  JOIN engineering_tasks parent
                    ON parent.id=d.parent_task_id
                   AND parent.tenant_id=d.tenant_id
                   AND parent.project_id=d.project_id
                  WHERE d.child_task_id=t.id
                    AND d.tenant_id=t.tenant_id
                    AND d.project_id=t.project_id
                    AND parent.status <> 'verified'
              )
              AND NOT EXISTS (
                  SELECT 1
                  FROM jsonb_array_elements_text(t.dependencies) AS dep(dependency_id)
                  LEFT JOIN engineering_tasks parent
                    ON parent.id=dep.dependency_id
                   AND parent.tenant_id=t.tenant_id
                   AND parent.project_id=t.project_id
                  WHERE parent.id IS NULL OR parent.status <> 'verified'
              )
            ORDER BY t.priority ASC,t.available_at ASC,t.created_at ASC
            FOR UPDATE SKIP LOCKED LIMIT 1
        )
        UPDATE engineering_tasks t
        SET status=CASE WHEN t.queue_name='build' THEN 'building' WHEN t.queue_name='test' THEN 'testing' WHEN t.queue_name='review' THEN 'review' ELSE 'in_progress' END,
            worker_id=$3, lease_token_hash=$4, lease_expires_at=NOW()+($5::text||' seconds')::interval,
            claimed_at=NOW(), lease_generation=t.lease_generation+1,
            attempt_count=t.attempt_count+1, version=t.version+1, updated_at=NOW()
        FROM candidate WHERE t.id=candidate.id
        RETURNING t.id,t.project_id,t.title,t.objective,t.acceptance_criteria,t.dependencies,t.metadata,t.candidate_sha,t.status,t.queue_name,t.version,t.lease_expires_at,t.lease_generation"
    ).bind(&body.queue).bind(project_id).bind(&body.worker_id).bind(&lease_hash).bind(lease_seconds).bind(tenant_id)
     .fetch_optional(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_claim_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let Some(row) = row else { return Ok(Json(json!({"task":null}))); };
    let task_id: String = row.try_get("id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_claim_failed"))?;
    audit(&state, &principal.actor, "engineering_task.claim", "engineering_task", &task_id, json!({"queue":body.queue,"worker_id":body.worker_id})).await;
    Ok(Json(json!({"task":{
        "id":task_id,"project_id":row.try_get::<String,_>("project_id").unwrap_or_default(),
        "title":row.try_get::<String,_>("title").unwrap_or_default(),
        "objective":row.try_get::<String,_>("objective").unwrap_or_default(),
        "acceptance_criteria":row.try_get::<Value,_>("acceptance_criteria").unwrap_or(json!([])),
        "dependencies":row.try_get::<Value,_>("dependencies").unwrap_or(json!([])),
        "metadata":row.try_get::<Value,_>("metadata").unwrap_or(json!({})),
        "candidate_sha":row.try_get::<Option<String>,_>("candidate_sha").unwrap_or(None),
        "status":row.try_get::<String,_>("status").unwrap_or_default(),
        "queue":row.try_get::<String,_>("queue_name").unwrap_or_default(),
        "version":row.try_get::<i64,_>("version").unwrap_or(1),
        "lease_generation":row.try_get::<i64,_>("lease_generation").unwrap_or(0),
        "lease_expires_at":row.try_get::<DateTime<Utc>,_>("lease_expires_at").ok()
    },"lease_token":raw_lease})))
}


#[derive(Deserialize)]
struct TaskLeaseBody { worker_id: String, lease_token: String }
async fn heartbeat_engineering_task(
    State(state): State<AppState>, headers: HeaderMap, Path(task_id): Path<String>, Json(body): Json<TaskLeaseBody>
) -> ApiResult<Json<Value>> {
    let principal = require_worker_identity(&state, &headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    if body.lease_token.len() < 32 || body.worker_id.trim().is_empty()
        || principal.worker_id.as_deref() != Some(body.worker_id.as_str()) {
        return Err(ApiError(StatusCode::FORBIDDEN, "worker_identity_mismatch"));
    }
    let project_id = principal.project_id.as_deref()
        .ok_or(ApiError(StatusCode::FORBIDDEN, "worker_project_scope_required"))?;
    let tenant_id = principal.tenant_id.as_deref()
        .ok_or(ApiError(StatusCode::FORBIDDEN, "worker_tenant_scope_required"))?;
    let hash = hex::encode(Sha256::digest(body.lease_token.as_bytes()));
    let row = sqlx::query("UPDATE engineering_tasks SET lease_expires_at=NOW()+INTERVAL '120 seconds',updated_at=NOW(),version=version+1 WHERE id=$1 AND lease_token_hash=$2 AND worker_id=$3 AND lease_expires_at>NOW() AND project_id=$4 AND tenant_id=$5 RETURNING project_id,lease_expires_at,lease_generation")
        .bind(&task_id).bind(hash).bind(&body.worker_id).bind(project_id).bind(tenant_id)
        .fetch_optional(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "task_heartbeat_failed"))?;
    let Some(row) = row else { return Err(ApiError(StatusCode::CONFLICT, "task_lease_expired_or_invalid")); };
    let task_expires:DateTime<Utc>=row.try_get("lease_expires_at")
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_heartbeat_failed"))?;
    sqlx::query("UPDATE engineering_task_path_locks l SET lease_expires_at=LEAST($1,w.lease_expires_at) FROM engineering_workspaces w WHERE l.tenant_id=$2 AND l.project_id=$3 AND l.task_id=$4 AND l.workspace_id=w.id AND w.state='leased' AND w.lease_expires_at>NOW()")
        .bind(task_expires).bind(tenant_id).bind(project_id).bind(&task_id)
        .execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_path_lock_heartbeat_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    Ok(Json(json!({"task_id":task_id,"lease_generation":row.try_get::<i64,_>("lease_generation").unwrap_or(0),"lease_expires_at":Some(task_expires)})))
}


#[derive(Deserialize)]
struct TaskCheckpointBody {
    worker_id: String,
    lease_token: String,
    stage: String,
    #[serde(default)]
    base_sha: Option<String>,
    #[serde(default)]
    candidate_sha: Option<String>,
    #[serde(default)]
    artifact_digest: Option<String>,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    git_ref: Option<String>,
    #[serde(default)]
    worktree_path: Option<String>,
    #[serde(default)]
    metadata: Value,
}
async fn create_engineering_checkpoint(
    State(state): State<AppState>, headers: HeaderMap, Path(task_id): Path<String>, Json(body): Json<TaskCheckpointBody>
) -> ApiResult<Json<Value>> {
    let principal = require_worker_identity(&state, &headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let valid_sha = |v: &Option<String>| v.as_ref().is_none_or(|s| s.len()==40 && s.chars().all(|c| c.is_ascii_hexdigit()));
    if body.lease_token.len()<32 || body.worker_id.trim().is_empty()
        || principal.worker_id.as_deref() != Some(body.worker_id.as_str())
        || body.stage.is_empty() || body.stage.len()>32
        || !valid_sha(&body.base_sha) || !valid_sha(&body.candidate_sha)
        || body.workspace_id.as_ref().is_some_and(|v| Uuid::parse_str(v).is_err())
        || body.git_ref.as_ref().is_some_and(|v| v.is_empty() || v.len()>240 || v.chars().any(char::is_control))
        || body.worktree_path.as_ref().is_some_and(|v| v.is_empty() || v.len()>1000 || v.contains('\0'))
        || body.artifact_digest.as_ref().is_some_and(|v| v.len()>128 || !v.chars().all(|c| c.is_ascii_hexdigit()))
        || body.metadata.to_string().len()>8192 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_checkpoint"));
    }
    let project_id = principal.project_id.as_deref()
        .ok_or(ApiError(StatusCode::FORBIDDEN, "worker_project_scope_required"))?;
    let tenant_id = principal.tenant_id.as_deref()
        .ok_or(ApiError(StatusCode::FORBIDDEN, "worker_tenant_scope_required"))?;
    let lease_hash = hex::encode(Sha256::digest(body.lease_token.as_bytes()));
    let row = sqlx::query("SELECT project_id FROM engineering_tasks WHERE id=$1 AND lease_token_hash=$2 AND worker_id=$3 AND lease_expires_at>NOW() AND project_id=$4 AND tenant_id=$5")
        .bind(&task_id).bind(lease_hash).bind(&body.worker_id).bind(project_id).bind(tenant_id)
        .fetch_optional(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "checkpoint_lease_check_failed"))?;
    let Some(row) = row else { return Err(ApiError(StatusCode::CONFLICT, "task_lease_expired_or_invalid")); };
    let project_id: String = row.try_get("project_id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "checkpoint_lease_check_failed"))?;
    let checkpoint_id=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO engineering_task_checkpoints (id,task_id,project_id,worker_id,stage,base_sha,candidate_sha,artifact_digest,workspace_id,git_ref,worktree_path,metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
        .bind(&checkpoint_id).bind(&task_id).bind(&project_id).bind(&body.worker_id).bind(&body.stage)
        .bind(&body.base_sha).bind(&body.candidate_sha).bind(&body.artifact_digest)
        .bind(&body.workspace_id).bind(&body.git_ref).bind(&body.worktree_path).bind(&body.metadata)
        .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "checkpoint_write_failed"))?;
    sqlx::query("UPDATE engineering_tasks SET last_checkpoint_id=$1,updated_at=NOW() WHERE id=$2 AND tenant_id=$3 AND project_id=$4")
        .bind(&checkpoint_id).bind(&task_id).bind(tenant_id).bind(&project_id)
        .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "checkpoint_task_update_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state, &principal.actor, "engineering_task.checkpoint", "engineering_task", &task_id, json!({"stage":body.stage,"checkpoint_id":checkpoint_id})).await;
    Ok(Json(json!({"checkpoint_id":checkpoint_id,"task_id":task_id,"stage":body.stage})))
}



async fn recover_expired_task_leases(db: &PgPool) -> Result<u64, sqlx::Error> {
    let mut tx=db.begin().await?;
    sqlx::query("SELECT set_config('app.is_admin','true',true),set_config('app.tenant_id','',true)")
        .execute(&mut *tx).await?;
    let rows = sqlx::query(
        "UPDATE engineering_tasks SET status=CASE WHEN attempt_count>=max_attempts THEN 'failed' ELSE 'recovery' END, queue_name='recovery', worker_id=NULL, lease_token_hash=NULL, lease_expires_at=NULL, last_error_code='worker_lease_expired', claimed_at=NULL, available_at=NOW(), version=version+1, updated_at=NOW() WHERE lease_expires_at IS NOT NULL AND lease_expires_at<NOW() AND status IN ('in_progress','building','testing','review') RETURNING id,project_id,status,attempt_count"
    ).fetch_all(&mut *tx).await?;
    for row in &rows {
        let id: String = row.try_get("id")?;
        let project_id: String = row.try_get("project_id")?;
        let status: String = row.try_get("status")?;
        let attempts: i32 = row.try_get("attempt_count")?;
        sqlx::query("INSERT INTO engineering_task_events (id,task_id,project_id,actor,event_type,payload) VALUES ($1,$2,$3,'system:lease-reaper','task.lease_expired',$4)")
            .bind(Uuid::new_v4().to_string()).bind(&id).bind(&project_id)
            .bind(json!({"status":status,"attempt_count":attempts}))
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(rows.len() as u64)
}

async fn recover_expired_workspace_leases(db: &PgPool) -> Result<u64, sqlx::Error> {
    let mut tx=db.begin().await?;
    sqlx::query("SELECT set_config('app.is_admin','true',true),set_config('app.tenant_id','',true)")
        .execute(&mut *tx).await?;
    let rows=sqlx::query(
        "UPDATE engineering_workspaces SET state='failed',lease_owner=NULL,lease_token_hash=NULL,lease_expires_at=NULL,last_heartbeat_at=NOW(),metadata=metadata||jsonb_build_object('lease_expired_at',NOW()),updated_at=NOW() WHERE state='leased' AND lease_expires_at<NOW() RETURNING id,tenant_id,project_id"
    ).fetch_all(&mut *tx).await?;
    for row in &rows {
        let workspace_id:String=row.try_get("id")?;
        let tenant_id:String=row.try_get("tenant_id")?;
        let project_id:String=row.try_get("project_id")?;
        let task_rows=sqlx::query("UPDATE engineering_task_workspace_assignments SET released_at=NOW() WHERE tenant_id=$1 AND project_id=$2 AND workspace_id=$3 AND released_at IS NULL RETURNING task_id")
            .bind(&tenant_id).bind(&project_id).bind(&workspace_id).fetch_all(&mut *tx).await?;
        sqlx::query("DELETE FROM engineering_task_path_locks WHERE tenant_id=$1 AND project_id=$2 AND workspace_id=$3")
            .bind(&tenant_id).bind(&project_id).bind(&workspace_id).execute(&mut *tx).await?;
        for task_row in task_rows {
            let task_id:String=task_row.try_get("task_id")?;
            sqlx::query("INSERT INTO engineering_task_events (id,task_id,project_id,actor,event_type,payload) VALUES ($1,$2,$3,'system:workspace-reaper','workspace.lease_expired',$4)")
                .bind(Uuid::new_v4().to_string()).bind(&task_id).bind(&project_id)
                .bind(json!({"workspace_id":workspace_id}))
                .execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(rows.len() as u64)
}

async fn healthz() -> Json<Value> { Json(json!({"status":"ok","runtime":"rust"})) }

async fn readyz(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    sqlx::query("SELECT 1").execute(&state.db).await
        .map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "database_not_ready"))?;
    Ok(Json(json!({"status":"ready","runtime":"rust"})))
}

#[derive(Deserialize)]
struct ProjectCreate {
    id: String,
    name: String,
    allowed_capabilities: Vec<String>,
    #[serde(default)]
    allowed_provider_ids: Vec<String>,
}

async fn create_project(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<ProjectCreate>) -> ApiResult<Json<Value>> {
    let principal = require_admin(&state, &headers).await?;
    let mut tx=begin_platform_transaction(&state).await?;
    if body.id.is_empty() || body.id.len() > 80 || !body.id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) || body.name.trim().is_empty() || body.name.len() > 160 || body.allowed_capabilities.is_empty() || body.allowed_capabilities.len() > 100 || body.allowed_provider_ids.len() > 100 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_project"));
    }
    let result = sqlx::query(
        "INSERT INTO projects (id,name,allowed_capabilities,allowed_provider_ids,created_at,updated_at) VALUES ($1,$2,$3,$4,NOW(),NOW()) ON CONFLICT (id) DO NOTHING"
    ).bind(&body.id).bind(&body.name).bind(json!(body.allowed_capabilities.clone())).bind(json!(body.allowed_provider_ids.clone()))
     .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_create_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if result.rows_affected() == 0 { return Err(ApiError(StatusCode::CONFLICT, "project_exists")); }
    audit(&state, &principal.actor, "project.create", "project", &body.id, json!({})).await;
    Ok(Json(json!({"id":body.id,"name":body.name,"allowed_capabilities":body.allowed_capabilities,"allowed_provider_ids":body.allowed_provider_ids})))
}

async fn list_projects(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers).await?;
    let mut tx=begin_platform_transaction(&state).await?;
    let rows = sqlx::query("SELECT id,name,allowed_capabilities,allowed_provider_ids,created_at FROM projects ORDER BY id")
        .fetch_all(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_list_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let values: Vec<Value> = rows.into_iter().map(|r| json!({
        "id":r.get::<String,_>("id"), "name":r.get::<String,_>("name"),
        "allowed_capabilities":r.get::<Value,_>("allowed_capabilities"),
        "allowed_provider_ids":r.get::<Value,_>("allowed_provider_ids"),
        "created_at":r.get::<DateTime<Utc>,_>("created_at")
    })).collect();
    Ok(Json(json!(values)))
}

#[derive(Deserialize)]
struct ProjectUpdate {
    name: String,
    allowed_capabilities: Vec<String>,
    #[serde(default)]
    allowed_provider_ids: Vec<String>,
}

async fn update_project(State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>, Json(body): Json<ProjectUpdate>) -> ApiResult<Json<Value>> {
    let principal = require_admin(&state, &headers).await?;
    let mut tx=begin_platform_transaction(&state).await?;
    if body.name.trim().is_empty() || body.name.len() > 160 || body.allowed_capabilities.is_empty() || body.allowed_capabilities.len() > 100 || body.allowed_provider_ids.len() > 100 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_project"));
    }
    let result = sqlx::query("UPDATE projects SET name=$1,allowed_capabilities=$2,allowed_provider_ids=$3,updated_at=NOW() WHERE id=$4")
        .bind(&body.name).bind(json!(body.allowed_capabilities)).bind(json!(body.allowed_provider_ids)).bind(&project_id)
        .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_update_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if result.rows_affected()==0 { return Err(ApiError(StatusCode::NOT_FOUND, "project_not_found")); }
    audit(&state, &principal.actor, "project.update", "project", &project_id, json!({})).await;
    Ok(Json(json!({"id":project_id,"name":body.name})))
}

async fn delete_project(State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>) -> ApiResult<StatusCode> {
    let principal = require_admin(&state, &headers).await?;
    let mut tx=begin_platform_transaction(&state).await?;
    let result = sqlx::query("DELETE FROM projects WHERE id=$1").bind(&project_id).execute(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_delete_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if result.rows_affected()==0 { return Err(ApiError(StatusCode::NOT_FOUND, "project_not_found")); }
    audit(&state, &principal.actor, "project.delete", "project", &project_id, json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_project_tokens(State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>) -> ApiResult<Json<Value>> {
    let principal=require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let rows = sqlx::query("SELECT id,project_id,name,role,created_at,last_used_at,revoked_at FROM project_tokens WHERE project_id=$1 ORDER BY created_at DESC")
        .bind(&project_id).fetch_all(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_token_list_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let values: Vec<Value> = rows.into_iter().map(|r| json!({
        "id":r.get::<String,_>("id"),"project_id":r.get::<String,_>("project_id"),
        "name":r.get::<String,_>("name"),"role":r.get::<String,_>("role"),
        "created_at":r.get::<DateTime<Utc>,_>("created_at"),
        "last_used_at":r.get::<Option<DateTime<Utc>>,_>("last_used_at"),
        "revoked_at":r.get::<Option<DateTime<Utc>>,_>("revoked_at")
    })).collect();
    Ok(Json(json!(values)))
}


#[derive(Deserialize)]
struct TokenCreate { name: String, #[serde(default = "default_role")] role: String }
fn default_role() -> String { "operator".into() }

async fn create_project_token(State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>, Json(body): Json<TokenCreate>) -> ApiResult<Json<Value>> {
    let principal = require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    if !["owner","operator","viewer"].contains(&body.role.as_str()) || body.name.trim().is_empty() || body.name.len() > 120 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_project_token"));
    }
    if !principal.admin && body.role == "owner" {
        return Err(ApiError(StatusCode::FORBIDDEN, "administrator_required_for_owner_token"));
    }
    let exists = sqlx::query("SELECT id FROM projects WHERE id=$1").bind(&project_id).fetch_optional(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_lookup_failed"))?;
    if exists.is_none() { return Err(ApiError(StatusCode::NOT_FOUND, "project_not_found")); }
    let raw: String = rand::rng().sample_iter(&Alphanumeric).take(48).map(char::from).collect();
    let hash = hex::encode(Sha256::digest(raw.as_bytes()));
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO project_tokens (id,project_id,name,role,token_hash,created_at) VALUES ($1,$2,$3,$4,$5,NOW())")
        .bind(&id).bind(&project_id).bind(&body.name).bind(&body.role).bind(hash).execute(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_token_create_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state, &principal.actor, "project.token.create", "project", &project_id, json!({"token_id":id.clone(),"role":body.role.clone()})).await;
    Ok(Json(json!({"id":id,"project_id":project_id,"name":body.name,"role":body.role,"token":raw})))
}

async fn revoke_project_token(State(state): State<AppState>, headers: HeaderMap, Path((project_id, token_id)): Path<(String,String)>) -> ApiResult<StatusCode> {
    let principal = require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let result = sqlx::query("UPDATE project_tokens SET revoked_at=NOW() WHERE id=$1 AND project_id=$2 AND revoked_at IS NULL")
        .bind(&token_id).bind(&project_id).execute(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_token_revoke_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if result.rows_affected() == 0 { return Err(ApiError(StatusCode::NOT_FOUND, "project_token_not_found")); }
    audit(&state, &principal.actor, "project.token.revoke", "project", &project_id, json!({"token_id":token_id})).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct EngineeringWorkerCreateBody {
    name: String,
    worker_id: String,
    worker_role: String,
    #[serde(default)]
    expires_in_seconds: Option<i64>,
}

async fn create_engineering_worker(
    State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>,
    Json(body): Json<EngineeringWorkerCreateBody>
) -> ApiResult<Json<Value>> {
    let principal = require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let queue_name = worker_queue_for_role(&body.worker_role)
        .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_worker_role"))?;
    if body.name.trim().is_empty() || body.name.len()>120
        || body.worker_id.trim().is_empty() || body.worker_id.len()>120
        || !body.worker_id.chars().all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c))
        || body.expires_in_seconds.is_some_and(|v| !(60..=31_536_000).contains(&v)) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_engineering_worker"));
    }
    let raw: String = rand::rng().sample_iter(&Alphanumeric).take(48).map(char::from).collect();
    let hash = hex::encode(Sha256::digest(raw.as_bytes()));
    let id = Uuid::new_v4().to_string();
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let result = sqlx::query(
        "INSERT INTO engineering_worker_tokens (id,project_id,name,worker_id,worker_role,queue_name,token_hash,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,CASE WHEN $8::bigint IS NULL THEN NULL ELSE NOW()+($8::text||' seconds')::interval END)"
    ).bind(&id).bind(&project_id).bind(&body.name).bind(&body.worker_id)
     .bind(&body.worker_role).bind(queue_name).bind(hash).bind(body.expires_in_seconds)
     .execute(&mut *tx).await;
    if let Err(error) = result {
        if error.as_database_error().is_some_and(|db_error| db_error.code().as_deref()==Some("23505")) {
            return Err(ApiError(StatusCode::CONFLICT, "engineering_worker_id_already_active"));
        }
        return Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, "engineering_worker_create_failed"));
    }
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state, &principal.actor, "engineering_worker.create", "project", &project_id, json!({"worker_id":body.worker_id,"worker_role":body.worker_role,"queue":queue_name})).await;
    Ok(Json(json!({"id":id,"project_id":project_id,"name":body.name,"worker_id":body.worker_id,"worker_role":body.worker_role,"queue":queue_name,"token":raw})))
}

async fn list_engineering_workers(
    State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>
) -> ApiResult<Json<Value>> {
    let principal=require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let rows = sqlx::query("SELECT id,name,worker_id,worker_role,queue_name,created_at,expires_at,last_used_at,revoked_at FROM engineering_worker_tokens WHERE project_id=$1 ORDER BY created_at DESC LIMIT 200")
        .bind(&project_id).fetch_all(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "engineering_worker_list_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let values: Vec<Value> = rows.into_iter().map(|row| json!({
        "id":row.get::<String,_>("id"),"name":row.get::<String,_>("name"),
        "worker_id":row.get::<String,_>("worker_id"),"worker_role":row.get::<String,_>("worker_role"),
        "queue":row.get::<String,_>("queue_name"),"created_at":row.get::<DateTime<Utc>,_>("created_at"),
        "expires_at":row.get::<Option<DateTime<Utc>>,_>("expires_at"),
        "last_used_at":row.get::<Option<DateTime<Utc>>,_>("last_used_at"),
        "revoked_at":row.get::<Option<DateTime<Utc>>,_>("revoked_at")
    })).collect();
    Ok(Json(json!(values)))
}

async fn revoke_engineering_worker(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id,worker_id)): Path<(String,String)>
) -> ApiResult<StatusCode> {
    let principal = require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let result = sqlx::query("UPDATE engineering_worker_tokens SET revoked_at=NOW() WHERE project_id=$1 AND worker_id=$2 AND revoked_at IS NULL")
        .bind(&project_id).bind(&worker_id).execute(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "engineering_worker_revoke_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if result.rows_affected()==0 { return Err(ApiError(StatusCode::NOT_FOUND, "engineering_worker_not_found")); }
    audit(&state, &principal.actor, "engineering_worker.revoke", "project", &project_id, json!({"worker_id":worker_id})).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn require_tenant_admin(state: &AppState, headers: &HeaderMap, tenant_id: &str) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if principal.admin || (
        principal.tenant_id.as_deref() == Some(tenant_id)
        && ["tenant_owner", "tenant_admin"].contains(&principal.role.as_str())
    ) {
        return Ok(principal);
    }
    Err(ApiError(StatusCode::FORBIDDEN, "tenant_admin_required"))
}

#[derive(Deserialize)]
struct TenantTokenCreateBody {
    name: String,
    role: String,
    #[serde(default)]
    expires_in_seconds: Option<i64>,
}

async fn create_tenant_token(
    State(state): State<AppState>, headers: HeaderMap, Path(tenant_id): Path<String>,
    Json(body): Json<TenantTokenCreateBody>
) -> ApiResult<Json<Value>> {
    let principal = require_tenant_admin(&state, &headers, &tenant_id).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let allowed_roles = [
        "tenant_owner", "tenant_admin", "workspace_admin", "project_maintainer",
        "developer", "reviewer", "operator", "auditor", "viewer", "service_agent",
    ];
    if !allowed_roles.contains(&body.role.as_str()) || body.name.trim().is_empty()
        || body.name.len()>120
        || body.expires_in_seconds.is_some_and(|v| !(60..=31_536_000).contains(&v)) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_tenant_token"));
    }
    if !principal.admin && body.role == "tenant_owner" {
        return Err(ApiError(StatusCode::FORBIDDEN, "administrator_required_for_tenant_owner_token"));
    }
    let exists = sqlx::query("SELECT id FROM tenants WHERE id=$1 AND status='active'")
        .bind(&tenant_id).fetch_optional(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "tenant_lookup_failed"))?;
    if exists.is_none() { return Err(ApiError(StatusCode::NOT_FOUND, "tenant_not_found")); }
    let raw: String = rand::rng().sample_iter(&Alphanumeric).take(48).map(char::from).collect();
    let hash = hex::encode(Sha256::digest(raw.as_bytes()));
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO tenant_tokens (id,tenant_id,name,role,token_hash,expires_at) VALUES ($1,$2,$3,$4,$5,CASE WHEN $6::bigint IS NULL THEN NULL ELSE NOW()+($6::text||' seconds')::interval END)")
        .bind(&id).bind(&tenant_id).bind(&body.name).bind(&body.role).bind(hash).bind(body.expires_in_seconds)
        .execute(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "tenant_token_create_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state, &principal.actor, "tenant.token.create", "tenant", &tenant_id, json!({"token_id":id,"role":body.role})).await;
    Ok(Json(json!({"id":id,"tenant_id":tenant_id,"name":body.name,"role":body.role,"token":raw})))
}

async fn revoke_tenant_token(
    State(state): State<AppState>, headers: HeaderMap, Path((tenant_id,token_id)): Path<(String,String)>
) -> ApiResult<StatusCode> {
    let principal = require_tenant_admin(&state, &headers, &tenant_id).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let result = sqlx::query("UPDATE tenant_tokens SET revoked_at=NOW() WHERE id=$1 AND tenant_id=$2 AND revoked_at IS NULL")
        .bind(&token_id).bind(&tenant_id).execute(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "tenant_token_revoke_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    if result.rows_affected()==0 { return Err(ApiError(StatusCode::NOT_FOUND, "tenant_token_not_found")); }
    audit(&state, &principal.actor, "tenant.token.revoke", "tenant", &tenant_id, json!({"token_id":token_id})).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct WorkspaceCreateBody {
    workspace_key: String,
    runtime_provider: String,
    repository: String,
    base_branch: String,
    base_sha: String,
    #[serde(default)]
    git_worktree_path: Option<String>,
    #[serde(default)]
    git_branch: Option<String>,
    #[serde(default)]
    metadata: Value,
}

async fn create_engineering_workspace(
    State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>,
    Json(body): Json<WorkspaceCreateBody>
) -> ApiResult<(StatusCode, Json<Value>)> {
    let principal = require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let valid_sha = body.base_sha.len()==40 && body.base_sha.chars().all(|c| c.is_ascii_hexdigit());
    let valid_branch = |v: &str| !v.is_empty() && v.len()<=200 && !v.starts_with('/')
        && !v.split('/').any(|part| part==".." || part.is_empty())
        && v.chars().all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c));
    let mut repository_parts=body.repository.split('/');
    let repository_owner=repository_parts.next().unwrap_or("");
    let repository_name=repository_parts.next().unwrap_or("");
    let valid_repository=!repository_owner.is_empty() && !repository_name.is_empty()
        && repository_parts.next().is_none()
        && [repository_owner,repository_name].iter().all(|part|
            part.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)));
    if body.workspace_key.trim().is_empty() || body.workspace_key.len()>160
        || !["railway","github_actions","local","other"].contains(&body.runtime_provider.as_str())
        || !valid_repository
        || !valid_branch(&body.base_branch) || !valid_sha
        || body.git_branch.as_ref().is_some_and(|v| !valid_branch(v))
        || body.git_worktree_path.as_ref().is_some_and(|v| !valid_worktree_path(v))
        || body.metadata.to_string().len()>8192 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_engineering_workspace"));
    }
    let id=Uuid::new_v4().to_string();
    let result=sqlx::query("INSERT INTO engineering_workspaces (id,project_id,workspace_key,runtime_provider,repository,base_branch,base_sha,git_worktree_path,git_branch,metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(&id).bind(&project_id).bind(&body.workspace_key).bind(&body.runtime_provider)
        .bind(&body.repository).bind(&body.base_branch).bind(&body.base_sha)
        .bind(&body.git_worktree_path).bind(&body.git_branch).bind(&body.metadata)
        .execute(&mut *tx).await;
    if let Err(error)=result {
        if error.as_database_error().is_some_and(|db_error| db_error.code().as_deref()==Some("23505")) {
            return Err(ApiError(StatusCode::CONFLICT, "workspace_key_already_exists"));
        }
        return Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, "engineering_workspace_create_failed"));
    }
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state,&principal.actor,"engineering_workspace.create","project",&project_id,json!({"workspace_id":id,"workspace_key":body.workspace_key})).await;
    Ok((StatusCode::CREATED,Json(json!({"id":id,"project_id":project_id,"workspace_key":body.workspace_key,"state":"idle","runtime_provider":body.runtime_provider,"repository":body.repository,"base_branch":body.base_branch,"base_sha":body.base_sha,"git_branch":body.git_branch}))))
}

async fn list_engineering_workspaces(
    State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>
) -> ApiResult<Json<Value>> {
    let principal=require_project_access(&state,&headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    task_project_scope(&state,&principal,&project_id,false).await?;
    let rows=sqlx::query("SELECT id,workspace_key,runtime_provider,repository,base_branch,base_sha,git_worktree_path,git_branch,head_sha,state,lease_owner,lease_expires_at,last_heartbeat_at,created_at,updated_at FROM engineering_workspaces WHERE project_id=$1 ORDER BY created_at DESC LIMIT 200")
        .bind(&project_id).fetch_all(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"engineering_workspace_list_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let values:Vec<Value>=rows.into_iter().map(|row|json!({
        "id":row.get::<String,_>("id"),"workspace_key":row.get::<String,_>("workspace_key"),
        "runtime_provider":row.get::<String,_>("runtime_provider"),"repository":row.get::<String,_>("repository"),
        "base_branch":row.get::<String,_>("base_branch"),"base_sha":row.get::<String,_>("base_sha"),
        "git_worktree_path":row.get::<Option<String>,_>("git_worktree_path"),
        "git_branch":row.get::<Option<String>,_>("git_branch"),"head_sha":row.get::<Option<String>,_>("head_sha"),
        "state":row.get::<String,_>("state"),"lease_owner":row.get::<Option<String>,_>("lease_owner"),
        "lease_expires_at":row.get::<Option<DateTime<Utc>>,_>("lease_expires_at"),
        "last_heartbeat_at":row.get::<Option<DateTime<Utc>>,_>("last_heartbeat_at"),
        "created_at":row.get::<DateTime<Utc>,_>("created_at"),"updated_at":row.get::<DateTime<Utc>,_>("updated_at")
    })).collect();
    Ok(Json(json!(values)))
}

#[derive(Deserialize)]
struct WorkspaceLeaseRequest {
    worker_id: String,
    #[serde(default)]
    lease_seconds: Option<i64>,
}

#[derive(Deserialize)]
struct WorkspaceLeaseBody {
    worker_id: String,
    lease_token: String,
    #[serde(default)]
    lease_seconds: Option<i64>,
    #[serde(default)]
    head_sha: Option<String>,
    #[serde(default)]
    git_worktree_path: Option<String>,
    #[serde(default)]
    git_branch: Option<String>,
    #[serde(default)]
    runtime_id: Option<String>,
}

#[derive(Deserialize)]
struct WorkspaceAssignmentBody {
    worker_id: String,
    task_lease_token: String,
    workspace_id: String,
    workspace_lease_token: String,
    worker_role: String,
    writer_scope: Vec<String>,
    base_sha: String,
    #[serde(default)]
    candidate_sha: Option<String>,
}

async fn assign_engineering_task_workspace(
    State(state): State<AppState>, headers: HeaderMap, Path(task_id): Path<String>,
    Json(body): Json<WorkspaceAssignmentBody>
) -> ApiResult<Json<Value>> {
    let principal=require_worker_identity(&state,&headers).await?;
    let project_id=principal.project_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_project_scope_required"))?;
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let worker_id=principal.worker_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_identity_required"))?;
    if body.worker_id!=worker_id || body.worker_role!=principal.worker_role.as_deref().unwrap_or("")
        || !["implementer","builder","tester","reviewer","recovery"].contains(&body.worker_role.as_str())
        || body.task_lease_token.len()<32 || body.workspace_lease_token.len()<32
        || Uuid::parse_str(&body.workspace_id).is_err()
        || body.base_sha.len()!=40 || !body.base_sha.chars().all(|c|c.is_ascii_hexdigit())
        || (body.writer_scope.is_empty() && body.worker_role!="reviewer") || body.writer_scope.len()>50
        || body.writer_scope.iter().any(|path| !safe_workspace_path_prefix(path))
        || (0..body.writer_scope.len()).any(|i|(i+1..body.writer_scope.len()).any(|j|workspace_paths_overlap(&body.writer_scope[i],&body.writer_scope[j])))
        || body.candidate_sha.as_ref().is_some_and(|sha|sha.len()!=40 || !sha.chars().all(|c|c.is_ascii_hexdigit()))
        || (body.worker_role=="reviewer" && body.candidate_sha.as_deref()!=Some(body.base_sha.as_str())) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY,"invalid_workspace_assignment"));
    }
    let task_lease_hash=hex::encode(Sha256::digest(body.task_lease_token.as_bytes()));
    let workspace_lease_hash=hex::encode(Sha256::digest(body.workspace_lease_token.as_bytes()));
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let task=sqlx::query("SELECT queue_name,lease_expires_at,candidate_sha FROM engineering_tasks WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND worker_id=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW() FOR UPDATE")
        .bind(&task_id).bind(tenant_id).bind(project_id).bind(worker_id).bind(&task_lease_hash)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_task_check_failed"))?;
    let Some(task)=task else{return Err(ApiError(StatusCode::CONFLICT,"task_lease_expired_or_invalid"));};
    let task_queue:String=task.try_get("queue_name").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_task_check_failed"))?;
    let task_candidate:Option<String>=task.try_get("candidate_sha").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_task_check_failed"))?;
    if body.worker_role=="reviewer" && body.candidate_sha.as_deref()!=task_candidate.as_deref() {
        return Err(ApiError(StatusCode::CONFLICT,"reviewer_candidate_sha_not_current"));
    }
    if principal.queue_name.as_deref()!=Some(task_queue.as_str()) {
        return Err(ApiError(StatusCode::FORBIDDEN,"worker_queue_scope_forbidden"));
    }
    let workspace=sqlx::query("SELECT base_sha,lease_expires_at FROM engineering_workspaces WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND state='leased' AND lease_owner=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW() FOR UPDATE")
        .bind(&body.workspace_id).bind(tenant_id).bind(project_id).bind(worker_id).bind(&workspace_lease_hash)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_workspace_check_failed"))?;
    let Some(workspace)=workspace else{return Err(ApiError(StatusCode::CONFLICT,"workspace_lease_expired_or_invalid"));};
    let workspace_base_sha:String=workspace.try_get("base_sha").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_workspace_check_failed"))?;
    if workspace_base_sha!=body.base_sha || (body.worker_role=="reviewer" && body.candidate_sha.as_deref()!=Some(workspace_base_sha.as_str())) {
        return Err(ApiError(StatusCode::CONFLICT,"workspace_base_sha_mismatch"));
    }
    let assignment_id=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO engineering_task_workspace_assignments (id,project_id,task_id,workspace_id,worker_role,writer_scope,base_sha,candidate_sha) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(&assignment_id).bind(project_id).bind(&task_id).bind(&body.workspace_id)
        .bind(&body.worker_role).bind(json!(body.writer_scope)).bind(&body.base_sha).bind(&body.candidate_sha)
        .execute(&mut *tx).await.map_err(|error| {
            if error.as_database_error().is_some_and(|db|db.code().as_deref()==Some("23505")) {
                ApiError(StatusCode::CONFLICT,"task_already_has_active_workspace")
            } else { ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_create_failed") }
        })?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1),hashtext($2))")
        .bind(tenant_id).bind(project_id).execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_path_lock_serialization_failed"))?;
    let existing_locks=sqlx::query("SELECT path_prefix FROM engineering_task_path_locks WHERE tenant_id=$1 AND project_id=$2 AND lease_expires_at>NOW()")
        .bind(tenant_id).bind(project_id).fetch_all(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_path_lock_check_failed"))?;
    for row in existing_locks {
        let existing_path:String=row.try_get("path_prefix").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_path_lock_check_failed"))?;
        if body.writer_scope.iter().any(|path|workspace_paths_overlap(path,&existing_path)) {
            return Err(ApiError(StatusCode::CONFLICT,"workspace_writer_scope_conflict"));
        }
    }
    for path_prefix in &body.writer_scope {
        let normalized_path=path_prefix.trim_end_matches('/');
        sqlx::query("INSERT INTO engineering_task_path_locks (id,project_id,task_id,workspace_id,path_prefix,lock_owner,lease_expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7)")
            .bind(Uuid::new_v4().to_string()).bind(project_id).bind(&task_id).bind(&body.workspace_id)
            .bind(normalized_path).bind(worker_id).bind(std::cmp::min(
                task.try_get::<DateTime<Utc>,_>("lease_expires_at").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_task_check_failed"))?,
                workspace.try_get::<DateTime<Utc>,_>("lease_expires_at").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_workspace_check_failed"))?
            ))
            .execute(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_path_lock_create_failed"))?;
    }
    let task_expires:DateTime<Utc>=task.try_get("lease_expires_at").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_task_check_failed"))?;
    let workspace_expires:DateTime<Utc>=workspace.try_get("lease_expires_at").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_workspace_check_failed"))?;
    let assignment_expires=std::cmp::min(task_expires,workspace_expires);
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_commit_failed"))?;
    audit(&state,&principal.actor,"engineering_task.workspace_assigned","engineering_task",&task_id,json!({"workspace_id":body.workspace_id,"assignment_id":assignment_id,"writer_scope":body.writer_scope})).await;
    Ok(Json(json!({"assignment_id":assignment_id,"task_id":task_id,"workspace_id":body.workspace_id,"worker_role":body.worker_role,"writer_scope":body.writer_scope,"candidate_sha":body.candidate_sha,"lease_expires_at":assignment_expires})))
}

#[derive(Deserialize)]
struct WorkspaceAssignmentReleaseBody {
    worker_id: String,
    task_lease_token: String,
    workspace_id: String,
    workspace_lease_token: String,
}

async fn release_engineering_task_workspace(
    State(state): State<AppState>, headers: HeaderMap, Path(task_id): Path<String>,
    Json(body): Json<WorkspaceAssignmentReleaseBody>
) -> ApiResult<StatusCode> {
    let principal=require_worker_identity(&state,&headers).await?;
    let project_id=principal.project_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_project_scope_required"))?;
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let worker_id=principal.worker_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_identity_required"))?;
    if body.worker_id!=worker_id || body.task_lease_token.len()<32 || body.workspace_lease_token.len()<32 {
        return Err(ApiError(StatusCode::FORBIDDEN,"workspace_assignment_worker_scope_forbidden"));
    }
    let task_hash=hex::encode(Sha256::digest(body.task_lease_token.as_bytes()));
    let workspace_hash=hex::encode(Sha256::digest(body.workspace_lease_token.as_bytes()));
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let valid=sqlx::query("SELECT t.id FROM engineering_tasks t JOIN engineering_workspaces w ON w.tenant_id=t.tenant_id AND w.project_id=t.project_id WHERE t.id=$1 AND t.tenant_id=$2 AND t.project_id=$3 AND t.worker_id=$4 AND t.lease_token_hash=$5 AND t.lease_expires_at>NOW() AND w.id=$6 AND w.lease_owner=$4 AND w.lease_token_hash=$7 AND w.lease_expires_at>NOW()")
        .bind(&task_id).bind(tenant_id).bind(project_id).bind(worker_id).bind(task_hash)
        .bind(&body.workspace_id).bind(workspace_hash).fetch_optional(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_lease_check_failed"))?;
    if valid.is_none(){return Err(ApiError(StatusCode::CONFLICT,"workspace_assignment_lease_expired_or_invalid"));}
    let released=sqlx::query("UPDATE engineering_task_workspace_assignments SET released_at=NOW() WHERE tenant_id=$1 AND project_id=$2 AND task_id=$3 AND workspace_id=$4 AND released_at IS NULL")
        .bind(tenant_id).bind(project_id).bind(&task_id).bind(&body.workspace_id)
        .execute(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_release_failed"))?;
    sqlx::query("DELETE FROM engineering_task_path_locks WHERE tenant_id=$1 AND project_id=$2 AND task_id=$3 AND workspace_id=$4")
        .bind(tenant_id).bind(project_id).bind(&task_id).bind(&body.workspace_id)
        .execute(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_path_lock_release_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_release_commit_failed"))?;
    audit(&state,&principal.actor,"engineering_task.workspace_released","engineering_task",&task_id,json!({"workspace_id":body.workspace_id,"released":released.rows_affected()})).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn lease_next_engineering_workspace(
    State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>,
    Json(body): Json<WorkspaceLeaseRequest>
) -> ApiResult<Json<Value>> {
    let principal=require_worker_identity(&state,&headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    if principal.project_id.as_deref()!=Some(project_id.as_str())
        || principal.worker_id.as_deref()!=Some(body.worker_id.as_str())
        || body.worker_id.trim().is_empty() {
        return Err(ApiError(StatusCode::FORBIDDEN,"workspace_worker_scope_forbidden"));
    }
    if !["implementer","builder","tester","reviewer","recovery"].contains(&principal.worker_role.as_deref().unwrap_or("")) {
        return Err(ApiError(StatusCode::FORBIDDEN,"worker_role_cannot_lease_workspace"));
    }
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let lease_seconds=body.lease_seconds.unwrap_or(300).clamp(30,1800);
    let raw:String=rand::rng().sample_iter(&Alphanumeric).take(48).map(char::from).collect();
    let hash=hex::encode(Sha256::digest(raw.as_bytes()));
    let row=sqlx::query(
        "WITH candidate AS (SELECT id FROM engineering_workspaces WHERE tenant_id=$1 AND project_id=$2 AND state='idle' ORDER BY updated_at,created_at FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE engineering_workspaces w SET state='leased',lease_owner=$3,lease_token_hash=$4,lease_expires_at=NOW()+($5::text||' seconds')::interval,last_heartbeat_at=NOW(),updated_at=NOW() FROM candidate WHERE w.id=candidate.id RETURNING w.id,w.workspace_key,w.repository,w.base_branch,w.base_sha,w.git_worktree_path,w.git_branch,w.state,w.lease_expires_at"
    ).bind(tenant_id).bind(&project_id).bind(&body.worker_id).bind(&hash).bind(lease_seconds)
     .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_lease_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let Some(row)=row else{return Ok(Json(json!({"workspace":null})));};
    let workspace_id:String=row.try_get("id").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_lease_failed"))?;
    audit(&state,&principal.actor,"engineering_workspace.lease","engineering_workspace",&workspace_id,json!({"worker_id":body.worker_id})).await;
    Ok(Json(json!({"workspace":{
        "id":workspace_id,"workspace_key":row.try_get::<String,_>("workspace_key").unwrap_or_default(),
        "repository":row.try_get::<String,_>("repository").unwrap_or_default(),
        "base_branch":row.try_get::<String,_>("base_branch").unwrap_or_default(),
        "base_sha":row.try_get::<String,_>("base_sha").unwrap_or_default(),
        "git_worktree_path":row.try_get::<Option<String>,_>("git_worktree_path").unwrap_or(None),
        "git_branch":row.try_get::<Option<String>,_>("git_branch").unwrap_or(None),
        "state":row.try_get::<String,_>("state").unwrap_or_default(),
        "lease_expires_at":row.try_get::<DateTime<Utc>,_>("lease_expires_at").ok()
    },"lease_token":raw})))
}

async fn heartbeat_engineering_workspace(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id,workspace_id)): Path<(String,String)>,
    Json(body): Json<WorkspaceLeaseBody>
) -> ApiResult<Json<Value>> {
    let principal=require_worker_identity(&state,&headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    if principal.project_id.as_deref()!=Some(project_id.as_str())
        || principal.worker_id.as_deref()!=Some(body.worker_id.as_str())
        || body.lease_token.len()<32
        || body.head_sha.as_ref().is_some_and(|sha|sha.len()!=40 || !sha.chars().all(|c|c.is_ascii_hexdigit()))
        || body.git_worktree_path.as_ref().is_some_and(|path|!valid_worktree_path(path))
        || body.git_branch.as_ref().is_some_and(|branch|branch.is_empty() || branch.len()>200 || branch.starts_with('/') || branch.split('/').any(|part|part.is_empty() || part=="." || part=="..") || !branch.chars().all(|c|c.is_ascii_alphanumeric() || "._/-".contains(c)))
        || body.runtime_id.as_ref().is_some_and(|id|id.is_empty() || id.len()>128 || id.chars().any(char::is_control)) {
        return Err(ApiError(StatusCode::FORBIDDEN,"workspace_worker_scope_forbidden"));
    }
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let hash=hex::encode(Sha256::digest(body.lease_token.as_bytes()));
    let lease_seconds=body.lease_seconds.unwrap_or(120).clamp(15,900);
    let row=sqlx::query("UPDATE engineering_workspaces SET lease_expires_at=NOW()+($6::text||' seconds')::interval,last_heartbeat_at=NOW(),head_sha=COALESCE($7,head_sha),git_worktree_path=COALESCE($8,git_worktree_path),git_branch=COALESCE($9,git_branch),metadata=CASE WHEN $10 IS NULL THEN metadata ELSE metadata||jsonb_build_object('runtime_id',$10) END,updated_at=NOW() WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND state='leased' AND lease_owner=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW() RETURNING lease_expires_at")
        .bind(&workspace_id).bind(tenant_id).bind(&project_id).bind(&body.worker_id).bind(hash)
        .bind(lease_seconds).bind(&body.head_sha).bind(&body.git_worktree_path).bind(&body.git_branch).bind(&body.runtime_id)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_heartbeat_failed"))?;
    let Some(row)=row else{return Err(ApiError(StatusCode::CONFLICT,"workspace_lease_expired_or_invalid"));};
    let workspace_expires:DateTime<Utc>=row.try_get("lease_expires_at")
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_heartbeat_failed"))?;
    sqlx::query("UPDATE engineering_task_path_locks l SET lease_expires_at=LEAST($1,t.lease_expires_at) FROM engineering_tasks t WHERE l.tenant_id=$2 AND l.project_id=$3 AND l.workspace_id=$4 AND l.task_id=t.id AND t.lease_expires_at>NOW()")
        .bind(workspace_expires).bind(tenant_id).bind(&project_id).bind(&workspace_id)
        .execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_path_lock_heartbeat_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    Ok(Json(json!({"workspace_id":workspace_id,"lease_expires_at":Some(workspace_expires)})))
}

async fn release_engineering_workspace(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id,workspace_id)): Path<(String,String)>,
    Json(body): Json<WorkspaceLeaseBody>
) -> ApiResult<StatusCode> {
    let principal=require_worker_identity(&state,&headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    if principal.project_id.as_deref()!=Some(project_id.as_str())
        || principal.worker_id.as_deref()!=Some(body.worker_id.as_str())
        || body.lease_token.len()<32 {
        return Err(ApiError(StatusCode::FORBIDDEN,"workspace_worker_scope_forbidden"));
    }
    if body.head_sha.as_ref().is_some_and(|v|v.len()!=40 || !v.chars().all(|c|c.is_ascii_hexdigit())) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY,"invalid_workspace_head_sha"));
    }
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let hash=hex::encode(Sha256::digest(body.lease_token.as_bytes()));
    let active_task=sqlx::query("SELECT a.task_id FROM engineering_task_workspace_assignments a JOIN engineering_tasks t ON t.id=a.task_id AND t.tenant_id=a.tenant_id AND t.project_id=a.project_id WHERE a.tenant_id=$1 AND a.project_id=$2 AND a.workspace_id=$3 AND a.released_at IS NULL AND t.lease_expires_at>NOW() LIMIT 1")
        .bind(tenant_id).bind(&project_id).bind(&workspace_id).fetch_optional(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_release_check_failed"))?;
    if active_task.is_some() {
        return Err(ApiError(StatusCode::CONFLICT,"active_task_workspace_assignment_must_be_released"));
    }
    sqlx::query("UPDATE engineering_task_workspace_assignments SET released_at=NOW() WHERE tenant_id=$1 AND project_id=$2 AND workspace_id=$3 AND released_at IS NULL")
        .bind(tenant_id).bind(&project_id).bind(&workspace_id).execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_assignment_recovery_release_failed"))?;
    sqlx::query("DELETE FROM engineering_task_path_locks WHERE tenant_id=$1 AND project_id=$2 AND workspace_id=$3")
        .bind(tenant_id).bind(&project_id).bind(&workspace_id).execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_path_lock_recovery_release_failed"))?;
    let result=sqlx::query("UPDATE engineering_workspaces SET state='idle',lease_owner=NULL,lease_token_hash=NULL,lease_expires_at=NULL,last_heartbeat_at=NOW(),head_sha=COALESCE($6,head_sha),updated_at=NOW() WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND state='leased' AND lease_owner=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW()")
        .bind(&workspace_id).bind(tenant_id).bind(&project_id).bind(&body.worker_id).bind(hash).bind(&body.head_sha)
        .execute(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"workspace_release_failed"))?;
    if result.rows_affected()==0{return Err(ApiError(StatusCode::CONFLICT,"workspace_lease_expired_or_invalid"));}
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state,&principal.actor,"engineering_workspace.release","engineering_workspace",&workspace_id,json!({"worker_id":body.worker_id})).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct TaskAttemptBody {
    worker_id: String,
    lease_token: String,
    #[serde(default)]
    provider_id: Option<String>,
    #[serde(default)]
    model_id: Option<String>,
    strategy: String,
    outcome: String,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    error_summary: Option<String>,
    acceptance_progress: f64,
    #[serde(default)]
    estimated_cost_minor: Option<i64>,
    #[serde(default)]
    actual_cost_minor: Option<i64>,
    #[serde(default)]
    metadata: Value,
}

fn evidence_has_nonempty_string(evidence: &Value, key: &str) -> bool {
    evidence.get(key).and_then(Value::as_str).is_some_and(|value|!value.trim().is_empty())
}

fn worker_completion_evidence_valid(
    role: &str,
    next_status: &str,
    candidate_sha: Option<&str>,
    evidence: &Value,
) -> bool {
    let candidate_matches=|key:&str| candidate_sha.is_some_and(|sha|evidence.get(key).and_then(Value::as_str)==Some(sha));
    match (role,next_status) {
        ("architect","queued") => evidence.get("plan").and_then(Value::as_object).is_some()
            && evidence.get("plan").and_then(|plan|plan.get("tasks")).and_then(Value::as_array).is_some_and(|tasks|!tasks.is_empty()),
        ("implementer","queued_build" | "queued_test" | "review") =>
            candidate_matches("commit_sha")
            && evidence.get("changed_files").and_then(Value::as_array).is_some_and(|files|!files.is_empty())
            && evidence.get("validation").and_then(Value::as_object).is_some(),
        ("builder","queued_test" | "review") =>
            evidence.get("build_status").and_then(Value::as_str)==Some("passed")
            && evidence_has_nonempty_string(evidence,"build_run_id")
            && candidate_matches("candidate_sha"),
        ("tester","review") =>
            evidence.get("test_status").and_then(Value::as_str)==Some("passed")
            && evidence_has_nonempty_string(evidence,"test_run_id")
            && candidate_matches("candidate_sha"),
        ("reviewer","verified") =>
            evidence.get("decision").and_then(Value::as_str)==Some("approved")
            && candidate_matches("reviewed_sha")
            && evidence.get("findings").and_then(Value::as_array).is_some(),
        ("recovery","queued" | "failed" | "blocked") =>
            evidence_has_nonempty_string(evidence,"recovery_reason")
            && evidence_has_nonempty_string(evidence,"recovery_action"),
        (_, "blocked" | "recovery" | "failed") =>
            evidence_has_nonempty_string(evidence,"reason")
            || evidence_has_nonempty_string(evidence,"error_code"),
        _ => false,
    }
}

fn valid_worktree_path(path: &str) -> bool {
    !path.ends_with('/')
        && safe_workspace_path_prefix(path)
        && !path.split('/').any(|part|part.eq_ignore_ascii_case(".git"))
        && path.chars().all(|c|c.is_ascii_alphanumeric() || "._/-".contains(c))
}

fn safe_workspace_path_prefix(path: &str) -> bool {
    if path.is_empty() || path.len()>500 || path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return false;
    }
    let normalized=path.trim_end_matches('/');
    !normalized.is_empty() && normalized.split('/').all(|part| !part.is_empty() && part!="." && part!="..")
}

fn workspace_paths_overlap(left: &str, right: &str) -> bool {
    let left=left.trim_end_matches('/');
    let right=right.trim_end_matches('/');
    left==right || left.strip_prefix(right).is_some_and(|suffix| suffix.starts_with('/'))
        || right.strip_prefix(left).is_some_and(|suffix| suffix.starts_with('/'))
}


#[derive(Deserialize)]
struct TaskPathLocksBody {
    worker_id: String,
    task_lease_token: String,
    workspace_id: String,
    workspace_lease_token: String,
    path_prefixes: Vec<String>,
}

async fn acquire_engineering_task_path_locks(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id,task_id)): Path<(String,String)>,
    Json(body): Json<TaskPathLocksBody>
) -> ApiResult<Json<Value>> {
    let principal=require_worker_identity(&state,&headers).await?;
    let worker_id=principal.worker_id.as_deref().unwrap_or("");
    if principal.project_id.as_deref()!=Some(project_id.as_str()) || worker_id!=body.worker_id
        || body.task_lease_token.len()<32 || body.workspace_lease_token.len()<32
        || Uuid::parse_str(&body.workspace_id).is_err()
        || body.path_prefixes.is_empty() || body.path_prefixes.len()>50
        || body.path_prefixes.iter().any(|path|!safe_workspace_path_prefix(path)) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY,"invalid_task_path_locks"));
    }
    if principal.worker_role.as_deref()==Some("reviewer") {
        return Err(ApiError(StatusCode::FORBIDDEN,"reviewer_workspace_is_read_only"));
    }
    for i in 0..body.path_prefixes.len() {
        for j in (i+1)..body.path_prefixes.len() {
            if workspace_paths_overlap(&body.path_prefixes[i],&body.path_prefixes[j]) {
                return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY,"requested_path_locks_overlap"));
            }
        }
    }
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1),hashtext($2))")
        .bind(tenant_id).bind(&project_id).execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_serialization_failed"))?;
    let task_hash=hex::encode(Sha256::digest(body.task_lease_token.as_bytes()));
    let workspace_hash=hex::encode(Sha256::digest(body.workspace_lease_token.as_bytes()));
    let task=sqlx::query("SELECT queue_name,lease_expires_at FROM engineering_tasks WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND worker_id=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW() FOR UPDATE")
        .bind(&task_id).bind(tenant_id).bind(&project_id).bind(worker_id).bind(task_hash)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_task_lease_check_failed"))?;
    let Some(task)=task else{return Err(ApiError(StatusCode::CONFLICT,"task_lease_expired_or_invalid"));};
    let task_queue:String=task.try_get("queue_name").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_task_lease_check_failed"))?;
    if principal.queue_name.as_deref()!=Some(task_queue.as_str()) {
        return Err(ApiError(StatusCode::FORBIDDEN,"worker_queue_scope_forbidden"));
    }
    let task_expiry:DateTime<Utc>=task.try_get("lease_expires_at").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_task_lease_check_failed"))?;
    let workspace=sqlx::query("SELECT lease_expires_at FROM engineering_workspaces WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND state='leased' AND lease_owner=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW() FOR UPDATE")
        .bind(&body.workspace_id).bind(tenant_id).bind(&project_id).bind(worker_id).bind(workspace_hash)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_workspace_lease_check_failed"))?;
    let Some(workspace)=workspace else{return Err(ApiError(StatusCode::CONFLICT,"workspace_lease_expired_or_invalid"));};
    let workspace_expiry:DateTime<Utc>=workspace.try_get("lease_expires_at").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_workspace_lease_check_failed"))?;
    let assignment=sqlx::query("SELECT writer_scope FROM engineering_task_workspace_assignments WHERE tenant_id=$1 AND project_id=$2 AND task_id=$3 AND workspace_id=$4 AND released_at IS NULL FOR UPDATE")
        .bind(tenant_id).bind(&project_id).bind(&task_id).bind(&body.workspace_id)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_assignment_check_failed"))?;
    let Some(assignment)=assignment else{return Err(ApiError(StatusCode::CONFLICT,"active_task_workspace_assignment_required"));};
    let writer_scope:Value=assignment.try_get("writer_scope").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_assignment_check_failed"))?;
    let allowed_scopes:Vec<String>=serde_json::from_value(writer_scope).unwrap_or_default();
    if body.path_prefixes.iter().any(|path|!allowed_scopes.iter().any(|scope|{
        let scope=scope.trim_end_matches('/');
        path==scope || path.starts_with(&format!("{scope}/"))
    })) {
        return Err(ApiError(StatusCode::FORBIDDEN,"path_lock_outside_assigned_writer_scope"));
    }
    let rows=sqlx::query("SELECT id,task_id,workspace_id,path_prefix,lock_owner FROM engineering_task_path_locks WHERE tenant_id=$1 AND project_id=$2 AND lease_expires_at>NOW()")
        .bind(tenant_id).bind(&project_id).fetch_all(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_conflict_check_failed"))?;
    let existing:Vec<(String,String,String,String,String)>=rows.into_iter().map(|row|{
        Ok((row.try_get("id")?,row.try_get("task_id")?,row.try_get("workspace_id")?,row.try_get("path_prefix")?,row.try_get("lock_owner")?))
    }).collect::<Result<_,sqlx::Error>>().map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_conflict_check_failed"))?;
    let expires_at=std::cmp::min(task_expiry,workspace_expiry);
    let mut ids=Vec::new();
    for path in &body.path_prefixes {
        let mut covered_lock:Option<String>=None;
        for (id,existing_task,existing_workspace,existing_path,existing_owner) in &existing {
            if !workspace_paths_overlap(path,existing_path) { continue; }
            let own_lock=existing_task==&task_id && existing_workspace==&body.workspace_id && existing_owner==worker_id;
            let covered_by_existing=path==existing_path || path.strip_prefix(existing_path).is_some_and(|suffix|suffix.starts_with('/'));
            if own_lock && covered_by_existing {
                covered_lock=Some(id.clone());
            } else {
                return Err(ApiError(StatusCode::CONFLICT,"path_scope_already_leased"));
            }
        }
        if let Some(id)=covered_lock {
            ids.push(id);
        } else {
            let id=Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO engineering_task_path_locks (id,project_id,task_id,workspace_id,path_prefix,lock_owner,lease_expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7)")
                .bind(&id).bind(&project_id).bind(&task_id).bind(&body.workspace_id).bind(path).bind(worker_id).bind(expires_at)
                .execute(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_write_failed"))?;
            ids.push(id);
        }
    }
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state,&principal.actor,"engineering_task.path_lock","engineering_task",&task_id,json!({"workspace_id":body.workspace_id,"paths":body.path_prefixes})).await;
    Ok(Json(json!({"task_id":task_id,"workspace_id":body.workspace_id,"lock_ids":ids,"lease_expires_at":expires_at})))
}

async fn release_engineering_task_path_locks(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id,task_id)): Path<(String,String)>,
    Json(body): Json<TaskLeaseBody>
) -> ApiResult<Json<Value>> {
    let principal=require_worker_identity(&state,&headers).await?;
    if principal.project_id.as_deref()!=Some(project_id.as_str())
        || principal.worker_id.as_deref()!=Some(body.worker_id.as_str())
        || body.lease_token.len()<32 {
        return Err(ApiError(StatusCode::FORBIDDEN,"path_lock_worker_scope_forbidden"));
    }
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let hash=hex::encode(Sha256::digest(body.lease_token.as_bytes()));
    let task=sqlx::query("SELECT id FROM engineering_tasks WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND worker_id=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW()")
        .bind(&task_id).bind(tenant_id).bind(&project_id).bind(&body.worker_id).bind(hash)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_task_lease_check_failed"))?;
    if task.is_none(){return Err(ApiError(StatusCode::CONFLICT,"task_lease_expired_or_invalid"));}
    let result=sqlx::query("DELETE FROM engineering_task_path_locks WHERE tenant_id=$1 AND project_id=$2 AND task_id=$3 AND lock_owner=$4")
        .bind(tenant_id).bind(&project_id).bind(&task_id).bind(&body.worker_id).execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"path_lock_release_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    Ok(Json(json!({"task_id":task_id,"released":result.rows_affected()})))
}

#[derive(Deserialize)]
struct TaskCompletionBody {
    worker_id: String,
    task_lease_token: String,
    workspace_id: String,
    workspace_lease_token: String,
    next_status: String,
    #[serde(default)]
    candidate_sha: Option<String>,
    #[serde(default)]
    evidence: Value,
}

async fn complete_engineering_task(
    State(state): State<AppState>, headers: HeaderMap, Path(task_id): Path<String>,
    Json(body): Json<TaskCompletionBody>
) -> ApiResult<Json<Value>> {
    let principal=require_worker_identity(&state,&headers).await?;
    let worker_id=principal.worker_id.as_deref().unwrap_or("");
    let role=principal.worker_role.as_deref().unwrap_or("");
    let project_id=principal.project_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_project_scope_required"))?;
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let allowed=match role {
        "architect" => ["queued","blocked","recovery"].contains(&body.next_status.as_str()),
        "implementer" => ["queued_build","queued_test","review","blocked","recovery"].contains(&body.next_status.as_str()),
        "builder" => ["queued_test","review","blocked","recovery"].contains(&body.next_status.as_str()),
        "tester" => ["review","blocked","recovery"].contains(&body.next_status.as_str()),
        "reviewer" => ["verified","blocked","recovery"].contains(&body.next_status.as_str()),
        "recovery" => ["queued","failed","blocked"].contains(&body.next_status.as_str()),
        _ => false,
    };
    if !allowed || body.worker_id!=worker_id || body.task_lease_token.len()<32
        || body.workspace_lease_token.len()<32 || Uuid::parse_str(&body.workspace_id).is_err()
        || !body.evidence.is_object() || body.evidence.to_string().len()>16_384
        || metadata_contains_sensitive_key(&body.evidence)
        || body.candidate_sha.as_ref().is_some_and(|sha|sha.len()!=40 || !sha.chars().all(|c|c.is_ascii_hexdigit()))
        || (["queued_build","queued_test","review","verified"].contains(&body.next_status.as_str()) && body.candidate_sha.is_none())
        || !worker_completion_evidence_valid(role,&body.next_status,body.candidate_sha.as_deref(),&body.evidence) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY,"invalid_worker_task_completion"));
    }
    let queue=match body.next_status.as_str() {
        "queued" => "implementation",
        "queued_build" => "build",
        "queued_test" => "test",
        "review" | "verified" => "review",
        "blocked" | "recovery" | "failed" => "recovery",
        _ => return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY,"invalid_worker_task_completion")),
    };
    let task_hash=hex::encode(Sha256::digest(body.task_lease_token.as_bytes()));
    let workspace_hash=hex::encode(Sha256::digest(body.workspace_lease_token.as_bytes()));
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let task=sqlx::query("SELECT queue_name,status,candidate_sha FROM engineering_tasks WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND worker_id=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW() FOR UPDATE")
        .bind(&task_id).bind(tenant_id).bind(project_id).bind(worker_id).bind(&task_hash)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_lease_check_failed"))?;
    let Some(task)=task else{return Err(ApiError(StatusCode::CONFLICT,"task_lease_expired_or_invalid"));};
    let task_queue:String=task.try_get("queue_name").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_lease_check_failed"))?;
    let current_status:String=task.try_get("status").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_lease_check_failed"))?;
    let existing_candidate:Option<String>=task.try_get("candidate_sha").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_lease_check_failed"))?;
    if principal.queue_name.as_deref()!=Some(task_queue.as_str()) {
        return Err(ApiError(StatusCode::FORBIDDEN,"worker_queue_scope_forbidden"));
    }
    let workspace=sqlx::query("SELECT id FROM engineering_workspaces WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND state='leased' AND lease_owner=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW() FOR UPDATE")
        .bind(&body.workspace_id).bind(tenant_id).bind(project_id).bind(worker_id).bind(&workspace_hash)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_workspace_check_failed"))?;
    if workspace.is_none(){return Err(ApiError(StatusCode::CONFLICT,"workspace_lease_expired_or_invalid"));}
    let assignment=sqlx::query("SELECT worker_role,writer_scope,candidate_sha FROM engineering_task_workspace_assignments WHERE tenant_id=$1 AND project_id=$2 AND task_id=$3 AND workspace_id=$4 AND released_at IS NULL FOR UPDATE")
        .bind(tenant_id).bind(project_id).bind(&task_id).bind(&body.workspace_id)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_assignment_check_failed"))?;
    let Some(assignment)=assignment else{return Err(ApiError(StatusCode::CONFLICT,"active_task_workspace_assignment_required"));};
    let assigned_role:String=assignment.try_get("worker_role").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_assignment_check_failed"))?;
    let assigned_scope:Value=assignment.try_get("writer_scope").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_assignment_check_failed"))?;
    let assigned_candidate:Option<String>=assignment.try_get("candidate_sha").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_assignment_check_failed"))?;
    if assigned_role!=role{return Err(ApiError(StatusCode::FORBIDDEN,"workspace_assignment_role_mismatch"));}
    if body.next_status=="verified" {
        if role!="reviewer" || assigned_scope.as_array().is_none_or(|scope|!scope.is_empty())
            || body.candidate_sha.as_deref()!=existing_candidate.as_deref()
            || assigned_candidate.as_deref()!=existing_candidate.as_deref() {
            return Err(ApiError(StatusCode::FORBIDDEN,"reviewer_candidate_not_pinned_or_read_only"));
        }
        let prior=sqlx::query("SELECT worker_id FROM engineering_task_attempts WHERE tenant_id=$1 AND project_id=$2 AND task_id=$3 AND worker_role IN ('implementer','builder','tester') ORDER BY attempt_no DESC LIMIT 1")
            .bind(tenant_id).bind(project_id).bind(&task_id).fetch_optional(&mut *tx).await
            .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"reviewer_independence_check_failed"))?;
        let Some(prior)=prior else{return Err(ApiError(StatusCode::CONFLICT,"independent_implementation_attempt_required"));};
        let prior_worker:String=prior.try_get("worker_id").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"reviewer_independence_check_failed"))?;
        if prior_worker==worker_id{return Err(ApiError(StatusCode::FORBIDDEN,"reviewer_must_be_independent"));}
    } else if body.candidate_sha.is_some() && existing_candidate.as_deref().is_some_and(|existing|body.candidate_sha.as_deref()!=Some(existing)) && role!="implementer" {
        return Err(ApiError(StatusCode::CONFLICT,"candidate_sha_change_requires_implementer"));
    }
    if !["blocked","recovery","failed"].contains(&body.next_status.as_str())
        && !matches!(
            (current_status.as_str(),body.next_status.as_str()),
            ("in_progress","queued") | ("in_progress","queued_build") | ("in_progress","queued_test")
            | ("in_progress","review") | ("building","queued_test") | ("building","review")
            | ("testing","review") | ("review","verified") | ("recovery","queued") | ("recovery","failed")
            | ("blocked","queued") | ("failed","queued")
        ) {
        return Err(ApiError(StatusCode::CONFLICT,"invalid_worker_task_state_transition"));
    }
    let updated=sqlx::query("UPDATE engineering_tasks SET status=$1,queue_name=$2,candidate_sha=COALESCE($3,candidate_sha),worker_id=NULL,lease_token_hash=NULL,lease_expires_at=NULL,claimed_at=NULL,available_at=NOW(),version=version+1,updated_at=NOW() WHERE id=$4 AND tenant_id=$5 AND project_id=$6 RETURNING version")
        .bind(&body.next_status).bind(queue).bind(&body.candidate_sha).bind(&task_id).bind(tenant_id).bind(project_id)
        .fetch_one(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_write_failed"))?;
    sqlx::query("INSERT INTO engineering_task_events (id,task_id,project_id,actor,event_type,payload) VALUES ($1,$2,$3,$4,'task.worker_completed',$5)")
        .bind(Uuid::new_v4().to_string()).bind(&task_id).bind(project_id).bind(&principal.actor)
        .bind(json!({"from":current_status,"to":body.next_status,"queue":queue,"candidate_sha":body.candidate_sha,"evidence":body.evidence}))
        .execute(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_event_failed"))?;
    sqlx::query("UPDATE engineering_task_workspace_assignments SET released_at=NOW() WHERE tenant_id=$1 AND project_id=$2 AND task_id=$3 AND workspace_id=$4 AND released_at IS NULL")
        .bind(tenant_id).bind(project_id).bind(&task_id).bind(&body.workspace_id).execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_assignment_release_failed"))?;
    sqlx::query("DELETE FROM engineering_task_path_locks WHERE tenant_id=$1 AND project_id=$2 AND task_id=$3 AND workspace_id=$4")
        .bind(tenant_id).bind(project_id).bind(&task_id).bind(&body.workspace_id).execute(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_completion_path_lock_release_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    audit(&state,&principal.actor,"engineering_task.complete","engineering_task",&task_id,json!({"status":body.next_status,"candidate_sha":body.candidate_sha})).await;
    Ok(Json(json!({"task_id":task_id,"status":body.next_status,"queue":queue,"version":updated.try_get::<i64,_>("version").unwrap_or(0),"candidate_sha":body.candidate_sha})))
}

async fn record_engineering_task_attempt(
    State(state): State<AppState>, headers: HeaderMap, Path(task_id): Path<String>,
    Json(body): Json<TaskAttemptBody>
) -> ApiResult<Json<Value>> {
    let principal=require_worker_identity(&state,&headers).await?;
    if principal.worker_id.as_deref()!=Some(body.worker_id.as_str())
        || body.lease_token.len()<32
        || body.strategy.trim().is_empty() || body.strategy.len()>80
        || !["started","succeeded","failed","yielded","abandoned"].contains(&body.outcome.as_str())
        || !body.acceptance_progress.is_finite() || !(0.0..=1.0).contains(&body.acceptance_progress)
        || body.provider_id.as_ref().is_some_and(|v|v.is_empty() || v.len()>80)
        || body.model_id.as_ref().is_some_and(|v|v.is_empty() || v.len()>160)
        || body.error_code.as_ref().is_some_and(|v|v.len()>120)
        || body.error_summary.as_ref().is_some_and(|v|v.len()>2000)
        || body.estimated_cost_minor.is_some_and(|v|v<0)
        || body.actual_cost_minor.is_some_and(|v|v<0)
        || body.metadata.to_string().len()>8192 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY,"invalid_task_attempt"));
    }
    let project_id=principal.project_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_project_scope_required"))?;
    let tenant_id=principal.tenant_id.as_deref().ok_or(ApiError(StatusCode::FORBIDDEN,"worker_tenant_scope_required"))?;
    let lease_hash=hex::encode(Sha256::digest(body.lease_token.as_bytes()));
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let row=sqlx::query("SELECT model_attempts,queue_name FROM engineering_tasks WHERE id=$1 AND tenant_id=$2 AND project_id=$3 AND worker_id=$4 AND lease_token_hash=$5 AND lease_expires_at>NOW() FOR UPDATE")
        .bind(&task_id).bind(tenant_id).bind(project_id).bind(&body.worker_id).bind(&lease_hash)
        .fetch_optional(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_attempt_lease_check_failed"))?;
    let Some(row)=row else{return Err(ApiError(StatusCode::CONFLICT,"task_lease_expired_or_invalid"));};
    let task_queue:String=row.try_get("queue_name").map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_attempt_lease_check_failed"))?;
    if principal.queue_name.as_deref()!=Some(task_queue.as_str()) {
        return Err(ApiError(StatusCode::FORBIDDEN,"worker_queue_scope_forbidden"));
    }
    let attempt_no:i32=row.try_get::<i32,_>("model_attempts").unwrap_or(0)+1;
    let attempt_id=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO engineering_task_attempts (id,tenant_id,project_id,task_id,attempt_no,worker_id,worker_role,provider_id,model_id,strategy,outcome,error_code,error_summary,acceptance_progress,estimated_cost_minor,actual_cost_minor,metadata,finished_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,($14::double precision)::numeric,$15,$16,$17,CASE WHEN $11='started' THEN NULL ELSE NOW() END)")
        .bind(&attempt_id).bind(tenant_id).bind(project_id).bind(&task_id).bind(attempt_no)
        .bind(&body.worker_id).bind(principal.worker_role.as_deref().unwrap_or("implementer"))
        .bind(&body.provider_id).bind(&body.model_id).bind(&body.strategy).bind(&body.outcome)
        .bind(&body.error_code).bind(&body.error_summary).bind(body.acceptance_progress)
        .bind(body.estimated_cost_minor).bind(body.actual_cost_minor).bind(&body.metadata)
        .execute(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_attempt_write_failed"))?;
    sqlx::query("UPDATE engineering_tasks SET model_attempts=$1,last_model_id=COALESCE($2,last_model_id),last_strategy=$3,acceptance_progress=($4::double precision)::numeric,blocked_reason=CASE WHEN $5='failed' THEN $6 ELSE NULL END,updated_at=NOW(),version=version+1 WHERE id=$7 AND tenant_id=$8 AND project_id=$9")
        .bind(attempt_no).bind(&body.model_id).bind(&body.strategy).bind(body.acceptance_progress)
        .bind(&body.outcome).bind(&body.error_summary).bind(&task_id).bind(tenant_id).bind(project_id)
        .execute(&mut *tx).await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_attempt_update_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_attempt_commit_failed"))?;
    audit(&state,&principal.actor,"engineering_task.attempt","engineering_task",&task_id,json!({"attempt_id":attempt_id,"attempt_no":attempt_no,"model_id":body.model_id,"strategy":body.strategy,"outcome":body.outcome})).await;
    Ok(Json(json!({"id":attempt_id,"task_id":task_id,"attempt_no":attempt_no,"outcome":body.outcome,"acceptance_progress":body.acceptance_progress})))
}

async fn list_engineering_task_attempts(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id,task_id)): Path<(String,String)>
) -> ApiResult<Json<Value>> {
    let principal=authenticate(&state,&headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    if principal.worker_id.is_some() {
        if principal.project_id.as_deref()!=Some(project_id.as_str()) {
            return Err(ApiError(StatusCode::FORBIDDEN,"project_scope_forbidden"));
        }
    } else {
        task_project_scope(&state,&principal,&project_id,false).await?;
    }
    let rows=sqlx::query("SELECT id,attempt_no,worker_id,worker_role,provider_id,model_id,strategy,outcome,error_code,error_summary,acceptance_progress::text AS acceptance_progress,estimated_cost_minor,actual_cost_minor,started_at,finished_at FROM engineering_task_attempts WHERE tenant_id=(SELECT tenant_id FROM projects WHERE id=$1) AND project_id=$1 AND task_id=$2 ORDER BY attempt_no ASC")
        .bind(&project_id).bind(&task_id).fetch_all(&mut *tx).await
        .map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"task_attempt_list_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let values:Vec<Value>=rows.into_iter().map(|row|json!({
        "id":row.get::<String,_>("id"),"attempt_no":row.get::<i32,_>("attempt_no"),
        "worker_id":row.get::<String,_>("worker_id"),"worker_role":row.get::<String,_>("worker_role"),
        "provider_id":row.get::<Option<String>,_>("provider_id"),"model_id":row.get::<Option<String>,_>("model_id"),
        "strategy":row.get::<String,_>("strategy"),"outcome":row.get::<String,_>("outcome"),
        "error_code":row.get::<Option<String>,_>("error_code"),"error_summary":row.get::<Option<String>,_>("error_summary"),
        "acceptance_progress":row.get::<String,_>("acceptance_progress"),
        "estimated_cost_minor":row.get::<Option<i64>,_>("estimated_cost_minor"),
        "actual_cost_minor":row.get::<Option<i64>,_>("actual_cost_minor"),
        "started_at":row.get::<DateTime<Utc>,_>("started_at"),"finished_at":row.get::<Option<DateTime<Utc>>,_>("finished_at")
    })).collect();
    Ok(Json(json!(values)))
}

async fn list_providers(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query("SELECT id,name,capabilities,priority,enabled,adapter_type,model,base_url,health_status,checked_at,latency_ms FROM providers ORDER BY priority DESC,id")
        .fetch_all(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "provider_list_failed"))?;
    let values: Vec<Value> = rows.into_iter().map(|r| json!({
        "id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),
        "capabilities":r.get::<Value,_>("capabilities"),"priority":r.get::<i32,_>("priority"),
        "enabled":r.get::<bool,_>("enabled"),"adapter_type":r.get::<String,_>("adapter_type"),
        "model":r.get::<Option<String>,_>("model"),"base_url":r.get::<Option<String>,_>("base_url"),
        "health_status":r.get::<String,_>("health_status"),"checked_at":r.get::<Option<DateTime<Utc>>,_>("checked_at"),
        "latency_ms":r.get::<Option<f64>,_>("latency_ms")
    })).collect();
    Ok(Json(json!(values)))
}


#[derive(Deserialize)]
struct S3CompatibleCredentials {
    endpoint_url: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
    #[serde(default)]
    session_token: Option<String>,
    #[serde(default)]
    bucket: Option<String>,
}

fn parse_s3_compatible_credentials(raw: &str) -> ApiResult<S3CompatibleCredentials> {
    let credentials: S3CompatibleCredentials = serde_json::from_str(raw)
        .map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_s3_credential_config"))?;
    if credentials.endpoint_url.is_empty()
        || credentials.endpoint_url.len() > 2048
        || credentials.region.is_empty()
        || credentials.region.len() > 64
        || !credentials.region.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        || credentials.access_key_id.is_empty()
        || credentials.access_key_id.len() > 256
        || credentials.secret_access_key.is_empty()
        || credentials.secret_access_key.len() > 1024
        || credentials.session_token.as_ref().is_some_and(|v| v.is_empty() || v.len() > 4096)
        || credentials.bucket.as_deref().is_some_and(|bucket| !valid_s3_bucket_name(bucket))
    {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_s3_credential_config"));
    }
    Ok(credentials)
}

fn valid_s3_bucket_name(bucket: &str) -> bool {
    let bytes = bucket.as_bytes();
    bytes.len() >= 3
        && bytes.len() <= 63
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.iter().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-' || *byte == b'.')
        && !bucket.contains("..")
        && bucket.parse::<IpAddr>().is_err()
}

#[derive(Deserialize)]
struct ProviderUpsert {
    name: String,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default = "default_priority")]
    priority: i32,
    #[serde(default = "default_true")]
    enabled: bool,
    health_url: Option<String>,
    #[serde(default = "default_adapter")]
    adapter_type: String,
    model: Option<String>,
    base_url: Option<String>,
    api_key_env: Option<String>,
    #[serde(default)]
    supports_idempotency: bool,
    #[serde(default = "default_timeout")]
    timeout_seconds: f64,
}
fn default_priority() -> i32 { 100 }
fn default_true() -> bool { true }
fn default_adapter() -> String { "none".into() }
fn default_timeout() -> f64 { 60.0 }

fn is_ai_adapter(adapter: &str) -> bool {
    ["openai_compatible", "anthropic", "gemini"].contains(&adapter)
}

fn is_control_api_adapter(adapter: &str) -> bool {
    ["northflank", "railway", "cloudflare", "netlify", "render", "digitalocean", "flyio", "supabase", "neon", "cloudflare_r2", "backblaze_b2", "postgresql", "redis", "supabase_storage", "s3_compatible"].contains(&adapter)
}

fn validate_https_url(raw: &str) -> Result<url::Url, ApiError> {
    let parsed = url::Url::parse(raw).map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_provider_url"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some() || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unsafe_provider_url"));
    }
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || host.ends_with(".internal") || host == "metadata.google.internal" {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unsafe_provider_host"));
    }
    if let Ok(ip) = host.parse::<IpAddr>() && !is_public_ip(ip) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "non_public_provider_ip"));
    }
    Ok(parsed)
}

fn validate_fixed_integration_url(raw: &str) -> Result<url::Url, ApiError> {
    let parsed = url::Url::parse(raw).map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "invalid_internal_integration_url"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some() || parsed.fragment().is_some() {
        return Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, "invalid_internal_integration_url"));
    }
    Ok(parsed)
}

fn parse_postgres_url(raw: &str) -> Result<url::Url, ApiError> {
    let parsed = url::Url::parse(raw).map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_database_url"))?;
    if !["postgres", "postgresql"].contains(&parsed.scheme()) || parsed.host_str().is_none() || parsed.fragment().is_some() {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_database_url"));
    }
    Ok(parsed)
}

fn postgres_connection_url(raw: &str) -> Result<url::Url, ApiError> {
    let mut parsed = parse_postgres_url(raw)?;
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed)
}

fn parse_redis_url(raw: &str, private_host_allowlist: &str) -> Result<url::Url, ApiError> {
    let parsed = url::Url::parse(raw).map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_redis_url"))?;
    let host = parsed.host_str().ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_redis_url"))?;
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_redis_url"));
    }
    let secure = parsed.scheme() == "rediss";
    let allowlisted_private = parsed.scheme() == "redis" && is_allowed_private_host(host, private_host_allowlist);
    if !secure && !allowlisted_private {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "redis_tls_required_unless_private_host_allowlisted"));
    }
    Ok(parsed)
}

fn is_private_network_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            octets[0] == 10
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 168)
        }
        IpAddr::V6(ip) => (ip.segments()[0] & 0xfe00) == 0xfc00,
    }
}

fn is_allowed_private_host(host: &str, allowlist: &str) -> bool {
    if host.parse::<IpAddr>().is_ok() { return false; }
    allowlist.split(',').any(|entry| entry.trim().eq_ignore_ascii_case(host))
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_broadcast() || v4.is_unspecified() || v4.is_multicast()
                || o[0] == 0 || (o[0] == 100 && (64..=127).contains(&o[1]))
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                || (o[0] == 192 && o[1] == 0 && o[2] == 2)
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
                || (o[0] == 198 && o[1] == 51 && o[2] == 100)
                || (o[0] == 203 && o[1] == 0 && o[2] == 113)
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            !(v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() || v6.is_unique_local() || v6.is_unicast_link_local()
                || (segments[0] & 0xe000) != 0x2000)
        }
    }
}

async fn ensure_public_destination(url: &url::Url) -> ApiResult<()> {
    let host = url.host_str().ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_provider_host"))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let resolved = tokio::net::lookup_host((host, port)).await
        .map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "provider_dns_failed"))?;
    let addresses: Vec<SocketAddr> = resolved.collect();
    if addresses.is_empty() {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unsafe_provider_dns_result"));
    }
    let all_public = addresses.iter().all(|addr| is_public_ip(addr.ip()));
    let all_private = addresses.iter().all(|addr| is_private_network_ip(addr.ip()));
    let allowlist = env::var("USH_ALLOWED_PRIVATE_HOSTS").unwrap_or_default();
    let private_allowed = all_private && is_allowed_private_host(host, &allowlist);
    if (url.scheme() == "redis" && !private_allowed) || (!all_public && !private_allowed) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unsafe_provider_dns_result"));
    }
    Ok(())
}

async fn upsert_provider(State(state): State<AppState>, headers: HeaderMap, Path(provider_id): Path<String>, Json(body): Json<ProviderUpsert>) -> ApiResult<StatusCode> {
    let principal = require_admin(&state, &headers).await?;
    if provider_id.is_empty() || provider_id.len() > 80 || !provider_id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        || body.name.trim().is_empty() || body.name.len() > 160 || body.capabilities.len() > 100 || body.timeout_seconds <= 0.0 || body.timeout_seconds > 120.0 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_provider"));
    }
    if body.adapter_type != "none" && !is_ai_adapter(&body.adapter_type) && !is_control_api_adapter(&body.adapter_type) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unsupported_adapter_type"));
    }
    if let Some(url) = body.base_url.as_deref() { validate_https_url(url)?; }
    if let Some(url) = body.health_url.as_deref() { validate_https_url(url)?; }
    if is_ai_adapter(&body.adapter_type) {
        if body.model.as_deref().is_none_or(str::is_empty) || body.base_url.is_none() || body.api_key_env.as_deref().is_none_or(|v| !(2..=120).contains(&v.len()) || !v.chars().next().is_some_and(|c| c.is_ascii_uppercase()) || !v.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')) {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "incomplete_ai_provider"));
        }
        if body.adapter_type == "gemini" && body.supports_idempotency {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "gemini_idempotency_not_supported"));
        }
    } else if is_control_api_adapter(&body.adapter_type) {
        if body.api_key_env.as_deref().is_none_or(|v| v.len() < 2 || v.len() > 120 || !v.chars().next().is_some_and(|c| c.is_ascii_uppercase()) || !v.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')) {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "integration_credential_reference_required"));
        }
        if body.supports_idempotency {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "control_api_idempotency_not_applicable"));
        }
    }
    sqlx::query("INSERT INTO providers (id,name,capabilities,priority,enabled,health_url,adapter_type,model,base_url,api_key_env,supports_idempotency,timeout_seconds,health_status,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'unknown',NOW(),NOW()) ON CONFLICT (id) DO UPDATE SET name=EXCLUDED.name,capabilities=EXCLUDED.capabilities,priority=EXCLUDED.priority,enabled=EXCLUDED.enabled,health_url=EXCLUDED.health_url,adapter_type=EXCLUDED.adapter_type,model=EXCLUDED.model,base_url=EXCLUDED.base_url,api_key_env=EXCLUDED.api_key_env,supports_idempotency=EXCLUDED.supports_idempotency,timeout_seconds=EXCLUDED.timeout_seconds,updated_at=NOW()")
        .bind(&provider_id).bind(&body.name).bind(json!(body.capabilities)).bind(body.priority).bind(body.enabled).bind(&body.health_url)
        .bind(&body.adapter_type).bind(&body.model).bind(&body.base_url).bind(&body.api_key_env).bind(body.supports_idempotency).bind(body.timeout_seconds)
        .execute(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "provider_upsert_failed"))?;
    audit(&state, &principal.actor, "provider.upsert", "provider", &provider_id, json!({"adapter_type":body.adapter_type,"capabilities":body.capabilities})).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn verify_integration(State(state): State<AppState>, headers: HeaderMap, Path(provider_id): Path<String>) -> ApiResult<Json<Value>> {
    let principal = require_admin(&state, &headers).await?;
    let row = sqlx::query("SELECT adapter_type,api_key_env,enabled FROM providers WHERE id=$1")
        .bind(&provider_id).fetch_optional(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "integration_lookup_failed"))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "provider_not_found"))?;
    let adapter: String = row.get("adapter_type");
    let enabled: bool = row.get("enabled");
    let key_name: Option<String> = row.get("api_key_env");
    if !enabled || !is_control_api_adapter(&adapter) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "control_api_integration_not_configured"));
    }
    let key_name = key_name.ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "integration_credential_reference_missing"))?;
    let token = resolve_provider_secret(&state, &key_name, None).await?
        .ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "integration_credential_unavailable"))?;
    let started = Instant::now();

    if adapter == "supabase_storage" {
        let config: Value = serde_json::from_str(&token).map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_supabase_storage_config"))?;
        let project_url = config["project_url"].as_str().filter(|v| !v.is_empty())
            .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "supabase_storage_project_url_required"))?;
        let service_role_key = config["service_role_key"].as_str().filter(|v| !v.is_empty())
            .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "supabase_storage_key_required"))?;
        let base = validate_https_url(project_url)?;
        if base.path() != "/" {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_supabase_project_url"));
        }
        ensure_public_destination(&base).await?;
        let endpoint = format!("{}/storage/v1/bucket", project_url.trim_end_matches('/'));
        let response = state.http.get(endpoint).header("apikey", service_role_key).bearer_auth(service_role_key)
            .timeout(Duration::from_secs(8)).send().await
            .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "supabase_storage_request_failed"))?;
        let status = response.status();
        if !status.is_success() {
            let code = if status == StatusCode::UNAUTHORIZED { StatusCode::UNAUTHORIZED } else if status == StatusCode::FORBIDDEN { StatusCode::FORBIDDEN } else if status == StatusCode::TOO_MANY_REQUESTS { StatusCode::TOO_MANY_REQUESTS } else { StatusCode::BAD_GATEWAY };
            audit(&state, &principal.actor, "integration.verify.failure", "provider", &provider_id, json!({"adapter":"supabase_storage","upstream_status":status.as_u16()})).await;
            return Err(ApiError(code, if code == StatusCode::UNAUTHORIZED { "integration_authentication_failed" } else if code == StatusCode::FORBIDDEN { "integration_permission_denied" } else if code == StatusCode::TOO_MANY_REQUESTS { "integration_rate_limited" } else { "integration_upstream_failed" }));
        }
        let buckets: Value = response.json().await.map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response"))?;
        let buckets = buckets.as_array().ok_or(ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response"))?;
        let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
        audit(&state, &principal.actor, "integration.verify.success", "provider", &provider_id, json!({"adapter":"supabase_storage","latency_ms":latency_ms,"resource_count":buckets.len()})).await;
        return Ok(Json(json!({"provider_id":provider_id,"adapter":"supabase_storage","connected":true,"verified":true,"resource_count":buckets.len(),"account_label":buckets.first().and_then(|v| v["name"].as_str()),"latency_ms":latency_ms})));
    }

    if adapter == "s3_compatible" {
        let verification: ApiResult<usize> = async {
            let credentials = parse_s3_compatible_credentials(&token)?;
            let endpoint = validate_https_url(&credentials.endpoint_url)?;
            if endpoint.path() != "/" {
                return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_s3_endpoint"));
            }
            ensure_public_destination(&endpoint).await?;
            let sdk_credentials = aws_credential_types::Credentials::new(
                credentials.access_key_id,
                credentials.secret_access_key,
                credentials.session_token,
                None,
                "universal-service-hub",
            );
            let sdk_config = aws_sdk_s3::Config::builder()
                .region(aws_sdk_s3::config::Region::new(credentials.region))
                .endpoint_url(endpoint.as_str())
                .credentials_provider(sdk_credentials)
                .force_path_style(true)
                .build();
            let s3_client = aws_sdk_s3::Client::from_conf(sdk_config);
            let bucket_count = if let Some(bucket) = credentials.bucket.as_deref() {
                tokio::time::timeout(
                    Duration::from_secs(15),
                    s3_client.head_bucket().bucket(bucket).send(),
                )
                .await
                .map_err(|_| ApiError(StatusCode::GATEWAY_TIMEOUT, "s3_head_bucket_timeout"))?
                .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "s3_authentication_or_request_failed"))?;
                1
            } else {
                let output = tokio::time::timeout(Duration::from_secs(15), s3_client.list_buckets().send())
                    .await
                    .map_err(|_| ApiError(StatusCode::GATEWAY_TIMEOUT, "s3_list_buckets_timeout"))?
                    .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "s3_authentication_or_request_failed"))?;
                output.buckets().len()
            };
            Ok(bucket_count)
        }
        .await;

        let bucket_count = match verification {
            Ok(count) => count,
            Err(error) => {
                audit(
                    &state,
                    &principal.actor,
                    "integration.verify.failure",
                    "provider",
                    &provider_id,
                    json!({"adapter":"s3_compatible","failure":error.1}),
                )
                .await;
                return Err(error);
            }
        };
        let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
        audit(
            &state,
            &principal.actor,
            "integration.verify.success",
            "provider",
            &provider_id,
            json!({"adapter":"s3_compatible","bucket_count":bucket_count,"latency_ms":latency_ms}),
        )
        .await;
        return Ok(Json(json!({
            "provider_id":provider_id,
            "adapter":"s3_compatible",
            "connected":true,
            "verified":true,
            "bucket_count":bucket_count,
            "latency_ms":latency_ms
        })));
    }

    if adapter == "redis" {
        let private_host_allowlist = env::var("USH_ALLOWED_PRIVATE_HOSTS").unwrap_or_default();
        let redis_url = parse_redis_url(&token, &private_host_allowlist)?;
        ensure_public_destination(&redis_url).await?;
        let client = redis::Client::open(token.as_str())
            .map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_redis_url"))?;
        let mut connection = tokio::time::timeout(Duration::from_secs(8), client.get_multiplexed_async_connection())
            .await.map_err(|_| ApiError(StatusCode::GATEWAY_TIMEOUT, "redis_connection_timeout"))?
            .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "redis_connection_failed"))?;
        let pong: String = tokio::time::timeout(Duration::from_secs(5), redis::cmd("PING").query_async(&mut connection))
            .await.map_err(|_| ApiError(StatusCode::GATEWAY_TIMEOUT, "redis_ping_timeout"))?
            .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "redis_ping_failed"))?;
        if pong != "PONG" { return Err(ApiError(StatusCode::BAD_GATEWAY, "redis_ping_invalid_response")); }
        let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
        audit(&state, &principal.actor, "integration.verify.success", "provider", &provider_id, json!({"adapter":"redis","latency_ms":latency_ms})).await;
        return Ok(Json(json!({"provider_id":provider_id,"adapter":"redis","connected":true,"verified":true,"latency_ms":latency_ms})));
    }

    if adapter == "postgresql" {
        let database_url = parse_postgres_url(&token)?;
        ensure_public_destination(&database_url).await?;
        let connection_url = postgres_connection_url(&token)?;
        let options = PgConnectOptions::from_str(connection_url.as_str())
            .map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_database_url"))?
            .ssl_mode(PgSslMode::VerifyFull);
        let pool = PgPoolOptions::new().max_connections(1).acquire_timeout(Duration::from_secs(8))
            .connect_with(options).await
            .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "database_connection_failed"))?;
        sqlx::query("SELECT 1").execute(&pool).await
            .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "database_query_failed"))?;
        pool.close().await;
        let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
        audit(&state, &principal.actor, "integration.verify.success", "provider", &provider_id, json!({"adapter":"postgresql","latency_ms":latency_ms})).await;
        return Ok(Json(json!({"provider_id":provider_id,"adapter":"postgresql","connected":true,"verified":true,"latency_ms":latency_ms})));
    }

    let (url, method, payload) = match adapter.as_str() {
        "northflank" => ("https://api.northflank.com/v1/teams?per_page=1", reqwest::Method::GET, None),
        "railway" => ("https://backboard.railway.com/graphql/v2", reqwest::Method::POST, Some(json!({"query":"query { me { id name email } }"}))),
        "cloudflare" => ("https://api.cloudflare.com/client/v4/user/tokens/verify", reqwest::Method::GET, None),
        "netlify" => ("https://api.netlify.com/api/v1/user", reqwest::Method::GET, None),
        "render" => ("https://api.render.com/v1/owners?limit=1", reqwest::Method::GET, None),
        "digitalocean" => ("https://api.digitalocean.com/v2/account", reqwest::Method::GET, None),
        "flyio" => ("https://api.fly.io/graphql", reqwest::Method::POST, Some(json!({"query":"query { viewer { email } }"}))),
        "supabase" => ("https://api.supabase.com/v1/projects?limit=1", reqwest::Method::GET, None),
        "neon" => ("https://console.neon.tech/api/v2/projects?limit=1", reqwest::Method::GET, None),
        "cloudflare_r2" => ("https://api.cloudflare.com/client/v4", reqwest::Method::GET, None),
        "backblaze_b2" => ("https://api.backblazeb2.com/b2api/v4/b2_authorize_account", reqwest::Method::GET, None),
        _ => return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unsupported_control_api_adapter")),
    };
    let (request_url, auth_token, b2_credentials) = if adapter == "cloudflare_r2" {
        let config: Value = serde_json::from_str(&token).map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_r2_credential_config"))?;
        let account_id = config["account_id"].as_str().filter(|v| v.len() == 32 && v.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_r2_account_id"))?;
        let api_token = config["api_token"].as_str().filter(|v| !v.is_empty())
            .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_r2_api_token"))?;
        (format!("{url}/accounts/{account_id}/r2/buckets"), Some(api_token.to_owned()), None)
    } else if adapter == "backblaze_b2" {
        let config: Value = serde_json::from_str(&token).map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_b2_credential_config"))?;
        let key_id = config["key_id"].as_str().filter(|v| !v.is_empty())
            .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_b2_key_id"))?;
        let application_key = config["application_key"].as_str().filter(|v| !v.is_empty())
            .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_b2_application_key"))?;
        (url.to_owned(), None, Some((key_id.to_owned(), application_key.to_owned())))
    } else {
        (url.to_owned(), Some(token.clone()), None)
    };
    let parsed = validate_fixed_integration_url(&request_url)?;
    ensure_public_destination(&parsed).await?;
    let mut request = state.http.request(method, parsed).timeout(Duration::from_secs(10));
    if let Some(auth_token) = auth_token { request = request.bearer_auth(auth_token); }
    if let Some((key_id, application_key)) = b2_credentials { request = request.basic_auth(key_id, Some(application_key)); }
    if let Some(body) = payload { request = request.json(&body); }
    let response = request.send().await.map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "integration_request_failed"))?;
    let status = response.status();
    if !status.is_success() {
        let code = if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN { StatusCode::UNAUTHORIZED } else if status == StatusCode::TOO_MANY_REQUESTS { StatusCode::TOO_MANY_REQUESTS } else { StatusCode::BAD_GATEWAY };
        audit(&state, &principal.actor, "integration.verify.failure", "provider", &provider_id, json!({"adapter":adapter,"upstream_status":status.as_u16()})).await;
        return Err(ApiError(code, if status == StatusCode::UNAUTHORIZED { "integration_authentication_failed" } else if status == StatusCode::FORBIDDEN { "integration_permission_denied" } else if status == StatusCode::TOO_MANY_REQUESTS { "integration_rate_limited" } else { "integration_upstream_failed" }));
    }
    let data: Value = response.json().await.map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response"))?;
    let (verified, resource_count, account_label) = match adapter.as_str() {
        "northflank" => {
            let teams = data["data"]["teams"].as_array().ok_or(ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response"))?;
            (true, Some(teams.len()), teams.first().and_then(|v| v["name"].as_str()).map(str::to_owned))
        }
        "railway" => {
            if data["errors"].as_array().is_some_and(|v| !v.is_empty()) || data["data"]["me"].is_null() { return Err(ApiError(StatusCode::BAD_GATEWAY, "integration_query_failed")); }
            (true, None, data["data"]["me"]["name"].as_str().map(str::to_owned))
        }
        "cloudflare" => {
            if data["success"].as_bool() != Some(true) || data["result"]["status"].as_str() != Some("active") { return Err(ApiError(StatusCode::UNAUTHORIZED, "integration_authentication_failed")); }
            (true, None, Some("Cloudflare API token active".into()))
        }
        "netlify" => (data["id"].is_string(), data["site_count"].as_u64().map(|v| v as usize), data["full_name"].as_str().map(str::to_owned)),
        "render" => {
            let owners = data.as_array().ok_or(ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response"))?;
            (true, Some(owners.len()), owners.first().and_then(|v| v["owner"]["name"].as_str()).map(str::to_owned))
        }
        "digitalocean" => (data["account"]["uuid"].is_string(), None, Some("DigitalOcean account authorized".into())),
        "flyio" => {
            if data["errors"].as_array().is_some_and(|v| !v.is_empty()) || data["data"]["viewer"]["email"].as_str().is_none() { return Err(ApiError(StatusCode::BAD_GATEWAY, "integration_query_failed")); }
            (true, None, Some("Fly.io account authorized".into()))
        }
        "supabase" => {
            let projects = data.as_array().ok_or(ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response"))?;
            (true, Some(projects.len()), projects.first().and_then(|v| v["name"].as_str()).map(str::to_owned))
        }
        "neon" => {
            let projects = data["projects"].as_array().ok_or(ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response"))?;
            (true, Some(projects.len()), projects.first().and_then(|v| v["name"].as_str()).map(str::to_owned))
        }
        "cloudflare_r2" => {
            if data["success"].as_bool() != Some(true) { return Err(ApiError(StatusCode::UNAUTHORIZED, "integration_authentication_failed")); }
            let buckets = data["result"]["buckets"].as_array().ok_or(ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response"))?;
            (true, Some(buckets.len()), buckets.first().and_then(|v| v["name"].as_str()).map(str::to_owned))
        }
        "backblaze_b2" => {
            let account_id = data["accountId"].as_str().filter(|v| !v.is_empty());
            if account_id.is_none() || data["authorizationToken"].as_str().is_none() { return Err(ApiError(StatusCode::UNAUTHORIZED, "integration_authentication_failed")); }
            (true, None, Some("Backblaze B2 account authorized".into()))
        }
        _ => (false, None, None),
    };
    if !verified { return Err(ApiError(StatusCode::BAD_GATEWAY, "integration_invalid_response")); }
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    audit(&state, &principal.actor, "integration.verify.success", "provider", &provider_id, json!({"adapter":adapter,"latency_ms":latency_ms,"resource_count":resource_count})).await;
    Ok(Json(json!({"provider_id":provider_id,"adapter":adapter,"connected":true,"verified":true,"resource_count":resource_count,"account_label":account_label,"latency_ms":latency_ms})))
}

async fn delete_provider(State(state): State<AppState>, headers: HeaderMap, Path(provider_id): Path<String>) -> ApiResult<StatusCode> {
    let principal = require_admin(&state, &headers).await?;
    let result = sqlx::query("DELETE FROM providers WHERE id=$1").bind(&provider_id).execute(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "provider_delete_failed"))?;
    if result.rows_affected() == 0 { return Err(ApiError(StatusCode::NOT_FOUND, "provider_not_found")); }
    audit(&state, &principal.actor, "provider.delete", "provider", &provider_id, json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct HealthCheckRequest {
    provider_id: Option<String>,
    #[serde(default = "default_health_limit")]
    limit: i64,
}
fn default_health_limit() -> i64 { 50 }

async fn health_check(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<HealthCheckRequest>) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers).await?;
    let limit = body.limit.clamp(1,100);
    let rows = if let Some(id) = body.provider_id {
        sqlx::query("SELECT id,health_url,timeout_seconds FROM providers WHERE id=$1 AND enabled=TRUE").bind(id).fetch_all(&state.db).await
    } else {
        sqlx::query("SELECT id,health_url,timeout_seconds FROM providers WHERE enabled=TRUE ORDER BY priority DESC LIMIT $1").bind(limit).fetch_all(&state.db).await
    }.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "provider_health_query_failed"))?;
    let mut results = Vec::new();
    for row in rows {
        let id: String = row.get("id");
        let health_url: Option<String> = row.get("health_url");
        let timeout: f64 = row.get("timeout_seconds");
        let started = Instant::now();
        let (status, detail) = if let Some(raw_url) = health_url {
            match validate_https_url(&raw_url) {
                Ok(url) => match ensure_public_destination(&url).await {
                    Ok(()) => match state.http.get(url).timeout(Duration::from_secs_f64(timeout.clamp(0.1, 5.0))).send().await {
                        Ok(response) if response.status().is_success() => ("healthy","http_success"),
                        Ok(response) => ("unhealthy", if response.status().is_server_error() { "http_5xx" } else { "http_non_success" }),
                        Err(_) => ("unhealthy","connection_failed"),
                    },
                    Err(_) => ("unhealthy","unsafe_destination"),
                },
                Err(_) => ("unhealthy","unsafe_url"),
            }
        } else { ("unknown","health_url_not_configured") };
        let latency = started.elapsed().as_secs_f64()*1000.0;
        let _ = sqlx::query("UPDATE providers SET health_status=$1,checked_at=NOW(),latency_ms=$2,health_detail=$3 WHERE id=$4")
            .bind(status).bind(latency).bind(detail).bind(&id).execute(&state.db).await;
        results.push(json!({"provider_id":id,"status":status,"detail":detail,"latency_ms":latency}));
    }
    Ok(Json(json!({"results":results})))
}


#[derive(Deserialize)]
struct ProviderSelect { capability: String, #[serde(default)] exclude: Vec<String> }

async fn select_provider(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<ProviderSelect>) -> ApiResult<Json<Value>> {
    let principal = require_project_access(&state, &headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    let allowed_provider_ids: Option<Vec<String>> = if let Some(project_id) = principal.project_id.as_deref() {
        let project = sqlx::query("SELECT allowed_capabilities,allowed_provider_ids FROM projects WHERE id=$1")
            .bind(project_id).fetch_optional(&mut *tx).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_scope_failed"))?
            .ok_or(ApiError(StatusCode::FORBIDDEN, "project_not_found"))?;
        let caps: Value = project.get("allowed_capabilities");
        if caps.as_array().is_none_or(|a| !a.iter().any(|v| v.as_str()==Some(&body.capability))) {
            return Err(ApiError(StatusCode::FORBIDDEN, "capability_not_allowed"));
        }
        let allowed: Value = project.get("allowed_provider_ids");
        let ids = allowed.as_array().map_or_else(Vec::new, |items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>());
        if ids.is_empty() { None } else { Some(ids) }
    } else { None };
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let row = sqlx::query("SELECT id,name,capabilities,priority,health_status FROM providers WHERE enabled=TRUE ORDER BY priority DESC,id")
        .fetch_all(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "provider_select_failed"))?
        .into_iter().find(|r| {
            let id: String = r.get("id");
            let caps: Value = r.get("capabilities");
            let health: String = r.get("health_status");
            !body.exclude.contains(&id) && health != "unhealthy" &&
              caps.as_array().is_some_and(|a| a.iter().any(|v| v.as_str()==Some(&body.capability))) &&
              allowed_provider_ids.as_ref().is_none_or(|ids| ids.contains(&id))
        }).ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "no_eligible_provider"))?;
    Ok(Json(json!({"provider":{"id":row.get::<String,_>("id"),"name":row.get::<String,_>("name"),"priority":row.get::<i32,_>("priority")},"fallbacks":[]})))
}

async fn resolve_provider_secret(state: &AppState, key_name: &str, project_id: Option<&str>) -> ApiResult<Option<String>> {
    if let Some(secrets) = state.infisical.as_ref() {
        let path = secrets.provider_path(project_id);
        secrets.get_secret(key_name, &path).await
    } else {
        Ok(env::var(key_name).ok().filter(|value| !value.is_empty()))
    }
}

async fn provider_models(State(state): State<AppState>, headers: HeaderMap, Path(provider_id): Path<String>) -> ApiResult<Json<Value>> {
    let principal = require_admin(&state, &headers).await?;
    let row = sqlx::query("SELECT adapter_type,base_url,api_key_env FROM providers WHERE id=$1 AND enabled=TRUE")
        .bind(&provider_id).fetch_optional(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "provider_lookup_failed"))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "provider_not_found"))?;
    let adapter: String = row.get("adapter_type");
    let base_url: Option<String> = row.get("base_url");
    let key_name: Option<String> = row.get("api_key_env");
    let (Some(base_url),Some(key_name)) = (base_url,key_name) else {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "provider_model_discovery_not_supported"));
    };
    let api_key = resolve_provider_secret(&state,&key_name,None).await?
        .ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "provider_credential_unavailable"))?;
    let base = validate_https_url(&base_url)?;
    ensure_public_destination(&base).await?;
    let request = match adapter.as_str() {
        "openai_compatible" => state.http.get(format!("{}/models",base_url.trim_end_matches('/'))).bearer_auth(&api_key),
        "anthropic" => state.http.get(format!("{}/v1/models",base_url.trim_end_matches('/')))
            .header("x-api-key",&api_key).header("anthropic-version","2023-06-01"),
        "gemini" => state.http.get(format!("{}/v1beta/models",base_url.trim_end_matches('/')))
            .header("x-goog-api-key",&api_key),
        _ => return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "provider_model_discovery_not_supported")),
    };
    let response = request.send().await.map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "provider_model_list_failed"))?;
    if !response.status().is_success() {
        return Err(ApiError(StatusCode::BAD_GATEWAY, "provider_model_list_failed"));
    }
    let data: Value = response.json().await.map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "provider_invalid_response"))?;
    let models = match adapter.as_str() {
        "openai_compatible" | "anthropic" => data["data"].as_array().map_or_else(Vec::new, |items| items.iter().map(|item| json!({
            "id":item["id"],"name":item.get("name").or_else(||item.get("display_name")).cloned().unwrap_or(Value::Null)
        })).collect::<Vec<_>>()),
        "gemini" => data["models"].as_array().map_or_else(Vec::new, |items| items.iter().map(|item| json!({
            "id":item["name"],"name":item["displayName"],"supported_methods":item["supportedGenerationMethods"]
        })).collect::<Vec<_>>()),
        _ => Vec::new(),
    };
    audit(&state,&principal.actor,"provider.models.list","provider",&provider_id,json!({"model_count":models.len()})).await;
    Ok(Json(json!({"provider_id":provider_id,"models":models})))
}


#[derive(Deserialize)]
struct ChatMessage { role: String, content: String }

#[derive(Deserialize)]
struct AIChatRequest {
    messages: Vec<ChatMessage>,
    #[serde(default = "default_temperature")]
    temperature: f64,
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
    idempotency_key: Option<String>,
}
fn default_temperature() -> f64 { 0.2 }
fn default_max_tokens() -> u32 { 2048 }

async fn ai_chat(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<AIChatRequest>) -> ApiResult<Json<Value>> {
    let principal = require_operator(&state, &headers).await?;
    let mut tx=begin_principal_transaction(&state,&principal).await?;
    if body.messages.is_empty() || body.messages.len() > 100 || body.max_tokens == 0 || body.max_tokens > 32768 || !(0.0..=2.0).contains(&body.temperature)
        || body.messages.iter().any(|m| !["system","user","assistant"].contains(&m.role.as_str()) || m.content.len() > 200_000) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_ai_request"));
    }
    if let Some(key) = body.idempotency_key.as_deref()
        && (key.len() < 8 || key.len() > 128 || !key.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)))
    {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_idempotency_key"));
    }
    let allowed_ids: Option<Vec<String>> = if let Some(project_id) = principal.project_id.as_deref() {
        let project = sqlx::query("SELECT allowed_capabilities,allowed_provider_ids FROM projects WHERE id=$1")
            .bind(project_id).fetch_optional(&mut *tx).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_scope_failed"))?
            .ok_or(ApiError(StatusCode::FORBIDDEN, "project_not_found"))?;
        let caps: Value = project.get("allowed_capabilities");
        if caps.as_array().is_none_or(|a| !a.iter().any(|v| v.as_str()==Some("ai.chat"))) {
            return Err(ApiError(StatusCode::FORBIDDEN, "capability_not_allowed"));
        }
        let ids: Value = project.get("allowed_provider_ids");
        let list = ids.as_array().map_or_else(Vec::new, |items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>());
        if list.is_empty() { None } else { Some(list) }
    } else { None };

    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let rows = sqlx::query("SELECT id,name,adapter_type,model,base_url,api_key_env,timeout_seconds,supports_idempotency FROM providers WHERE enabled=TRUE AND health_status <> 'unhealthy' AND EXISTS (SELECT 1 FROM jsonb_array_elements_text(capabilities::jsonb) AS cap WHERE cap='ai.chat') ORDER BY priority DESC,id")
        .fetch_all(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "provider_query_failed"))?;
    let candidates: Vec<_> = rows.into_iter().filter(|r| {
        let id: String = r.get("id");
        allowed_ids.as_ref().is_none_or(|ids| ids.contains(&id))
    }).collect();
    if candidates.is_empty() { return Err(ApiError(StatusCode::SERVICE_UNAVAILABLE, "no_eligible_ai_provider")); }

    let mut attempts = 0usize;
    for row in candidates {
        let provider_id: String = row.get("id");
        let adapter: String = row.get("adapter_type");
        let model: Option<String> = row.get("model");
        let base_url: Option<String> = row.get("base_url");
        let key_env: Option<String> = row.get("api_key_env");
        let timeout: f64 = row.get("timeout_seconds");
        let idempotent: bool = row.get("supports_idempotency");
        if adapter == "none" { continue; }
        let (Some(model), Some(base_url), Some(key_env)) = (model, base_url, key_env) else { continue; };
        let api_key_result = resolve_provider_secret(&state, &key_env, principal.project_id.as_deref()).await;
        let api_key = match api_key_result {
            Ok(Some(value)) => value,
            _ => {
                audit(&state, &principal.actor, "ai.chat.failure", "provider", &provider_id, json!({"reason":"credential_unavailable"})).await;
                continue;
            }
        };
        let base = match validate_https_url(&base_url) {
            Ok(url) => url,
            Err(_) => continue,
        };
        if ensure_public_destination(&base).await.is_err() { continue; }
        let request = match adapter.as_str() {
            "openai_compatible" => {
                let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
                let mut req = state.http.post(url).bearer_auth(&api_key).json(&json!({
                    "model":model,"messages":body.messages.iter().map(|m| json!({"role":m.role,"content":m.content})).collect::<Vec<_>>(),
                    "temperature":body.temperature,"max_tokens":body.max_tokens
                }));
                if idempotent && let Some(key) = body.idempotency_key.as_deref() { req=req.header("Idempotency-Key",key); }
                req
            }
            "anthropic" => {
                let system = body.messages.iter().filter(|m| m.role=="system").map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n\n");
                let turns = body.messages.iter().filter(|m| m.role!="system").map(|m| json!({"role":m.role,"content":m.content})).collect::<Vec<_>>();
                if turns.is_empty() { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "messages_required")); }
                let mut payload = json!({"model":model,"messages":turns,"temperature":body.temperature,"max_tokens":body.max_tokens});
                if !system.is_empty() { payload["system"] = json!(system); }
                let mut req = state.http.post(format!("{}/v1/messages",base_url.trim_end_matches('/')))
                    .header("x-api-key",&api_key).header("anthropic-version","2023-06-01").json(&payload);
                if idempotent && let Some(key) = body.idempotency_key.as_deref() { req=req.header("Idempotency-Key",key); }
                req
            }
            "gemini" => {
                let system = body.messages.iter().filter(|m| m.role=="system").map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n\n");
                let turns = body.messages.iter().filter(|m| m.role!="system").map(|m| json!({
                    "role":if m.role=="assistant" {"model"} else {"user"},"parts":[{"text":m.content}]
                })).collect::<Vec<_>>();
                if turns.is_empty() { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "messages_required")); }
                let mut payload = json!({"contents":turns,"generationConfig":{"temperature":body.temperature,"maxOutputTokens":body.max_tokens}});
                if !system.is_empty() { payload["systemInstruction"] = json!({"parts":[{"text":system}]}); }
                let encoded = url::form_urlencoded::byte_serialize(model.as_bytes()).collect::<String>();
                state.http.post(format!("{}/v1beta/models/{}:generateContent",base_url.trim_end_matches('/'),encoded))
                    .header("x-goog-api-key",&api_key).json(&payload)
            }
            _ => continue,
        }.timeout(Duration::from_secs_f64(timeout.clamp(1.0,120.0)));
        attempts += 1;
        let started = Instant::now();
        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                audit(&state, &principal.actor, "ai.chat.failure", "provider", &provider_id, json!({"reason":if err.is_connect() {"connection_failed"} else {"network_error"}})).await;
                if err.is_connect() && idempotent && body.idempotency_key.is_some() { continue; }
                return Err(ApiError(StatusCode::BAD_GATEWAY, "provider_connection_failed"));
            }
        };
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            audit(&state, &principal.actor, "ai.chat.failure", "provider", &provider_id, json!({"reason":"rate_limited"})).await;
            if idempotent && body.idempotency_key.is_some() { continue; }
            return Err(ApiError(StatusCode::BAD_GATEWAY, "provider_rate_limited"));
        }
        if !response.status().is_success() {
            let code = response.status();
            audit(&state, &principal.actor, "ai.chat.failure", "provider", &provider_id, json!({"reason":"upstream_rejected","status":code.as_u16()})).await;
            return Err(ApiError(StatusCode::BAD_GATEWAY, "provider_request_failed"));
        }
        let data: Value = response.json().await.map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "provider_invalid_response"))?;
        let (content, usage) = match adapter.as_str() {
            "openai_compatible" => {
                let text = data["choices"][0]["message"]["content"].as_str().map(str::to_owned)
                    .or_else(|| data["choices"][0]["message"]["content"].as_array().map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<String>()))
                    .ok_or(ApiError(StatusCode::BAD_GATEWAY, "provider_invalid_response"))?;
                (text,data.get("usage").cloned().unwrap_or_else(||json!({})))
            }
            "anthropic" => {
                let text = data["content"].as_array().map(|parts| parts.iter().filter_map(|p| if p["type"]=="text" {p["text"].as_str()} else {None}).collect::<String>())
                    .ok_or(ApiError(StatusCode::BAD_GATEWAY, "provider_invalid_response"))?;
                (text,data.get("usage").cloned().unwrap_or_else(||json!({})))
            }
            "gemini" => {
                let text = data["candidates"][0]["content"]["parts"].as_array().map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<String>())
                    .ok_or(ApiError(StatusCode::BAD_GATEWAY, "provider_invalid_response"))?;
                (text,data.get("usageMetadata").cloned().unwrap_or_else(||json!({})))
            }
            _ => return Err(ApiError(StatusCode::BAD_GATEWAY, "unsupported_adapter")),
        };
        let latency_ms = started.elapsed().as_secs_f64()*1000.0;
        audit(&state, &principal.actor, "ai.chat.success", "provider", &provider_id, json!({"model":model,"latency_ms":latency_ms,"attempts":attempts,"usage":usage})).await;
        return Ok(Json(json!({"provider_id":provider_id,"model":model,"content":content,"usage":usage,"latency_ms":latency_ms,"attempts":attempts})));
    }
    Err(ApiError(StatusCode::SERVICE_UNAVAILABLE, "no_provider_available"))
}


#[derive(Deserialize)]
struct AuditQuery { #[serde(default = "default_limit")] limit: i64 }
fn default_limit() -> i64 { 50 }

async fn list_audit(State(state): State<AppState>, headers: HeaderMap, Query(query): Query<AuditQuery>) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers).await?;
    let mut tx=begin_platform_transaction(&state).await?;
    let limit = query.limit.clamp(1,100);
    let rows = sqlx::query("SELECT id,occurred_at,actor,action,resource_type,resource_id,details FROM audit_events ORDER BY occurred_at DESC LIMIT $1")
        .bind(limit).fetch_all(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "audit_list_failed"))?;
    tx.commit().await.map_err(|_|ApiError(StatusCode::INTERNAL_SERVER_ERROR,"database_transaction_commit_failed"))?;
    let values: Vec<Value> = rows.into_iter().map(|r| json!({
        "id":r.get::<String,_>("id"),"occurred_at":r.get::<DateTime<Utc>,_>("occurred_at"),
        "actor":r.get::<String,_>("actor"),"action":r.get::<String,_>("action"),
        "resource_type":r.get::<String,_>("resource_type"),"resource_id":r.get::<String,_>("resource_id"),
        "details":r.get::<Value,_>("details")
    })).collect();
    Ok(Json(json!(values)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();
    let environment = env::var("USH_ENVIRONMENT").unwrap_or_else(|_| "development".into());
    let secret_backend = env::var("USH_SECRET_BACKEND").unwrap_or_else(|_| if environment == "production" { "infisical".into() } else { "env".into() });
    if environment == "production" && secret_backend != "infisical" {
        anyhow::bail!("production requires USH_SECRET_BACKEND=infisical");
    }
    if !["env", "infisical"].contains(&secret_backend.as_str()) {
        anyhow::bail!("unsupported USH_SECRET_BACKEND");
    }
    let database_url = env::var("USH_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"))?;
    if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
        anyhow::bail!("USH_DATABASE_URL must use PostgreSQL");
    }
    let admin_token = env::var("USH_API_TOKEN")?;
    if admin_token.len() < 32 { anyhow::bail!("USH_API_TOKEN must contain at least 32 characters"); }
    let rate_limit_per_minute = match env::var("USH_RATE_LIMIT_PER_MINUTE") {
        Ok(value) => value.parse::<i64>()?,
        Err(_) => 120,
    };
    if !(1..=10_000).contains(&rate_limit_per_minute) {
        anyhow::bail!("USH_RATE_LIMIT_PER_MINUTE must be between 1 and 10000");
    }
    // Migrations run with the configured database login. Application traffic must
    // always execute as ush_runtime, a non-login role with SUPERUSER/BYPASSRLS disabled.
    let migration_db = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&database_url).await?;
    sqlx::migrate!("./rust-migrations").run(&migration_db).await?;
    migration_db.close().await;

    let db = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(|connection, _metadata| Box::pin(async move {
            sqlx::query("SET ROLE ush_runtime").execute(connection).await?;
            Ok(())
        }))
        .connect(&database_url).await?;
    let runtime_role = sqlx::query(
        "SELECT current_user, r.rolsuper, r.rolbypassrls FROM pg_roles r WHERE r.rolname=current_user"
    ).fetch_one(&db).await?;
    let runtime_role_name: String = runtime_role.try_get("current_user")?;
    let runtime_is_superuser: bool = runtime_role.try_get("rolsuper")?;
    let runtime_bypasses_rls: bool = runtime_role.try_get("rolbypassrls")?;
    if runtime_role_name != "ush_runtime" || runtime_is_superuser || runtime_bypasses_rls {
        anyhow::bail!("USH runtime database role must be ush_runtime with SUPERUSER and BYPASSRLS disabled");
    }
    prune_rate_limit_rows(&db).await?;
    let cleanup_db = db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            match prune_rate_limit_rows(&cleanup_db).await {
                Ok(removed) if removed > 0 => info!(removed, "Expired API rate-limit fingerprints pruned"),
                Ok(_) => {}
                Err(err) => error!(error = %err, "Failed to prune expired API rate-limit fingerprints"),
            }
        }
    });
    let recovery_db = db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        interval.tick().await;
        loop {
            interval.tick().await;
            match recover_expired_task_leases(&recovery_db).await {
                Ok(recovered) if recovered > 0 => info!(recovered, "Expired engineering task leases recovered"),
                Ok(_) => {}
                Err(err) => error!(error = %err, "Engineering task lease recovery failed"),
            }
            match recover_expired_workspace_leases(&recovery_db).await {
                Ok(recovered) if recovered > 0 => info!(recovered, "Expired engineering workspace leases quarantined"),
                Ok(_) => {}
                Err(err) => error!(error = %err, "Engineering workspace lease recovery failed"),
            }
        }
    });
    let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).redirect(reqwest::redirect::Policy::none()).build()?;
    let runtime_environment = env::var("USH_ENVIRONMENT").unwrap_or_else(|_| "development".into());
    let secret_backend = env::var("USH_SECRET_BACKEND").unwrap_or_else(|_| {
        if runtime_environment == "production" { "infisical".into() } else { "env".into() }
    });
    if !["env", "infisical"].contains(&secret_backend.as_str()) {
        anyhow::bail!("USH_SECRET_BACKEND must be env or infisical");
    }
    if runtime_environment == "production" && secret_backend != "infisical" {
        anyhow::bail!("Production requires USH_SECRET_BACKEND=infisical");
    }
    let infisical_credentials = (
        env::var("INFISICAL_CLIENT_ID").ok(),
        env::var("INFISICAL_CLIENT_SECRET").ok(),
        env::var("INFISICAL_PROJECT_ID").ok(),
        env::var("INFISICAL_ENVIRONMENT").ok(),
    );
    let infisical = match (infisical_credentials, secret_backend.as_str()) {
        ((Some(client_id), Some(client_secret), Some(project_id), Some(environment)), _) => {
            let base_url = env::var("INFISICAL_URL").unwrap_or_else(|_| "https://app.infisical.com".into());
            let infisical_url = validate_https_url(&base_url)
                .map_err(|_| anyhow::anyhow!("INFISICAL_URL must be a public HTTPS URL"))?;
            ensure_public_destination(&infisical_url).await
                .map_err(|_| anyhow::anyhow!("INFISICAL_URL must resolve only to public IP addresses"))?;
            Some(InfisicalClient {
                http: http.clone(),
                base_url,
                client_id,
                client_secret,
                project_id,
                environment,
                path_prefix: env::var("USH_INFISICAL_SECRET_PATH_PREFIX").unwrap_or_else(|_| "/projects".into()),
                token_cache: Arc::new(RwLock::new(None)),
            })
        }
        ((None, None, None, None), "env") if runtime_environment != "production" => None,
        _ => anyhow::bail!("Infisical backend requires client ID, client secret, project ID and environment"),
    };
    let state = AppState { db, admin_token: Arc::new(admin_token), http, infisical, rate_limit_per_minute };
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/projects", get(list_projects).post(create_project))
        .route("/v1/projects/{project_id}/tasks", get(list_engineering_tasks).post(create_engineering_task))
        .route("/v1/projects/{project_id}/tasks/{task_id}/events", post(append_engineering_task_event))
        .route("/v1/projects/{project_id}/tasks/{task_id}/transition", post(transition_engineering_task))
        .route("/v1/tasks/claim", post(claim_engineering_task))
        .route("/v1/tasks/{task_id}/heartbeat", post(heartbeat_engineering_task))
        .route("/v1/tasks/{task_id}/checkpoints", post(create_engineering_checkpoint))
        .route("/v1/tasks/{task_id}/workspace-assignment", post(assign_engineering_task_workspace).delete(release_engineering_task_workspace))
        .route("/v1/projects/{project_id}/tasks/{task_id}/path-locks", post(acquire_engineering_task_path_locks).delete(release_engineering_task_path_locks))
        .route("/v1/tasks/{task_id}/attempts", post(record_engineering_task_attempt))
        .route("/v1/tasks/{task_id}/complete", post(complete_engineering_task))
        .route("/v1/projects/{project_id}/tasks/{task_id}/attempts", get(list_engineering_task_attempts))
        .route("/v1/projects/{project_id}", put(update_project).delete(delete_project))
        .route("/v1/projects/{project_id}/tokens", get(list_project_tokens).post(create_project_token))
        .route("/v1/projects/{project_id}/tokens/{token_id}", delete(revoke_project_token))
        .route("/v1/projects/{project_id}/workers", get(list_engineering_workers).post(create_engineering_worker))
        .route("/v1/projects/{project_id}/workers/{worker_id}", delete(revoke_engineering_worker))
        .route("/v1/projects/{project_id}/workspaces", get(list_engineering_workspaces).post(create_engineering_workspace))
        .route("/v1/projects/{project_id}/workspaces/lease-next", post(lease_next_engineering_workspace))
        .route("/v1/projects/{project_id}/workspaces/{workspace_id}/heartbeat", post(heartbeat_engineering_workspace))
        .route("/v1/projects/{project_id}/workspaces/{workspace_id}/release", post(release_engineering_workspace))
        .route("/v1/tenants/{tenant_id}/tokens", post(create_tenant_token))
        .route("/v1/tenants/{tenant_id}/tokens/{token_id}", delete(revoke_tenant_token))
        .route("/v1/providers", get(list_providers))
        .route("/v1/providers/{provider_id}", put(upsert_provider).delete(delete_provider))
        .route("/v1/providers/{provider_id}/models", get(provider_models))
        .route("/v1/integrations/{provider_id}/verify", post(verify_integration))
        .route("/v1/providers/health-check", post(health_check))
        .route("/v1/ai/chat", post(ai_chat))
        .route("/v1/providers/select", post(select_provider))
        .route("/v1/audit", get(list_audit))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024));
    let address: IpAddr = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()).parse()?;
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8000".into()).parse()?;
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(address, port)).await?;
    info!(port, "Universal Service Hub Rust runtime started");
    axum::serve(listener, app).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engineering_worker_roles_are_bound_to_one_queue() {
        assert_eq!(worker_queue_for_role("architect"), Some("planning"));
        assert_eq!(worker_queue_for_role("implementer"), Some("implementation"));
        assert_eq!(worker_queue_for_role("builder"), Some("build"));
        assert_eq!(worker_queue_for_role("tester"), Some("test"));
        assert_eq!(worker_queue_for_role("reviewer"), Some("review"));
        assert_eq!(worker_queue_for_role("recovery"), Some("recovery"));
        assert_eq!(worker_queue_for_role("operator"), None);
        assert_eq!(worker_queue_for_role("admin"), None);
    }

    #[test]
    fn bearer_parser_accepts_only_long_bearer_tokens() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer 01234567890123456789012345678901".parse().unwrap());
        assert_eq!(
            bearer(&headers).unwrap(),
            "01234567890123456789012345678901"
        );
        headers.insert(AUTHORIZATION, "Basic abcdefghijklmnopqrstuvwxyz012345".parse().unwrap());
        assert!(bearer(&headers).is_err());
        headers.insert(AUTHORIZATION, "Bearer short".parse().unwrap());
        assert!(bearer(&headers).is_err());
    }

    #[test]
    fn project_identifiers_reject_path_and_whitespace_characters() {
        let valid = "sherlock-scholes.prod_1";
        assert!(valid.len() <= 80 && valid.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)));
        for invalid in ["../admin", "project/name", "project name", ""] {
            assert!(invalid.is_empty() || !invalid.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)));
        }
    }

    #[test]
    fn token_hash_is_one_way_sha256_hex() {
        let raw = "test-project-token-that-is-never-stored";
        let digest = hex::encode(Sha256::digest(raw.as_bytes()));
        assert_eq!(digest.len(), 64);
        assert_ne!(digest, raw);
    }

    #[test]
    fn integration_adapter_catalog_is_explicit_and_bounded() {
        for adapter in ["northflank", "railway", "cloudflare", "netlify", "render", "digitalocean", "flyio", "supabase", "neon", "cloudflare_r2", "backblaze_b2", "postgresql", "redis", "supabase_storage", "s3_compatible"] {
            assert!(is_control_api_adapter(adapter));
            assert!(!is_ai_adapter(adapter));
        }
        assert!(!is_control_api_adapter("unknown"));
    }

    #[test]
    fn s3_compatible_credentials_require_all_fields() {
        let valid = r#"{"endpoint_url":"https://s3.us-east-1.amazonaws.com","region":"us-east-1","access_key_id":"access","secret_access_key":"secret","session_token":"temporary","bucket":"project-assets"}"#;
        let credentials = parse_s3_compatible_credentials(valid).unwrap();
        assert_eq!(credentials.region, "us-east-1");
        assert_eq!(credentials.bucket.as_deref(), Some("project-assets"));
        assert!(!valid_s3_bucket_name("bad..bucket"));
        assert!(!valid_s3_bucket_name("127.0.0.1"));
        assert!(valid_s3_bucket_name("project-assets"));
        assert!(parse_s3_compatible_credentials(r#"{"endpoint_url":"https://s3.example.com"}"#).is_err());
    }

    #[test]
    fn s3_compatible_endpoint_requires_public_https() {
        assert!(validate_https_url("https://s3.us-east-1.amazonaws.com").is_ok());
        assert!(validate_https_url("http://s3.example.com").is_err());
        assert!(validate_https_url("https://127.0.0.1").is_err());
    }

    #[test]
    fn infisical_secret_paths_are_project_isolated() {
        let client = InfisicalClient {
            http: reqwest::Client::new(),
            base_url: "https://app.infisical.com".into(),
            client_id: "client".into(),
            client_secret: "secret".into(),
            project_id: "infisical-project".into(),
            environment: "prod".into(),
            path_prefix: "/projects".into(),
            token_cache: Arc::new(RwLock::new(None)),
        };
        assert_eq!(client.provider_path(Some("sherlock-scholes")), "/projects/sherlock-scholes/providers");
        assert_eq!(client.provider_path(None), "/projects/shared/providers");
    }

}


#[cfg(test)]
mod outbound_tests {
    use super::*;

    #[test]
    fn private_network_allowlist_requires_exact_hostnames() {
        assert!(is_allowed_private_host("redis.railway.internal", "redis.railway.internal,db.internal"));
        assert!(!is_allowed_private_host("evil.redis.railway.internal", "redis.railway.internal"));
        assert!(!is_allowed_private_host("10.0.0.5", "10.0.0.5"));
        assert!(is_private_network_ip("10.0.0.5".parse().unwrap()));
        assert!(is_private_network_ip("172.20.0.2".parse().unwrap()));
        assert!(is_private_network_ip("192.168.1.4".parse().unwrap()));
        assert!(is_private_network_ip("fd00::1".parse().unwrap()));
        assert!(!is_private_network_ip("169.254.169.254".parse().unwrap()));
        assert!(!is_private_network_ip("127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn public_address_filter_rejects_private_and_metadata_ranges() {
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_public_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("10.0.0.8".parse().unwrap()));
        assert!(!is_public_ip("169.254.169.254".parse().unwrap()));
        assert!(!is_public_ip("192.168.1.1".parse().unwrap()));
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(!is_public_ip("fc00::1".parse().unwrap()));
    }

    #[test]
    fn database_and_redis_connection_urls_require_supported_schemes() {
        assert!(parse_postgres_url("postgresql://user:pass@db.example.test:5432/app").is_ok());
        assert!(parse_postgres_url("https://db.example.test").is_err());
        assert!(parse_redis_url("rediss://default:secret@cache.example.test:6379", "").is_ok());
        assert!(parse_redis_url("redis://default:secret@cache.example.test:6379", "").is_err());
        assert!(parse_redis_url("redis://default:secret@redis.railway.internal:6379", "redis.railway.internal").is_ok());
    }

    #[test]
    fn fixed_integration_urls_allow_static_query_parameters_only_on_https() {
        assert!(validate_fixed_integration_url("https://api.example.com/v1/items?limit=1").is_ok());
        assert!(validate_fixed_integration_url("http://api.example.com/v1/items?limit=1").is_err());
        assert!(validate_fixed_integration_url("https://user:pass@api.example.com/v1").is_err());
        assert!(validate_fixed_integration_url("https://api.example.com/v1#fragment").is_err());
    }

    #[test]
    fn provider_url_policy_requires_clean_https_url() {
        assert!(validate_https_url("https://api.example.com/v1").is_ok());
        assert!(validate_https_url("http://api.example.com").is_err());
        assert!(validate_https_url("https://127.0.0.1").is_err());
        assert!(validate_https_url("https://api.example.com/?token=secret").is_err());
        assert!(validate_https_url("https://user:pass@api.example.com").is_err());
        assert!(validate_https_url("https://localhost").is_err());
    }
}



#[cfg(test)]
mod project_permission_tests {
    use super::*;

    fn principal(role: &str, project_id: Option<&str>, admin: bool) -> Principal {
        Principal {
            actor: "test".into(),
            role: role.into(),
            tenant_id: None,
            project_id: project_id.map(str::to_owned),
            worker_id: None,
            worker_role: None,
            queue_name: None,
            admin,
        }
    }

    #[test]
    fn viewer_is_read_only_and_project_owner_is_scoped() {
        let admin = principal("admin", None, true);
        let owner_alpha = principal("owner", Some("alpha"), false);
        let operator_alpha = principal("operator", Some("alpha"), false);
        let viewer_alpha = principal("viewer", Some("alpha"), false);
        let owner_beta = principal("owner", Some("beta"), false);

        assert!(can_access_project(&admin));
        assert!(can_access_project(&viewer_alpha));
        assert!(!can_invoke_ai(&viewer_alpha));
        assert!(can_invoke_ai(&operator_alpha));
        assert!(can_manage_project_tokens(&admin, "alpha"));
        assert!(can_manage_project_tokens(&owner_alpha, "alpha"));
        assert!(!can_manage_project_tokens(&owner_beta, "alpha"));
        assert!(!can_manage_project_tokens(&operator_alpha, "alpha"));
        assert!(!can_manage_project_tokens(&viewer_alpha, "alpha"));
    }
}


#[cfg(test)]
mod integration_security_tests {
    use super::*;

    #[test]
    fn worker_completion_requires_role_specific_verifiable_evidence() {
        let sha="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert!(worker_completion_evidence_valid("implementer","review",Some(sha),&json!({
            "commit_sha":sha,"changed_files":["universal-service-hub/src/main.rs"],"validation":{"rustfmt":"passed"}
        })));
        assert!(!worker_completion_evidence_valid("implementer","review",Some(sha),&json!({"commit_sha":sha})));
        assert!(worker_completion_evidence_valid("builder","queued_test",Some(sha),&json!({
            "build_status":"passed","build_run_id":"run-123","candidate_sha":sha
        })));
        assert!(worker_completion_evidence_valid("tester","review",Some(sha),&json!({
            "test_status":"passed","test_run_id":"run-456","candidate_sha":sha
        })));
        assert!(worker_completion_evidence_valid("reviewer","verified",Some(sha),&json!({
            "decision":"approved","reviewed_sha":sha,"findings":[]
        })));
        assert!(!worker_completion_evidence_valid("reviewer","verified",Some(sha),&json!({
            "decision":"approved","reviewed_sha":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","findings":[]
        })));
        assert!(worker_completion_evidence_valid("architect","queued",None,&json!({
            "plan":{"tasks":[{"title":"Implement API"}]}
        })));
        assert!(worker_completion_evidence_valid("recovery","queued",None,&json!({
            "recovery_reason":"lease expired","recovery_action":"requeue from checkpoint"
        })));
    }

    #[test]
    fn workspace_worktree_paths_are_relative_and_cannot_escape_the_workspace_root() {
        assert!(valid_worktree_path("tenants/aloews/projects/ush/tasks/task-123"));
        assert!(!valid_worktree_path("/tmp/worktree"));
        assert!(!valid_worktree_path("../escape"));
        assert!(!valid_worktree_path("tenants/aloews/../../etc"));
        assert!(!valid_worktree_path("tenants/aloews/.git"));
        assert!(!valid_worktree_path("tenants\\aloews\\worktree"));
        assert!(!valid_worktree_path("tenants/aloews/worktree/"));
    }

    #[test]
    fn postgres_connection_url_discards_untrusted_query_options() {
        let parsed = postgres_connection_url(
            "postgresql://user:pass@db.example:5432/app?host=127.0.0.1&sslmode=disable",
        )
        .expect("valid PostgreSQL URL");
        assert_eq!(parsed.host_str(), Some("db.example"));
        assert!(parsed.query().is_none());
        assert_eq!(parsed.scheme(), "postgresql");
    }

    #[test]
    fn postgres_url_rejects_fragments_and_invalid_schemes() {
        assert!(parse_postgres_url("postgres://user:pass@db.example/app#fragment").is_err());
        assert!(parse_postgres_url("http://db.example/app").is_err());
    }

    #[test]
    fn redis_requires_tls_or_an_exact_private_hostname_and_rejects_query_options() {
        assert!(parse_redis_url("redis://cache.example:6379", "").is_err());
        assert!(parse_redis_url("redis://cache.internal:6379", "cache.internal").is_ok());
        assert!(parse_redis_url("redis://cache.internal:6379", "*.internal").is_err());
        assert!(parse_redis_url("rediss://cache.example:6380?insecure=true", "").is_err());
    }

    #[test]
    fn private_network_allowlist_is_exact_and_excludes_loopback_and_link_local() {
        assert!(is_private_network_ip("10.0.0.2".parse().unwrap()));
        assert!(is_private_network_ip("172.16.0.2".parse().unwrap()));
        assert!(is_private_network_ip("192.168.1.2".parse().unwrap()));
        assert!(is_private_network_ip("fd00::2".parse().unwrap()));
        assert!(!is_private_network_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_private_network_ip("169.254.169.254".parse().unwrap()));
        assert!(!is_allowed_private_host("cache.internal", "*.internal"));
        assert!(is_allowed_private_host("cache.internal", "cache.internal"));
    }

    #[test]
    fn s3_credentials_preserve_optional_session_token_and_validate_bucket() {
        let raw = r#"{
            "endpoint_url":"https://s3.example.com",
            "region":"us-east-1",
            "access_key_id":"access",
            "secret_access_key":"secret",
            "session_token":"temporary-session",
            "bucket":"project-assets"
        }"#;
        let credentials = parse_s3_compatible_credentials(raw).expect("valid S3 credentials");
        assert_eq!(credentials.session_token.as_deref(), Some("temporary-session"));
        assert!(valid_s3_bucket_name("project-assets"));
        assert!(!valid_s3_bucket_name("Project-Assets"));
        assert!(!valid_s3_bucket_name("192.168.1.1"));
    }
}
