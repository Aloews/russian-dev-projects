#[allow(dead_code)]
mod artifact_store;
mod schedule_recovery;

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

#[derive(Clone)]
struct AppState {
    db: PgPool,
    admin_token: Arc<String>,
    http: reqwest::Client,
    infisical: Option<InfisicalClient>,
    artifact_store: Option<artifact_store::ArtifactStore>,
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
    project_id: Option<String>,
    admin: bool,
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
    let result = sqlx::query(
        "DELETE FROM api_rate_limits WHERE updated_at < NOW() - INTERVAL '24 hours'"
    ).execute(db).await?;
    Ok(result.rows_affected())
}

async fn enforce_rate_limit(state: &AppState, token_hash: &str) -> ApiResult<()> {
    let row = sqlx::query(
        "INSERT INTO api_rate_limits (token_hash, window_started_at, request_count, updated_at) VALUES ($1, date_trunc('minute', NOW()), 1, NOW()) ON CONFLICT (token_hash) DO UPDATE SET request_count = CASE WHEN api_rate_limits.window_started_at < date_trunc('minute', NOW()) THEN 1 ELSE api_rate_limits.request_count + 1 END, window_started_at = CASE WHEN api_rate_limits.window_started_at < date_trunc('minute', NOW()) THEN date_trunc('minute', NOW()) ELSE api_rate_limits.window_started_at END, updated_at = NOW() RETURNING request_count"
    ).bind(token_hash).fetch_one(&state.db).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "rate_limit_store_error"))?;
    let count: i32 = row.try_get("request_count")
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "rate_limit_store_error"))?;
    if i64::from(count) > state.rate_limit_per_minute {
        return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "rate_limit_exceeded"));
    }
    Ok(())
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let token = bearer(headers)?;
    let digest = hex::encode(Sha256::digest(token.as_bytes()));
    if token.as_bytes().ct_eq(state.admin_token.as_bytes()).into() {
        enforce_rate_limit(state, &digest).await?;
        return Ok(Principal { actor: "service-token".into(), role: "admin".into(), project_id: None, admin: true });
    }
    let row = sqlx::query(
        "SELECT id, project_id, role FROM project_tokens WHERE token_hash = $1 AND revoked_at IS NULL"
    ).bind(&digest).fetch_optional(&state.db).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?
      .ok_or(ApiError(StatusCode::UNAUTHORIZED, "invalid_bearer_token"))?;
    enforce_rate_limit(state, &digest).await?;
    let id: String = row.try_get("id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    let project_id: String = row.try_get("project_id").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    let role: String = row.try_get("role").map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?;
    let _ = sqlx::query("UPDATE project_tokens SET last_used_at = NOW() WHERE id = $1").bind(&id).execute(&state.db).await;
    Ok(Principal { actor: format!("project:{project_id}:token:{id}"), role, project_id: Some(project_id), admin: false })
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if !principal.admin { return Err(ApiError(StatusCode::FORBIDDEN, "administrator_required")); }
    Ok(principal)
}

fn can_invoke_ai(principal: &Principal) -> bool {
    principal.admin || principal.role == "owner" || principal.role == "operator"
}

async fn require_operator(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if !can_invoke_ai(&principal) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_operator_required"));
    }
    Ok(principal)
}

fn can_access_project(principal: &Principal) -> bool {
    principal.admin || ["owner", "operator", "viewer"].contains(&principal.role.as_str())
}

fn can_manage_delivery_schedule(principal: &Principal, project_id: &str) -> bool {
    principal.admin
        || (["owner", "operator"].contains(&principal.role.as_str())
            && principal.project_id.as_deref() == Some(project_id))
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
    if !can_manage_project_tokens(&principal, project_id) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_owner_required"));
    }
    Ok(principal)
}

async fn audit(state: &AppState, actor: &str, action: &str, resource_type: &str, resource_id: &str, details: Value) {
    if let Err(err) = sqlx::query(
        "INSERT INTO audit_events (id, occurred_at, actor, action, resource_type, resource_id, details) VALUES ($1,$2,$3,$4,$5,$6,$7)"
    ).bind(Uuid::new_v4().to_string()).bind(Utc::now()).bind(actor).bind(action)
     .bind(resource_type).bind(resource_id).bind(details).execute(&state.db).await {
        error!(error = %err, action, "audit persistence failed");
    }
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
    if body.id.is_empty() || body.id.len() > 80 || !body.id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) || body.name.trim().is_empty() || body.name.len() > 160 || body.allowed_capabilities.is_empty() || body.allowed_capabilities.len() > 100 || body.allowed_provider_ids.len() > 100 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_project"));
    }
    let result = sqlx::query(
        "INSERT INTO projects (id,name,allowed_capabilities,allowed_provider_ids,created_at,updated_at) VALUES ($1,$2,$3,$4,NOW(),NOW()) ON CONFLICT (id) DO NOTHING"
    ).bind(&body.id).bind(&body.name).bind(json!(body.allowed_capabilities.clone())).bind(json!(body.allowed_provider_ids.clone()))
     .execute(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_create_failed"))?;
    if result.rows_affected() == 0 { return Err(ApiError(StatusCode::CONFLICT, "project_exists")); }
    audit(&state, &principal.actor, "project.create", "project", &body.id, json!({})).await;
    Ok(Json(json!({"id":body.id,"name":body.name,"allowed_capabilities":body.allowed_capabilities,"allowed_provider_ids":body.allowed_provider_ids})))
}

