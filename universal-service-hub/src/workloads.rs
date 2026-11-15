use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Leased,
    Running,
    Checkpointed,
    Succeeded,
    Paused,
    CancelRequested,
    Cancelled,
    RetryWait,
    SubmissionUnknown,
    FailedPermanent,
    DeadLetter,
}

impl JobState {
    pub fn can_transition_to(self, next: Self) -> bool {
        use JobState::*;
        matches!(
            (self, next),
            (Queued, Leased | Paused | CancelRequested)
                | (Leased, Running | Queued | CancelRequested)
                | (
                    Running,
                    Checkpointed
                        | RetryWait
                        | SubmissionUnknown
                        | FailedPermanent
                        | CancelRequested
                )
                | (
                    Checkpointed,
                    Running | Succeeded | RetryWait | CancelRequested
                )
                | (RetryWait, Queued | DeadLetter)
                | (
                    SubmissionUnknown,
                    Running | FailedPermanent | CancelRequested
                )
                | (CancelRequested, Cancelled)
                | (Paused, Queued | Cancelled)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Pending,
    Ready,
    Leased,
    Running,
    Checkpointed,
    Succeeded,
    RetryWait,
    SubmissionUnknown,
    FailedPermanent,
    Cancelled,
}

impl StepState {
    pub fn can_transition_to(self, next: Self) -> bool {
        use StepState::*;
        matches!(
            (self, next),
            (Pending, Ready | Cancelled)
                | (Ready, Leased | Cancelled)
                | (Leased, Running | Ready | Cancelled)
                | (
                    Running,
                    Checkpointed | RetryWait | SubmissionUnknown | FailedPermanent | Cancelled
                )
                | (Checkpointed, Running | Succeeded | RetryWait | Cancelled)
                | (RetryWait, Ready | Cancelled)
                | (SubmissionUnknown, Running | FailedPermanent | Cancelled)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    Transient,
    Permanent,
    BillingAmbiguous,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkloadJobContract {
    pub schema_version: u16,
    pub project_id: String,
    pub job_id: String,
    pub workload_type: String,
    pub state: JobState,
    pub idempotency_key: String,
    pub input_ref: Value,
    pub budget_ceiling_usd: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkloadStepContract {
    pub step_id: String,
    pub job_id: String,
    pub ordinal: u32,
    pub objective: String,
    pub capability: String,
    pub dependencies: Vec<String>,
    pub state: StepState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AttemptFailure {
    pub class: FailureClass,
    pub code: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CheckpointContract {
    pub step_id: String,
    pub version: u32,
    pub output_ref: Value,
    pub checksum_sha256: String,
    pub schema_version: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn job_contract_round_trips_stable_wire_names() {
        let contract = WorkloadJobContract {
            schema_version: 1,
            project_id: "music-demo".into(),
            job_id: "job-1".into(),
            workload_type: "music.brief".into(),
            state: JobState::SubmissionUnknown,
            idempotency_key: "brief-1".into(),
            input_ref: json!({"brief_ref":"object://brief-1"}),
            budget_ceiling_usd: Some("0.50".into()),
        };
        let encoded = serde_json::to_value(&contract).expect("contract serializes");
        assert_eq!(encoded["state"], "submission_unknown");
        assert_eq!(
            serde_json::from_value::<WorkloadJobContract>(encoded).unwrap(),
            contract
        );
    }

    #[test]
    fn lifecycle_rejects_unsafe_or_backward_transitions() {
        assert!(JobState::Queued.can_transition_to(JobState::Leased));
        assert!(JobState::Running.can_transition_to(JobState::SubmissionUnknown));
        assert!(!JobState::Succeeded.can_transition_to(JobState::Running));
        assert!(!JobState::Running.can_transition_to(JobState::Succeeded));
    }

    #[test]
    fn step_lifecycle_allows_only_explicit_transitions() {
        use StepState::*;
        let states = [
            Pending,
            Ready,
            Leased,
            Running,
            Checkpointed,
            Succeeded,
            RetryWait,
            SubmissionUnknown,
            FailedPermanent,
            Cancelled,
        ];
        let allowed = [
            (Pending, Ready),
            (Pending, Cancelled),
            (Ready, Leased),
            (Ready, Cancelled),
            (Leased, Running),
            (Leased, Ready),
            (Leased, Cancelled),
            (Running, Checkpointed),
            (Running, RetryWait),
            (Running, SubmissionUnknown),
            (Running, FailedPermanent),
            (Running, Cancelled),
            (Checkpointed, Running),
            (Checkpointed, Succeeded),
            (Checkpointed, RetryWait),
            (Checkpointed, Cancelled),
            (RetryWait, Ready),
            (RetryWait, Cancelled),
            (SubmissionUnknown, Running),
            (SubmissionUnknown, FailedPermanent),
            (SubmissionUnknown, Cancelled),
        ];
        for from in states {
            for to in states {
                assert_eq!(
                    from.can_transition_to(to),
                    allowed.contains(&(from, to)),
                    "unexpected step transition: {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn job_lifecycle_has_no_transition_out_of_terminal_states() {
        use JobState::*;
        for terminal in [Succeeded, Cancelled, FailedPermanent, DeadLetter] {
            for next in [
                Queued,
                Leased,
                Running,
                Checkpointed,
                Succeeded,
                Paused,
                CancelRequested,
                Cancelled,
                RetryWait,
                SubmissionUnknown,
                FailedPermanent,
                DeadLetter,
            ] {
                assert!(
                    !terminal.can_transition_to(next),
                    "terminal job state {terminal:?} must not transition to {next:?}"
                );
            }
        }
    }

    #[test]
    fn billing_ambiguity_is_distinct_from_transient_failure() {
        let encoded = serde_json::to_value(AttemptFailure {
            class: FailureClass::BillingAmbiguous,
            code: "provider_timeout_after_submit".into(),
            retryable: false,
        })
        .unwrap();
        assert_eq!(encoded["class"], "billing_ambiguous");
        assert!(!encoded["retryable"].as_bool().unwrap());
    }
}
