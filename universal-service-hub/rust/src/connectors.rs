use anyhow::{anyhow, bail, Context, Result};
use reqwest::{Client, Method};
use serde_json::{json, Value};
use std::env;

fn required(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing credential {name}"))
}

async fn request(client: &Client, method: Method, url: &str, token: &str) -> Result<u16> {
    let response = client.request(method, url).bearer_auth(token).send().await?;
    let status = response.status();
    if !status.is_success() {
        bail!("upstream returned HTTP {}", status.as_u16());
    }
    Ok(status.as_u16())
}

pub async fn test(client: &Client, provider: &str) -> Result<u16> {
    match provider {
        "northflank" => request(client, Method::GET, "https://api.northflank.com/v1/projects", &required("NORTHFLANK_API_TOKEN")?).await,
        "render" => request(client, Method::GET, "https://api.render.com/v1/services?limit=1", &required("RENDER_API_KEY")?).await,
        "netlify" => request(client, Method::GET, "https://api.netlify.com/api/v1/sites?per_page=1", &required("NETLIFY_ACCESS_TOKEN")?).await,
        "flyio" => request(client, Method::GET, "https://api.machines.dev/v1/apps", &required("FLY_API_TOKEN")?).await,
        "digitalocean" => request(client, Method::GET, "https://api.digitalocean.com/v2/account", &required("DIGITALOCEAN_API_TOKEN")?).await,
        "supabase" => request(client, Method::GET, "https://api.supabase.com/v1/projects", &required("SUPABASE_ACCESS_TOKEN")?).await,
        "neon" => request(client, Method::GET, "https://console.neon.tech/api/v2/projects", &required("NEON_API_KEY")?).await,
        "cloudflare" => {
            let account = required("CLOUDFLARE_ACCOUNT_ID")?;
            let token = required("CLOUDFLARE_API_TOKEN")?;
            let url = format!("https://api.cloudflare.com/client/v4/accounts/{account}/r2/buckets");
            let response = client.get(url).bearer_auth(token).send().await?;
            let status = response.status();
            let body: Value = response.json().await?;
            if !status.is_success() || body.get("success").and_then(Value::as_bool) != Some(true) {
                bail!("Cloudflare API rejected the R2 bucket query");
            }
            Ok(status.as_u16())
        }
        "railway" => {
            let token = required("RAILWAY_API_TOKEN")?;
            let response = client.post("https://backboard.railway.com/graphql/v2")
                .bearer_auth(token)
                .json(&json!({"query":"query { me { id name } }"}))
                .send().await?;
            let status = response.status();
            let body: Value = response.json().await?;
            if !status.is_success() || body.get("errors").is_some() || body.get("data").and_then(|d| d.get("me")).is_none() {
                bail!("Railway GraphQL account query failed");
            }
            Ok(status.as_u16())
        }
        "backblaze" => {
            let key_id = required("BACKBLAZE_APPLICATION_KEY_ID")?;
            let key = required("BACKBLAZE_APPLICATION_KEY")?;
            let response = client.post("https://api.backblazeb2.com/b2api/v3/b2_authorize_account")
                .basic_auth(key_id, Some(key))
                .send().await?;
            let status = response.status();
            if !status.is_success() {
                bail!("Backblaze authorization failed with HTTP {}", status.as_u16());
            }
            Ok(status.as_u16())
        }
        "postgres" => bail!("PostgreSQL wire-protocol adapter is scheduled for the database connector increment"),
        "redis" => bail!("Redis RESP adapter is scheduled for the queue connector increment"),
        "s3" => bail!("S3 signing adapter is scheduled for the object-storage connector increment"),
        _ => Err(anyhow!("unsupported provider")),
    }
}