async fn list_projects(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query("SELECT id,name,allowed_capabilities,allowed_provider_ids,created_at FROM projects ORDER BY id")
        .fetch_all(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_list_failed"))?;
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
    if body.name.trim().is_empty() || body.name.len() > 160 || body.allowed_capabilities.is_empty() || body.allowed_capabilities.len() > 100 || body.allowed_provider_ids.len() > 100 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_project"));
    }
    let result = sqlx::query("UPDATE projects SET name=$1,allowed_capabilities=$2,allowed_provider_ids=$3,updated_at=NOW() WHERE id=$4")
        .bind(&body.name).bind(json!(body.allowed_capabilities)).bind(json!(body.allowed_provider_ids)).bind(&project_id)
        .execute(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_update_failed"))?;
    if result.rows_affected()==0 { return Err(ApiError(StatusCode::NOT_FOUND, "project_not_found")); }
    audit(&state, &principal.actor, "project.update", "project", &project_id, json!({})).await;
    Ok(Json(json!({"id":project_id,"name":body.name})))
}

async fn delete_project(State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>) -> ApiResult<StatusCode> {
    let principal = require_admin(&state, &headers).await?;
    let result = sqlx::query("DELETE FROM projects WHERE id=$1").bind(&project_id).execute(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_delete_failed"))?;
    if result.rows_affected()==0 { return Err(ApiError(StatusCode::NOT_FOUND, "project_not_found")); }
    audit(&state, &principal.actor, "project.delete", "project", &project_id, json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_project_tokens(State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>) -> ApiResult<Json<Value>> {
    require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let rows = sqlx::query("SELECT id,project_id,name,role,created_at,last_used_at,revoked_at FROM project_tokens WHERE project_id=$1 ORDER BY created_at DESC")
        .bind(&project_id).fetch_all(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_token_list_failed"))?;
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
    if !["owner","operator","viewer"].contains(&body.role.as_str()) || body.name.trim().is_empty() || body.name.len() > 120 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_project_token"));
    }
    if !principal.admin && body.role == "owner" {
        return Err(ApiError(StatusCode::FORBIDDEN, "administrator_required_for_owner_token"));
    }
    let exists = sqlx::query("SELECT id FROM projects WHERE id=$1").bind(&project_id).fetch_optional(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_lookup_failed"))?;
    if exists.is_none() { return Err(ApiError(StatusCode::NOT_FOUND, "project_not_found")); }
    let raw: String = rand::rng().sample_iter(&Alphanumeric).take(48).map(char::from).collect();
    let hash = hex::encode(Sha256::digest(raw.as_bytes()));
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO project_tokens (id,project_id,name,role,token_hash,created_at) VALUES ($1,$2,$3,$4,$5,NOW())")
        .bind(&id).bind(&project_id).bind(&body.name).bind(&body.role).bind(hash).execute(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_token_create_failed"))?;
    audit(&state, &principal.actor, "project.token.create", "project", &project_id, json!({"token_id":id.clone(),"role":body.role.clone()})).await;
    Ok(Json(json!({"id":id,"project_id":project_id,"name":body.name,"role":body.role,"token":raw})))
}

async fn revoke_project_token(State(state): State<AppState>, headers: HeaderMap, Path((project_id, token_id)): Path<(String,String)>) -> ApiResult<StatusCode> {
    let principal = require_project_owner_or_admin(&state, &headers, &project_id).await?;
    let result = sqlx::query("UPDATE project_tokens SET revoked_at=NOW() WHERE id=$1 AND project_id=$2 AND revoked_at IS NULL")
        .bind(&token_id).bind(&project_id).execute(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_token_revoke_failed"))?;
    if result.rows_affected() == 0 { return Err(ApiError(StatusCode::NOT_FOUND, "project_token_not_found")); }
    audit(&state, &principal.actor, "project.token.revoke", "project", &project_id, json!({"token_id":token_id})).await;
    Ok(StatusCode::NO_CONTENT)
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
    ["openai_compatible", "anthropic", "gemini", "ollama"].contains(&adapter)
}

fn validate_ollama_url(raw: &str) -> Result<url::Url, ApiError> {
    let parsed = url::Url::parse(raw).map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_ollama_url"))?;
    if !["http", "https"].contains(&parsed.scheme()) || parsed.host_str().is_none()
        || !parsed.username().is_empty() || parsed.password().is_some()
        || parsed.query().is_some() || parsed.fragment().is_some()
        || !["", "/"].contains(&parsed.path())
    {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "unsafe_ollama_url"));
    }
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let loopback = host == "localhost" || host == "127.0.0.1" || host == "::1";
    let allowed = env::var("USH_OLLAMA_ALLOWED_HOSTS").unwrap_or_default()
        .split(',').map(str::trim).any(|entry| !entry.is_empty() && entry.eq_ignore_ascii_case(&host));
    if !loopback && !allowed {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "ollama_host_not_allowlisted"));
    }
    Ok(parsed)
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

fn is_allowed_ollama_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_loopback() || is_private_network_ip(IpAddr::V4(ipv4)),
        IpAddr::V6(ipv6) => {
            if let Some(mapped) = ipv6.to_ipv4_mapped() {
                mapped.is_loopback() || is_private_network_ip(IpAddr::V4(mapped))
            } else {
                ipv6.is_loopback() || is_private_network_ip(IpAddr::V6(ipv6))
            }
        }
    }
}

fn build_pinned_ollama_client(url: &url::Url, resolved: &[SocketAddr]) -> ApiResult<reqwest::Client> {
    let host = url.host_str().ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_ollama_host"))?;
    let port = url.port_or_known_default().unwrap_or(11434);
    if resolved.is_empty() || resolved.iter().any(|address| address.port() != port || !is_allowed_ollama_ip(address.ip())) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "ollama_destination_not_private"));
    }
    let pinned = resolved[0];
    if let Ok(literal_ip) = host.parse::<IpAddr>()
        && resolved.iter().any(|address| address.ip() != literal_ip)
    {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "ollama_ip_literal_mismatch"));
    }
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none());
    // Keep the URL hostname for HTTP Host and TLS SNI, but bypass all later DNS lookups.
    if host.parse::<IpAddr>().is_err() {
        builder = builder.resolve(host, pinned);
    }
    builder.build().map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "ollama_http_client_build_failed"))
}

