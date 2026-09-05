//! Return-shape checks for public WASM entry points.
//!
//! The mirror of `finstack-quant-py/tests/parity/test_return_shapes.py`. The
//! two files are deliberately kept in the same order with the same entry
//! names, so a cross-language divergence reads as a one-screen diff.
//!
//! These assertions are made against the hand-written `index.d.ts`, which is
//! the published contract JS consumers compile against.

use std::fs;
use std::path::PathBuf;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn index_dts() -> String {
    fs::read_to_string(manifest_dir().join("index.d.ts")).expect("read index.d.ts")
}

/// All Rust sources under `src/`, concatenated with their paths, so a check
/// can assert on the binding layer as a whole.
fn api_sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &PathBuf, out: &mut Vec<(PathBuf, String)>) {
        for entry in fs::read_dir(dir).expect("read src dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let body = fs::read_to_string(&path).expect("read source");
                out.push((path, body));
            }
        }
    }
    let mut out = Vec::new();
    walk(&manifest_dir().join("src"), &mut out);
    out
}

/// The declared return type of an exported function in `index.d.ts`.
///
/// Handles both single-line and multi-line signatures, and ignores mentions
/// inside doc comments (the name appears in prose far more often than in a
/// declaration). Returns `None` when the export is not declared at all, so a
/// caller can distinguish that from "declared with the wrong type".
fn declared_return(dts: &str, export: &str) -> Option<String> {
    let lines: Vec<&str> = dts.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        // Skip prose: doc-comment bodies, `//` comments, and `*` continuations.
        if trimmed.starts_with('*') || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }
        if !trimmed.starts_with(&format!("{export}(")) {
            continue;
        }
        // Single-line: `name(args): Ret;`
        if let Some(colon) = line.rfind("):") {
            return Some(line[colon + 2..].trim_end_matches(';').trim().to_string());
        }
        // Multi-line: scan forward for the closing `):`.
        for follow in lines.iter().skip(idx + 1).take(40) {
            if let Some(colon) = follow.rfind("):") {
                return Some(follow[colon + 2..].trim_end_matches(';').trim().to_string());
            }
        }
    }
    None
}

/// Raw `serde_wasm_bindgen::to_value` emits ES2015 `Map`s for Rust maps, and
/// `JSON.stringify` silently drops those. `crate::utils::to_js_value` uses the
/// `json_compatible` serializer instead. The helper itself is the only place
/// the raw call is legal.
///
/// This is also enforced by `mise run wasm-check-serializer`; keeping it here
/// too means a plain `cargo test` catches it.
#[test]
fn no_binding_bypasses_the_json_compatible_serializer() {
    let offenders: Vec<String> = api_sources()
        .into_iter()
        .filter(|(path, _)| !path.ends_with("utils/mod.rs"))
        .filter(|(_, body)| body.contains("serde_wasm_bindgen::to_value"))
        .map(|(path, _)| path.display().to_string())
        .collect();

    assert!(
        offenders.is_empty(),
        "these files call serde_wasm_bindgen::to_value directly instead of \
         crate::utils::to_js_value; the raw serializer emits ES Maps that \
         JSON.stringify drops: {offenders:?}"
    );
}

/// A `Json`-suffixed export returns a JSON string; an unsuffixed one returns a
/// structured value. The suffix is the only signal a JS caller has, so it must
/// not lie — `accruedInterestJson` used to return a number.
#[test]
fn json_suffixed_exports_return_strings() {
    let dts = index_dts();
    // Exports whose names promise a JSON document.
    for export in [
        "validateCovenantSpecJson",
        "validateCovenantReportJson",
        "validateCovenantEngineJson",
        "validateValuationResultJson",
        "instrumentCashflowsJson",
    ] {
        let Some(ret) = declared_return(&dts, export) else {
            continue; // not part of this build's surface
        };
        assert!(
            ret.contains("string"),
            "{export} is Json-suffixed but index.d.ts declares it returning {ret:?}; \
             the suffix must mean 'returns a JSON string'"
        );
    }
}

/// Computation results must not be declared as bare strings.
#[test]
fn computation_results_are_structured_not_strings() {
    let dts = index_dts();
    for export in [
        "priceInstrument",
        "priceInstrumentWithMarket",
        "calibrate",
        "attributePnl",
        "structuredCreditTrancheOas",
        "structuredCreditTrancheMetrics",
        "structuredCreditTrancheScenarioTable",
        "runChecks",
        "runThreeStatementChecks",
        "runCreditUnderwritingChecks",
        "generateTornadoEntries",
        "covarianceAt",
        "factorModelAt",
        "price",
    ] {
        let Some(ret) = declared_return(&dts, export) else {
            panic!("{export} is missing from index.d.ts");
        };
        assert!(
            !ret.trim_end_matches(';').trim().eq("string"),
            "{export} is a computation result but index.d.ts declares it \
             returning a bare string; it should be a structured object (or be \
             renamed with a Json suffix if it is really a wire surface)"
        );
    }
}

/// Text-returning exports say so in their names. They share the
/// `Result<String, JsValue>` signature with ~130 genuine JSON exports, so the
/// name is the only way a caller can tell prose from a parseable document.
#[test]
fn prose_returning_exports_are_named_text() {
    let dts = index_dts();
    for export in [
        "parseFormulaText",
        "plSummaryReportText",
        "creditAssessmentReportText",
        "explainFormulaText",
    ] {
        assert!(
            dts.contains(&format!("{export}(")),
            "{export} is missing from index.d.ts; prose-returning exports must \
             carry the Text suffix so they are not mistaken for JSON"
        );
    }
    // The pre-refactor names must be gone, not aliased.
    for stale in [
        "parseFormula(",
        "plSummaryReport(",
        "creditAssessmentReport(",
    ] {
        assert!(
            !dts.contains(stale),
            "the pre-refactor export {stale:?} is still declared; renames \
             replace, they do not alias"
        );
    }
}

/// Numeric vectors cross the boundary as `Float64Array`, not as boxed-`Number`
/// JS arrays. Rust-side that means returning `Box<[f64]>`.
#[test]
fn numeric_vector_exports_declare_float64array() {
    let dts = index_dts();
    for export in [
        "correlationBounds",
        "jointProbabilities",
        "nearestCorrelation",
        "generateSmile",
        "snowballCouponProfile",
        "inverseFloaterCouponProfile",
    ] {
        let Some(ret) = declared_return(&dts, export) else {
            continue;
        };
        assert!(
            ret.contains("Float64Array"),
            "{export} returns a numeric vector but index.d.ts declares {ret:?}; \
             flat numeric vectors cross as Float64Array"
        );
    }
}

/// The JS facade is a pure namespace re-export. A `JSON.parse` in it means the
/// wasm export and the declared type disagree, and it applies inconsistently:
/// `calibrate` used to be parsed while its sibling `dryRun` was not.
#[test]
fn facade_does_no_json_parsing() {
    let exports_dir = manifest_dir().join("exports");
    let mut offenders = Vec::new();
    fn walk(dir: &PathBuf, offenders: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("read exports dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, offenders);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "js") {
                let body = fs::read_to_string(&path).expect("read facade file");
                if body.contains("JSON.parse") {
                    offenders.push(path.display().to_string());
                }
            }
        }
    }
    walk(&exports_dir, &mut offenders);

    assert!(
        offenders.is_empty(),
        "the facade must be a pure namespace re-export, but these files call \
         JSON.parse: {offenders:?}. Convert the underlying wasm export to \
         return an object instead."
    );
}
