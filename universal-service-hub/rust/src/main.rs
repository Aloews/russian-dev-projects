use axum::{extract::State, http::StatusCode, routing::{get, post}, Json, Router};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, env, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Provider {
    id: String,
    name: String,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    priority: i32,
    #[serde(default = "enabled")]
    enabled: bool,
    #[serde(default)]
    health_url: Option<String>,
    #[serde(default)]
    api_base_url: Option<String>,
    #[serde(default)]
    verify_path: Option<String>,
    #[serde(default)]
    verify_method: Option<String>,
    #[serde(default)]
    verify_body: Option<serde_json::Value>,
    #[serde(default, skip_serializing)]
    api_token_env: Option<String>,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
}
fn enabled() -> bool { true }
fn default_timeout() -> u64 { 5 }

#[derive(Clone, Debug, Serialize)]
struct ProviderState {
    provider: Provider,
    health_status: String,
    checked_at: Option<DateTime<Utc>>,
    latency_ms: Option<u128>,
    detail: Option<String>,
}
#[derive(Clone)]
struct AppState { providers: Arc<RwLock<Vec<ProviderState>>>, client: Client }

#[derive(Deserialize)]
struct SelectRequest { capability: String, #[serde(default)] exclude: Vec<String> }
#[derive(Serialize)]
struct SelectResponse { capability: String, provider: Option<ProviderState>, reason: String }
#[derive(Deserialize)]
struct ProbeRequest { provider_id: Option<String> }

fn initial_state() -> Vec<ProviderState> {
    let providers = env::var("USH_PROVIDERS_JSON").ok()
        .and_then(|v| serde_json::from_str::<Vec<Provider>>(&v).ok()).unwrap_or_default();
    providers.into_iter().map(|provider| ProviderState {
        provider, health_status: "unknown".into(), checked_at: None, latency_ms: None, detail: None
    }).collect()
}
fn choose(states: &[ProviderState], capability: &str, exclude: &[String]) -> Option<ProviderState> {
    let excluded: HashSet<&str> = exclude.iter().map(String::as_str).collect();
    states.iter()
        .filter(|s| s.provider.enabled && s.provider.capabilities.iter().any(|c| c == capability))
        .filter(|s| !excluded.contains(s.provider.id.as_str()))
        .filter(|s| s.health_status != "unhealthy")
        .max_by_key(|s| s.provider.priority).cloned()
}
async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","timestamp":Utc::now()}))
}
async fn readyz(State(state): State<AppState>) -> Json<serde_json::Value> {
    let count = state.providers.read().await.len();
    Json(serde_json::json!({"status":"ready","providers_configured":count}))
}
async fn list_providers(State(state): State<AppState>) -> Json<Vec<ProviderState>> {
    Json(state.providers.read().await.clone())
}
async fn select_provider(State(state): State<AppState>, Json(req): Json<SelectRequest>) -> Json<SelectResponse> {
    let states = state.providers.read().await;
    let provider = choose(&states, &req.capability, &req.exclude);
    Json(SelectResponse {
        capability: req.capability,
        reason: if provider.is_some() { "highest-priority-eligible-provider".into() } else { "no-eligible-provider".into() },
        provider
    })
}
async fn probe_one(state: &AppState, index: usize) {
    let snapshot = { state.providers.read().await.get(index).cloned() };
    let Some(snapshot) = snapshot else { return };
    let url = snapshot.provider.health_url.clone().or_else(|| {
        let base = snapshot.provider.api_base_url.as_ref()?;
        let path = snapshot.provider.verify_path.as_deref().unwrap_or("/");
        reqwest::Url::parse(base).ok()?.join(path).ok().map(|u| u.to_string())
    });
    let Some(url) = url else { return };
    let started = std::time::Instant::now();
    let method = snapshot.provider.verify_method.as_deref().unwrap_or("GET")
        .parse::<reqwest::Method>().unwrap_or(reqwest::Method::GET);
    let mut request = state.client.request(method, url);
    if let Some(body) = &snapshot.provider.verify_body {
        request = request.json(body);
    }
    if let Some(token_env) = &snapshot.provider.api_token_env {
        match env::var(token_env) {
            Ok(token) if !token.trim().is_empty() => { request = request.bearer_auth(token); }
            _ => {
                let mut providers = state.providers.write().await;
                if let Some(item) = providers.get_mut(index) {
                    item.health_status = "unhealthy".into();
                    item.checked_at = Some(Utc::now());
                    item.latency_ms = Some(started.elapsed().as_millis());
                    item.detail = Some("credential environment variable is not configured".into());
                }
                return;
            }
        }
    }
    let result = tokio::time::timeout(
        Duration::from_secs(snapshot.provider.timeout_seconds.clamp(1, 30)),
        request.send()
    ).await;
    let (status, detail) = match result {
        Ok(Ok(response)) if response.status().is_success() => ("healthy".to_string(), None),
        Ok(Ok(response)) => ("unhealthy".to_string(), Some(format!("HTTP {}", response.status()))),
        Ok(Err(error)) => ("unhealthy".to_string(), Some(error.to_string())),
        Err(_) => ("unhealthy".to_string(), Some("health check timed out".into())),
    };
    let mut providers = state.providers.write().await;
    if let Some(item) = providers.get_mut(index) {
        item.health_status = status;
        item.checked_at = Some(Utc::now());
        item.latency_ms = Some(started.elapsed().as_millis());
        item.detail = detail;
    }
}
async fn health_check(State(state): State<AppState>, Json(req): Json<ProbeRequest>)
    -> Result<Json<Vec<ProviderState>>, (StatusCode, String)> {
    let indices: Vec<usize> = {
        let providers = state.providers.read().await;
        if let Some(id) = req.provider_id {
            let Some(index) = providers.iter().position(|p| p.provider.id == id) else {
                return Err((StatusCode::NOT_FOUND, "provider not found".into()));
            };
            vec![index]
        } else { (0..providers.len()).collect() }
    };
    for index in indices { probe_one(&state, index).await; }
    Ok(Json(state.providers.read().await.clone()))
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(
        env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=info".into())
    ).init();
    let state = AppState {
        providers: Arc::new(RwLock::new(initial_state())),
        client: Client::builder().connect_timeout(Duration::from_secs(3)).build().expect("HTTP client")
    };
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/providers", get(list_providers))
        .route("/v1/providers/health-check", post(health_check))
        .route("/v1/providers/select", post(select_provider))
        .layer(TraceLayer::new_for_http()).with_state(state);
    let addr = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("{addr}:{port}")).await.expect("bind");
    axum::serve(listener, app).await.expect("server");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(id: &str, priority: i32, health: &str) -> ProviderState {
        ProviderState {
            provider: Provider { id:id.into(), name:id.into(), capabilities:vec!["compute".into()],
                priority, enabled:true, health_url:None, api_base_url:None, verify_path:None, verify_method:None, verify_body:None, api_token_env:None, timeout_seconds:5 },
            health_status:health.into(), checked_at:None, latency_ms:None, detail:None
        }
    }
    #[test]
    fn selects_highest_priority_healthy_or_unknown() {
        let providers = vec![state("low", 1, "healthy"), state("high", 10, "unknown")];
        assert_eq!(choose(&providers, "compute", &[]).unwrap().provider.id, "high");
    }
    #[test]
    fn excludes_unhealthy_and_disabled() {
        let mut disabled = state("disabled", 100, "healthy"); disabled.provider.enabled = false;
        let providers = vec![state("down", 90, "unhealthy"), disabled, state("backup", 5, "healthy")];
        assert_eq!(choose(&providers, "compute", &[]).unwrap().provider.id, "backup");
    }
    #[test]
    fn honors_exclusions_and_capability() {
        let providers = vec![state("one", 10, "healthy"), state("two", 5, "healthy")];
        assert_eq!(choose(&providers, "compute", &["one".into()]).unwrap().provider.id, "two");
        assert!(choose(&providers, "database", &[]).is_none());
    }
}
