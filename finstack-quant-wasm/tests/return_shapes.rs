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

/// One method declared inside an `interface` or `class` body of `index.d.ts`.
struct Member {
    owner: String,
    name: String,
    ret: String,
}

/// Every method declared in `index.d.ts`, with its owning interface or class.
///
/// Members sit at a two-space indent (the file is prettier-formatted);
/// multi-line signatures end with a `  ): Ret;` line. Doc-comment prose is
/// never at that indent with a `name(` shape, so it is not mistaken for a
/// declaration.
fn members(dts: &str) -> Vec<Member> {
    let lines: Vec<&str> = dts.lines().collect();
    let mut out = Vec::new();
    let mut owner: Option<String> = None;
    for (idx, line) in lines.iter().enumerate() {
        let head = line
            .trim_start_matches("export ")
            .trim_start_matches("declare ");
        if let Some(rest) = head
            .strip_prefix("interface ")
            .or_else(|| head.strip_prefix("class "))
        {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            owner = Some(name);
            continue;
        }
        if line.starts_with('}') {
            owner = None;
            continue;
        }
        let Some(owner_name) = owner.as_ref() else {
            continue;
        };
        let Some(body) = line.strip_prefix("  ") else {
            continue;
        };
        if body.starts_with(' ') {
            continue;
        }
        let body = body
            .trim_start_matches("static ")
            .trim_start_matches("readonly ");
        let name: String = body
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let after = &body[name.len()..];
        if name.is_empty() || !(after.starts_with('(') || after.starts_with('<')) {
            continue;
        }
        let ret = if let Some(colon) = line.rfind("):") {
            Some(line[colon + 2..].to_string())
        } else {
            lines
                .iter()
                .skip(idx + 1)
                .take(80)
                .find_map(|l| l.strip_prefix("  ):").map(str::to_string))
        };
        if let Some(ret) = ret {
            out.push(Member {
                owner: owner_name.clone(),
                name,
                ret: ret.trim().trim_end_matches(';').trim().to_string(),
            });
        }
    }
    out
}

/// The declared return type of `owner.name` in `index.d.ts`.
///
/// Panics when the member is not declared: a pinned export that disappears
/// must fail the gate, not silently skip it.
fn declared(dts: &str, owner: &str, name: &str) -> String {
    members(dts)
        .into_iter()
        .find(|m| m.owner == owner && m.name == name)
        .map(|m| m.ret)
        .unwrap_or_else(|| panic!("{owner}.{name} is not declared in index.d.ts"))
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
    for (owner, export) in [
        ("CovenantsNamespace", "validateCovenantSpecJson"),
        ("CovenantsNamespace", "validateCovenantReportJson"),
        ("CovenantsNamespace", "validateCovenantEngineJson"),
        ("ValuationsNamespace", "validateValuationResultJson"),
        ("ValuationsNamespace", "valuationResultToJson"),
        ("ValuationInstrumentsNamespace", "instrumentCashflowsJson"),
    ] {
        let ret = declared(&dts, owner, export);
        assert_eq!(
            ret, "string",
            "{owner}.{export} is Json-suffixed but index.d.ts declares it returning {ret:?}; \
             the suffix must mean 'returns a JSON string'"
        );
    }
    // And the rule holds for every Json-suffixed method, pinned or not
    // (`fromJson` is a constructor, not a wire surface).
    for member in members(&dts) {
        if member.name.ends_with("Json") && !member.name.starts_with("from") {
            assert_eq!(
                member.ret, "string",
                "{}.{} is Json-suffixed but returns {:?}",
                member.owner, member.name, member.ret
            );
        }
    }
}

