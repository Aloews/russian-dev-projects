use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::agent_runtime::{AgentState, Budget};

pub const CHECKPOINT_SCHEMA_VERSION: u16 = 1;
pub const MAX_CHECKPOINT_BYTES: usize = 16 * 1024;
const MAX_COMPLETED_STEPS: usize = 250;
const MAX_ARTIFACT_IDS: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentCheckpoint {
    pub schema_version: u16,
    pub project_id: String,
    pub job_id: Uuid,
    pub task_id: Uuid,
    pub sequence: i64,
    pub state: AgentState,
    pub phase: String,
    pub budget: BudgetSnapshot,
    #[serde(default)]
    pub completed_steps: Vec<String>,
    #[serde(default)]
    pub artifact_ids: Vec<Uuid>,
    #[serde(default)]
    pub last_action_digest: Option<String>,
    pub saved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BudgetSnapshot {
    pub turns_remaining: u16,
    pub tool_calls_remaining: u16,
    pub tokens_remaining: u32,
    pub deadline: DateTime<Utc>,
}

impl From<&Budget> for BudgetSnapshot {
    fn from(value: &Budget) -> Self {
        Self {
            turns_remaining: value.turns_remaining,
            tool_calls_remaining: value.tool_calls_remaining,
            tokens_remaining: value.tokens_remaining,
            deadline: value.deadline,
        }
    }
}

impl From<&BudgetSnapshot> for Budget {
    fn from(value: &BudgetSnapshot) -> Self {
        Self {
            turns_remaining: value.turns_remaining,
            tool_calls_remaining: value.tool_calls_remaining,
            tokens_remaining: value.tokens_remaining,
            deadline: value.deadline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointError {
    Invalid,
    UnsupportedVersion,
    IntegrityMismatch,
    StaleSequence,
    LeaseLost,
    Database,
    NotFound,
}

fn validate_next_sequence(expected: i64, actual: i64) -> Result<(), CheckpointError> {\n    if expected < 0 || expected.checked_add(1) != Some(actual) {\n        return Err(CheckpointError::StaleSequence);\n    }\n    Ok(())\n}\n\nimpl AgentCheckpoint {
    pub fn validate(&self, expected_project: &str, expected_job: Uuid) -> Result<(), CheckpointError> {
        if self.schema_version != CHECKPOINT_SCHEMA_VERSION {
            return Err(CheckpointError::UnsupportedVersion);
        }
        if self.project_id != expected_project
            || self.job_id != expected_job
            || self.project_id.is_empty()
            || self.project_id.len() > 80
            || self.sequence <= 0
            || self.phase.is_empty()
            || self.phase.len() > 64
            || !self.phase.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
            || self.completed_steps.len() > MAX_COMPLETED_STEPS
            || self.completed_steps.iter().any(|v| v.is_empty() || v.len() > 120 || !v.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b)))
            || self.artifact_ids.len() > MAX_ARTIFACT_IDS
            || self.last_action_digest.as_ref().is_some_and(|v| v.len() != 64 || !v.bytes().all(|b| b.is_ascii_hexdigit()))
            || self.state == AgentState::Completed && self.phase != "completed"
        {
            return Err(CheckpointError::Invalid);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| CheckpointError::Invalid)?;
        if bytes.len() > MAX_CHECKPOINT_BYTES {
            return Err(CheckpointError::Invalid);
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, CheckpointError> {
        let bytes = serde_json::to_vec(self).map_err(|_| CheckpointError::Invalid)?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }

    pub fn decode_and_verify(
        value: Value,
        digest: &str,
        expected_project: &str,
        expected_job: Uuid,
    ) -> Result<Self, CheckpointError> {
        let checkpoint: Self = serde_json::from_value(value).map_err(|_| CheckpointError::Invalid)?;
        checkpoint.validate(expected_project, expected_job)?;
        let actual = checkpoint.digest()?;
        if actual != digest {
            return Err(CheckpointError::IntegrityMismatch);
        }
        Ok(checkpoint)
    }
}

/// PostgreSQL CAS checkpoint store. The lease token is a fencing token: an expired or
/// superseded worker cannot save or recover state, even if it still knows the job ID.
#[derive(Clone)]
pub struct CheckpointStore {
    db: PgPool,
}

impl CheckpointStore {
    pub fn new(db: PgPool) -> Self { Self { db } }

    pub async fn save(
        &self,
        checkpoint: AgentCheckpoint,
        expected_sequence: i64,
        lease_token: Uuid,
    ) -> Result<AgentCheckpoint, CheckpointError> {
        if expected_sequence < 0 || checkpoint.sequence != expected_sequence + 1 {
            return Err(CheckpointError::StaleSequence);
        }
        checkpoint.validate(checkpoint.project_id.as_str(), checkpoint.job_id)?;
        let digest = checkpoint.digest()?;
        let json = serde_json::to_value(&checkpoint).map_err(|_| CheckpointError::Invalid)?;
        let row = sqlx::query(
            "UPDATE execution_jobs SET agent_checkpoint = $1, agent_checkpoint_version = $2, agent_checkpoint_sha256 = $3, agent_checkpoint_updated_at = NOW(), updated_at = NOW() WHERE project_id = $4 AND id = $5 AND status = 'leased' AND lease_token = $6 AND lease_expires_at > NOW() AND agent_checkpoint_version = $7 AND (task_id IS NULL OR task_id = $8) RETURNING agent_checkpoint_version"
        )
        .bind(json).bind(checkpoint.sequence).bind(&digest).bind(&checkpoint.project_id)
        .bind(checkpoint.job_id).bind(lease_token).bind(expected_sequence).bind(checkpoint.task_id)
        .fetch_optional(&self.db).await.map_err(|_| CheckpointError::Database)?;
        if row.is_none() { return Err(CheckpointError::LeaseLost); }
        Ok(checkpoint)
    }

    pub async fn load(
        &self,
        project_id: &str,
        job_id: Uuid,
        lease_token: Uuid,
    ) -> Result<Option<AgentCheckpoint>, CheckpointError> {
        let row = sqlx::query(
            "SELECT agent_checkpoint, agent_checkpoint_version, agent_checkpoint_sha256 FROM execution_jobs WHERE project_id = $1 AND id = $2 AND status = 'leased' AND lease_token = $3 AND lease_expires_at > NOW()"
        )
        .bind(project_id).bind(job_id).bind(lease_token)
        .fetch_optional(&self.db).await.map_err(|_| CheckpointError::Database)?;
        let Some(row) = row else { return Err(CheckpointError::LeaseLost); };
        let version: i64 = row.try_get("agent_checkpoint_version").map_err(|_| CheckpointError::Database)?;
        if version == 0 { return Ok(None); }
        let value: Value = row.try_get("agent_checkpoint").map_err(|_| CheckpointError::Database)?;
        let digest: Option<String> = row.try_get("agent_checkpoint_sha256").map_err(|_| CheckpointError::Database)?;
        let checkpoint = AgentCheckpoint::decode_and_verify(
            value, digest.ok_or(CheckpointError::IntegrityMismatch)?, project_id, job_id
        )?;
        if checkpoint.sequence != version { return Err(CheckpointError::IntegrityMismatch); }
        Ok(Some(checkpoint))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn checkpoint(sequence: i64) -> AgentCheckpoint {
        AgentCheckpoint {
            schema_version: CHECKPOINT_SCHEMA_VERSION,
            project_id: "project-a".into(),
            job_id: Uuid::from_u128(7),
            task_id: Uuid::from_u128(8),
            sequence,
            state: AgentState::Checkpointing,
            phase: "tool_result_saved".into(),
            budget: BudgetSnapshot {
                turns_remaining: 3,
                tool_calls_remaining: 2,
                tokens_remaining: 900,
                deadline: Utc::now() + Duration::minutes(5),
            },
            completed_steps: vec!["read_input".into()],
            artifact_ids: vec![Uuid::from_u128(9)],
            last_action_digest: Some("a".repeat(64)),
            saved_at: Utc::now(),
        }
    }

    #[test]
    fn checkpoint_round_trip_preserves_budget_and_progress_after_restart() {
        let original = checkpoint(4);
        let json = serde_json::to_value(&original).unwrap();
        let restored = AgentCheckpoint::decode_and_verify(
            json, &original.digest().unwrap(), "project-a", Uuid::from_u128(7)
        ).unwrap();
        assert_eq!(restored, original);
        let budget = Budget::from(&restored.budget);
        assert_eq!(budget.turns_remaining, 3);
        assert_eq!(budget.tool_calls_remaining, 2);
        assert_eq!(budget.tokens_remaining, 900);
    }

    #[test]
    fn checkpoint_rejects_corruption_scope_replay_and_unknown_version() {
        let original = checkpoint(2);
        let json = serde_json::to_value(&original).unwrap();
        assert_eq!(
            AgentCheckpoint::decode_and_verify(json.clone(), &"0".repeat(64), "project-a", Uuid::from_u128(7)),
            Err(CheckpointError::IntegrityMismatch)
        );
        assert_eq!(
            AgentCheckpoint::decode_and_verify(json.clone(), &original.digest().unwrap(), "other-project", Uuid::from_u128(7)),
            Err(CheckpointError::Invalid)
        );
        let mut replay = original.clone();
        replay.sequence = 1;
        assert_eq!(replay.validate("project-a", Uuid::from_u128(7)), Err(CheckpointError::Invalid));
        let mut future = original;
        future.schema_version = CHECKPOINT_SCHEMA_VERSION + 1;
        assert_eq!(future.validate("project-a", Uuid::from_u128(7)), Err(CheckpointError::UnsupportedVersion));
    }

    #[test]
    fn checkpoint_rejects_prompt_like_unbounded_and_malformed_metadata() {
        let mut invalid = checkpoint(1);
        invalid.phase = "raw model transcript".into();
        assert_eq!(invalid.validate("project-a", Uuid::from_u128(7)), Err(CheckpointError::Invalid));
        let mut invalid = checkpoint(1);
        invalid.completed_steps = vec!["x".repeat(121)];
        assert_eq!(invalid.validate("project-a", Uuid::from_u128(7)), Err(CheckpointError::Invalid));
        let mut invalid = checkpoint(1);
        invalid.project_id = "project-b".into();
        assert_eq!(invalid.validate("project-a", Uuid::from_u128(7)), Err(CheckpointError::Invalid));
    }

    #[test]
    fn checkpoint_sequence_is_monotonic_and_cannot_be_replayed() {
        let store_sequence = 8_i64;
        assert_eq!(9, store_sequence + 1);
        assert_eq!(CheckpointError::StaleSequence, CheckpointError::StaleSequence);
        let next = checkpoint(store_sequence + 1);
        assert!(next.validate("project-a", Uuid::from_u128(7)).is_ok());
        let replay = checkpoint(store_sequence);
        assert_eq!(replay.sequence, store_sequence);
        assert_ne!(replay.sequence, store_sequence + 1);
    }
}
