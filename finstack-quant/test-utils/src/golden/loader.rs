//! Golden test suite loaders.
//!
//! This module provides functions for loading golden test suites from files
//! and strings.

use crate::Error;
use crate::golden::types::GoldenSuite;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;

/// Load a golden suite from a JSON file.
///
/// The file must use the canonical `{ "meta": {...}, "cases": [...] }`
/// envelope and explicitly declare `meta.schema_version: 1`.
///
/// # Arguments
///
/// * `path` - Filesystem path to a UTF-8 JSON fixture in any supported golden
///   suite format.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
///
/// # Example
///
/// ```no_run
/// use finstack_quant_test_utils::golden::load_suite_from_path;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct MyTestCase {
///     id: String,
///     expected: f64,
/// }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let suite = load_suite_from_path::<MyTestCase>("tests/golden/data/my_suite.json")?;
/// for case in &suite.cases {
///     println!("{}: {}", case.id, case.expected);
/// }
/// # Ok(())
/// # }
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
/// Requires the canonical v1 suite envelope used by [`load_suite_from_path`].
///
/// # Arguments
///
/// * `json` - UTF-8 JSON text containing a canonical v1 golden suite.
///
/// # Errors
///
/// Returns [`Error::Validation`] when `json` is neither a serialized
/// [`GoldenSuite`].
pub fn load_suite_from_str<T>(json: &str) -> Result<GoldenSuite<T>, Error>
where
    T: DeserializeOwned,
{
    serde_json::from_str::<GoldenSuite<T>>(json).map_err(|error| {
        Error::Validation(format!("Failed to parse canonical golden suite: {error}"))
    })
}

/// Construct a path to a golden data file relative to CARGO_MANIFEST_DIR.
///
/// This is typically used in test code:
///
/// ```
/// use finstack_quant_test_utils::golden::golden_path;
///
/// let path = golden_path(env!("CARGO_MANIFEST_DIR"), "data/my_suite.json");
/// assert!(path.ends_with("tests/golden/data/my_suite.json"));
/// ```
///
/// # Arguments
///
/// * `manifest_dir` - Calling crate's manifest directory, normally
///   `env!("CARGO_MANIFEST_DIR")`.
/// * `relative_path` - Path relative to that crate's `tests/golden` directory.
pub fn golden_path(manifest_dir: &str, relative_path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(manifest_dir)
        .join("tests")
        .join("golden")
        .join(relative_path)
}

/// Macro to construct a path to a golden file relative to the calling crate.
///
/// # Usage
///
/// ```
/// use finstack_quant_test_utils::golden_path;
///
/// let path = golden_path!("data/my_suite.json");
/// // Expands to: finstack_quant_test_utils::golden::golden_path(env!("CARGO_MANIFEST_DIR"), "data/my_suite.json")
/// ```
#[macro_export]
macro_rules! golden_path {
    ($relative:expr_2021) => {
        $crate::golden::golden_path(env!("CARGO_MANIFEST_DIR"), $relative)
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
    fn rejects_array_format() {
        let json = r#"[
            { "id": "case1", "value": 1.0 },
            { "id": "case2", "value": 2.0 }
        ]"#;

        let result = load_suite_from_str::<SimpleCase>(json);
        assert!(result.is_err(), "array format must be rejected");
    }

    #[test]
    fn rejects_single_object_format() {
        let json = r#"{ "id": "case1", "value": 1.0 }"#;

        let result = load_suite_from_str::<SimpleCase>(json);
        assert!(result.is_err(), "single-object format must be rejected");
    }
}
