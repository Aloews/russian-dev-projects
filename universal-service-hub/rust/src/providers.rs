use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub api_type: &'static str,
    pub credential_env: &'static [&'static str],
    pub connection_test: &'static str,
    pub docs_url: &'static str,
}

pub fn catalog() -> Vec<Provider> {
    vec![
        Provider { id: "northflank", name: "Northflank", category: "compute", api_type: "REST", credential_env: &["NORTHFLANK_API_TOKEN"], connection_test: "List projects", docs_url: "https://northflank.com/docs/v1/api/introduction" },
        Provider { id: "railway", name: "Railway", category: "compute", api_type: "GraphQL", credential_env: &["RAILWAY_API_TOKEN"], connection_test: "Read current account", docs_url: "https://docs.railway.com/integrations/api" },
        Provider { id: "flyio", name: "Fly.io", category: "compute", api_type: "REST", credential_env: &["FLY_API_TOKEN"], connection_test: "List apps", docs_url: "https://fly.io/docs/machines/api/" },
        Provider { id: "render", name: "Render", category: "compute", api_type: "REST", credential_env: &["RENDER_API_KEY"], connection_test: "List services", docs_url: "https://render.com/docs/api" },
        Provider { id: "netlify", name: "Netlify", category: "compute", api_type: "REST", credential_env: &["NETLIFY_ACCESS_TOKEN"], connection_test: "List sites", docs_url: "https://docs.netlify.com/api-and-cli-guides/api-guides/" },
        Provider { id: "digitalocean", name: "DigitalOcean", category: "compute", api_type: "REST", credential_env: &["DIGITALOCEAN_API_TOKEN"], connection_test: "Read account", docs_url: "https://docs.digitalocean.com/reference/api/" },
        Provider { id: "cloudflare", name: "Cloudflare", category: "edge_storage", api_type: "REST", credential_env: &["CLOUDFLARE_API_TOKEN", "CLOUDFLARE_ACCOUNT_ID"], connection_test: "List R2 buckets", docs_url: "https://developers.cloudflare.com/r2/api/" },
        Provider { id: "supabase", name: "Supabase", category: "database_storage", api_type: "REST", credential_env: &["SUPABASE_ACCESS_TOKEN"], connection_test: "List projects", docs_url: "https://supabase.com/docs/reference/api/introduction" },
        Provider { id: "neon", name: "Neon", category: "database", api_type: "REST", credential_env: &["NEON_API_KEY"], connection_test: "List projects", docs_url: "https://neon.tech/docs/reference/api-reference" },
        Provider { id: "backblaze", name: "Backblaze B2", category: "object_storage", api_type: "REST", credential_env: &["BACKBLAZE_APPLICATION_KEY_ID", "BACKBLAZE_APPLICATION_KEY"], connection_test: "Authorize account", docs_url: "https://www.backblaze.com/apidocs/b2-authorize-account" },
        Provider { id: "postgres", name: "PostgreSQL", category: "database", api_type: "PostgreSQL wire protocol", credential_env: &["USH_POSTGRES_URL"], connection_test: "SELECT 1", docs_url: "https://www.postgresql.org/docs/current/protocol.html" },
        Provider { id: "redis", name: "Redis", category: "queue_cache", api_type: "RESP", credential_env: &["USH_REDIS_URL"], connection_test: "PING", docs_url: "https://redis.io/docs/latest/develop/reference/protocol-spec/" },
        Provider { id: "s3", name: "S3-compatible storage", category: "object_storage", api_type: "S3 API", credential_env: &["USH_S3_ENDPOINT", "USH_S3_ACCESS_KEY_ID", "USH_S3_SECRET_ACCESS_KEY"], connection_test: "S3 signed bucket check (next adapter increment)", docs_url: "https://docs.aws.amazon.com/AmazonS3/latest/API/Welcome.html" },
    ]
}

pub fn find(id: &str) -> Option<Provider> {
    catalog().into_iter().find(|provider| provider.id == id)
}
