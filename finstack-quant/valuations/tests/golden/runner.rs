//! Fixture execution and comparison reporting.

use crate::golden::schema::GoldenFixture;
use crate::golden::tolerance::{compare, ComparisonResult};
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const REPORT_HEADER: &str = "runner,fixture,metric,actual,expected,abs_diff,rel_diff,abs_tolerance,rel_tolerance,passed,tolerance_reason\n";
const REPORT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
const REPORT_LOCK_POLL: Duration = Duration::from_millis(10);

fn is_pricing_domain(domain: &str) -> bool {
    matches!(
        domain,
        "fixed_income.bond"
            | "fixed_income.bond_future"
            | "fixed_income.convertible"
            | "fixed_income.inflation_linked_bond"
            | "fixed_income.term_loan"
            | "fixed_income.structured_credit"
            | "equity.equity_option"
            | "equity.equity_index_future"
            | "credit.cds"
            | "credit.cds_option"
            | "credit.cds_tranche"
            | "fx.fx_swap"
            | "fx.fx_forward"
            | "fx.fx_option"
            | "fx.fx_digital_option"
            | "fx.fx_barrier_option"
            | "fx.quanto_option"
            | "rates.cap_floor"
            | "rates.deposit"
            | "rates.fra"
            | "rates.irs"
            | "rates.ir_future"
            | "rates.inflation_swap"
            | "rates.swaption"
            | "exotics.barrier_option"
            | "exotics.asian_option"
            | "exotics.lookback_option"
            | "rates.cms_option"
            | "rates.cms_swap"
            | "rates.cms_spread_option"
    )
}

/// Run a fixture end-to-end and return one comparison result per expected metric.
pub fn run_fixture(fixture: &GoldenFixture) -> Result<Vec<ComparisonResult>, String> {
    let domain = fixture.metadata.domain.as_str();
    let actuals = if domain == "volatility.sabr" {
        crate::golden::sabr::run_sabr_fixture(fixture)?
    } else if is_pricing_domain(domain) {
        crate::golden::pricing_common::run_pricing_fixture(fixture)?
    } else {
        return Err(format!("no runner registered for domain '{domain}'"));
    };
    fixture
        .expected
        .iter()
        .map(|(metric, expected)| {
            let actual = actuals
                .get(metric)
                .copied()
                .ok_or_else(|| format!("runner did not produce metric '{metric}'"))?;
            let tolerance = fixture
                .tolerances
                .get(metric)
                .ok_or_else(|| format!("no tolerance for metric '{metric}'"))?;
            Ok(compare(metric, actual, *expected, tolerance))
        })
        .collect()
}

/// Run one golden fixture from disk, write a CSV comparison report, and return failures.
pub fn run_golden_at_path(path: &Path) -> Result<Vec<ComparisonResult>, String> {
    crate::golden::walk::validate_fixture(path)
        .map_err(|err| format!("validate fixture {path:?}: {err}"))?;
    let raw =
        std::fs::read_to_string(path).map_err(|err| format!("read fixture {path:?}: {err}"))?;
    let fixture: GoldenFixture =
        serde_json::from_str(&raw).map_err(|err| format!("parse fixture {path:?}: {err}"))?;
    let results = run_fixture(&fixture)?;
    write_comparison_csv(path, &results)?;
    Ok(results)
}

fn write_comparison_csv(path: &Path, results: &[ComparisonResult]) -> Result<(), String> {
    let report_path = comparison_report_path();
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create report directory {parent:?}: {err}"))?;
    }

    let fixture = fixture_relative_path(path)?;
    let _lock = ReportLock::acquire(&report_path)?;
    let mut rows = existing_comparison_rows(&report_path, "rust", &fixture)?;
    for result in results {
        rows.push(format!(
            "{},{},{},{:.12},{:.12},{:.12e},{:.12e},{},{},{},{}",
            "rust",
            csv_escape(&fixture),
            csv_escape(&result.metric),
            result.actual,
            result.expected,
            result.abs_diff,
            result.rel_diff,
            optional_f64(result.used_tolerance.abs),
            optional_f64(result.used_tolerance.rel),
            result.passed,
            csv_escape(
                result
                    .used_tolerance
                    .tolerance_reason
                    .as_deref()
                    .unwrap_or("")
            ),
        ));
    }

    let mut csv = String::from(REPORT_HEADER);
    csv.push_str(&rows.join("\n"));
    if !rows.is_empty() {
        csv.push('\n');
    }

    write_report_atomically(&report_path, csv)
}

