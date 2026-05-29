//! Pricing-domain golden tests.

use crate::golden::runner::run_golden_at_path;
use crate::golden::walk::collect_fixture_paths_under;
use serde::Deserialize;
use std::path::Path;

/// One entry in `known_non_executable.json`.
#[derive(Deserialize)]
struct NonExecutableEntry {
    /// Fixture path relative to `tests/golden/data/`.
    path: String,
    /// Why the fixture cannot yet be reproduced by the executable runner.
    reason: String,
}

#[derive(Deserialize)]
struct NonExecutableFile {
    fixtures: Vec<NonExecutableEntry>,
}

/// Load the shared list of fixtures whose failures are reported but not fatal.
///
/// The same `known_non_executable.json` drives the Python layer's
/// `pytest.mark.xfail(strict=False)` markers, so the allowlist lives in one
/// place across both languages.
///
/// Setting `GOLDEN_IGNORE_NON_EXECUTABLE` returns an empty allowlist so every
/// fixture is compared strictly and all failures surface (see `mise goldens-test-strict`).
fn known_non_executable() -> Vec<NonExecutableEntry> {
    if std::env::var_os("GOLDEN_IGNORE_NON_EXECUTABLE").is_some() {
        return Vec::new();
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/known_non_executable.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let parsed: NonExecutableFile =
        serde_json::from_str(&raw).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()));
    parsed.fixtures
}

/// Return the non-executable reason for `path`, if it is on the allowlist.
fn non_executable_reason<'a>(entries: &'a [NonExecutableEntry], path: &Path) -> Option<&'a str> {
    let path = path.to_string_lossy();
    entries
        .iter()
        .find(|entry| path.ends_with(&entry.path))
        .map(|entry| entry.reason.as_str())
}

#[test]
#[ignore = "slow: covered by mise goldens-test or mise rust-test-slow"]
fn golden_pricing_fixtures_from_existing_json_files() {
    let mut paths =
        collect_fixture_paths_under("pricing").expect("pricing fixture discovery should succeed");
    if let Ok(filter) = std::env::var("GOLDEN_FIXTURE_FILTER") {
        paths.retain(|path| path.to_string_lossy().contains(&filter));
    }
    assert!(
        !paths.is_empty(),
        "pricing fixture discovery did not find any JSON files"
    );

    let allowlist = known_non_executable();
    let mut failures = Vec::new();
    for path in paths {
        let reason = non_executable_reason(&allowlist, &path);
        match run_golden_at_path(&path) {
            Ok(results) => {
                let mismatches = results
                    .iter()
                    .filter(|result| !result.passed)
                    .map(|result| result.failure_message(&path.display().to_string()))
                    .collect::<Vec<_>>();
                match reason {
                    Some(reason) => {
                        for mismatch in &mismatches {
                            eprintln!("xfail (known non-executable: {reason}):\n{mismatch}");
                        }
                    }
                    None => failures.extend(mismatches),
                }
            }
            Err(err) => match reason {
                Some(reason) => {
                    eprintln!(
                        "xfail (known non-executable: {reason}): run fixture {path:?}: {err}"
                    );
                }
                None => failures.push(format!("run fixture {path:?}: {err}")),
            },
        }
    }

    assert!(
        failures.is_empty(),
        "{} pricing golden fixture failure(s):\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