async fn ollama_http_client(url: &url::Url) -> ApiResult<reqwest::Client> {
    let host = url.host_str().ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_ollama_host"))?;
    let port = url.port_or_known_default().unwrap_or(11434);
    let resolved: Vec<SocketAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host, port)).await
            .map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "ollama_dns_failed"))?
            .collect()
    };
    build_pinned_ollama_client(url, &resolved)
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
    if body.adapter_type == "ollama" {
        let url = body.base_url.as_deref().ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "ollama_base_url_required"))?;
        validate_ollama_url(url)?;
        if body.model.as_deref().is_none_or(str::is_empty) || body.api_key_env.is_some() || body.health_url.is_some() || body.supports_idempotency {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_ollama_provider"));
        }
    } else {
        if let Some(url) = body.base_url.as_deref() { validate_https_url(url)?; }
        if let Some(url) = body.health_url.as_deref() { validate_https_url(url)?; }
    }
    if is_ai_adapter(&body.adapter_type) && body.adapter_type != "ollama" {
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
        sqlx::query("SELECT id,adapter_type,base_url,health_url,timeout_seconds FROM providers WHERE id=$1 AND enabled=TRUE").bind(id).fetch_all(&state.db).await
    } else {
        sqlx::query("SELECT id,adapter_type,base_url,health_url,timeout_seconds FROM providers WHERE enabled=TRUE ORDER BY priority DESC LIMIT $1").bind(limit).fetch_all(&state.db).await
    }.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "provider_health_query_failed"))?;
    let mut results = Vec::new();
    for row in rows {
        let id: String = row.get("id");
        let adapter: String = row.get("adapter_type");
        let base_url: Option<String> = row.get("base_url");
        let health_url: Option<String> = row.get("health_url");
        let timeout: f64 = row.get("timeout_seconds");
        let started = Instant::now();
        let (status, detail) = if adapter == "ollama" {
            if let Some(raw_url) = base_url {
                match validate_ollama_url(&raw_url) {
                    Ok(url) => match ollama_http_client(&url).await {
                        Ok(client) => match client.get(format!("{}/api/tags", raw_url.trim_end_matches('/')))
                            .timeout(Duration::from_secs_f64(timeout.clamp(0.1, 5.0))).send().await {
                                Ok(response) if response.status().is_success() => ("healthy","ollama_tags_ok"),
                                Ok(_) => ("unhealthy","ollama_http_non_success"),
                                Err(_) => ("unhealthy","ollama_connection_failed"),
                            },
                        Err(_) => ("unhealthy","ollama_destination_not_private"),
                    },
                    Err(_) => ("unhealthy","ollama_url_not_allowed"),
                }
            } else { ("unhealthy","ollama_base_url_missing") }
        } else if let Some(raw_url) = health_url {
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
    let allowed_provider_ids: Option<Vec<String>> = if let Some(project_id) = principal.project_id {
        let project = sqlx::query("SELECT allowed_capabilities,allowed_provider_ids FROM projects WHERE id=$1")
            .bind(project_id).fetch_optional(&state.db).await
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
    let Some(base_url) = base_url else {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "provider_model_discovery_not_supported"));
    };
    let request = if adapter == "ollama" {
        let base = validate_ollama_url(&base_url)?;
        let client = ollama_http_client(&base).await?;
        client.get(format!("{}/api/tags", base_url.trim_end_matches('/')))
    } else {
        let key_name = key_name.ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "provider_credential_reference_required"))?;
        let api_key = resolve_provider_secret(&state,&key_name,None).await?
            .ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "provider_credential_unavailable"))?;
        let base = validate_https_url(&base_url)?;
        ensure_public_destination(&base).await?;
        match adapter.as_str() {
        "openai_compatible" => state.http.get(format!("{}/models",base_url.trim_end_matches('/'))).bearer_auth(&api_key),
        "anthropic" => state.http.get(format!("{}/v1/models",base_url.trim_end_matches('/')))
            .header("x-api-key",&api_key).header("anthropic-version","2023-06-01"),
        "gemini" => state.http.get(format!("{}/v1beta/models",base_url.trim_end_matches('/')))
            .header("x-goog-api-key",&api_key),
        _ => return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "provider_model_discovery_not_supported")),
        }
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
        "ollama" => data["models"].as_array().map_or_else(Vec::new, |items| items.iter().map(|item| json!({
            "id":item.get("model").or_else(||item.get("name")).cloned().unwrap_or(Value::Null),
            "name":item.get("name").cloned().unwrap_or(Value::Null),
            "size":item.get("size").cloned().unwrap_or(Value::Null)
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
            .bind(project_id).fetch_optional(&state.db).await
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
        let (Some(model), Some(base_url)) = (model, base_url) else { continue; };
        let api_key = if adapter == "ollama" {
            None
        } else {
            let Some(key_env) = key_env else { continue; };
            match resolve_provider_secret(&state, &key_env, principal.project_id.as_deref()).await {
                Ok(Some(value)) => Some(value),
                _ => {
                    audit(&state, &principal.actor, "ai.chat.failure", "provider", &provider_id, json!({"reason":"credential_unavailable"})).await;
                    continue;
                }
            }
        };
        let ollama_client = if adapter == "ollama" {
            match validate_ollama_url(&base_url) {
                Ok(url) => match ollama_http_client(&url).await {
                    Ok(client) => Some(client),
                    Err(_) => {
                        audit(&state, &principal.actor, "ai.chat.failure", "provider", &provider_id, json!({"reason":"local_endpoint_not_allowed"})).await;
                        continue;
                    }
                },
                Err(_) => {
                    audit(&state, &principal.actor, "ai.chat.failure", "provider", &provider_id, json!({"reason":"local_endpoint_not_allowed"})).await;
                    continue;
                }
            }
        } else {
            match validate_https_url(&base_url) {
                Ok(url) if ensure_public_destination(&url).await.is_ok() => {},
                _ => continue,
            }
            None
        };
        let request = match adapter.as_str() {
            "ollama" => ollama_client.as_ref().ok_or(ApiError(StatusCode::BAD_GATEWAY, "ollama_client_unavailable"))?
                .post(format!("{}/api/chat", base_url.trim_end_matches('/')))
                .json(&json!({
                    "model":model,
                    "messages":body.messages.iter().map(|m| json!({"role":m.role,"content":m.content})).collect::<Vec<_>>(),
                    "stream":false,
                    "options":{"temperature":body.temperature,"num_predict":body.max_tokens}
                })),
            "openai_compatible" => {
                let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
                let mut req = state.http.post(url).bearer_auth(api_key.as_deref().unwrap_or_default()).json(&json!({
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
                    .header("x-api-key",api_key.as_deref().unwrap_or_default()).header("anthropic-version","2023-06-01").json(&payload);
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
                    .header("x-goog-api-key",api_key.as_deref().unwrap_or_default()).json(&payload)
            }
            _ => continue,
        }.timeout(Duration::from_secs_f64(timeout.clamp(1.0,120.0)));
        attempts += 1;
        let started = Instant::now();
        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                audit(&state, &principal.actor, "ai.chat.failure", "provider", &provider_id, json!({"reason":if err.is_connect() {"connection_failed"} else {"network_error"}})).await;
                if err.is_connect() {
                    audit(&state, &principal.actor, "ai.chat.failover", "provider", &provider_id, json!({"reason":"connection_failed","next_candidate":true})).await;
                    continue;
                }
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
            "ollama" => {
                let text = data["message"]["content"].as_str().map(str::to_owned)
                    .ok_or(ApiError(StatusCode::BAD_GATEWAY, "provider_invalid_response"))?;
                (text, json!({"prompt_eval_count":data["prompt_eval_count"],"eval_count":data["eval_count"]}))
            }
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



async fn delivery_recovery_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(request): Json<schedule_recovery::RecoveryRequest>,
) -> ApiResult<Json<Value>> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch"));
    }
    let exists = sqlx::query("SELECT id FROM projects WHERE id=$1")
        .bind(&project_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_lookup_failed"))?;
    if exists.is_none() {
        return Err(ApiError(StatusCode::NOT_FOUND, "project_not_found"));
    }

    let task_count = request.tasks.len();
    let plan = schedule_recovery::assess(request).map_err(|error| {
        let code = match error {
            schedule_recovery::RecoveryError::DependencyCycle => "recovery_dependency_cycle",
            schedule_recovery::RecoveryError::UnknownDependency => "recovery_unknown_dependency",
            schedule_recovery::RecoveryError::DuplicateTaskId => "recovery_duplicate_task",
            schedule_recovery::RecoveryError::DuplicateDependency => "recovery_duplicate_dependency",
            _ => "invalid_recovery_plan",
        };
        ApiError(StatusCode::UNPROCESSABLE_ENTITY, code)
    })?;
    audit(
        &state,
        &principal.actor,
        "delivery.recovery_plan.preview",
        "project",
        &project_id,
        json!({
            "task_count": task_count,
            "status": plan.status,
            "delay_hours": plan.delay_hours,
            "delay_percent": plan.delay_percent,
            "critical_path_length": plan.critical_path_task_ids.len()
        }),
    )
    .await;
    Ok(Json(json!(plan)))
}




#[derive(Deserialize)]
struct JobEnqueueRequest {
    idempotency_key: String,
    job_type: String,
    agent_role: String,
    task_id: Option<Uuid>,
    #[serde(default = "default_job_priority")]
    priority: i16,
    #[serde(default = "default_max_attempts")]
    max_attempts: i32,
}
fn default_job_priority() -> i16 { 100 }
fn default_max_attempts() -> i32 { 3 }

fn valid_agent_role(value: &str) -> bool {
    ["planner", "architect", "implementation", "qa", "security_reviewer", "independent_reviewer", "researcher", "integrator"].contains(&value)
}

fn valid_worker_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 160 &&
        value.chars().all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c))
}

async fn upload_execution_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, job_id)): Path<(String, Uuid)>,
    body: axum::body::Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch"));
    }
    let store = state.artifact_store.as_ref()
        .ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "artifact_storage_unconfigured"))?;
    if body.is_empty() || body.len() > 2 * 1024 * 1024 {
        return Err(ApiError(StatusCode::PAYLOAD_TOO_LARGE, "artifact_size_limit"));
    }
    let content_type = headers.get("content-type").and_then(|v| v.to_str().ok())
        .ok_or(ApiError(StatusCode::UNSUPPORTED_MEDIA_TYPE, "artifact_content_type_required"))?;
    let artifact_type = headers.get("x-artifact-type").and_then(|v| v.to_str().ok())
        .ok_or(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "artifact_type_required"))?;
    let task_id = headers.get("x-task-id").and_then(|v| v.to_str().ok())
        .map(Uuid::parse_str).transpose()
        .map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_task_id"))?;

    let job_exists = sqlx::query("SELECT 1 FROM execution_jobs WHERE project_id=$1 AND id=$2")
        .bind(&project_id).bind(job_id).fetch_optional(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "artifact_job_lookup_failed"))?;
    if job_exists.is_none() { return Err(ApiError(StatusCode::NOT_FOUND, "execution_job_not_found")); }
    if let Some(task_id) = task_id {
        let task_exists = sqlx::query("SELECT 1 FROM project_tasks WHERE project_id=$1 AND id=$2")
            .bind(&project_id).bind(task_id).fetch_optional(&state.db).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "artifact_task_lookup_failed"))?;
        if task_exists.is_none() { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "task_not_in_project")); }
    }
    let artifact = store.put_and_register(&state.db, &project_id, job_id, task_id, artifact_type, content_type, &body)
        .await.map_err(|error| {
            tracing::error!(project_id, job_id = %job_id, error = ?error, "artifact upload failed");
            ApiError(StatusCode::INTERNAL_SERVER_ERROR, "artifact_upload_failed")
        })?;
    audit(&state, &principal.actor, "execution.artifact.upload", "execution_artifact", &artifact.artifact_id.to_string(),
        json!({"project_id":project_id,"job_id":job_id,"artifact_type":artifact_type,"size_bytes":artifact.size_bytes,"sha256":artifact.sha256})).await;
    Ok((StatusCode::CREATED, Json(json!({
        "artifact_id":artifact.artifact_id,"job_id":job_id,"size_bytes":artifact.size_bytes,
        "sha256":artifact.sha256,"content_type":artifact.content_type
    }))))
}

