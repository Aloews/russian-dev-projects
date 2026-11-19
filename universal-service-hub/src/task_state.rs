#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    Received, Classified, Planned, Queued, InProgress, CandidateReady,
    QueuedBuild, Building, QueuedTest, Testing, Review, Verified,
    Blocked, Stalled, Recovery, Cancelled, Failed,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Received => "received", Self::Classified => "classified",
            Self::Planned => "planned", Self::Queued => "queued",
            Self::InProgress => "in_progress", Self::CandidateReady => "candidate_ready",
            Self::QueuedBuild => "queued_build", Self::Building => "building",
            Self::QueuedTest => "queued_test", Self::Testing => "testing",
            Self::Review => "review", Self::Verified => "verified",
            Self::Blocked => "blocked", Self::Stalled => "stalled",
            Self::Recovery => "recovery", Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "received" => Self::Received, "classified" => Self::Classified,
            "planned" => Self::Planned, "queued" => Self::Queued,
            "in_progress" => Self::InProgress, "candidate_ready" => Self::CandidateReady,
            "queued_build" => Self::QueuedBuild, "building" => Self::Building,
            "queued_test" => Self::QueuedTest, "testing" => Self::Testing,
            "review" => Self::Review, "verified" => Self::Verified,
            "blocked" => Self::Blocked, "stalled" => Self::Stalled,
            "recovery" => Self::Recovery, "cancelled" => Self::Cancelled,
            "failed" => Self::Failed, _ => return None,
        })
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use TaskStatus::*;
        matches!((self, next),
            (Received, Classified | Blocked | Cancelled) |
            (Classified, Planned | Blocked | Cancelled) |
            (Planned, Queued | Blocked | Cancelled) |
            (Queued, InProgress | Recovery | Cancelled | Blocked) |
            (InProgress, CandidateReady | QueuedBuild | QueuedTest | Review | Blocked | Stalled | Recovery | Failed | Cancelled) |
            (CandidateReady, QueuedBuild | QueuedTest | Review | Recovery | Cancelled) |
            (QueuedBuild, Building | Recovery | Cancelled | Blocked) |
            (Building, QueuedTest | Review | CandidateReady | Blocked | Stalled | Recovery | Failed | Cancelled) |
            (QueuedTest, Testing | Recovery | Cancelled | Blocked) |
            (Testing, Review | CandidateReady | Blocked | Stalled | Recovery | Failed | Cancelled) |
            (Review, Verified | CandidateReady | Blocked | Recovery | Failed) |
            (Blocked, Classified | Planned | Queued | Cancelled) |
            (Stalled, Recovery | Queued | Cancelled | Failed) |
            (Recovery, Queued | InProgress | QueuedBuild | QueuedTest | Failed | Cancelled)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::TaskStatus::*;

    #[test]
    fn successful_pipeline_has_explicit_gates() {
        assert!(Received.can_transition_to(Classified));
        assert!(Classified.can_transition_to(Planned));
        assert!(Planned.can_transition_to(Queued));
        assert!(Queued.can_transition_to(InProgress));
        assert!(InProgress.can_transition_to(CandidateReady));
        assert!(CandidateReady.can_transition_to(QueuedBuild));
        assert!(QueuedBuild.can_transition_to(Building));
        assert!(Building.can_transition_to(QueuedTest));
        assert!(QueuedTest.can_transition_to(Testing));
        assert!(Testing.can_transition_to(Review));
        assert!(Review.can_transition_to(Verified));
    }

    #[test]
    fn terminal_states_cannot_be_reopened() {
        for terminal in [Verified, Cancelled, Failed] {
            for next in [Received, Queued, InProgress, Recovery] {
                assert!(!terminal.can_transition_to(next));
            }
        }
    }

    #[test]
    fn illegal_shortcuts_are_rejected() {
        assert!(!Received.can_transition_to(InProgress));
        assert!(!Queued.can_transition_to(Verified));
        assert!(!Building.can_transition_to(Verified));
        assert!(!Testing.can_transition_to(Verified));
    }

    #[test]
    fn recovery_requires_explicit_requeue_or_resume() {
        assert!(Stalled.can_transition_to(Recovery));
        assert!(Recovery.can_transition_to(Queued));
        assert!(Recovery.can_transition_to(InProgress));
        assert!(!Recovery.can_transition_to(Verified));
    }
}
