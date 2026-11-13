use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    extract::{Path, Query, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use rand::{distr::Alphanumeric, Rng};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use subtle::ConstantTimeEq;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: PgPool,
    admin_token: Arc<String>,
}

#[derive(Debug)]
struct ApiError(StatusCode, &'static str);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
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
    let token = value.strip_prefix("Bearer ")
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "bearer_token_required"))?;
    if token.len() < 32 {
        return Err(ApiError(StatusCode::UNAUTHORIZED, "invalid_bearer_token"));
    }
    Ok(token)
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let token = bearer(headers)?;
    if token.as_bytes().ct_eq(state.admin_token.as_bytes()).into() {
        return Ok(Principal { actor: "service-token".into(), role: "admin".into(), project_id: None, admin: true });
    }
    let digest = hex::encode(Sha256::digest(token.as_bytes()));
    let row = sqlx::query(
        "SELECT id, project_id, role FROM project_tokens WHERE token_hash = $1 AND revoked_at IS NULL"
    ).bind(digest).fetch_optional(&state.db).await
      .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "authentication_store_error"))?
      .ok_or(ApiError(StatusCode::UNAUTHORIZED, "invalid_bearer_token"))?;
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

async fn require_operator(state: &AppState, headers: &HeaderMap) -> ApiResult<Principal> {
    let principal = authenticate(state, headers).await?;
    if !principal.admin && principal.role != "owner" && principal.role != "operator" {
        return Err(ApiError(StatusCode::FORBIDDEN, "project_operator_required"));
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
    if body.id.is_empty() || body.id.len() > 80 || !body.id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) || body.name.trim().is_empty() || body.allowed_capabilities.is_empty() {
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
struct TokenCreate { name: String, #[serde(default = "default_role")] role: String }
fn default_role() -> String { "operator".into() }

async fn create_project_token(State(state): State<AppState>, headers: HeaderMap, Path(project_id): Path<String>, Json(body): Json<TokenCreate>) -> ApiResult<Json<Value>> {
    let principal = require_admin(&state, &headers).await?;
    if !["owner","operator","viewer"].contains(&body.role.as_str()) || body.name.trim().is_empty() || body.name.len() > 120 {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "invalid_project_token"));
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
    let principal = require_admin(&state, &headers).await?;
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
struct ProviderSelect { capability: String, #[serde(default)] exclude: Vec<String> }

async fn select_provider(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<ProviderSelect>) -> ApiResult<Json<Value>> {
    let principal = require_operator(&state, &headers).await?;
    let allowed_provider_ids: Option<Vec<String>> = if let Some(project_id) = principal.project_id {
        let project = sqlx::query("SELECT allowed_capabilities,allowed_provider_ids FROM projects WHERE id=$1")
            .bind(project_id).fetch_optional(&state.db).await
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "project_scope_failed"))?
            .ok_or(ApiError(StatusCode::FORBIDDEN, "project_not_found"))?;
        let caps: Value = project.get("allowed_capabilities");
        if !caps.as_array().is_some_and(|a| a.iter().any(|v| v.as_str()==Some(&body.capability))) {
            return Err(ApiError(StatusCode::FORBIDDEN, "capability_not_allowed"));
        }
        let allowed: Value = project.get("allowed_provider_ids");
        let ids = allowed.as_array().map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>()).unwrap_or_default();
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();
    let database_url = env::var("USH_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"))?;
    let admin_token = env::var("USH_API_TOKEN")?;
    if admin_token.len() < 32 { anyhow::bail!("USH_API_TOKEN must contain at least 32 characters"); }
    let db = PgPoolOptions::new().max_connections(20).acquire_timeout(Duration::from_secs(10)).connect(&database_url).await?;
    sqlx::migrate!("./rust-migrations").run(&db).await?;
    let state = AppState { db, admin_token: Arc::new(admin_token) };
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/projects", get(list_projects).post(create_project))
        .route("/v1/projects/{project_id}/tokens", post(create_project_token))
        .route("/v1/projects/{project_id}/tokens/{token_id}", delete(revoke_project_token))
        .route("/v1/providers", get(list_providers))
        .route("/v1/providers/select", post(select_provider))
        .route("/v1/audit", get(list_audit))
        .with_state(state)
        .layer(TraceLayer::new_for_http());
    let address: SocketAddr = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()).parse()?;
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8000".into()).parse()?;
    let listener = tokio::net::TcpListener::bind((address,port)).await?;
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
            assert!(!invalid.is_empty() && !invalid.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)));
        }
    }

    #[test]
    fn token_hash_is_one_way_sha256_hex() {
        let raw = "test-project-token-that-is-never-stored";
        let digest = hex::encode(Sha256::digest(raw.as_bytes()));
        assert_eq!(digest.len(), 64);
        assert_ne!(digest, raw);
    }
}
