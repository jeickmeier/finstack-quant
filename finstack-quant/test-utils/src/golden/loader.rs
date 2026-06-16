//! Golden test suite loaders.
//!
//! This module provides functions for loading golden test suites from
//! various sources: files, strings, and directories.

use crate::golden::types::{GoldenSuite, SuiteMeta};
use crate::Error;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;

// =============================================================================
// Core loaders
// =============================================================================

/// Load a golden suite from a JSON file.
///
/// This function supports multiple JSON formats:
/// - Canonical format: `{ "meta": {...}, "cases": [...] }`
/// - Array format: `[...]` (cases only, no metadata)
/// - Single object format: `{...}` (single case, no metadata)
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
///
/// # Example
///
/// ```ignore
/// use finstack_quant_test_utils::golden::load_suite_from_path;
///
/// let suite = load_suite_from_path::<MyTestCase>("tests/golden/data/my_suite.json")?;
/// for case in &suite.cases {
///     // test each case
/// }
/// ```
pub fn load_suite_from_path<T>(path: impl AsRef<Path>) -> Result<GoldenSuite<T>, Error>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|e| {
        Error::Validation(format!(
            "Failed to read golden file '{}': {}",
            path.display(),
            e
        ))
    })?;

    load_suite_from_str(&content).map_err(|e| {
        Error::Validation(format!(
            "Failed to parse golden file '{}': {}",
            path.display(),
            e
        ))
    })
}

/// Load a golden suite from a JSON string.
///
/// Supports the same formats as [`load_suite_from_path`].
pub fn load_suite_from_str<T>(json: &str) -> Result<GoldenSuite<T>, Error>
where
    T: DeserializeOwned,
{
    // Try canonical format first
    if let Ok(suite) = serde_json::from_str::<GoldenSuite<T>>(json) {
        return Ok(suite);
    }

    // Try array format (just cases, no metadata)
    if let Ok(cases) = serde_json::from_str::<Vec<T>>(json) {
        return Ok(GoldenSuite {
            meta: SuiteMeta::default(),
            cases,
        });
    }

    // Try single object format
    if let Ok(case) = serde_json::from_str::<T>(json) {
        return Ok(GoldenSuite {
            meta: SuiteMeta::default(),
            cases: vec![case],
        });
    }

    Err(Error::Validation(
        "Failed to parse JSON as any known golden suite format".to_string(),
    ))
}

/// Load test cases from a directory of JSON files.
///
/// Each file in the directory should contain a single test case object.
/// Files matching the glob pattern are loaded and their contents collected.
///
/// This is useful for the CDS-style "one file per test case" layout.
///
/// # Arguments
///
/// * `dir` - Directory containing JSON files
/// * `exclude_pattern` - Optional substring to exclude files (e.g., "schema")
///
/// # Example
///
/// ```ignore
/// use finstack_quant_test_utils::golden::load_cases_from_dir;
///
/// let cases = load_cases_from_dir::<CdsVector>("tests/golden/cds/", Some("schema"))?;
/// ```
pub fn load_cases_from_dir<T>(
    dir: impl AsRef<Path>,
    exclude_pattern: Option<&str>,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned,
{
    let dir = dir.as_ref();
    let entries = fs::read_dir(dir).map_err(|e| {
        Error::Validation(format!(
            "Failed to read golden directory '{}': {}",
            dir.display(),
            e
        ))
    })?;

    let mut cases = Vec::new();

    for entry in entries {
        let entry = entry
            .map_err(|e| Error::Validation(format!("Failed to read directory entry: {}", e)))?;

        let path = entry.path();

        // Skip non-JSON files
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }

        // Skip files matching exclude pattern
        if let Some(pattern) = exclude_pattern {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().contains(pattern) {
                    continue;
                }
            }
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            Error::Validation(format!("Failed to read file '{}': {}", path.display(), e))
        })?;

        let case: T = serde_json::from_str(&content).map_err(|e| {
            Error::Validation(format!("Failed to parse '{}': {}", path.display(), e))
        })?;

        cases.push(case);
    }

    Ok(cases)
}

/// Load a golden suite from a directory where each file is a test case.
///
/// Wraps the loaded cases in a `GoldenSuite` with default metadata.
pub fn load_suite_from_dir<T>(
    dir: impl AsRef<Path>,
    exclude_pattern: Option<&str>,
) -> Result<GoldenSuite<T>, Error>
where
    T: DeserializeOwned,
{
    let cases = load_cases_from_dir(dir, exclude_pattern)?;
    Ok(GoldenSuite {
        meta: SuiteMeta::default(),
        cases,
    })
}

// =============================================================================
// Status filtering
// =============================================================================