async fn download_execution_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, artifact_id)): Path<(String, Uuid)>,
) -> ApiResult<Response> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch"));
    }
    let store = state.artifact_store.as_ref()
        .ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "artifact_storage_unconfigured"))?;
    let row = sqlx::query("SELECT storage_key, content_sha256, size_bytes, content_type, job_id FROM execution_artifacts WHERE project_id=$1 AND id=$2")
        .bind(&project_id).bind(artifact_id).fetch_optional(&state.db).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "artifact_lookup_failed"))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "artifact_not_found"))?;
    let key: String = row.get("storage_key");
    let expected_sha: String = row.get("content_sha256");
    let expected_size: i64 = row.get("size_bytes");
    let content_type: String = row.get("content_type");
    let job_id: Option<Uuid> = row.get("job_id");
    let (bytes, stored_content_type) = store.get(&key).await.map_err(|error| {
        tracing::error!(artifact_id = %artifact_id, error = ?error, "artifact download failed");
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, "artifact_download_failed")
    })?;
    let actual_sha = hex::encode(Sha256::digest(&bytes));
    if bytes.len() as i64 != expected_size || actual_sha != expected_sha {
        return Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, "artifact_integrity_check_failed"));
    }
    let response_content_type = stored_content_type.unwrap_or(content_type);
    let content_type_header = HeaderValue::from_str(&response_content_type)
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "invalid_artifact_content_type"))?;
    audit(&state, &principal.actor, "execution.artifact.download", "execution_artifact", &artifact_id.to_string(),
        json!({"project_id":project_id,"job_id":job_id,"size_bytes":expected_size,"sha256":expected_sha})).await;
    let mut response = Response::new(axum::body::Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert("content-type", content_type_header);
    response.headers_mut().insert("x-artifact-sha256", HeaderValue::from_str(&expected_sha)
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "invalid_artifact_hash"))?);
    response.headers_mut().insert("content-length", HeaderValue::from_str(&expected_size.to_string())
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "invalid_artifact_size"))?);
    response.headers_mut().insert("content-disposition", HeaderValue::from_static("attachment; filename=artifact"));
    response.headers_mut().insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    Ok(response)
}

async fn enqueue_execution_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(body): Json<JobEnqueueRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch"));
    }
    if body.idempotency_key.is_empty() || body.idempotency_key.len() > 160
        || !body.idempotency_key.chars().all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c))
        || body.job_type.is_empty() || body.job_type.len() > 64
        || !body.job_type.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c))
        || !valid_agent_role(&body.agent_role)
        || !(0..=1000).contains(&body.priority)
        || !(1..=20).contains(&body.max_attempts)
    {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_execution_job"));
    }
    if let Some(task_id) = body.task_id {
        let exists = sqlx::query("SELECT 1 FROM project_tasks WHERE project_id=$1 AND id=$2")
            .bind(&project_id).bind(task_id).fetch_optional(&state.db).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_task_lookup_failed"))?;
        if exists.is_none() { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "task_not_in_project")); }
    }
    let id = Uuid::new_v4();
    let payload = json!({"agent_role":body.agent_role});
    let row = sqlx::query(
        "INSERT INTO execution_jobs (id,project_id,task_id,job_type,status,priority,idempotency_key,payload,max_attempts,created_at,updated_at) VALUES ($1,$2,$3,$4,'queued',$5,$6,$7,$8,NOW(),NOW()) ON CONFLICT (project_id,idempotency_key) DO UPDATE SET idempotency_key=EXCLUDED.idempotency_key WHERE execution_jobs.job_type=EXCLUDED.job_type AND execution_jobs.task_id IS NOT DISTINCT FROM EXCLUDED.task_id AND execution_jobs.payload=EXCLUDED.payload AND execution_jobs.priority=EXCLUDED.priority AND execution_jobs.max_attempts=EXCLUDED.max_attempts RETURNING id,status,created_at"
    ).bind(id).bind(&project_id).bind(body.task_id).bind(&body.job_type).bind(body.priority)
     .bind(&body.idempotency_key).bind(payload).bind(body.max_attempts)
     .fetch_optional(&state.db).await
     .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_enqueue_failed"))?
     .ok_or(ApiError(StatusCode::CONFLICT, "idempotency_key_payload_mismatch"))?;
    let job_id: Uuid = row.get("id");
    let status: String = row.get("status");
    audit(&state, &principal.actor, "execution.job.enqueue", "execution_job", &job_id.to_string(),
        json!({"project_id":project_id,"job_type":body.job_type,"agent_role":body.agent_role,"status":status})).await;
    let created = job_id == id;
    Ok((if created { StatusCode::CREATED } else { StatusCode::OK },
        Json(json!({"job_id":job_id,"status":status,"created":created,"created_at":row.get::<DateTime<Utc>,_>("created_at")}))))
}

