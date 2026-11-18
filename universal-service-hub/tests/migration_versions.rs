use std::{collections::BTreeMap, fs, path::PathBuf};

#[test]
fn sqlx_migration_versions_are_unique_and_well_formed() {
    let migration_dir = PathBuf::from("rust-migrations");
    let entries = fs::read_dir(&migration_dir)
        .expect("rust-migrations directory must exist");
    let mut versions = BTreeMap::new();

    for entry in entries {
        let path = entry.expect("migration directory entry must be readable").path();
        if path.extension().and_then(|value| value.to_str()) != Some("sql") {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .expect("migration filename must be valid UTF-8");
        let (version_text, description) = stem
            .split_once('_')
            .expect("SQLx migration filename must contain a version and description");
        assert!(!description.is_empty(), "migration description must not be empty: {}", path.display());

        let version: i64 = version_text
            .parse()
            .unwrap_or_else(|_| panic!("migration version must be numeric: {}", path.display()));
        assert!(
            versions.insert(version, path.clone()).is_none(),
            "duplicate SQLx migration version {version}: {}",
            path.display()
        );
    }

    assert!(!versions.is_empty(), "at least one SQLx migration must exist");
}


#[test]
fn trusted_github_verification_migration_is_tenant_scoped_and_immutable() {
    let migration = fs::read_to_string(
        "rust-migrations/0011_trusted_github_verification.sql",
    )
    .expect("trusted verification migration must exist");
    for required in [
        "engineering_task_verification_evidence",
        "UNIQUE (repository, run_id, run_attempt, task_id)",
        "ENABLE ROW LEVEL SECURITY",
        "FORCE ROW LEVEL SECURITY",
        "CREATE POLICY tenant_execution_isolation",
        "REVOKE UPDATE, DELETE ON engineering_task_verification_evidence FROM ush_runtime",
        "ush_validate_verification_evidence_parent",
        "ush_prevent_verification_evidence_update",
        "conclusion IN ('success','failure','cancelled'",
        "job_workflow_sha",
        "run_started_at",
        "run_completed_at",
        "artifact_digest VARCHAR(71) CHECK (artifact_digest IS NULL OR",
        "length(artifact_digest)=71",
        "attestation_run_id BIGINT NOT NULL",
        "attestation_run_attempt INTEGER NOT NULL",
        "attestation_sha VARCHAR(40) NOT NULL",
        "run_completed_at >= run_started_at",
        "event_name VARCHAR(40) NOT NULL CHECK (event_name IN ('push','workflow_dispatch'))",
    ] {
        assert!(
            migration.contains(required),
            "trusted verification migration is missing required control: {required}"
        );
    }
}