/// Check if a suite is ready for testing based on its status.
///
/// Returns `true` if status is "certified", `false` otherwise.
/// Prints a message if the suite is skipped.
pub fn is_suite_ready(meta: &SuiteMeta, label: &str) -> bool {
    let status = meta.status.to_ascii_lowercase();
    if status == "certified" {
        true
    } else {
        tracing::info!(
            suite = label,
            status = %meta.status,
            "skipping non-certified golden suite"
        );
        false
    }
}

// =============================================================================
// Path utilities
// =============================================================================

/// Construct a path to a golden data file relative to CARGO_MANIFEST_DIR.
///
/// This is typically used in test code:
///
/// ```ignore
/// let path = golden_path(env!("CARGO_MANIFEST_DIR"), "data/my_suite.json");
/// ```
pub fn golden_path(manifest_dir: &str, relative_path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(manifest_dir)
        .join("tests")
        .join("golden")
        .join(relative_path)
}

/// Construct a path to the golden data directory relative to CARGO_MANIFEST_DIR.
pub fn golden_data_dir(manifest_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(manifest_dir)
        .join("tests")
        .join("golden")
        .join("data")
}

/// Construct a path to the golden root directory relative to CARGO_MANIFEST_DIR.
pub fn golden_dir(manifest_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(manifest_dir)
        .join("tests")
        .join("golden")
}

// =============================================================================
// Macros for path construction
// =============================================================================

/// Macro to construct a path to a golden file relative to the calling crate.
///
/// # Usage
///
/// ```ignore
/// use finstack_quant_test_utils::golden_path;
///
/// let path = golden_path!("data/my_suite.json");
/// // Expands to: finstack_quant_test_utils::golden::golden_path(env!("CARGO_MANIFEST_DIR"), "data/my_suite.json")
/// ```
#[macro_export]
macro_rules! golden_path {
    ($relative:expr) => {
        $crate::golden::golden_path(env!("CARGO_MANIFEST_DIR"), $relative)
    };
}

/// Macro to get the golden data directory for the calling crate.
///
/// # Usage
///
/// ```ignore
/// use finstack_quant_test_utils::golden_data_dir;
///
/// let dir = golden_data_dir!();
/// // Expands to: <crate>/tests/golden/data
/// ```
#[macro_export]
macro_rules! golden_data_dir {
    () => {
        $crate::golden::golden_data_dir(env!("CARGO_MANIFEST_DIR"))
    };
}

/// Macro to get the golden root directory for the calling crate.
///
/// # Usage
///
/// ```ignore
/// use finstack_quant_test_utils::golden_dir;
///
/// let dir = golden_dir!();
/// // Expands to: <crate>/tests/golden
/// ```
#[macro_export]
macro_rules! golden_dir {
    () => {
        $crate::golden::golden_dir(env!("CARGO_MANIFEST_DIR"))
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct SimpleCase {
        id: String,
        value: f64,
    }

    #[test]
    fn test_load_canonical_format() {
        let json = r#"{
            "meta": {
                "suite_id": "test",
                "description": "Test",
                "reference_source": { "name": "manual" },
                "generated": { "at": "2025-01-15", "by": "test" },
                "status": "certified",
                "schema_version": 1
            },
            "cases": [
                { "id": "case1", "value": 1.0 },
                { "id": "case2", "value": 2.0 }
            ]
        }"#;

        let result = load_suite_from_str::<SimpleCase>(json);
        assert!(result.is_ok(), "Should parse canonical format");
        if let Ok(suite) = result {
            assert_eq!(suite.meta.suite_id, "test");
            assert_eq!(suite.cases.len(), 2);
            assert_eq!(suite.cases[0].id, "case1");
        }
    }

    #[test]
    fn test_load_array_format() {
        let json = r#"[
            { "id": "case1", "value": 1.0 },
            { "id": "case2", "value": 2.0 }
        ]"#;

        let result = load_suite_from_str::<SimpleCase>(json);
        assert!(result.is_ok(), "Should parse array format");
        if let Ok(suite) = result {
            assert_eq!(suite.cases.len(), 2);
        }
    }

    #[test]
    fn test_load_single_object() {
        let json = r#"{ "id": "case1", "value": 1.0 }"#;

        let result = load_suite_from_str::<SimpleCase>(json);
        assert!(result.is_ok(), "Should parse single object format");
        if let Ok(suite) = result {
            assert_eq!(suite.cases.len(), 1);
            assert_eq!(suite.cases[0].id, "case1");
        }
    }

    #[test]
    fn test_is_suite_ready() {
        let certified_meta = SuiteMeta {
            status: "certified".to_string(),
            ..Default::default()
        };
        assert!(is_suite_ready(&certified_meta, "test"));

        let provisional_meta = SuiteMeta {
            status: "provisional".to_string(),
            ..Default::default()
        };
        assert!(!is_suite_ready(&provisional_meta, "test"));
    }
}