#[derive(Deserialize)]
struct WorkerClaimRequest {
    worker_id: String,
    #[serde(default = "default_lease_seconds")]
    lease_seconds: i64,
}
fn default_lease_seconds() -> i64 { 60 }

async fn claim_execution_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(body): Json<WorkerClaimRequest>,
) -> ApiResult<Json<Value>> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch"));
    }
    if !valid_worker_id(&body.worker_id) || !(10..=300).contains(&body.lease_seconds) {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_worker_claim"));
    }
    let mut tx = state.db.begin().await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_transaction_failed"))?;

    // Expired leases are made retryable (or dead-lettered) before a fresh claim.
    sqlx::query(
        "UPDATE execution_attempts a SET status='lease_expired',finished_at=NOW(),duration_ms=GREATEST(0,(EXTRACT(EPOCH FROM (NOW()-a.started_at))*1000)::bigint) FROM execution_jobs j WHERE a.project_id=$1 AND j.project_id=$1 AND a.job_id=j.id AND a.lease_token=j.lease_token AND a.status='running' AND j.status='leased' AND j.lease_expires_at<=NOW()"
    ).bind(&project_id).execute(&mut *tx).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_recovery_failed"))?;
    sqlx::query(
        "UPDATE execution_jobs SET status=CASE WHEN attempt_count>=max_attempts THEN 'dead_letter' ELSE 'retry_wait' END,available_at=NOW(),lease_owner=NULL,lease_token=NULL,lease_expires_at=NULL,heartbeat_at=NULL,finished_at=CASE WHEN attempt_count>=max_attempts THEN NOW() ELSE NULL END,last_error_code='lease_expired',updated_at=NOW() WHERE project_id=$1 AND status='leased' AND lease_expires_at<=NOW()"
    ).bind(&project_id).execute(&mut *tx).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_recovery_failed"))?;

    let row = sqlx::query(
        "SELECT j.id,j.task_id,j.job_type,j.payload,j.checkpoint,j.attempt_count,j.max_attempts,j.priority,t.title AS task_title,t.objective AS task_objective,t.owner_role AS task_owner_role FROM execution_jobs j LEFT JOIN project_tasks t ON t.project_id=j.project_id AND t.id=j.task_id WHERE j.project_id=$1 AND j.status IN ('queued','retry_wait') AND j.available_at<=NOW() AND j.attempt_count<j.max_attempts ORDER BY j.priority DESC,j.available_at,j.created_at LIMIT 1 FOR UPDATE OF j SKIP LOCKED"
    ).bind(&project_id).fetch_optional(&mut *tx).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_claim_failed"))?;
    let Some(row) = row else {
        tx.commit().await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_transaction_failed"))?;
        return Ok(Json(json!({"job":null})));
    };
    let job_id: Uuid = row.get("id");
    let task_id: Option<Uuid> = row.get("task_id");
    let job_type: String = row.get("job_type");
    let payload: Value = row.get("payload");
    let checkpoint: Value = row.get("checkpoint");
    let task_title: Option<String> = row.try_get("task_title").ok();
    let task_objective: Option<String> = row.try_get("task_objective").ok();
    let task_owner_role: Option<String> = row.try_get("task_owner_role").ok();
    let attempt_number: i32 = row.get::<i32,_>("attempt_count") + 1;
    let lease_token = Uuid::new_v4();
    let updated = sqlx::query(
        "UPDATE execution_jobs SET status='leased',attempt_count=$1,lease_owner=$2,lease_token=$3,lease_expires_at=NOW()+($4 * INTERVAL '1 second'),heartbeat_at=NOW(),updated_at=NOW() WHERE project_id=$5 AND id=$6 RETURNING lease_expires_at"
    ).bind(attempt_number).bind(&body.worker_id).bind(lease_token).bind(body.lease_seconds)
     .bind(&project_id).bind(job_id).fetch_one(&mut *tx).await
     .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_claim_update_failed"))?;
    sqlx::query(
        "INSERT INTO execution_attempts (project_id,job_id,attempt_number,worker_id,lease_token,status) VALUES ($1,$2,$3,$4,$5,'running')"
    ).bind(&project_id).bind(job_id).bind(attempt_number).bind(&body.worker_id).bind(lease_token)
     .execute(&mut *tx).await
     .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_attempt_create_failed"))?;
    sqlx::query(
        "INSERT INTO project_events (project_id,task_id,event_type,actor,metadata) VALUES ($1,$2,'execution.job.claimed',$3,$4)"
    ).bind(&project_id).bind(task_id).bind(&body.worker_id)
     .bind(json!({"job_id":job_id,"attempt":attempt_number,"job_type":job_type}))
     .execute(&mut *tx).await
     .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_event_write_failed"))?;
    tx.commit().await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_transaction_failed"))?;
    Ok(Json(json!({"job":{
        "id":job_id,"project_id":project_id,"task_id":task_id,"job_type":job_type,
        "payload":payload,"checkpoint":checkpoint,"attempt":attempt_number,
        "task_context":task_id.map(|_| json!({"title":task_title,"objective":task_objective,"owner_role":task_owner_role})),
        "worker_id":body.worker_id,"lease_token":lease_token,
        "lease_expires_at":updated.get::<DateTime<Utc>,_>("lease_expires_at")
    }})))
}

#[derive(Deserialize)]
struct WorkerLeaseRequest {
    worker_id: String,
    lease_token: Uuid,
    #[serde(default = "default_lease_seconds")]
    lease_seconds: i64,
}

