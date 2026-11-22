use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolReceiptIdentity {
    pub project_id: String,
    pub job_id: Uuid,
    pub task_id: Uuid,
    pub action_id: Uuid,
    pub tool_id: String,
    pub tool_version: u16,
    pub action_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolReceiptReservation {
    Reserved,
    Completed { artifact_id: Uuid, sha256: String },
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolReceiptError {
    Invalid,
    Database,
    LeaseLost,
    Conflict,
    IntegrityMismatch,
}

fn valid_component(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub fn derive_tool_idempotency_key(
    project_id: &str,
    job_id: Uuid,
    task_id: Uuid,
    action_id: Uuid,
    tool_id: &str,
    tool_version: u16,
    action_sha256: &str,
) -> Result<String, ToolReceiptError> {
    if !valid_component(project_id, 80)
        || !valid_component(tool_id, 96)
        || job_id == Uuid::nil()
        || task_id == Uuid::nil()
        || action_id == Uuid::nil()
        || tool_version == 0
        || !valid_sha256(action_sha256)
    {
        return Err(ToolReceiptError::Invalid);
    }
    let material = format!(
        "ush-tool-v1|{project_id}|{job_id}|{task_id}|{action_id}|{tool_id}|{tool_version}|{action_sha256}"
    );
    Ok(format!(
        "ush-tool-v1-{}",
        hex::encode(Sha256::digest(material.as_bytes()))
    ))
}

impl ToolReceiptIdentity {
    pub fn new(
        project_id: String,
        job_id: Uuid,
        task_id: Uuid,
        action_id: Uuid,
        tool_id: String,
        tool_version: u16,
        action_sha256: String,
    ) -> Result<Self, ToolReceiptError> {
        let idempotency_key = derive_tool_idempotency_key(
            &project_id,
            job_id,
            task_id,
            action_id,
            &tool_id,
            tool_version,
            &action_sha256,
        )?;
        Ok(Self {
            project_id,
            job_id,
            task_id,
            action_id,
            tool_id,
            tool_version,
            action_sha256,
            idempotency_key,
        })
    }

    fn validate(&self) -> Result<(), ToolReceiptError> {
        let expected = derive_tool_idempotency_key(
            &self.project_id,
            self.job_id,
            self.task_id,
            self.action_id,
            &self.tool_id,
            self.tool_version,
            &self.action_sha256,
        )?;
        if expected != self.idempotency_key {
            return Err(ToolReceiptError::Invalid);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ToolReceiptStore {
    db: PgPool,
}

impl ToolReceiptStore {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Existing reservations without a completed result are never replayed automatically.
    pub async fn reserve(
        &self,
        identity: &ToolReceiptIdentity,
        lease_token: Uuid,
    ) -> Result<ToolReceiptReservation, ToolReceiptError> {
        identity.validate()?;
        if lease_token == Uuid::nil() {
            return Err(ToolReceiptError::Invalid);
        }
        let mut tx = self.db.begin().await.map_err(|_| ToolReceiptError::Database)?;
        let lease = sqlx::query(
            "SELECT 1 FROM execution_jobs WHERE project_id=$1 AND id=$2 AND task_id=$3 AND status='leased' AND lease_token=$4 AND lease_expires_at>NOW() FOR UPDATE"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(lease_token)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if lease.is_none() {
            return Err(ToolReceiptError::LeaseLost);
        }
        let inserted = sqlx::query(
            "INSERT INTO execution_tool_receipts (project_id,job_id,task_id,action_id,tool_id,tool_version,action_sha256,idempotency_key,status) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'reserved') ON CONFLICT DO NOTHING RETURNING id"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(identity.action_id)
        .bind(&identity.tool_id)
        .bind(i32::from(identity.tool_version))
        .bind(&identity.action_sha256)
        .bind(&identity.idempotency_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if inserted.is_some() {
            tx.commit().await.map_err(|_| ToolReceiptError::Database)?;
            return Ok(ToolReceiptReservation::Reserved);
        }
        let row = sqlx::query(
            "SELECT task_id,action_id,tool_id,tool_version,action_sha256,idempotency_key,status,result_artifact_id,result_sha256 FROM execution_tool_receipts WHERE project_id=$1 AND job_id=$2 AND ((task_id=$3 AND action_id=$4) OR idempotency_key=$5) ORDER BY CASE WHEN action_id=$4 THEN 0 ELSE 1 END LIMIT 1 FOR UPDATE"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(identity.action_id)
        .bind(&identity.idempotency_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        let Some(row) = row else {
            return Err(ToolReceiptError::Conflict);
        };
        let stored_task: Uuid = row.try_get("task_id").map_err(|_| ToolReceiptError::Database)?;
        let stored_action: Uuid = row.try_get("action_id").map_err(|_| ToolReceiptError::Database)?;
        let stored_tool: String = row.try_get("tool_id").map_err(|_| ToolReceiptError::Database)?;
        let stored_version: i32 = row.try_get("tool_version").map_err(|_| ToolReceiptError::Database)?;
        let stored_digest: String = row.try_get("action_sha256").map_err(|_| ToolReceiptError::Database)?;
        let stored_key: String = row.try_get("idempotency_key").map_err(|_| ToolReceiptError::Database)?;
        let status: String = row.try_get("status").map_err(|_| ToolReceiptError::Database)?;
        if stored_task != identity.task_id
            || stored_action != identity.action_id
            || stored_tool != identity.tool_id
            || stored_version != i32::from(identity.tool_version)
            || stored_digest != identity.action_sha256
            || stored_key != identity.idempotency_key
        {
            return Err(ToolReceiptError::Conflict);
        }
        let outcome = match status.as_str() {
            "completed" => {
                let artifact_id: Option<Uuid> =
                    row.try_get("result_artifact_id").map_err(|_| ToolReceiptError::Database)?;
                let sha256: Option<String> =
                    row.try_get("result_sha256").map_err(|_| ToolReceiptError::Database)?;
                ToolReceiptReservation::Completed {
                    artifact_id: artifact_id.ok_or(ToolReceiptError::IntegrityMismatch)?,
                    sha256: sha256
                        .filter(|value| valid_sha256(value))
                        .ok_or(ToolReceiptError::IntegrityMismatch)?,
                }
            }
            "reserved" | "uncertain" => ToolReceiptReservation::Uncertain,
            _ => return Err(ToolReceiptError::IntegrityMismatch),
        };
        tx.commit().await.map_err(|_| ToolReceiptError::Database)?;
        Ok(outcome)
    }

    pub async fn complete(
        &self,
        identity: &ToolReceiptIdentity,
        lease_token: Uuid,
        artifact_id: Uuid,
        result_sha256: &str,
    ) -> Result<(), ToolReceiptError> {
        identity.validate()?;
        if lease_token == Uuid::nil()
            || artifact_id == Uuid::nil()
            || !valid_sha256(result_sha256)
        {
            return Err(ToolReceiptError::Invalid);
        }
        let mut tx = self.db.begin().await.map_err(|_| ToolReceiptError::Database)?;
        let lease = sqlx::query(
            "SELECT 1 FROM execution_jobs WHERE project_id=$1 AND id=$2 AND task_id=$3 AND status='leased' AND lease_token=$4 AND lease_expires_at>NOW() FOR UPDATE"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(lease_token)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if lease.is_none() {
            return Err(ToolReceiptError::LeaseLost);
        }
        let registered_artifact = sqlx::query(
            "SELECT 1 FROM execution_artifacts WHERE project_id=$1 AND job_id=$2 AND task_id=$3 AND id=$4 AND content_sha256=$5"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(artifact_id)
        .bind(result_sha256)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if registered_artifact.is_none() {
            return Err(ToolReceiptError::IntegrityMismatch);
        }
        let updated = sqlx::query(
            "UPDATE execution_tool_receipts SET status='completed',result_artifact_id=$9,result_sha256=$10,updated_at=NOW() WHERE project_id=$1 AND job_id=$2 AND task_id=$3 AND action_id=$4 AND tool_id=$5 AND tool_version=$6 AND action_sha256=$7 AND idempotency_key=$8 AND status='reserved'"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(identity.action_id)
        .bind(&identity.tool_id)
        .bind(i32::from(identity.tool_version))
        .bind(&identity.action_sha256)
        .bind(&identity.idempotency_key)
        .bind(artifact_id)
        .bind(result_sha256)
        .execute(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if updated.rows_affected() == 1 {
            tx.commit().await.map_err(|_| ToolReceiptError::Database)?;
            return Ok(());
        }
        let existing = sqlx::query(
            "SELECT status,result_artifact_id,result_sha256 FROM execution_tool_receipts WHERE project_id=$1 AND job_id=$2 AND task_id=$3 AND action_id=$4 FOR UPDATE"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(identity.action_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if let Some(row) = existing {
            let status: String = row.try_get("status").map_err(|_| ToolReceiptError::Database)?;
            let artifact: Option<Uuid> =
                row.try_get("result_artifact_id").map_err(|_| ToolReceiptError::Database)?;
            let sha: Option<String> =
                row.try_get("result_sha256").map_err(|_| ToolReceiptError::Database)?;
            if status == "completed"
                && artifact == Some(artifact_id)
                && sha.as_deref() == Some(result_sha256)
            {
                tx.commit().await.map_err(|_| ToolReceiptError::Database)?;
                return Ok(());
            }
        }
        Err(ToolReceiptError::Conflict)
    }

    /// Fail closed after an executor error: its side-effect status may be unknown.
    pub async fn mark_uncertain(
        &self,
        identity: &ToolReceiptIdentity,
        lease_token: Uuid,
    ) -> Result<(), ToolReceiptError> {
        identity.validate()?;
        if lease_token == Uuid::nil() {
            return Err(ToolReceiptError::Invalid);
        }
        let mut tx = self.db.begin().await.map_err(|_| ToolReceiptError::Database)?;
        let lease = sqlx::query(
            "SELECT 1 FROM execution_jobs WHERE project_id=$1 AND id=$2 AND task_id=$3 AND status='leased' AND lease_token=$4 AND lease_expires_at>NOW() FOR UPDATE"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(lease_token)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if lease.is_none() {
            return Err(ToolReceiptError::LeaseLost);
        }
        let updated = sqlx::query(
            "UPDATE execution_tool_receipts SET status='uncertain',updated_at=NOW() WHERE project_id=$1 AND job_id=$2 AND task_id=$3 AND action_id=$4 AND tool_id=$5 AND tool_version=$6 AND action_sha256=$7 AND idempotency_key=$8 AND status='reserved'"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(identity.action_id)
        .bind(&identity.tool_id)
        .bind(i32::from(identity.tool_version))
        .bind(&identity.action_sha256)
        .bind(&identity.idempotency_key)
        .execute(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if updated.rows_affected() == 1 {
            tx.commit().await.map_err(|_| ToolReceiptError::Database)?;
            return Ok(());
        }
        let existing = sqlx::query(
            "SELECT status FROM execution_tool_receipts WHERE project_id=$1 AND job_id=$2 AND task_id=$3 AND action_id=$4 FOR UPDATE"
        )
        .bind(&identity.project_id)
        .bind(identity.job_id)
        .bind(identity.task_id)
        .bind(identity.action_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ToolReceiptError::Database)?;
        if let Some(row) = existing {
            let status: String = row.try_get("status").map_err(|_| ToolReceiptError::Database)?;
            if status == "uncertain" {
                tx.commit().await.map_err(|_| ToolReceiptError::Database)?;
                return Ok(());
            }
        }
        Err(ToolReceiptError::Conflict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(project: &str, job: Uuid, task: Uuid, action: Uuid) -> ToolReceiptIdentity {
        ToolReceiptIdentity::new(
            project.to_owned(),
            job,
            task,
            action,
            "artifact.read".to_owned(),
            1,
            "a".repeat(64),
        )
        .unwrap()
    }

    #[test]
    fn deterministic_key_is_bound_to_scope_action_and_digest() {
        let job = Uuid::from_u128(10);
        let task = Uuid::from_u128(11);
        let action = Uuid::from_u128(12);
        let key = derive_tool_idempotency_key(
            "project-a", job, task, action, "artifact.read", 1, &"a".repeat(64),
        )
        .unwrap();
        assert_eq!(
            key,
            derive_tool_idempotency_key(
                "project-a", job, task, action, "artifact.read", 1, &"a".repeat(64),
            )
            .unwrap()
        );
        assert_ne!(
            key,
            derive_tool_idempotency_key(
                "project-b", job, task, action, "artifact.read", 1, &"a".repeat(64),
            )
            .unwrap()
        );
        assert_ne!(
            key,
            derive_tool_idempotency_key(
                "project-a", job, Uuid::from_u128(13), action, "artifact.read", 1, &"a".repeat(64),
            )
            .unwrap()
        );
        assert_ne!(
            key,
            derive_tool_idempotency_key(
                "project-a", job, task, action, "artifact.read", 1, &"b".repeat(64),
            )
            .unwrap()
        );
        assert!(derive_tool_idempotency_key(
            "../unsafe", job, task, action, "artifact.read", 1, &"a".repeat(64),
        ).is_err());
        assert!(derive_tool_idempotency_key(
            "project-a", job, task, Uuid::nil(), "artifact.read", 1, &"a".repeat(64),
        ).is_err());
    }

    #[tokio::test]
    async fn postgres_receipts_reserve_complete_uncertain_and_fence_stale_leases() {
        let Ok(database_url) = std::env::var("USH_CHECKPOINT_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPool::connect(&database_url).await.unwrap();
        sqlx::migrate!("./rust-migrations").run(&pool).await.unwrap();
        let project = format!("receipt-{}", Uuid::new_v4().simple());
        let job = Uuid::new_v4();
        let task = Uuid::new_v4();
        let lease = Uuid::new_v4();
        sqlx::query("INSERT INTO projects (id,name) VALUES ($1,'receipt test')")
            .bind(&project).execute(&pool).await.unwrap();
        let plan = Uuid::new_v4();
        sqlx::query("INSERT INTO project_plans (id,project_id,version,state,created_by) VALUES ($1,$2,1,'active','receipt-test')")
            .bind(plan).bind(&project).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO project_tasks (id,project_id,plan_id,task_key,title) VALUES ($1,$2,$3,'receipt-task','Tool receipt integration')")
            .bind(task).bind(&project).bind(plan).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO execution_jobs (id,project_id,task_id,job_type,status,idempotency_key,lease_owner,lease_token,lease_expires_at) VALUES ($1,$2,$3,'agent.receipt.test','leased',$4,'receipt-test',$5,NOW()+INTERVAL '5 minutes')")
            .bind(job).bind(&project).bind(task).bind(format!("receipt-{job}")).bind(lease)
            .execute(&pool).await.unwrap();

        let store = ToolReceiptStore::new(pool.clone());
        let first = identity(&project, job, task, Uuid::new_v4());
        let (a, b) = tokio::join!(store.reserve(&first, lease), store.reserve(&first, lease));
        let outcomes = [a.unwrap(), b.unwrap()];
        assert_eq!(
            outcomes.iter().filter(|o| *o == &ToolReceiptReservation::Reserved).count(),
            1
        );
        assert_eq!(
            outcomes.iter().filter(|o| *o == &ToolReceiptReservation::Uncertain).count(),
            1
        );
        let artifact = Uuid::new_v4();
        let result_sha = "b".repeat(64);
        let missing_artifact = Uuid::new_v4();
        assert_eq!(
            store.complete(&first, lease, missing_artifact, &result_sha).await,
            Err(ToolReceiptError::IntegrityMismatch)
        );
        sqlx::query(
            "INSERT INTO execution_artifacts (id,project_id,job_id,task_id,artifact_type,storage_backend,storage_key,content_sha256,size_bytes,content_type) VALUES ($1,$2,$3,$4,'agent_tool_output','test','receipt-test-object',$5,1,'application/json')"
        )
        .bind(artifact)
        .bind(&project)
        .bind(job)
        .bind(task)
        .bind(&result_sha)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            store.complete(&first, lease, artifact, &"c".repeat(64)).await,
            Err(ToolReceiptError::IntegrityMismatch)
        );
        store.complete(&first, lease, artifact, &result_sha).await.unwrap();
        assert_eq!(
            store.reserve(&first, lease).await.unwrap(),
            ToolReceiptReservation::Completed {
                artifact_id: artifact,
                sha256: result_sha.clone(),
            }
        );
        let changed = ToolReceiptIdentity::new(
            project.clone(), job, task, first.action_id, first.tool_id.clone(),
            first.tool_version, "c".repeat(64),
        )
        .unwrap();
        assert_eq!(
            store.reserve(&changed, lease).await,
            Err(ToolReceiptError::Conflict)
        );

        let uncertain = identity(&project, job, task, Uuid::new_v4());
        assert_eq!(
            store.reserve(&uncertain, lease).await.unwrap(),
            ToolReceiptReservation::Reserved
        );
        store.mark_uncertain(&uncertain, lease).await.unwrap();
        assert_eq!(
            store.reserve(&uncertain, lease).await.unwrap(),
            ToolReceiptReservation::Uncertain
        );

        let stale = identity(&project, job, task, Uuid::new_v4());
        let new_lease = Uuid::new_v4();
        sqlx::query("UPDATE execution_jobs SET lease_token=$1 WHERE project_id=$2 AND id=$3")
            .bind(new_lease).bind(&project).bind(job).execute(&pool).await.unwrap();
        assert_eq!(
            store.reserve(&stale, lease).await,
            Err(ToolReceiptError::LeaseLost)
        );
        sqlx::query("DELETE FROM projects WHERE id=$1").bind(&project).execute(&pool).await.unwrap();
        pool.close().await;
    }
}
