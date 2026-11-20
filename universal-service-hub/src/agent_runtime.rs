use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const CONTRACT_VERSION: u16 = 1;
pub const MAX_TASK_BYTES: usize = 32 * 1024;
pub const MAX_ACTION_BYTES: usize = 16 * 1024;
pub const MAX_TOOL_INPUT_BYTES: usize = 64 * 1024;
pub const MAX_TOOL_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Coordinator,
    Planner,
    Researcher,
    Coder,
    Reviewer,
    Qa,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTask {
    pub schema_version: u16,
    pub task_id: Uuid,
    pub tenant_id: String,
    pub project_id: String,
    pub job_id: Uuid,
    pub agent_role: AgentRole,
    pub objective_ref: String,
    #[serde(default)]
    pub input_artifact_ids: Vec<Uuid>,
    #[serde(default)]
    pub allowed_tool_ids: Vec<String>,
    pub model_policy_id: String,
    pub turn_budget: u16,
    pub tool_call_budget: u16,
    pub token_budget: u32,
    pub deadline: DateTime<Utc>,
    pub idempotency_key: String,
    pub acceptance_criteria: Vec<String>,
}

impl AgentTask {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), ContractError> {
        if self.schema_version != CONTRACT_VERSION
            || self.tenant_id.is_empty()
            || self.tenant_id.len() > 80
            || self.project_id.is_empty()
            || self.project_id.len() > 80
            || self.objective_ref.is_empty()
            || self.objective_ref.len() > 256
            || self.model_policy_id.is_empty()
            || self.model_policy_id.len() > 120
            || self.idempotency_key.is_empty()
            || self.idempotency_key.len() > 160
            || self.turn_budget == 0
            || self.turn_budget > 100
            || self.tool_call_budget > 500
            || self.token_budget == 0
            || self.token_budget > 2_000_000
            || self.deadline <= now
            || self.acceptance_criteria.is_empty()
            || self.acceptance_criteria.len() > 32
            || self.allowed_tool_ids.len() > 64
            || self.input_artifact_ids.len() > 128
            || self.allowed_tool_ids.iter().any(|id| !valid_identifier(id, 96))
            || self.acceptance_criteria.iter().any(|v| v.is_empty() || v.len() > 1000)
            || serde_json::to_vec(self).map_err(|_| ContractError::InvalidTask)?.len() > MAX_TASK_BYTES
        {
            return Err(ContractError::InvalidTask);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    InvokeTool,
    Finish,
    Delegate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionEnvelope {
    pub schema_version: u16,
    pub action_id: Uuid,
    pub kind: ActionKind,
    #[serde(default)]
    pub tool_id: Option<String>,
    #[serde(default)]
    pub tool_version: Option<u16>,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub reason_code: Option<String>,
}

impl ActionEnvelope {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != CONTRACT_VERSION
            || serde_json::to_vec(self).map_err(|_| ContractError::InvalidAction)?.len() > MAX_ACTION_BYTES
            || self.reason_code.as_ref().is_some_and(|v| v.len() > 120)
        {
            return Err(ContractError::InvalidAction);
        }
        match self.kind {
            ActionKind::InvokeTool => {
                if self.tool_id.as_deref().is_none_or(|id| !valid_identifier(id, 96))
                    || self.tool_version.is_none_or(|version| version == 0)
                    || !self.arguments.is_object()
                {
                    return Err(ContractError::InvalidAction);
                }
            }
            ActionKind::Finish | ActionKind::Delegate => {
                if self.tool_id.is_some() || self.tool_version.is_some() {
                    return Err(ContractError::InvalidAction);
                }
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, ContractError> {
        let canonical = serde_json::to_vec(self).map_err(|_| ContractError::InvalidAction)?;
        Ok(hex::encode(Sha256::digest(canonical)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    String,
    Boolean,
    Number,
    Object,
    Array,
    Null,
}

impl ValueKind {
    fn matches(&self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Boolean => value.is_boolean(),
            Self::Number => value.is_number(),
            Self::Object => value.is_object(),
            Self::Array => value.is_array(),
            Self::Null => value.is_null(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolInputContract {
    pub required: BTreeMap<String, ValueKind>,
    #[serde(default)]
    pub optional: BTreeMap<String, ValueKind>,
}

impl ToolInputContract {
    pub fn validate(&self, value: &Value) -> Result<(), ContractError> {
        let object = value.as_object().ok_or(ContractError::InvalidToolInput)?;
        if serde_json::to_vec(value).map_err(|_| ContractError::InvalidToolInput)?.len() > MAX_TOOL_INPUT_BYTES {
            return Err(ContractError::InvalidToolInput);
        }
        for (key, kind) in &self.required {
            if !object.get(key).is_some_and(|v| kind.matches(v)) {
                return Err(ContractError::InvalidToolInput);
            }
        }
        for (key, value) in object {
            let kind = self.required.get(key).or_else(|| self.optional.get(key))
                .ok_or(ContractError::InvalidToolInput)?;
            if !kind.matches(value) {
                return Err(ContractError::InvalidToolInput);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ProjectRead,
    ArtifactRead,
    ArtifactWrite,
    GitRead,
    GitBranchWrite,
    TestRun,
    ProviderInfer,
    ExternalPublish,
    DataDelete,
    ProductionDeploy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    ReadOnly,
    ReversibleMutation,
    ExternalSideEffect,
    Destructive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDefinition {
    pub tool_id: String,
    pub version: u16,
    pub capability: Capability,
    pub risk: RiskClass,
    pub input: ToolInputContract,
    pub timeout_ms: u32,
    pub max_output_bytes: usize,
    pub idempotent: bool,
}

impl ToolDefinition {
    pub fn validate_definition(&self) -> Result<(), ContractError> {
        if !valid_identifier(&self.tool_id, 96)
            || self.version == 0
            || self.timeout_ms == 0
            || self.timeout_ms > 120_000
            || self.max_output_bytes == 0
            || self.max_output_bytes > MAX_TOOL_OUTPUT_BYTES
            || self.input.required.keys().any(|k| !valid_identifier(k, 64))
            || self.input.optional.keys().any(|k| !valid_identifier(k, 64) || self.input.required.contains_key(k))
        {
            return Err(ContractError::InvalidToolDefinition);
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, ToolDefinition>,
}

impl ToolRegistry {
    pub fn register(&mut self, definition: ToolDefinition) -> Result<(), ContractError> {
        definition.validate_definition()?;
        let id = definition.tool_id.clone();
        if self.tools.contains_key(&id) {
            return Err(ContractError::DuplicateTool);
        }
        self.tools.insert(id, definition);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ToolDefinition> {
        self.tools.get(id)
    }
}

#[derive(Debug, Clone)]
pub struct PolicyContext<'a> {
    pub tenant_id: &'a str,
    pub project_id: &'a str,
    pub task_id: Uuid,
    pub actor_id: &'a str,
    pub allowed_tool_ids: &'a BTreeSet<String>,
    pub granted_capabilities: &'a BTreeSet<Capability>,
}

#[derive(Debug, Clone)]
pub struct Approval {
    pub action_digest: String,
    pub target: String,
    pub approved_by: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny(&'static str),
    RequiresApproval,
}

pub fn authorize_tool(
    registry: &ToolRegistry,
    action: &ActionEnvelope,
    context: &PolicyContext<'_>,
    task_tenant_id: &str,
    task_project_id: &str,
    task_id: Uuid,
    approval: Option<&Approval>,
    now: DateTime<Utc>,
) -> PolicyDecision {
    if action.validate().is_err() || !matches!(action.kind, ActionKind::InvokeTool) {
        return PolicyDecision::Deny("invalid_action");
    }
    if context.tenant_id != task_tenant_id
        || context.project_id != task_project_id
        || context.task_id != task_id
        || context.actor_id.is_empty()
    {
        return PolicyDecision::Deny("scope_mismatch");
    }
    let Some(tool_id) = action.tool_id.as_deref() else {
        return PolicyDecision::Deny("tool_missing");
    };
    if !context.allowed_tool_ids.contains(tool_id) {
        return PolicyDecision::Deny("tool_not_in_task_allowlist");
    }
    let Some(tool) = registry.get(tool_id) else {
        return PolicyDecision::Deny("tool_not_registered");
    };
    if action.tool_version != Some(tool.version) {
        return PolicyDecision::Deny("tool_version_mismatch");
    }
    if !context.granted_capabilities.contains(&tool.capability) {
        return PolicyDecision::Deny("capability_not_granted");
    }
    if tool.input.validate(&action.arguments).is_err() {
        return PolicyDecision::Deny("tool_input_invalid");
    }
    match tool.risk {
        RiskClass::ReadOnly => PolicyDecision::Allow,
        RiskClass::ReversibleMutation => {
            if tool.idempotent { PolicyDecision::Allow } else { PolicyDecision::RequiresApproval }
        }
        RiskClass::ExternalSideEffect | RiskClass::Destructive => {
            let Ok(digest) = action.digest() else { return PolicyDecision::Deny("action_digest_failed"); };
            match approval {
                Some(approval)
                    if approval.action_digest == digest
                        && approval.target == tool_id
                        && approval.expires_at > now
                        && approval.approved_by != context.actor_id =>
                {
                    PolicyDecision::Allow
                }
                _ => PolicyDecision::RequiresApproval,
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub turns_remaining: u16,
    pub tool_calls_remaining: u16,
    pub tokens_remaining: u32,
    pub deadline: DateTime<Utc>,
}

impl Budget {
    pub fn consume_turn(&mut self, now: DateTime<Utc>) -> Result<(), ContractError> {
        if now >= self.deadline || self.turns_remaining == 0 {
            return Err(ContractError::BudgetExhausted);
        }
        self.turns_remaining -= 1;
        Ok(())
    }

    pub fn consume_tool_call(&mut self, now: DateTime<Utc>) -> Result<(), ContractError> {
        if now >= self.deadline || self.tool_calls_remaining == 0 {
            return Err(ContractError::BudgetExhausted);
        }
        self.tool_calls_remaining -= 1;
        Ok(())
    }

    pub fn consume_tokens(&mut self, tokens: u32, now: DateTime<Utc>) -> Result<(), ContractError> {
        if now >= self.deadline || tokens > self.tokens_remaining {
            return Err(ContractError::BudgetExhausted);
        }
        self.tokens_remaining -= tokens;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Queued,
    Leased,
    Planning,
    AwaitingModel,
    ValidatingAction,
    Authorizing,
    ExecutingTool,
    Checkpointing,
    Delegating,
    Completed,
    RetryWait,
    DeadLetter,
    Cancelled,
}

impl AgentState {
    pub fn can_transition_to(self, next: Self) -> bool {
        use AgentState::*;
        matches!(
            (self, next),
            (Queued, Leased)
                | (Leased, Planning)
                | (Planning, AwaitingModel)
                | (AwaitingModel, ValidatingAction)
                | (ValidatingAction, Authorizing)
                | (Authorizing, ExecutingTool)
                | (Authorizing, Completed)
                | (Authorizing, Delegating)
                | (ExecutingTool, Checkpointing)
                | (Checkpointing, Planning)
                | (Checkpointing, Completed)
                | (Checkpointing, Delegating)
                | (Leased, RetryWait)
                | (Planning, RetryWait)
                | (AwaitingModel, RetryWait)
                | (ValidatingAction, RetryWait)
                | (Authorizing, RetryWait)
                | (ExecutingTool, RetryWait)
                | (Checkpointing, RetryWait)
                | (RetryWait, Leased)
                | (Queued, Cancelled)
                | (Leased, Cancelled)
                | (Planning, Cancelled)
                | (AwaitingModel, Cancelled)
                | (ValidatingAction, Cancelled)
                | (Authorizing, Cancelled)
                | (ExecutingTool, Cancelled)
                | (Checkpointing, Cancelled)
                | (RetryWait, Cancelled)
                | (RetryWait, DeadLetter)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    InvalidTask,
    InvalidAction,
    InvalidToolInput,
    InvalidToolDefinition,
    DuplicateTool,
    BudgetExhausted,
}

fn valid_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;

    fn sample_tool(risk: RiskClass) -> ToolDefinition {
        ToolDefinition {
            tool_id: "artifact.read".into(),
            version: 1,
            capability: Capability::ArtifactRead,
            risk,
            input: ToolInputContract {
                required: BTreeMap::from([("artifact_id".into(), ValueKind::String)]),
                optional: BTreeMap::new(),
            },
            timeout_ms: 2_000,
            max_output_bytes: 16_384,
            idempotent: true,
        }
    }

    fn sample_action() -> ActionEnvelope {
        ActionEnvelope {
            schema_version: CONTRACT_VERSION,
            action_id: Uuid::nil(),
            kind: ActionKind::InvokeTool,
            tool_id: Some("artifact.read".into()),
            tool_version: Some(1),
            arguments: json!({"artifact_id":"a-123"}),
            reason_code: Some("read_task_input".into()),
        }
    }

    #[test]
    fn tool_registry_and_default_deny_authorization() {
        let mut registry = ToolRegistry::default();
        registry.register(sample_tool(RiskClass::ReadOnly)).unwrap();
        assert_eq!(registry.register(sample_tool(RiskClass::ReadOnly)), Err(ContractError::DuplicateTool));

        let allowed = BTreeSet::from(["artifact.read".to_owned()]);
        let capabilities = BTreeSet::from([Capability::ArtifactRead]);
        let context = PolicyContext {
            tenant_id: "tenant-a",
            project_id: "project-a",
            task_id: Uuid::nil(),
            actor_id: "agent-a",
            allowed_tool_ids: &allowed,
            granted_capabilities: &capabilities,
        };
        assert_eq!(
            authorize_tool(&registry, &sample_action(), &context, "tenant-a", "project-a", Uuid::nil(), None, Utc::now()),
            PolicyDecision::Allow
        );

        let no_tools = BTreeSet::new();
        let denied_context = PolicyContext { allowed_tool_ids: &no_tools, ..context.clone() };
        assert_eq!(
            authorize_tool(&registry, &sample_action(), &denied_context, "tenant-a", "project-a", Uuid::nil(), None, Utc::now()),
            PolicyDecision::Deny("tool_not_in_task_allowlist")
        );
        assert_eq!(
            authorize_tool(&registry, &sample_action(), &context, "tenant-a", "another-project", Uuid::nil(), None, Utc::now()),
            PolicyDecision::Deny("scope_mismatch")
        );
        assert_eq!(
            authorize_tool(&registry, &sample_action(), &context, "tenant-b", "project-a", Uuid::nil(), None, Utc::now()),
            PolicyDecision::Deny("scope_mismatch")
        );
        assert_eq!(
            authorize_tool(&registry, &sample_action(), &context, "tenant-a", "project-a", Uuid::new_v4(), None, Utc::now()),
            PolicyDecision::Deny("scope_mismatch")
        );
    }

    #[test]
    fn tool_input_rejects_unknown_keys_and_wrong_types() {
        let tool = sample_tool(RiskClass::ReadOnly);
        assert!(tool.input.validate(&json!({"artifact_id":"x"})).is_ok());
        assert_eq!(
            tool.input.validate(&json!({"artifact_id":7})),
            Err(ContractError::InvalidToolInput)
        );
        assert_eq!(
            tool.input.validate(&json!({"artifact_id":"x","path":"../../secret"})),
            Err(ContractError::InvalidToolInput)
        );
    }

    #[test]
    fn side_effect_requires_digest_bound_external_approval() {
        let mut registry = ToolRegistry::default();
        let mut tool = sample_tool(RiskClass::ExternalSideEffect);
        tool.capability = Capability::ExternalPublish;
        registry.register(tool).unwrap();
        let allowed = BTreeSet::from(["artifact.read".to_owned()]);
        let capabilities = BTreeSet::from([Capability::ExternalPublish]);
        let context = PolicyContext {
            tenant_id: "tenant-a",
            project_id: "project-a",
            task_id: Uuid::nil(),
            actor_id: "coordinator",
            allowed_tool_ids: &allowed,
            granted_capabilities: &capabilities,
        };
        let action = sample_action();
        assert_eq!(
            authorize_tool(&registry, &action, &context, "tenant-a", "project-a", Uuid::nil(), None, Utc::now()),
            PolicyDecision::RequiresApproval
        );
        let approval = Approval {
            action_digest: action.digest().unwrap(),
            target: "artifact.read".into(),
            approved_by: "human-owner".into(),
            expires_at: Utc::now() + Duration::minutes(5),
        };
        assert_eq!(
            authorize_tool(&registry, &action, &context, "tenant-a", "project-a", Uuid::nil(), Some(&approval), Utc::now()),
            PolicyDecision::Allow
        );
        let mut changed = sample_action();
        changed.arguments = json!({"artifact_id":"different"});
        assert_eq!(
            authorize_tool(&registry, &changed, &context, "tenant-a", "project-a", Uuid::nil(), Some(&approval), Utc::now()),
            PolicyDecision::RequiresApproval
        );
    }

    #[test]
    fn budget_cannot_be_extended_by_agent() {
        let now = Utc::now();
        let mut budget = Budget {
            turns_remaining: 1,
            tool_calls_remaining: 1,
            tokens_remaining: 100,
            deadline: now + Duration::minutes(1),
        };
        assert!(budget.consume_turn(now).is_ok());
        assert_eq!(budget.consume_turn(now), Err(ContractError::BudgetExhausted));
        assert!(budget.consume_tokens(80, now).is_ok());
        assert_eq!(budget.consume_tokens(21, now), Err(ContractError::BudgetExhausted));
    }

    #[test]
    fn state_machine_rejects_skipped_authorization_and_terminal_reentry() {
        assert!(!AgentState::AwaitingModel.can_transition_to(AgentState::ExecutingTool));
        assert!(AgentState::Authorizing.can_transition_to(AgentState::ExecutingTool));
        assert!(!AgentState::Completed.can_transition_to(AgentState::Planning));
        assert!(AgentState::RetryWait.can_transition_to(AgentState::Leased));
    }

    #[test]
    fn action_rejects_unknown_version_and_malformed_tool_call() {
        let mut action = sample_action();
        action.schema_version = 2;
        assert_eq!(action.validate(), Err(ContractError::InvalidAction));
        let mut action = sample_action();
        action.tool_version = None;
        assert_eq!(action.validate(), Err(ContractError::InvalidAction));
    }
}


#[derive(Debug, Clone)]
pub struct ToolObservation {
    pub tool_id: String,
    pub output: Value,
}

pub struct AgentTurnRequest<'a> {
    pub task: &'a AgentTask,
    pub observations: &'a [ToolObservation],
}

pub struct ModelResponse {
    pub action: ActionEnvelope,
    pub tokens_used: u32,
}

/// Provider adapters implement this boundary. Implementations must not log prompts or responses.
pub trait AgentModel {
    fn next_action(&mut self, request: &AgentTurnRequest<'_>) -> Result<ModelResponse, RuntimeError>;
}

/// Tool adapters receive only the validated arguments and a registered tool definition.
pub trait ToolExecutor {
    fn execute(&mut self, tool: &ToolDefinition, arguments: &Value) -> Result<Value, RuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    InvalidTask,
    ModelFailure,
    InvalidModelAction,
    PolicyDenied(&'static str),
    ToolFailure,
    ToolOutputTooLarge,
    BudgetExhausted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentRunResult {
    Completed { output: Value, turns_used: u16, tool_calls_used: u16 },
    DelegationRequested { payload: Value, turns_used: u16, tool_calls_used: u16 },
    AwaitingApproval { tool_id: String, action_digest: String, turns_used: u16, tool_calls_used: u16 },
}

pub fn run_agent_loop<M: AgentModel, E: ToolExecutor>(
    task: &AgentTask,
    model: &mut M,
    executor: &mut E,
    registry: &ToolRegistry,
    policy_context: &PolicyContext<'_>,
    approval: Option<&Approval>,
    budget: &mut Budget,
    now: DateTime<Utc>,
) -> Result<AgentRunResult, RuntimeError> {
    task.validate(now).map_err(|_| RuntimeError::InvalidTask)?;
    if policy_context.tenant_id != task.tenant_id
        || policy_context.project_id != task.project_id
        || policy_context.task_id != task.task_id
    {
        return Err(RuntimeError::PolicyDenied("scope_mismatch"));
    }

    let mut observations = Vec::new();
    let mut turns_used = 0_u16;
    let mut tool_calls_used = 0_u16;

    loop {
        budget.consume_turn(Utc::now()).map_err(|_| RuntimeError::BudgetExhausted)?;
        turns_used = turns_used.saturating_add(1);

        let request = AgentTurnRequest { task, observations: &observations };
        let response = model.next_action(&request).map_err(|_| RuntimeError::ModelFailure)?;
        budget.consume_tokens(response.tokens_used, Utc::now())
            .map_err(|_| RuntimeError::BudgetExhausted)?;
        response.action.validate().map_err(|_| RuntimeError::InvalidModelAction)?;

        match response.action.kind {
            ActionKind::Finish => {
                let output_size = serde_json::to_vec(&response.action.arguments)
                    .map_err(|_| RuntimeError::InvalidModelAction)?.len();
                if output_size > MAX_TOOL_OUTPUT_BYTES {
                    return Err(RuntimeError::ToolOutputTooLarge);
                }
                return Ok(AgentRunResult::Completed {
                    output: response.action.arguments,
                    turns_used,
                    tool_calls_used,
                });
            }
            ActionKind::Delegate => {
                return Ok(AgentRunResult::DelegationRequested {
                    payload: response.action.arguments,
                    turns_used,
                    tool_calls_used,
                });
            }
            ActionKind::InvokeTool => {
                budget.consume_tool_call(Utc::now()).map_err(|_| RuntimeError::BudgetExhausted)?;
                let decision = authorize_tool(
                    registry,
                    &response.action,
                    policy_context,
                    &task.tenant_id,
                    &task.project_id,
                    task.task_id,
                    approval,
                    Utc::now(),
                );
                match decision {
                    PolicyDecision::Allow => {}
                    PolicyDecision::Deny(reason) => return Err(RuntimeError::PolicyDenied(reason)),
                    PolicyDecision::RequiresApproval => {
                        let tool_id = response.action.tool_id.clone().unwrap_or_default();
                        return Ok(AgentRunResult::AwaitingApproval {
                            tool_id,
                            action_digest: response.action.digest().map_err(|_| RuntimeError::InvalidModelAction)?,
                            turns_used,
                            tool_calls_used,
                        });
                    }
                }
                let tool_id = response.action.tool_id.as_deref().ok_or(RuntimeError::InvalidModelAction)?;
                let tool = registry.get(tool_id).ok_or(RuntimeError::PolicyDenied("tool_not_registered"))?;
                let output = executor.execute(tool, &response.action.arguments)?;
                let output_size = serde_json::to_vec(&output).map_err(|_| RuntimeError::ToolFailure)?.len();
                if output_size > tool.max_output_bytes || output_size > MAX_TOOL_OUTPUT_BYTES {
                    return Err(RuntimeError::ToolOutputTooLarge);
                }
                tool_calls_used = tool_calls_used.saturating_add(1);
                observations.push(ToolObservation { tool_id: tool_id.to_owned(), output });
            }
        }
    }
}