async fn heartbeat_execution_job(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id, job_id)): Path<(String, Uuid)>,
    Json(body): Json<WorkerLeaseRequest>,
) -> ApiResult<Json<Value>> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) { return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch")); }
    if !valid_worker_id(&body.worker_id) || !(10..=300).contains(&body.lease_seconds) { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_worker_heartbeat")); }
    let row = sqlx::query("UPDATE execution_jobs SET heartbeat_at=NOW(),lease_expires_at=NOW()+($1 * INTERVAL '1 second'),updated_at=NOW() WHERE project_id=$2 AND id=$3 AND status='leased' AND lease_owner=$4 AND lease_token=$5 AND lease_expires_at>NOW() RETURNING lease_expires_at")
        .bind(body.lease_seconds).bind(&project_id).bind(job_id).bind(&body.worker_id).bind(body.lease_token)
        .fetch_optional(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_heartbeat_failed"))?
        .ok_or(ApiError(StatusCode::CONFLICT, "execution_lease_lost"))?;
    Ok(Json(json!({"job_id":job_id,"lease_expires_at":row.get::<DateTime<Utc>,_>("lease_expires_at")})))
}

#[derive(Deserialize)]
struct WorkerCheckpointRequest {
    worker_id: String,
    lease_token: Uuid,
    checkpoint: Value,
}
fn valid_execution_checkpoint(value: &Value) -> bool {
    let Some(object) = value.as_object() else { return false; };
    if object.len() > 5 { return false; }
    for (key, item) in object {
        match key.as_str() {
            "phase" => if !item.as_str().is_some_and(|v| v.len() <= 48 && v.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))) { return false; },
            "cursor" => if !item.as_str().is_some_and(|v| v.len() <= 256 && !v.chars().any(|c| c == '\n' || c == '\r')) { return false; },
            "progress" => if !item.as_f64().is_some_and(|v| v.is_finite() && (0.0..=100.0).contains(&v)) { return false; },
            "completed_steps" => if !item.as_array().is_some_and(|v| v.len() <= 100 && v.iter().all(|x| x.as_str().is_some_and(|s| s.len() <= 80))) { return false; },
            "artifact_ids" => if !item.as_array().is_some_and(|v| v.len() <= 50 && v.iter().all(|x| x.as_str().and_then(|s| Uuid::parse_str(s).ok()).is_some())) { return false; },
            _ => return false,
        }
    }
    true
}

async fn checkpoint_execution_job(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id, job_id)): Path<(String, Uuid)>,
    Json(body): Json<WorkerCheckpointRequest>,
) -> ApiResult<StatusCode> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) { return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch")); }
    if !valid_worker_id(&body.worker_id) || !valid_execution_checkpoint(&body.checkpoint) { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_execution_checkpoint")); }
    let result = sqlx::query("UPDATE execution_jobs SET checkpoint=$1,updated_at=NOW() WHERE project_id=$2 AND id=$3 AND status='leased' AND lease_owner=$4 AND lease_token=$5 AND lease_expires_at>NOW()")
        .bind(&body.checkpoint).bind(&project_id).bind(job_id).bind(&body.worker_id).bind(body.lease_token)
        .execute(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_checkpoint_failed"))?;
    if result.rows_affected()==0 { return Err(ApiError(StatusCode::CONFLICT, "execution_lease_lost")); }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct WorkerCompleteRequest {
    worker_id: String,
    lease_token: Uuid,
    #[serde(default)]
    output_artifact_ids: Vec<Uuid>,
    next_agent_role: Option<String>,
    next_job_type: Option<String>,
    next_idempotency_key: Option<String>,
}

async fn complete_execution_job(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id, job_id)): Path<(String, Uuid)>,
    Json(body): Json<WorkerCompleteRequest>,
) -> ApiResult<Json<Value>> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) { return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch")); }
    if !valid_worker_id(&body.worker_id) || body.output_artifact_ids.len()>50
        || body.output_artifact_ids.iter().collect::<std::collections::HashSet<_>>().len()!=body.output_artifact_ids.len()
        || body.next_agent_role.as_deref().is_some_and(|v| !valid_agent_role(v))
        || body.next_job_type.as_deref().is_some_and(|v| v.is_empty() || v.len()>64 || !v.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c)))
        || body.next_idempotency_key.as_deref().is_some_and(|v| v.is_empty() || v.len()>160 || !v.chars().all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c)))
        || (body.next_agent_role.is_some() != body.next_job_type.is_some() || body.next_agent_role.is_some() != body.next_idempotency_key.is_some())
    { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_execution_completion")); }
    let mut tx = state.db.begin().await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_transaction_failed"))?;
    for artifact_id in &body.output_artifact_ids {
        let exists = sqlx::query("SELECT 1 FROM execution_artifacts WHERE project_id=$1 AND id=$2 AND job_id=$3")
            .bind(&project_id).bind(artifact_id).bind(job_id).fetch_optional(&mut *tx).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_artifact_lookup_failed"))?;
        if exists.is_none() { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "artifact_not_from_job")); }
    }
    let row = sqlx::query("UPDATE execution_jobs SET status='succeeded',lease_owner=NULL,lease_token=NULL,lease_expires_at=NULL,heartbeat_at=NULL,finished_at=NOW(),updated_at=NOW() WHERE project_id=$1 AND id=$2 AND status='leased' AND lease_owner=$3 AND lease_token=$4 AND lease_expires_at>NOW() RETURNING task_id")
        .bind(&project_id).bind(job_id).bind(&body.worker_id).bind(body.lease_token).fetch_optional(&mut *tx).await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_completion_failed"))?
        .ok_or(ApiError(StatusCode::CONFLICT, "execution_lease_lost"))?;
    let task_id: Option<Uuid> = row.get("task_id");
    let attempt_update = sqlx::query("UPDATE execution_attempts SET status='succeeded',finished_at=NOW(),duration_ms=GREATEST(0,(EXTRACT(EPOCH FROM (NOW()-started_at))*1000)::bigint),result_metadata=$1 WHERE project_id=$2 AND job_id=$3 AND lease_token=$4 AND status='running'")
        .bind(json!({"output_artifact_ids":body.output_artifact_ids})).bind(&project_id).bind(job_id).bind(body.lease_token)
        .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_attempt_update_failed"))?;
    if attempt_update.rows_affected()!=1 { return Err(ApiError(StatusCode::CONFLICT, "execution_attempt_not_running")); }
    let handoff_id = if let (Some(role),Some(kind),Some(key)) = (&body.next_agent_role,&body.next_job_type,&body.next_idempotency_key) {
        let next_payload = json!({"agent_role":role,"source_job_id":job_id,"input_artifact_ids":body.output_artifact_ids});
        let inserted = sqlx::query("INSERT INTO execution_jobs (project_id,task_id,job_type,status,priority,idempotency_key,payload,max_attempts) VALUES ($1,$2,$3,'queued',100,$4,$5,3) ON CONFLICT (project_id,idempotency_key) DO NOTHING RETURNING id")
            .bind(&project_id).bind(task_id).bind(kind).bind(key).bind(&next_payload).fetch_optional(&mut *tx).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_handoff_failed"))?;
        if let Some(row) = inserted {
            Some(row.get::<Uuid,_>("id"))
        } else {
            let existing = sqlx::query("SELECT id FROM execution_jobs WHERE project_id=$1 AND idempotency_key=$2 AND job_type=$3 AND task_id IS NOT DISTINCT FROM $4 AND payload=$5")
                .bind(&project_id).bind(key).bind(kind).bind(task_id).bind(&next_payload).fetch_optional(&mut *tx).await
                .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_handoff_lookup_failed"))?
                .ok_or(ApiError(StatusCode::CONFLICT, "handoff_idempotency_payload_mismatch"))?;
            Some(existing.get::<Uuid,_>("id"))
        }
    } else { None };
    sqlx::query("INSERT INTO project_events (project_id,task_id,event_type,actor,metadata) VALUES ($1,$2,'execution.job.succeeded',$3,$4)")
        .bind(&project_id).bind(task_id).bind(&body.worker_id)
        .bind(json!({"job_id":job_id,"output_artifact_count":body.output_artifact_ids.len(),"handoff_job_id":handoff_id}))
        .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_event_write_failed"))?;
    tx.commit().await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_transaction_failed"))?;
    Ok(Json(json!({"job_id":job_id,"status":"succeeded","handoff_job_id":handoff_id})))
}

