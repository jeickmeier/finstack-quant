//! Pins the standard pricer and metric registries.
//!
//! The snapshot (`tests/data/registry_snapshot.json`) lists, per instrument
//! type, the registered model keys and the applicable metric identifiers. It is
//! the one pinned list of models and metrics per instrument type. The
//! `gen_registry_snapshot` binary renders it: `mise run gen-write` runs it with
//! `--write` after an intentional registration change, and this test runs it
//! with `--check`.

#[test]
fn standard_registries_match_the_pinned_snapshot() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gen_registry_snapshot"))
        .arg("--check")
        .output()
        .expect("run gen_registry_snapshot");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