fn comparison_report_path() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../target/golden-reports/golden-comparisons.csv")
}

fn existing_comparison_rows(
    report_path: &Path,
    runner: &str,
    fixture: &str,
) -> Result<Vec<String>, String> {
    if !report_path.exists() {
        return Ok(Vec::new());
    }

    let raw = std::fs::read_to_string(report_path)
        .map_err(|err| format!("read comparison report {report_path:?}: {err}"))?;
    Ok(raw
        .lines()
        .skip(1)
        .filter(|line| !line.starts_with(&format!("{runner},{fixture},")))
        .map(str::to_string)
        .collect())
}

struct ReportLock {
    path: PathBuf,
}

impl ReportLock {
    fn acquire(report_path: &Path) -> Result<Self, String> {
        let lock_path = report_path.with_extension("csv.lock");
        let deadline = Instant::now() + REPORT_LOCK_TIMEOUT;

        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => return Ok(Self { path: lock_path }),
                Err(err) if err.kind() == ErrorKind::AlreadyExists && Instant::now() < deadline => {
                    thread::sleep(REPORT_LOCK_POLL);
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    return Err(format!("timed out waiting for report lock {lock_path:?}"));
                }
                Err(err) => return Err(format!("create report lock {lock_path:?}: {err}")),
            }
        }
    }
}

impl Drop for ReportLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn write_report_atomically(report_path: &Path, csv: String) -> Result<(), String> {
    let temp_path = report_path.with_extension(format!("csv.{}.tmp", std::process::id()));
    std::fs::write(&temp_path, csv)
        .map_err(|err| format!("write temporary comparison report {temp_path:?}: {err}"))?;
    std::fs::rename(&temp_path, report_path).map_err(|err| {
        let _ = std::fs::remove_file(&temp_path);
        format!("replace comparison report {report_path:?}: {err}")
    })
}

fn fixture_relative_path(path: &Path) -> Result<String, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path
        .strip_prefix(manifest_dir.join("tests/golden/data"))
        .map_err(|err| format!("fixture path {path:?} is outside tests/golden/data: {err}"))?;
    Ok(relative.to_string_lossy().to_string())
}

fn optional_f64(value: Option<f64>) -> String {
    value.map_or_else(String::new, |v| format!("{v:.12}"))
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_golden_at_path_writes_comparison_csv() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let fixture_path = Path::new(manifest_dir)
            .join("tests/golden/data/pricing/quantlib/deposit/usd_deposit_3m.json");
        let report_path =
            Path::new(manifest_dir).join("../../target/golden-reports/golden-comparisons.csv");

        run_golden_at_path(&fixture_path).expect("golden should pass and write CSV report");

        let csv = std::fs::read_to_string(&report_path).expect("CSV report should exist");
        assert!(csv.contains("runner,fixture,metric,actual,expected,abs_diff,rel_diff,abs_tolerance,rel_tolerance,passed,tolerance_reason"));
        assert!(csv.contains("rust,pricing/quantlib/deposit/usd_deposit_3m.json,npv,"));
        assert!(csv.contains(",true,"));
    }

    #[test]
    fn run_golden_at_path_validates_fixture_before_execution() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let fixture_path = Path::new(manifest_dir).join(format!(
            "../../target/golden-test-invalid-schema-{}.json",
            std::process::id()
        ));
        std::fs::write(
            &fixture_path,
            r#"{
              "schema_version": "finstack_quant.golden/0",
              "metadata": {
                "name": "invalid_schema",
                "domain": "rates.irs",
                "description": "Fixture with stale schema version.",
                "valuation_date": "2026-04-30",
                "source": "formula",
                "source_detail": "unit test",
                "captured_by": "test",
                "captured_on": "2026-04-30",
                "last_reviewed_by": "test",
                "last_reviewed_on": "2026-04-30",
                "review_interval_months": 6,
                "regen_command": ""
              },
              "kind": "pricing",
              "model": "discounting",
              "market": {"kind": "envelope", "envelope": {"schema": "finstack_quant.calibration"}},
              "instrument": {},
              "expected": {"npv": 0.0},
              "tolerances": {"npv": {"abs": 0.0}}
            }"#,
        )
        .expect("write invalid fixture");

        let err = run_golden_at_path(&fixture_path).expect_err("fixture validation should fail");
        std::fs::remove_file(&fixture_path).expect("remove invalid fixture");

        assert!(
            err.contains("schema_version is 'finstack_quant.golden/0'"),
            "unexpected error: {err}"
        );
    }
}