#[derive(Deserialize)]
struct WorkerFailRequest {
    worker_id: String,
    lease_token: Uuid,
    error_code: String,
    retryable: bool,
}

async fn fail_execution_job(
    State(state): State<AppState>, headers: HeaderMap, Path((project_id, job_id)): Path<(String, Uuid)>,
    Json(body): Json<WorkerFailRequest>,
) -> ApiResult<Json<Value>> {
    let principal = require_operator(&state, &headers).await?;
    if !can_manage_delivery_schedule(&principal, &project_id) { return Err(ApiError(StatusCode::FORBIDDEN, "project_scope_mismatch")); }
    if !valid_worker_id(&body.worker_id) || body.error_code.is_empty() || body.error_code.len()>100
        || !body.error_code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c))
    { return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_execution_failure")); }
    let mut tx = state.db.begin().await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_transaction_failed"))?;
    let row = sqlx::query("UPDATE execution_jobs SET status=CASE WHEN $1 AND attempt_count<max_attempts THEN 'retry_wait' WHEN $1 THEN 'dead_letter' ELSE 'failed' END,available_at=NOW()+LEAST(300,POWER(2,LEAST(attempt_count,8))::integer)*INTERVAL '1 second',lease_owner=NULL,lease_token=NULL,lease_expires_at=NULL,heartbeat_at=NULL,finished_at=CASE WHEN NOT $1 OR attempt_count>=max_attempts THEN NOW() ELSE NULL END,last_error_code=$2,updated_at=NOW() WHERE project_id=$3 AND id=$4 AND status='leased' AND lease_owner=$5 AND lease_token=$6 AND lease_expires_at>NOW() RETURNING status")
        .bind(body.retryable).bind(&body.error_code).bind(&project_id).bind(job_id).bind(&body.worker_id).bind(body.lease_token)
        .fetch_optional(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_failure_update_failed"))?
        .ok_or(ApiError(StatusCode::CONFLICT, "execution_lease_lost"))?;
    let status: String = row.get("status");
    let attempt_status = if body.retryable { "retryable_failure" } else { "permanent_failure" };
    let attempt_update = sqlx::query("UPDATE execution_attempts SET status=$1,finished_at=NOW(),duration_ms=GREATEST(0,(EXTRACT(EPOCH FROM (NOW()-started_at))*1000)::bigint),error_code=$2 WHERE project_id=$3 AND job_id=$4 AND lease_token=$5 AND status='running'")
        .bind(attempt_status).bind(&body.error_code).bind(&project_id).bind(job_id).bind(body.lease_token)
        .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_attempt_update_failed"))?;
    if attempt_update.rows_affected()!=1 { return Err(ApiError(StatusCode::CONFLICT, "execution_attempt_not_running")); }
    sqlx::query("INSERT INTO project_events (project_id,event_type,actor,metadata) VALUES ($1,'execution.job.failed',$2,$3)")
        .bind(&project_id).bind(&body.worker_id).bind(json!({"job_id":job_id,"error_code":body.error_code,"status":status}))
        .execute(&mut *tx).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_event_write_failed"))?;
    tx.commit().await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "execution_transaction_failed"))?;
    Ok(Json(json!({"job_id":job_id,"status":status})))
}


