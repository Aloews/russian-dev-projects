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