/// Computation results must not be declared as bare strings.
#[test]
fn computation_results_are_structured_not_strings() {
    let dts = index_dts();
    for (owner, export) in [
        ("ValuationInstrumentsNamespace", "priceInstrument"),
        ("ValuationInstrumentsNamespace", "priceInstrumentWithMarket"),
        ("CalibrationNamespace", "calibrate"),
        ("AttributionNamespace", "attributePnl"),
        (
            "ValuationInstrumentsNamespace",
            "structuredCreditTrancheOas",
        ),
        (
            "ValuationInstrumentsNamespace",
            "structuredCreditTrancheMetrics",
        ),
        (
            "ValuationInstrumentsNamespace",
            "structuredCreditTrancheScenarioTable",
        ),
        ("StatementsAnalyticsNamespace", "runChecks"),
        ("StatementsAnalyticsNamespace", "runThreeStatementChecks"),
        (
            "StatementsAnalyticsNamespace",
            "runCreditUnderwritingChecks",
        ),
        ("StatementsAnalyticsNamespace", "generateTornadoEntries"),
        ("FactorCovarianceForecast", "covarianceAt"),
        ("FactorCovarianceForecast", "factorModelAt"),
        ("FxInstrument", "price"),
    ] {
        let ret = declared(&dts, owner, export);
        assert_ne!(
            ret, "string",
            "{owner}.{export} is a computation result but index.d.ts declares it \
             returning a bare string; it should be a structured object (or be \
             renamed with a Json suffix if it is really a wire surface)"
        );
    }
}

/// A method that returns a bare `string` must say what the string is: a
/// `Json` document, `Text` prose, or `Html`. The allowlist names the few
/// strings that are the value itself; the `ratchet` entries are defects that a
/// later slice of the WASM-audit remediation removes, so the list only shrinks.
#[test]
fn bare_string_returns_are_named_or_allowlisted() {
    const VALUE_STRINGS: &[(&str, &str)] = &[
        ("Currency", "toString"),
        ("Money", "toString"),
        ("DayCount", "toString"),
        ("Tenor", "toString"),
        ("FxConversionPolicy", "toString"),
        ("FxQuoteConvention", "toString"),
        // Exact decimal text of the amount.
        ("Money", "amountDecimal"),
        // Display formatting of the amount.
        ("Money", "formatWith"),
        // Canonical return-frequency label.
        ("Performance", "frequency"),
        // Canonical liquidity-tier label.
        ("LiquidityNamespace", "liquidityTier"),
        // Canonical formula text (re-parseable), the Python `parse_formula` twin.
        ("StatementsNamespace", "parseFormula"),
    ];
    const RATCHET: &[(&str, &str)] = &[
        // S14: becomes a typed report with a `dryRunJson` sibling.
        ("CalibrationNamespace", "dryRun"),
        // S11: becomes a structured dependency tree plus a `*Text` rendering.
        ("StatementsAnalyticsNamespace", "traceDependencies"),
    ];
    let dts = index_dts();
    let offenders: Vec<String> = members(&dts)
        .into_iter()
        .filter(|m| m.ret == "string")
        .filter(|m| !["Json", "Text", "Html"].iter().any(|s| m.name.ends_with(s)))
        .filter(|m| {
            let key = (m.owner.as_str(), m.name.as_str());
            !VALUE_STRINGS.contains(&key) && !RATCHET.contains(&key)
        })
        .map(|m| format!("{}.{}", m.owner, m.name))
        .collect();
    assert!(
        offenders.is_empty(),
        "these methods return a bare string without a Json/Text/Html suffix; \
         return a structured object or name the string: {offenders:?}"
    );
}

/// Text-returning exports say so in their names. They share the
/// `Result<String, JsValue>` signature with ~130 genuine JSON exports, so the
/// name is the only way a caller can tell prose from a parseable document.
#[test]
fn prose_returning_exports_are_named_text() {
    let dts = index_dts();
    for export in [
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
        "parseFormulaText(",
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
    for (owner, export) in [
        ("CorrelationNamespace", "correlationBounds"),
        ("CorrelationNamespace", "jointProbabilities"),
        ("CorrelationNamespace", "nearestCorrelation"),
        ("SabrSmile", "generateSmile"),
        ("ValuationsNamespace", "snowballCouponProfile"),
        ("ValuationsNamespace", "inverseFloaterCouponProfile"),
    ] {
        let ret = declared(&dts, owner, export);
        assert!(
            ret.contains("Float64Array"),
            "{owner}.{export} returns a numeric vector but index.d.ts declares {ret:?}; \
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
