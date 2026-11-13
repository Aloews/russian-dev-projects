mod connectors;
mod providers;

use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::{env, sync::Arc, time::Instant};
use subtle::ConstantTimeEq;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    api_token: Arc<String>,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    runtime: &'static str,
}

#[derive(Serialize)]
struct ConnectionTest {
    provider_id: String,
    status: &'static str,
    http_status: u16,
    latency_ms: u128,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

fn authorized(headers: &HeaderMap, expected: &str) -> bool {
    let Some(value) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return false;
    };
    token.len() == expected.len() && bool::from(token.as_bytes().ct_eq(expected.as_bytes()))
}

async fn healthz() -> Json<Health> {
    Json(Health { status: "ok", runtime: "rust" })
}

async fn providers(headers: HeaderMap, State(state): State<AppState>) -> impl IntoResponse {
    if !authorized(&headers, &state.api_token) {
        return (StatusCode::UNAUTHORIZED, Json(ErrorBody { error: "unauthorized" })).into_response();
    }
    (StatusCode::OK, Json(providers::catalog())).into_response()
}

async fn test_connection(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> impl IntoResponse {
    if !authorized(&headers, &state.api_token) {
        return (StatusCode::UNAUTHORIZED, Json(ErrorBody { error: "unauthorized" })).into_response();
    }
    if providers::find(&provider_id).is_none() {
        return (StatusCode::NOT_FOUND, Json(ErrorBody { error: "unknown_provider" })).into_response();
    }
    let started = Instant::now();
    match connectors::test(&state.client, &provider_id).await {
        Ok(http_status) => (
            StatusCode::OK,
            Json(ConnectionTest {
                provider_id,
                status: "connected",
                http_status,
                latency_ms: started.elapsed().as_millis(),
            }),
        ).into_response(),
        Err(error) => {
            tracing::warn!(provider = %provider_id, error = %error, "provider connection test failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorBody { error: "provider_connection_failed" }),
            ).into_response()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let token = env::var("USH_API_TOKEN").map_err(|_| anyhow::anyhow!("USH_API_TOKEN is required"))?;
    if token.len() < 32 {
        anyhow::bail!("USH_API_TOKEN must contain at least 32 characters");
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("universal-service-hub/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let state = AppState { api_token: Arc::new(token), client };
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/providers", get(providers))
        .route("/v1/connections/{provider_id}/test", post(test_connection))
        .with_state(state);
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_owned());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!(%port, "Universal Service Hub Rust API listening");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_and_malformed_authorization() {
        let headers = HeaderMap::new();
        assert!(!authorized(&headers, "a".repeat(32).as_str()));
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Token nope".parse().unwrap());
        assert!(!authorized(&headers, "a".repeat(32).as_str()));
    }

    #[test]
    fn accepts_only_exact_bearer_token() {
        let token = "a".repeat(40);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        assert!(authorized(&headers, &token));
        assert!(!authorized(&headers, &(token.clone() + "x")));
    }

    #[test]
    fn catalog_contains_requested_infrastructure_vendors() {
        for id in ["northflank", "cloudflare", "netlify", "render", "railway", "flyio",
                   "digitalocean", "supabase", "neon", "backblaze"] {
            assert!(providers::find(id).is_some(), "missing provider {id}");
        }
    }
}