#[derive(Deserialize)]
struct AuditQuery { #[serde(default = "default_limit")] limit: i64 }
fn default_limit() -> i64 { 50 }

async fn list_audit(State(state): State<AppState>, headers: HeaderMap, Query(query): Query<AuditQuery>) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers).await?;
    let limit = query.limit.clamp(1,100);
    let rows = sqlx::query("SELECT id,occurred_at,actor,action,resource_type,resource_id,details FROM audit_events ORDER BY occurred_at DESC LIMIT $1")
        .bind(limit).fetch_all(&state.db).await.map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "audit_list_failed"))?;
    let values: Vec<Value> = rows.into_iter().map(|r| json!({
        "id":r.get::<String,_>("id"),"occurred_at":r.get::<DateTime<Utc>,_>("occurred_at"),
        "actor":r.get::<String,_>("actor"),"action":r.get::<String,_>("action"),
        "resource_type":r.get::<String,_>("resource_type"),"resource_id":r.get::<String,_>("resource_id"),
        "details":r.get::<Value,_>("details")
    })).collect();
    Ok(Json(json!(values)))
}

    #[cfg(test)]
    #[test]
    fn ollama_adapter_requires_private_allowlisted_endpoint() {
        assert!(is_ai_adapter("ollama"));
        assert!(validate_ollama_url("http://localhost:11434").is_ok());
        assert!(validate_ollama_url("http://127.0.0.1:11434").is_ok());
        assert!(validate_ollama_url("https://api.example.com").is_err());
        assert!(validate_ollama_url("http://169.254.169.254").is_err());
        assert!(validate_ollama_url("http://ollama:11434").is_err());
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
    let db = PgPoolOptions::new().max_connections(20).acquire_timeout(Duration::from_secs(10)).connect(&database_url).await?;
    sqlx::migrate!("./rust-migrations").run(&db).await?;
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
    let artifact_configured = ["USH_ARTIFACT_BUCKET", "USH_ARTIFACT_REGION", "USH_ARTIFACT_ENDPOINT", "USH_ARTIFACT_ACCESS_KEY_ID", "USH_ARTIFACT_SECRET_ACCESS_KEY", "USH_ARTIFACT_PREFIX", "USH_ARTIFACT_MAX_BYTES"]
        .iter().any(|name| env::var(name).ok().is_some_and(|value| !value.trim().is_empty()));
    let artifact_store = if artifact_configured { Some(artifact_store::ArtifactStore::from_env()?) } else { None };
    let state = AppState { db, admin_token: Arc::new(admin_token), http, infisical, artifact_store, rate_limit_per_minute };
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/projects", get(list_projects).post(create_project))
        .route("/v1/projects/{project_id}", put(update_project).delete(delete_project))
        .route("/v1/projects/{project_id}/tokens", get(list_project_tokens).post(create_project_token))
        .route("/v1/projects/{project_id}/tokens/{token_id}", delete(revoke_project_token))
        .route("/v1/providers", get(list_providers))
        .route("/v1/providers/{provider_id}", put(upsert_provider).delete(delete_provider))
        .route("/v1/providers/{provider_id}/models", get(provider_models))
        .route("/v1/integrations/{provider_id}/verify", post(verify_integration))
        .route("/v1/providers/health-check", post(health_check))
        .route("/v1/ai/chat", post(ai_chat))
        .route("/v1/projects/{project_id}/delivery/recovery-plan", post(delivery_recovery_plan))
        .route("/v1/projects/{project_id}/jobs", post(enqueue_execution_job))
        .route("/v1/projects/{project_id}/jobs/{job_id}/artifacts", post(upload_execution_artifact))
        .route("/v1/projects/{project_id}/artifacts/{artifact_id}", get(download_execution_artifact))
        .route("/v1/projects/{project_id}/workers/claim", post(claim_execution_job))
        .route("/v1/projects/{project_id}/jobs/{job_id}/heartbeat", post(heartbeat_execution_job))
        .route("/v1/projects/{project_id}/jobs/{job_id}/checkpoint", put(checkpoint_execution_job))
        .route("/v1/projects/{project_id}/jobs/{job_id}/complete", post(complete_execution_job))
        .route("/v1/projects/{project_id}/jobs/{job_id}/fail", post(fail_execution_job))
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
    fn delivery_schedule_access_is_project_scoped_and_operator_only() {
        let owner = Principal { actor: "owner".into(), role: "owner".into(), project_id: Some("alpha".into()), admin: false };
        let operator = Principal { actor: "operator".into(), role: "operator".into(), project_id: Some("alpha".into()), admin: false };
        let viewer = Principal { actor: "viewer".into(), role: "viewer".into(), project_id: Some("alpha".into()), admin: false };
        let other_project = Principal { actor: "other".into(), role: "operator".into(), project_id: Some("beta".into()), admin: false };
        let admin = Principal { actor: "admin".into(), role: "admin".into(), project_id: None, admin: true };

        assert!(can_manage_delivery_schedule(&owner, "alpha"));
        assert!(can_manage_delivery_schedule(&operator, "alpha"));
        assert!(!can_manage_delivery_schedule(&viewer, "alpha"));
        assert!(!can_manage_delivery_schedule(&other_project, "alpha"));
        assert!(can_manage_delivery_schedule(&admin, "alpha"));
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
    fn ollama_pinning_rejects_mixed_public_private_and_metadata_answers() {
        let url = url::Url::parse("http://ollama.internal:11434").unwrap();
        let private = "10.2.3.4:11434".parse::<SocketAddr>().unwrap();
        let public = "8.8.8.8:11434".parse::<SocketAddr>().unwrap();
        let metadata = "169.254.169.254:11434".parse::<SocketAddr>().unwrap();
        assert!(build_pinned_ollama_client(&url, &[private]).is_ok());
        assert!(build_pinned_ollama_client(&url, &[private, public]).is_err());
        assert!(build_pinned_ollama_client(&url, &[metadata]).is_err());
        assert!(build_pinned_ollama_client(&url, &[]).is_err());

        let private_mapped = "[::ffff:10.2.3.4]:11434".parse::<SocketAddr>().unwrap();
        assert!(build_pinned_ollama_client(&url, &[private_mapped]).is_ok());
        let literal_url = url::Url::parse("http://10.0.0.2:11434").unwrap();
        let wrong_literal = "10.0.0.3:11434".parse::<SocketAddr>().unwrap();
        let wrong_port = "10.0.0.2:11435".parse::<SocketAddr>().unwrap();
        let matching_literal = "10.0.0.2:11434".parse::<SocketAddr>().unwrap();
        assert!(build_pinned_ollama_client(&literal_url, &[wrong_literal]).is_err());
        assert!(build_pinned_ollama_client(&literal_url, &[wrong_port]).is_err());
        assert!(build_pinned_ollama_client(&literal_url, &[matching_literal]).is_ok());
    }

    #[test]
    fn ollama_pinning_accepts_only_loopback_rfc1918_and_ula() {
        for raw in ["127.0.0.1:11434", "10.0.0.2:11434", "172.16.0.2:11434", "192.168.1.2:11434", "[fd00::1]:11434", "[::1]:11434"] {
            let address = raw.parse::<SocketAddr>().unwrap();
            assert!(is_allowed_ollama_ip(address.ip()), "{raw}");
        }
        for raw in ["169.254.169.254:11434", "100.64.0.1:11434", "8.8.8.8:11434", "[fe80::1]:11434", "[2001:4860:4860::8888]:11434", "[::ffff:169.254.169.254]:11434"] {
            let address = raw.parse::<SocketAddr>().unwrap();
            assert!(!is_allowed_ollama_ip(address.ip()), "{raw}");
        }
    }

    #[tokio::test]
    async fn ollama_client_pins_dns_result_preserves_host_and_disables_redirects() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let count = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..count]).to_ascii_lowercase();
            assert!(request.contains(&format!("host: ollama.test:{}", address.port())));
            assert!(request.starts_with("get /api/tags "));
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://169.254.169.254/latest/meta-data/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let url = url::Url::parse(&format!("http://ollama.test:{}/api/tags", address.port())).unwrap();
        let pinned = SocketAddr::new("127.0.0.1".parse().unwrap(), address.port());
        let client = build_pinned_ollama_client(&url, &[pinned]).unwrap();
        let response = client.get(url).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        server.join().unwrap();
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
            project_id: project_id.map(str::to_owned),
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

#[cfg(test)]
mod worker_contract_tests {
    use super::*;

    #[test]
    fn specialist_roles_are_an_explicit_allowlist() {
        for role in ["planner", "architect", "implementation", "qa", "security_reviewer", "independent_reviewer", "researcher", "integrator"] {
            assert!(valid_agent_role(role));
        }
        for role in ["admin", "shell", "../implementation", "unknown"] {
            assert!(!valid_agent_role(role));
        }
    }

    #[test]
    fn worker_identity_is_bounded_and_rejects_path_or_whitespace_characters() {
        assert!(valid_worker_id("worker-eu-01"));
        assert!(valid_worker_id("runner:job_123"));
        assert!(!valid_worker_id(""));
        assert!(!valid_worker_id("../worker"));
        assert!(!valid_worker_id("worker id"));
        assert!(!valid_worker_id(&"w".repeat(161)));
    }

    #[test]
    fn checkpoints_accept_only_small_non_sensitive_metadata_contract() {
        assert!(valid_execution_checkpoint(&json!({
            "phase":"implementation",
            "progress":42.5,
            "completed_steps":["inspect","patch"],
            "artifact_ids":[Uuid::new_v4().to_string()]
        })));
        assert!(valid_execution_checkpoint(&json!({"cursor":"page:3"})));
        assert!(!valid_execution_checkpoint(&json!({"prompt":"write a feature"})));
        assert!(!valid_execution_checkpoint(&json!({"phase":"bad phase"})));
        assert!(!valid_execution_checkpoint(&json!({"progress":101.0})));
        assert!(!valid_execution_checkpoint(&json!({"cursor":"line\nsecret"})));
        assert!(!valid_execution_checkpoint(&json!({"unknown":true})));
    }
}
