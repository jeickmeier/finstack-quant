//! SABR volatility-domain golden tests.
//!
//! Mirrors `pricing.rs` but walks `data/market_data/sabr/`. Each fixture
//! carries SABR parameters, a forward, a time-to-expiry, and a list of strikes
//! with per-strike expected implied vols derived independently from the Hagan
//! et al. (2002) formula.

use crate::golden::runner::run_golden_at_path;
use crate::golden::walk::collect_fixture_paths_under;

/// Entry point for all `volatility.sabr` domain fixtures.
///
/// The runner is wired in `runner.rs` → `run_sabr_fixture` in this module.
/// This test discovers every `*.json` under `tests/golden/data/market_data/sabr/`
/// and fails if any fixture does not pass within its stated tolerance.
#[test]
fn golden_sabr_fixtures_from_existing_json_files() {
    let mut paths = collect_fixture_paths_under("market_data/sabr")
        .expect("SABR fixture discovery should succeed");
    if let Ok(filter) = std::env::var("GOLDEN_FIXTURE_FILTER") {
        paths.retain(|path| path.to_string_lossy().contains(&filter));
    }
    assert!(
        !paths.is_empty(),
        "SABR fixture discovery did not find any JSON files under market_data/sabr/"
    );

    let mut failures = Vec::new();
    for path in paths {
        match run_golden_at_path(&path) {
            Ok(results) => {
                failures.extend(
                    results
                        .iter()
                        .filter(|result| !result.passed)
                        .map(|result| result.failure_message(&path.display().to_string())),
                );
            }
            Err(err) => failures.push(format!("run fixture {path:?}: {err}")),
        }
    }

    assert!(
        failures.is_empty(),
        "{} SABR golden fixture failure(s):\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// SABR smile runner: deserialises inputs, builds [`SABRModel`], and returns
/// one implied-vol value per strike keyed by the user-provided string key.
///
/// Called from `runner::run_fixture` when `domain == "volatility.sabr"`.
pub(crate) fn run_sabr_fixture(
    fixture: &crate::golden::schema::GoldenFixture,
) -> Result<std::collections::BTreeMap<String, f64>, String> {
    use finstack_valuations::models::{SABRModel, SABRParameters};

    let inputs = fixture
        .sabr()
        .ok_or("SABR runner requires a 'sabr_smile' fixture body")?;

    let params = if let Some(shift) = inputs.shift {
        SABRParameters::new_with_shift(inputs.alpha, inputs.beta, inputs.nu, inputs.rho, shift)
    } else {
        SABRParameters::new(inputs.alpha, inputs.beta, inputs.nu, inputs.rho)
    }
    .map_err(|err| format!("build SABRParameters: {err}"))?;

    let model = SABRModel::new(params);

    let mut actuals = std::collections::BTreeMap::new();
    for entry in &inputs.strikes {
        let vol = model
            .implied_volatility(inputs.forward, entry.strike, inputs.time_to_expiry)
            .map_err(|err| {
                format!(
                    "SABRModel::implied_volatility for key '{}' (strike={}): {err}",
                    entry.key, entry.strike
                )
            })?;
        actuals.insert(entry.key.clone(), vol);
    }

    Ok(actuals)
}
