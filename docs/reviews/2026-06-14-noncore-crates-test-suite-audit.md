# Finstack Non-Core / Non-Valuations Crates — Test Suite Audit

**Date:** 2026-06-14
**Scope:** Test suites of the 11 workspace crates outside `core` and `valuations` (audited separately on 2026-06-13): `analytics`, `attribution`, `covenants`, `factor-model`, `cashflows`, `scenarios`, `statements`, `monte_carlo`, `portfolio`, `statements-analytics`, `margin` — ~3,360 test functions across inline `#[cfg(test)]` modules and `tests/` integration files.
**Method:** 21 module-group auditors (large crates split by domain so inline + integration tests are read together to catch inline-vs-integration duplication) produced removal candidates and coverage holes. Each unit's findings were then re-checked by an independent adversarial verifier that re-read every removal candidate against the cited code and grepped the *entire* crate test tree before confirming a hole (to reject "already-covered-elsewhere" false positives). Of the audited claims, **23** removals were confirmed, **19** downgraded to consolidate/strengthen, **12** rejected; **147** coverage holes survived (51 High / 73 Medium / 23 Low) and **37** were rejected as already-covered or low-value. (A manual spot-check after synthesis corrected one margin "High bug" false positive — the margin recovery-rate NaN guard is actually safe — and added one genuine High finding in statements-analytics; counts and the margin/statements-analytics sections reflect that correction.)

> This is an **analysis document**. No tests were changed. Every removal cites `path:line`; verify before deleting.

## Executive summary

| Category | Count | Notes |
|---|---|---|
| Clean removals (duplicate / dead / unnecessary) | **23** | Verifier-confirmed; safe to delete |
| Consolidate / strengthen (not a blind delete) | **19** | Verifier-downgraded; keep-one-of-pair or strengthen the survivor |
| Coverage holes — High | **51** | Untested public/error path that can corrupt financial output |
| Coverage holes — Medium | **73** | Untested error branch / edge case / serde-stability invariant |
| Coverage holes — Low | **23** | Nice-to-have wire-format / convention pins |
| False positives caught by verification | **49** | 12 removals + 37 holes rejected; **not** acted on |

### Dominant themes

1. **Inline-vs-integration duplication** — as in the core/valuations audits, the largest clean-removal class: an inline `#[cfg(test)]` test re-asserts exactly what an integration test already covers (portfolio builder validation, portfolio margin aggregation, ECL engine cases). The integration copy is the superset; the inline copy is removable.
2. **Untested `Result::Err` branches are the #1 hole.** In every crate, validation guards — currency mismatch, dimension mismatch, NaN/Inf/zero rejection, inverted ranges, missing curve/metric/version lookups — have happy-path coverage but never trigger their error arms. These guards exist precisely to stop silent NaN / wrong-number propagation.
3. **NaN/Inf edge cases in validators — mostly coverage gaps, one real bug.** *Correction to the automated finding (caught in manual spot-check):* the common `!(lo..=hi).contains(&x)` idiom (used by margin `XvaConfig::validate` recovery rate, analytics `multi_factor_greeks`, the cashflows amortization `pct` check) is **NaN-safe** — NaN is "not contained", so `!contains` is `true` and the guard rejects it (verified: `!(0.0..=1.0).contains(&f64::NAN) == true`). The original workflow mis-flagged the margin recovery-rate check as a "concrete bug"; it is not. The genuinely unsafe idiom is the bare `x < lo || x > hi`, which passes NaN (both comparisons `false`). A repo-wide grep found exactly one **unguarded** instance in scope: `CeclConfig::validate`'s `historical_annual_pd` at [cecl.rs:154](finstack-quant/statements-analytics/src/analysis/ecl/cecl.rs:154) — a NaN PD passes validation (its sibling fields use `.is_finite()`; this one does not). Added as a High finding under statements-analytics below. Everywhere else, the flagged NaN/Inf items are coverage gaps on guards that already correctly reject NaN — add a test, no code change needed.
4. **Serde-stability gaps.** Many types declare `#[serde(deny_unknown_fields)]` (or rely on `serde(default)`) but have no field-name golden or reject-unknown test: analytics result types, attribution envelopes, statements `FinancialModelSpec` + schema-version, margin `VmParameters`/`ImParameters`/`CsaSpec`/collateral schedules, scenario and ECL specs.
5. **Determinism / seed reproducibility is asserted by design, not by test.** Monte-Carlo seed derivation, the portfolio simulation decomposer, covenant stochastic forecasts, and `Performance` metrics all document "same input/seed → identical output" but no test pins bit-identical results.
6. **Derive-only / tautological removables.** A handful of removable tests exercise only enum pattern-matching, a trivial getter, or a hand-constructed value whose asserted field is fixed by the constructor.

---

## Part A & B — Findings by crate

Each crate section lists **Tests to remove** (confirmed clean removals, plus a *Consolidate / strengthen* sub-list of verifier-downgraded near-duplicates) and **Coverage holes** (High → Medium → Low). Only verifier-confirmed or -downgraded items are included; rejected false positives are excluded.

## analytics

Audit of `finstack-quant/analytics` (27 integration tests + ~179 inline tests across 23 source files).

### Tests to remove

#### Consolidate / strengthen (not a blind delete)

- **[unnecessary]** `rust_core_matches_api_invariants_fixture` (`finstack-quant/analytics/src/fixture_test.rs:67`) — This inline `#[cfg(test)]` module validates crate-internal building-block functions (`cagr`, `sharpe`, `sortino`, `value_at_risk`, `expected_shortfall`, `rolling_greeks`, `multi_factor_greeks`) against a JSON golden fixture, none of which are part of the public `Performance` API; integration tests in `performance_smoke.rs` and `correctness_regressions.rs` already exercise the same metrics through the public facade. *Verifier downgrade:* not entirely redundant — `Performance.cagr()`/`sharpe()` delegate to these same `pub(crate)` functions, but the separately-maintained JSON golden file can catch numeric drift the hand-coded integration expectations would miss if updated in sync. Treat as partial overlap, not dead code. *Action:* Rather than blind removal, consolidate the golden-file regression into the public `Performance` path (or strengthen the integration tests to cover the same metrics) before retiring `src/fixture_test.rs` and `api_invariants_data.json`.

### Coverage holes

#### High

- **multi_factor_greeks error handling** — `benchmark.rs:1034 pub(crate) fn multi_factor_greeks(...) -> crate::Result<MultiFactorResult>` — The validation at line 1039 checks `!ann_factor.is_finite() || ann_factor <= 0.0`, but the inline test `standalone_multi_factor_greeks_errors_on_non_positive_ann_factor` (line 1808) only covers `ann_factor = 0.0` and `-252.0`, never `f64::NAN`/`f64::INFINITY`. The non-finite returns path (line 1062) and non-finite factors path (line 1070) have zero coverage — critical for a financial library where malformed factor data must fail fast. suggested: `let result = multi_factor_greeks(&[0.01, 0.02, 0.03, 0.04, 0.05], &[&[0.01, 0.02, 0.03, f64::NAN, 0.05]], 252.0); assert!(result.is_err());` plus NAN-in-returns, and extend the ann_factor test with `f64::NAN` and `f64::INFINITY`.
- **Performance construction validation** — `performance.rs:447 pub fn Performance::new(...) -> crate::Result<Self>` — Code at lines 458–468 validates `ticker_names.len() == prices.len()`, but no integration test in `performance_smoke.rs` or `correctness_regressions.rs` constructs a `Performance` with mismatched ticker count vs price panel width. Line 143 of the smoke test exercises a different error path (benchmark name `'missing'`) with matching dimensions. suggested: in `tests/correctness_regressions.rs`, build with 2 dates, a single price column, and 3 ticker names (`["A","B","C"]`) and assert the constructor returns `Err`.

#### Medium

- **Serde serialization stability** — `BetaResult, GreeksResult, RollingGreeks, MultiFactorResult, DrawdownEpisode, PeriodStats` (crate root) — `serde_roundtrip.rs` round-trips these types (lines 31–135) but never tests unknown-field injection; none of the derives use `#[serde(deny_unknown_fields)]`, so older JSON with extra fields deserializes silently. Medium risk for a serialization-heavy financial library: downstream consumers parsing old JSON could lose data when a field is added. suggested: in `serde_roundtrip.rs`, serialize a `BetaResult`, inject an unknown field (e.g. `"extra_field": 123`), deserialize, assert it is ignored, and add a comment flagging `deny_unknown_fields` for future strict versioning.

#### Low

- **Determinism and reproducibility** — `performance.rs` (all public methods) — No test verifies that a `Performance` object constructed twice with identical inputs yields bitwise-identical metrics. *Verifier downgrade (Medium → Low):* metric calculations use stable Neumaier accumulators, existing `correctness_regressions` tests repeatedly call the same methods and would catch non-determinism, and bitwise reproducibility is typically ensured by harness/CI stability — nice-to-have rather than a real gap. suggested: in `tests/correctness_regressions.rs`, construct the same `Performance` twice, call all metrics (`cagr`, `sharpe`, `drawdown_series`, `beta`, etc.) on both, and assert bitwise (or `1e-14` ULP) equality, with a comment guarding against future summation-order/quantile refactors.

---

## attribution

141 tests across 12 inline `src` modules and 2 integration files (~9233 lines); core methodologies (parallel, waterfall, taylor, metrics-based) well covered, JSON binding surface and the minimal bridge function under-tested.

### Tests to remove

- **unnecessary** `test_parallel_attribution_simple` (`finstack-quant/attribution/src/parallel.rs:1439`) — Manually constructs `PnlAttribution` without calling `attribute_pnl_parallel`; the assertion `residual.amount() == 100.0` is guaranteed by the constructor (`result.rs:416: residual: total_pnl`), and the test comment (lines 459–460) admits it constructs the attribution by hand. No coverage of parallel machinery. *Action:* Remove the inline test; full parallel coverage exists in the integration tests.
- **unnecessary** `test_parallel_attribution_with_curve_change` (`finstack-quant/attribution/src/parallel.rs:1477`) — Creates curves and markets but never calls `attribute_pnl_parallel`; only exercises `MarketSnapshot::extract`/`restore_market` (lines 1500–1505), which is comprehensively covered by `tests/attribution/factors_snapshot.rs` (511 lines, 20 test functions). *Action:* Remove the inline test; snapshot round-trip belongs in `factors_snapshot.rs`.

#### Consolidate / strengthen (not a blind delete)

- **consolidate** `test_default_waterfall_order` / `test_waterfall_requires_order` (`finstack-quant/attribution/src/waterfall.rs:721-755`) — *Downgraded from the duplicate framing:* these two inline tests target distinct concerns — `test_default_waterfall_order` checks `default_waterfall_order()` returns exactly 9 factors starting with `Carry` (pure-function test), while `test_waterfall_requires_order` checks empty-order rejection (error case). Integration tests in `tests/attribution/fx_attribution.rs` (lines 647, 656, 694, 714) cover broader order validation (non-Carry-first, duplicate-factor). They are not true duplicates; confidence is medium. *Action:* Optionally consolidate into one inline test covering default structure + empty-order rejection + non-Carry-first rejection, or move ownership of order validation entirely to `fx_attribution.rs`; do not blind-delete.

### Coverage holes

**High**

- **Untested public API: simple_pnl_bridge** — `simple_pnl_bridge` at `src/lib.rs:448-461` — Top-level entry point computing total P&L without factor attribution; `grep -r 'simple_pnl_bridge' /tests` returns nothing. Error cases (reprice failure at T0/T1, FX conversion failure, currency mismatch) are entirely untested. Suggested: `test_simple_pnl_bridge_basic` (price a fixed-income bond at T0 and T1 with a rate shift, assert returned P&L matches hand-computed `val_t1 - val_t0` in the target currency) plus `test_simple_pnl_bridge_errors_on_missing_fx` (translate to a currency with no FX rate, expect `Error::Validation`).
- **Serde deserialization error handling** — `AttributionSpec::from_json_inputs` / `parse_input_json` at `src/spec.rs:159-177, 186-190` — The inline success-path test `test_attribution_spec_from_json_inputs` (`src/spec.rs:375`) is the only coverage; no tests for malformed instrument JSON, invalid ISO date format, unknown method variant, or missing required market fields, all of which are public binding entry points. Suggested: `test_attribution_spec_from_json_inputs_rejects_malformed_json` passing (1) `instrument={}`, (2) `market_t0='invalid json'`, (3) `as_of_t0='2025-13-45'`, (4) `method='UnknownMethod'`, each expected to error with `Validation` and a message naming the failing field.

**Medium**

- **Untested deserialization rejection: deny_unknown_fields** — `AttributionEnvelope`, `AttributionSpec`, `AttributionConfig`, `AttributionResult`, `AttributionResultEnvelope` (`#[serde(deny_unknown_fields)]`) at `src/spec.rs:30, 65, 110, 231, 256` — All five top-level types declare `deny_unknown_fields`, but no test verifies enforcement; the inline `test_attribution_envelope_json_envelope_trait` (`src/spec.rs:444`) only round-trips valid JSON. Suggested: `test_attribution_spec_rejects_unknown_field_in_json` deserializing valid `AttributionSpec` JSON with an added `unknown_field`, asserting a serde error mentioning "unknown field".
- **Untested error branch: same-day attribution and theta zeroing** — `validate_attribution_period` at `src/helpers.rs:356-407`, `compute_taylor_result` at `src/taylor.rs` — Same-day periods (`as_of_t0 == as_of_t1`) are accepted (tested at `helpers.rs:402-405`), and `taylor.rs:1071-1077` emits a warning and zeroes theta, but no test verifies the warning is emitted, that `theta_pnl` is exactly zero, or that the message contains "Same-day attribution". Suggested: `test_same_day_attribution_zeroes_theta_with_warning` in `src/taylor.rs` calling `attribute_pnl_taylor` with `as_of_t0 == as_of_t1`, asserting `theta_pnl` is zero and `meta.notes` contains "Same-day attribution".
- **Untested credit factor detail computation error recovery** — `compute_credit_factor_detail` / `compute_carry_credit_split_and_decomposition` at `src/execution.rs:170-200` — The execution path (`src/execution.rs:183-190`) catches the case where `credit_factor_model` is present but the instrument has no resolvable issuer, returning `None` and appending a note; no test supplies a `credit_factor_model` with an issuer-less instrument to verify the diagnostic note and `credit_factor_detail == None`. Suggested: integration test `test_execution_spec_credit_detail_graceful_degradation` asserting `attribution.credit_factor_detail` is `None` and `meta.notes` contains "no resolvable issuer".

**Low**

- **Untested serde skip_serializing_if field behaviors** — `PnlAttribution` and related types (`src/types/result.rs`, `detail.rs`) with `#[serde(skip_serializing_if = "Option::is_none")]` and custom defaults — The inline `test_attribution_config_optional_fields` (`src/spec.rs:353`) verifies `None` fields are omitted on serialization, but no test verifies deserializing JSON with a missing optional field reconstructs the default. Suggested: `test_attribution_config_deserialize_missing_optional_field` constructing valid `AttributionConfig` JSON without the `metrics` field, asserting `config.metrics == None`.

---

## covenants

Scope: covenants crate test suite (`tests/integration.rs`, `engine_conventions.rs`, `serialization.rs`, `src/forward.rs`); audit flagged 8 removals and 10 coverage holes.

### Tests to remove

- **[unnecessary]** threshold_test_maximum (`finstack-quant/covenants/tests/integration.rs:329`) — Pure enum variant pattern matching with no invariant checking; `ThresholdTest::Maximum(5.0)` is constructed and matched, with identical structural coverage from the serde roundtrip. *duplicate_of:* `threshold_test_roundtrip` (`serialization.rs:74`). *Action:* Delete; enum variant structure is validated by serde roundtrips.
- **[unnecessary]** threshold_test_minimum (`finstack-quant/covenants/tests/integration.rs:338`) — Symmetric to `threshold_test_maximum`; pure pattern matching with no assertion beyond variant equality. *duplicate_of:* `threshold_test_roundtrip` (`serialization.rs:74`). *Action:* Delete; covered implicitly by the roundtrip test.

#### Consolidate / strengthen (not a blind delete)

- **[unnecessary]** max_covenant_type_pass_fail_logic (`finstack-quant/covenants/tests/integration.rs:198`) — Pure `actual <= threshold` comparison with no engine involvement; comparison semantics are reached end-to-end via engine integration tests. *Verifier downgraded:* this is an explicit contract test for the comparison operators including the `actual == threshold` boundary, so fold the edge-case assertions into an engine-path test rather than dropping them outright. *Action:* Consolidate the IEEE edge-case coverage into an `engine.evaluate()` test before removing.
- **[unnecessary]** min_covenant_type_pass_fail_logic (`finstack-quant/covenants/tests/integration.rs:225`) — Symmetric `actual >= threshold` comparison with independent edge-case assertions. *Verifier downgraded:* same as the max variant — preserve the `>=` boundary semantics via an engine test before deletion. *Action:* Consolidate edge-case assertions into engine evaluation coverage, then remove.
- **[unnecessary]** test_covenant_report_smoke (`finstack-quant/covenants/tests/integration.rs:21`) — Tests the `CovenantReport` fluent API (`.failed().with_actual().with_threshold()`) with no assertions beyond the boolean field. *Verifier downgraded:* `covenant_report_failed_with_negative_headroom` (line 45) only validates a failed report with headroom and `covenant_report_passed_with_all_fields` (line 30) exercises the `.passed` path; removing the smoke test leaves the immediate `.passed=false` boolean state from `CovenantReport::failed()` unasserted. *Action:* Migrate the `.failed()` boolean-state assertion into an existing failed-report test, then remove.
- **[unnecessary]** headroom_calculation_max_covenant (`finstack-quant/covenants/tests/integration.rs:59`) — Tests `(threshold - actual) / threshold` arithmetic in isolation; headroom is exercised end-to-end by engine evaluation tests. *Verifier downgraded:* `engine_conventions.rs:196` (`relative_headroom_keeps_sign_for_negative_threshold`) covers the negative-threshold sign convention but not this positive-threshold formula, so the explicit formula test still guards against regressions. *Action:* Retain or fold into an engine evaluation assertion rather than deleting blindly.
- **[unnecessary]** headroom_calculation_min_covenant (`finstack-quant/covenants/tests/integration.rs:82`) — Tests `(actual - threshold) / threshold`, the min-covenant formula inversion. *Verifier downgraded:* the sign-convention test does not cover this specific numerator swap for minimum covenants, so the formula assertion remains valuable. *Action:* Fold the inverted-formula assertion into engine evaluation coverage before removing.

### Coverage holes

#### High

- **Error handling: missing metrics** — `CovenantEngine::evaluate` (`finstack_quant_covenants/src/engine.rs:811`) — When a required metric is absent from the `CovenantMetricSource`, `get_metric()` (`engine.rs:1264`) returns `InputError::NotFound`, but no test exercises this path; all engine tests pre-populate `HashMapMetricSource` with every required metric. Suggested: `test_evaluate_with_missing_metric` — add a covenant requiring `debt_to_ebitda`, call `evaluate()` with a source containing only `interest_coverage`, assert `error.kind` is `NotFound` and the message names the missing metric.
- **Evaluation: untested covenant types** — `CovenantEngine::evaluate` for `MinAssetCoverage`, `MinLiquidity`, `MaxNetDebtToEBITDA`, `MaxCapex` (`src/engine.rs:811`) — Only `MinDSCR` is functionally evaluated; `MinAssetCoverage` appears solely in `serialization.rs:90` for serde roundtrip, and `MinLiquidity`/`MaxNetDebtToEBITDA`/`MaxCapex` appear in no test file, leaving pass/fail/headroom logic unverified. Suggested: `test_evaluate_min_asset_coverage` — build engine with `MinAssetCoverage { threshold: 1.2 }`, evaluate with `asset_coverage=1.5` (pass) and `0.9` (fail), assert status, actual, and headroom direction match the `>=` comparison; repeat for `MinLiquidity`, `MaxNetDebtToEBITDA`, `MaxCapex`.

#### Medium

- **Evaluation: negative-valued metrics and sign conventions** — `headroom_for`, `is_covenant_breached` (`src/engine.rs:1354`, `1373`) — *Verifier downgraded High→Medium:* NaN is covered by `forecast_breaches_generic_reports_nan_periods_as_breaches` (`forward.rs:784`/`801`), but `f64::INFINITY`/`NEG_INFINITY` metrics and thresholds are untested; `headroom_for` returns NaN for non-finite inputs (line 1356) yet is never exercised with infinity. Suggested: `test_evaluate_with_infinite_metric` — evaluate `MaxDebtToEBITDA` with metric=`f64::INFINITY`, threshold=5.0 (assert breached); evaluate `MinInterestCoverage` with metric=`f64::NEG_INFINITY`, threshold=1.5 (assert pass since `-inf < 1.5` does not breach a min covenant the wrong way); confirm headroom is NaN for both.
- **Validation: waiver constraints** — `CovenantEngine::validate` (`src/engine.rs:746-764`) — *Verifier downgraded High→Medium:* `engine_conventions.rs:329` covers overlapping windows and negative cure periods but not waiver error cases (`expiry_date < effective_date` or `amended_threshold` non-finite). Suggested: `test_validate_waiver_errors` — (1) `add_waiver` with expiry < effective, assert a `Validation` error about expiry < effective; (2) `add_waiver` with `amended_threshold=f64::NAN`, assert a `Validation` error about a finite threshold.
- **Validation: window constraints** — `CovenantEngine::validate` (`src/engine.rs:714-743`) — *Verifier downgraded:* `engine_conventions.rs:329` covers overlapping windows (lines 345-363) only; `start > end` (lines 714-718) and duplicate windows (lines 737-744) are untested. Suggested: `test_validate_window_errors` — (1) `add_window` with start > end, assert a `Validation` error about start > end; (2) add two identical windows `[2025-01-01, 2025-06-30]`, assert a `Validation` error about a duplicate window.
- **Evaluation scope filtering** — `CovenantEngine::evaluate_for_trigger` (`src/engine.rs:931`) — Confirmed untested; no test calls `evaluate_for_trigger`. `integration.rs:277` (`covenant_scope_maintenance_vs_incurrence`) validates the scope field but not trigger-based filtering. Suggested: `test_evaluate_for_trigger_maintenance_filters_incurrence` — add one Maintenance and one Incurrence covenant, call `evaluate_for_trigger(..., EvaluationTrigger::Maintenance)`, assert only the Maintenance covenant appears in reports; repeat for the Incurrence trigger.
- **Error handling: missing covenant spec in consequence application** — `CovenantEngine::apply_consequences` (`src/engine.rs:1081-1087`) — Confirmed untested; the `ok_or(InputError::NotFound)` on line 1085 fires only for an orphaned breach, but all `apply_consequences` callers (`engine_conventions.rs:102`, `273`, `321`) build breaches from evaluated covenants so the spec always exists. Suggested: `test_apply_consequences_missing_spec` — push a `CovenantBreach` with `covenant_id='orphaned'` (no matching spec) into `breach_history`, call `apply_consequences()` on it, assert `InputError::NotFound`.

#### Low

- **Determinism: random seed reproducibility in forecasts** — `forecast_covenant_generic` (`src/forward.rs:620+`), `CovenantForecastConfig.random_seed` — *Verifier downgraded (confirmed Low):* the `random_seed` field roundtrips in `serialization.rs:364` but no test runs two forecasts with identical config and asserts equal breach probabilities. Suggested: `test_stochastic_forecast_seed_reproducibility` — run `forecast_covenant_generic()` twice with the same config (seed=42, num_paths=1000) and assert the `breach_probability` vectors are equal element-by-element.

---

## factor-model

Scope: error variants, covariance fallbacks, sensitivity-matrix bounds, and serde/round-trip coverage across `error.rs`, `covariance.rs`, `sensitivity_matrix.rs`, `config.rs`, and the matching/primitives modules.

### Tests to remove

_None found.*

### Coverage holes

**High**

- **FactorModelError::UnmatchedDependency** — `finstack-quant/factor-model/src/error.rs:12` — The UnmatchedDependency variant is defined but never instantiated in tests, so error rendering and serialization cannot be verified. This is a public error variant that callers may encounter; untested Display output and error handling in workflows. *(Verifier note: actively used in `finstack-quant/portfolio/src/factor_model/model.rs:255`, so it is exercised in integration context — but no dedicated Display/serialization assertion exists.)* suggested: Create a test in src/error.rs that constructs UnmatchedDependency with a real MarketDependency and verifies the error message contains both position_id and dependency details.

**Medium**

- **FactorModelError::InvalidCovariance** — `finstack-quant/factor-model/src/error.rs:25` — The InvalidCovariance variant is never tested. Callers relying on this error to detect covariance problems have no assurance the message is correct. *(Downgraded from High: Display impl is mechanically correct via thiserror and no upstream caller was found constructing it, so production risk is limited absent proof of active use.)* suggested: Test that InvalidCovariance displays a clear reason string, e.g., InvalidCovariance { reason: "matrix is singular".into() }.
- **FactorModelError::RepricingFailed** — `finstack-quant/factor-model/src/error.rs:31` — The RepricingFailed variant wraps a boxed error; the error chain and Display output are untested, meaning financial workflows that fail repricing may not surface diagnostics correctly. *(Downgraded from High: thiserror guarantees Display includes all fields and no construction site was found in either factor-model or portfolio, suggesting a vestigial or future-only path.)* suggested: Test RepricingFailed with a nested error (e.g., std::io::Error), verify Display includes position_id, factor_id, and the source message.
- **FactorModelError::AmbiguousMatch** — `finstack-quant/factor-model/src/error.rs:42` — The AmbiguousMatch variant is never tested. When matching produces multiple candidates, the error output is untested. *(Downgraded from High: Display impl is guaranteed correct by the thiserror macro and no active use was found, suggesting future-proofing or a removed code path.)* suggested: Test AmbiguousMatch with a non-empty candidates list, verify the display includes position_id and all candidate factor IDs.
- **FactorCovarianceMatrix::variance unknown factor fallback** — `finstack-quant/factor-model/src/covariance.rs:104` — The variance() method silently returns 0.0 for unknown factors. This is documented but untested—a caller querying a typo'd factor ID gets zero risk with no warning, which is a silent bug risk in financial computations. suggested: Test that variance() returns exactly 0.0 for a factor not in the matrix (e.g., query 'Unknown' when only 'Rates' and 'Credit' exist). Verify this is deliberate vs. accidental.
- **FactorCovarianceMatrix::covariance unknown factor fallback** — `finstack-quant/factor-model/src/covariance.rs:113` — Like variance(), covariance() returns 0.0 for unknown factors. Portfolio variance computations silently treat missing factors as uncorrelated, masking data errors. suggested: Test that covariance() returns 0.0 when either factor is unknown. Consider whether this silent zero is the right default or if an error should be raised.
- **SensitivityMatrix panic bounds checking** — `finstack-quant/factor-model/src/sensitivity_matrix.rs:63` — The delta() and set_delta() methods use hard asserts for bounds checking (not debug_assert). The panic paths themselves are never explicitly tested to verify the assert messages are correct. suggested: Add a test that calls delta() with out-of-bounds indices and verifies the panic message contains the index values and bounds, confirming hard asserts work as intended.

**Low**

- **FactorCovarianceMatrix::correlation with zero variance** — `finstack-quant/factor-model/src/covariance.rs:125` — The correlation() method returns 0.0 if either factor has zero or negative variance. This edge case is untested; financial models may rely on correlation definitions in degenerate cases. *(Downgraded from Medium: the proptest at covariance.rs:269/194-209 exercises correlation() broadly and the zero-variance path is a single trivial conditional, so isolated coverage is valuable but not critical.)* suggested: Test correlation() when one or both factors have exactly 0.0 variance; verify 0.0 is returned and verify behavior is stable across multiple calls.

---

## cashflows

Two units audited: `cashflows-builder` (integration tests under `tests/cashflows/builder/`) and `cashflows-core` (44 inline tests across `aggregation.rs`, `accrual.rs`, `traits.rs`). 2 confirmed removals; 16 coverage holes after verifier adjudication.

### Tests to remove

- **[duplicate]** test_notional_par (`finstack-quant/cashflows/tests/cashflows/builder/amortization.rs:8`) — Pure tautology: only verifies `Notional::par()` wraps its amount/currency into the `initial` field and defaults `amort` to `None`. No validation or logic; every integration test in the file (e.g. `test_amortization_spec_none_validation` at line 23) implicitly exercises `par()`. *Action:* Remove; coverage is preserved by integration construction paths.
- **[duplicate]** test_notional_currency (`finstack-quant/cashflows/tests/cashflows/builder/amortization.rs:17`) — Getter-only tautology: calls `Notional::par()` then asserts `.currency()` (a one-liner returning `self.initial.currency()`) returns the value it was initialized with. Exercised implicitly by every amortization validation test (lines 47, 56). *Action:* Remove; duplicate_of `test_notional_par`.

### Coverage holes

#### High

- **Recovery rate validation in credit-adjusted PV** — `pv_by_period_credit_adjusted_detailed_with_timing:675` — Recovery rate is validated to `[0.0, 1.0]` (lines 685-690, `InputError::Invalid`) but no inline test exercises the rejection paths; the only indirect coverage runs through `DefaultEvent::validate()` in `credit_models.rs`, a separate check. suggested: Call `pv_by_period_credit_adjusted_detailed_with_timing` with `Some(-0.1)` and `Some(1.5)` recovery rates; assert both return `Error::Input(Invalid)`.
- **Survival probability non-finite check in PV aggregation** — `pv_by_period_cashflows_sorted_checked:416` — The finite-survival guard in `time_discount_survival` (lines 596-600) is untested; the existing `pv_by_period_errors_on_nan_discount_curve` test only covers the NaN discount-curve path, never a Survival curve returning NaN/∞. suggested: Mock a Survival curve that returns NaN for positive `t`, call `pv_by_period_cashflows_sorted_checked` with a future flow, and assert a Validation error containing 'non-finite'.
- **Credit adjustment without hazard curve when recovery_rate is supplied** — `pv_by_period_credit_adjusted_detailed_with_timing:675` — Lines 710-714 require `hazard=Some()` when `recovery_rate=Some()`, but existing tests always pass `Some(&hazard)`; the `hazard=None` rejection path is untested. suggested: Call with `recovery_rate=Some(0.5)` and `hazard=None`; assert `Error::Input(NotFound)` with id containing 'hazard'.

#### Medium

- **Floating rate error branches** — `project_floating_rate_from_market()` at `rate_helpers.rs:519` — Inline tests (`rate_helpers.rs:748-914`) cover successful projections only; the curve-not-found path (`market.get_forward(index_id)?`) is not directly tested. Verifier downgraded from Medium-as-claimed: the fallback-error case is covered by `floating_rate.rs:135` (`test_floating_rate_fallback_error_no_curve`); direct curve-not-found testing remains thin. suggested: Call `project_floating_rate_from_market` with an `index_id` not in the `MarketContext`; verify the error propagates rather than silently falling back. Add a `ForwardCurve` returning NaN from `forward_rate()`.
- **Serde validation - amortization specs** — `AmortizationSpec` enum at `src/builder/specs/amortization.rs:17` — `deny_unknown_fields` is present on `AmortizationSpec` (line 16) and `Notional` (line 56), and the pattern is demonstrated by `floating_rate_spec_rejects_unknown_fields` (`tests/cashflows/schedule.rs:436`), but no test injects a typo'd field into an `AmortizationSpec` variant specifically. Verifier downgraded: serde rejection works globally and roundtrips are covered (`schema_roundtrip.rs:279-287`); spec-specific typo tests are thin. suggested: Serialize a valid `AmortizationSpec::StepRemaining`, inject a typo (`schedule`→`shedule`), and assert deserialization errors; repeat for `LinearTo`, `PercentOfOriginalPerPeriod`, `CustomPrincipal`, and `None`.
- **Determinism & reproducibility - seed/curve idempotence** — `CashFlowSchedule::build_with_curves()` at `orchestrator.rs` — No test calls `build_with_curves()` twice with identical inputs to assert byte-identical output. Verifier downgraded: determinism is enforced indirectly via `linear_vs_step_parity` (`tests/cashflows/builder/schedule.rs:40`, flow-by-flow assertion at lines 96-106) and golden-value tests (`amortization.rs:384, 485, 544, 860`); a pure same-input-twice idempotence test is still absent. suggested: Hardcode a 5% semi-annual 1M bullet, call `build_with_curves()` twice, and assert the flow lists are deep-equal on dates/amounts/kinds/rates with the expected sequence pinned as a golden value.
- **ExCouponRule calendar resolution error** — `ExCouponRule::ex_date:178` — `ex_date` calls `calendar_by_id` (line 192) and returns `InputError::NotFound` for an unknown calendar; the existing `ex_coupon_rule_rejects_days_above_366` test always uses `calendar_id=None`. suggested: Build `ExCouponRule` with `calendar_id='nonexistent_calendar'`, call `ex_date()`, and assert `Error::Input(NotFound)` with the calendar ID in the message.
- **Empty flows handling in aggregate_cashflows_checked** — `aggregate_cashflows_checked:293` — Documented to return `Ok(Money::new(0.0, target))` for empty flows, but integration tests (`test_single_currency_aggregation`, `test_cross_currency_aggregation_error`) only use 2-flow inputs. suggested: Call `aggregate_cashflows_checked(&[], Currency::USD)` and assert `Ok(Money::new(0.0, Currency::USD))`.
- **Empty flows/periods in aggregate_by_period** — `aggregate_by_period:232` — Lines 237-239 early-return `Ok(IndexMap::new())` for empty flows or periods, but no inline test in `aggregation.rs` exercises these paths. Verifier kept at Medium (trivial early return; broader crate integration tests may cover it). suggested: Add `aggregate_by_period(&[], periods)` and `aggregate_by_period(flows, &[])`, each asserting `Ok(empty map)`.
- **Accrued interest boundary cases** — `accrued_interest_amount:286` — Documented to return 0.0 when `as_of` is outside all coupon periods; inline tests cover ex-coupon windows and mid-period accrual but never `as_of` before the first or after the last coupon period. suggested: With coupons on `[2025-07-01, 2026-01-01]`, call `accrued_interest_amount` with `as_of=2025-06-01` (before first) and `as_of=2026-02-01` (after last); assert both return 0.0.
- **Credit-adjusted PV function tested only indirectly** — `credit_adjusted_period_pv:501` — This `pub(crate)` core formula (lines 501-534) branches on `CFKind` and applies the recovery term `r*(1-sp)` only to `Amortization`/`Notional`/`PrePayment`; it is reached only via a closure in `pv_by_period_credit_adjusted_detailed_with_timing` (line 721), never directly unit-tested. suggested: Call `credit_adjusted_period_pv` directly with known `(df, sp, recovery_rate, CFKind)` inputs across variants (Amortization, Fixed, Recovery, DefaultedNotional) and assert hand-computed PVs.

#### Low

- **Day count convention stability - Act/Act edge cases** — `date_generation::build_dates()` and coupon emission with `DayCount::ActActIsma` — Tests lean on Act365F and Act360; no full-schedule test verifies `ActActIsma` accrual factors. Verifier downgraded to Low: a dedicated `tests/cashflows/day_count.rs` module exists and likely covers the convention; a canonical ISDA-example bond test would be nice-to-have, not critical. suggested: Build a semi-annual `ActActIsma` schedule, verify coupon accrual factors sum to exactly 1.0 over the period, and pin a specific coupon amount against a known ISDA example.

---

## scenarios

Audit of `finstack-quant/scenarios` adapters (time_roll, asset_corr, statements, fx, vol, curves) and engine/templates layers; covers inline + integration tests across both units, yielding 4 confirmed-or-downgraded removals and 11 surviving coverage holes.

### Tests to remove

- **unnecessary** test_builder_validation_empty_id (`finstack-quant/scenarios/src/templates/builder.rs:630`) — `build()` (line 291) delegates to `spec.validate()`, already covered comprehensively by `spec_validation_test.rs:10-26` (`scenario_validate_rejects_empty_id`); the builder test adds no coverage beyond verifying the fluent API calls `validate()`. *Action:* Remove; spec-level validation is the authoritative test.
- **unnecessary** test_asset_class_set_coverage (`finstack-quant/scenarios/src/templates/metadata.rs:174`) — manually builds a `HashSet` of all six `AssetClass` variants and asserts membership, which `derive(Copy, Eq, Hash)` guarantees automatically; real serde/filtering contract is covered by metadata serde roundtrips (lines 103-122) and `templates_integration.rs`. *Action:* Remove; manual enum-membership checks add no financial or serde-contract value.

#### Consolidate / strengthen (not a blind delete)

- **consolidate** test_builder_compose_preserves_resolution_mode (`finstack-quant/scenarios/src/templates/builder.rs:412`) — verifier downgraded the blind-delete: the inline test (lines 413-422) exercises `ScenarioSpecBuilder::compose()` while the integration test at `hierarchy_targeting.rs:516-541` exercises `ScenarioEngine::try_compose()`, so they cover complementary API surfaces rather than duplicating. *Action:* Keep both, OR strengthen the builder test to cover the mixed-mode error case before removing.

### Coverage holes

#### High

- **statements adapter: rate conversion** — `convert_continuous_rate` at `statements.rs:210` — validates `year_fraction` is finite and positive (line 215), returning `Error::Validation`; no test in the crate exercises the zero/negative/NaN rejection path on this financial calculation. suggested: Test `apply_forecast_assign`/`update_rate_from_binding` with `accrual_years ≤ 0` or NaN (e.g., forward curve with zero/negative tenor) raises a Validation error with appropriate message.
- **Serde stability / schema evolution** — `spec.rs::ScenarioSpec`, `OperationSpec` (all variants) — `serde_roundtrip_test.rs:111-121` tests `deny_unknown_fields` rejection, but no test verifies (1) adding a new optional field to existing JSON round-trips safely, or (2) built-in template JSON files re-serialize bit-for-bit; forward-compatibility for FFI callers deserializing legacy-tool JSON is untested. suggested: Add a test that round-trips a scenario JSON with an extra unknown field (lax vs strict outcome) and verifies built-in template JSON parses and re-serializes deterministically.

#### Medium

- **curves adapter: discount curve resolution heuristic** — `resolve_discount_curve_id` at `curves.rs:224` — verifier downgraded from High: the ambiguity path (lines 256-259, multiple matching CCY-prefix curves) and no-discount-curves path (lines 241-244) are real untested error paths, but the no-curves case is unlikely in practice and ambiguity only triggers under non-standard naming. suggested: Test `curve_parallel_effects(CurveKind::ParCDS, …, discount_curve_id=None)` with a market containing two USD discount curves (`USD-OIS`, `USD-SOFR`) raises a Validation error about ambiguity.
- **vol adapter: bucket tenor snapping** — `snap_to_grid_expiry` at `vol.rs:213` — verifier confirmed (severity unchanged at Medium): the function (lines 213-231) errors when no grid expiry is within `GRID_EXPIRY_SNAP_TOLERANCE_YEARS`; inline test `test_bucket_shock_warns_on_post_bump_arbitrage` (line 426) uses a valid `6M` tenor, leaving the out-of-tolerance path untested. suggested: Test `vol_bucket_effects` with tenor `11.5Y` on a surface with expiries `[0.5, 1.0, 5.0, 10.0]` fails with a tenor-snap error.
- **Financial correctness / non-finite handling** — `adapters/curves.rs`, `adapters/equity.rs`, `adapters/fx.rs` (via engine dispatch) — verifier downgraded from High: `spec_validation_test.rs:220-264` rejects NaN pct values at the spec layer, but no test inserts NaN/Inf into market data (e.g., a NaN discount factor) and applies a scenario to verify the adapter detects/fails or warns rather than silently corrupting downstream P&L. suggested: Insert a `DiscountCurve` or equity price with NaN/Inf, apply a scenario, and verify the operation errors or emits a warning rather than propagating silently.
- **Determinism / parallel consistency** — `engine.rs::apply()` line 885 (Phase 1 batching) — verifier confirmed: `engine_edge_cases_test.rs:48-95` verifies last-wins composition on a single equity, but no test verifies (1) curve bumps in order A→B vs B→A produce identical state, (2) `would_conflict_with_pending()` triggers flushing, or (3) Phase 1 batching ensures determinism across schedules. suggested: Add tests applying two curve parallel bumps on different curves in both orders (commutative) and sequential bumps on the same curve (verify `(1+a)*(1+b)` composition and conflict-triggered flushing).
- **Public API coverage** — `horizon.rs::HorizonResult::factor_contribution` (line 341) — verifier confirmed: the method is used only once inline (line 699) to verify NaN on currency mismatch; no test verifies each `AttributionFactor` variant returns the correct contribution or that the sum of all factor contributions equals `total_return_pct()` within epsilon (the attribution invariant). suggested: Construct a `HorizonResult` with known attribution values, assert `factor_contribution()` per factor variant, and assert the factors sum to `total_return_pct()` within epsilon.

#### Low

- **Edge case / zero / inverted ranges** — `utils.rs::parse_tenor_to_years` (line 56), `parse_period_to_days` (line 221) — verifier confirmed: tests (lines 338-393) cover valid tenors and invalid inputs but not `0Y`/`0D`/`0M` (should yield 0), negative tenors like `-1Y` (should reject), or large values like `999Y`; boundary behavior is unpinned and matters for time-roll no-ops. suggested: Assert `parse_tenor_to_years('0Y') == 0.0`, `parse_period_to_days('0D') == 0`, that `-1Y` is rejected/handled, that `999Y` parses to 999.0, and that a `0D` time-roll applies as a no-op.

---

## statements

Audit of the `statements` crate test suite across three units (evaluator, forecast/capital-structure, registry/integration), spanning unit, integration, and serde-stability coverage.

### Tests to remove

**Clean removals (verdict: confirmed):**

- **consolidate** test_circular_dependency_detection (finstack-quant/statements/tests/evaluator_tests.rs:342) — duplicate_of finstack-quant/statements/src/evaluator/dag.rs:368. Both build the same 3-node cycle (a→b→c→a) and assert `build()` errors; the integration test additionally asserts the `Error::CircularDependency` variant while the dag unit test only checks the message string. *Action:* Strengthen test_cycle_detection in dag.rs to also assert the `CircularDependency` variant, then delete the integration test as it adds no integration-level value.

**Consolidate / strengthen (not a blind delete):**

- **duplicate** test_context_set_and_get_value (finstack-quant/statements/tests/evaluator_tests.rs:11) — duplicate_of finstack-quant/statements/src/evaluator/context.rs:410. The integration test sets two nodes (revenue 100k, cogs 60k) vs. the inline test's one node; verifier confirms they are similar-but-not-identical, with the integration variant adding minimal value beyond verifying IndexMap holds 2 entries instead of 1. *Action:* Keep the inline unit test in context.rs; delete the integration test, which adds no new assertions or edge-case coverage.

- **consolidate** inline tests in adjustments/engine.rs (test_fixed_adjustment, test_percentage_adjustment, test_capped_adjustment) (finstack-quant/statements/src/adjustments/engine.rs:199-315) — share an identical local `mock_results()` setup. Verifier notes these directly test private `NormalizationEngine` methods in isolation and should NOT be moved to integration tests (that would lose direct-engine coverage). *Action:* Keep the tests as-is; optionally extract `mock_results()` into a shared test helper to reduce setup duplication.

- **dead** test_converts_statements_errors_to_core_error (finstack-quant/statements/src/error.rs:204) — covers only `Error::InvalidInput → Validation`, while `impl From<Error>` handles 6 variants. Verifier downgrades from dead to incomplete: the test does verify the conversion happy path and should be expanded, not removed. *Action:* Expand to cover the remaining variants (CurrencyMismatch → CurrencyMismatch, Serde → Internal, Io → Internal, etc.) rather than deleting.

### Coverage holes

**High**

- **Capital Structure: multi-currency aggregate cashflows without FX** — src/capital_structure/cashflows.rs:370 get_total_interest — the existing error test (cashflows.rs:647) inserts USD and EUR keys into `totals_by_currency` but both maps are empty (no period values), so the currency-mismatch guard is never exercised against real data. suggested: Build two bonds in different currencies (USD, EUR), run `aggregate_instrument_cashflows` without `reporting_currency`, and confirm `get_total_interest()` and `get_total_debt_balance()` return `Err` citing the currency mismatch rather than silently defaulting.

- **Override forecast: period outside forecast_periods range** — src/forecast/override_method.rs:16 apply_override — containment is validated at lines 54-58, but the only integration test (forecast_tests.rs:175-223) covers happy-path overrides within range; no test exercises an out-of-range override through the full evaluator stack. suggested: Build a model with periods 2025Q1..Q4, apply `ForecastMethod::Override` for '2026Q1', and verify the evaluator returns an error mentioning the period mismatch rather than a silent ignore.

- **Seasonal forecast: insufficient historical data** — src/forecast/timeseries.rs:377 seasonal_forecast — the `hist_data.len() >= season_length * 2` check is only tested inline for zero season_length (timeseries.rs:893); no integration test covers the common insufficient-history case through ModelBuilder→Evaluator. suggested: Build a seasonal-forecast model with historical=['100','90','110'] (3 points) and season_length=4 and verify the evaluator raises the 2-season-minimum error rather than silently degrading to a default.

- **Serde stability: FinancialModelSpec deny_unknown_fields** — FinancialModelSpec::deserialize (types/model.rs:24-25) — derives `#[serde(deny_unknown_fields)]` but no test verifies unknown JSON fields are rejected; a typo'd field name would silently drop instead of failing loudly. suggested: Add test_reject_unknown_field_in_model_spec deserializing `{"id":"test","periods":[],"nodes":{},"unknown_field":123}` and assert it returns `Err` containing "unknown field".

- **Schema versioning: validate_schema_version rejection** — validate_schema_version (types/model.rs:239-246) — rejects version 0 and version > CURRENT_SCHEMA_VERSION=2, but no test exercises the rejection (only the default value 2 is checked at builder_tests.rs:262). suggested: Deserialize a FinancialModelSpec with `schema_version: 3` and assert `Err` containing "unsupported"; also test `schema_version: 0`.

**Medium**

- **Decimal conversion boundary (overflow)** — EvaluationContext::get_value_decimal (context.rs:241-260) — the `is_finite()` rejection path is covered (context.rs:470), but the `Decimal::try_from(f64)` error branch (lines 250-256) is untested; a finite f64 exceeding Decimal's ~10^28 range would fail unverified. (Verifier downgraded from High to Medium-High: path exists and is structurally sound; the gap is the explicit overflow case.) suggested: Add test_get_value_decimal_rejects_overflow setting a value beyond Decimal range and assert an error containing 'conversion'/'range', with a well-formed message including node_id and period.

- **DAG dependency extraction for capital structure** — dag::extract_dependencies via DependencyGraph::from_model (dag.rs:396-400) — CS refs parse and evaluate correctly (capital_structure_dsl_tests.rs:22-36, term_loan integration tests), but no test verifies `extract_dependencies()` extracts CS-prefixed refs as dependencies or that the DAG correctly orders/cycle-checks CS-dependent nodes. suggested: Add test_extract_dependencies_capital_structure parsing 'lag(__cs__debt_balance__total, 1) + revenue' and verify `{'__cs__debt_balance__total','revenue'}` is returned and DAG ordering is correct.

- **Prepare method error handling** — Evaluator::prepare (engine.rs:368-376) — only the happy path is tested (evaluator_tests.rs:1521); `prepare()` calls `compile_formulas()` whose error propagation (invalid formula, undefined ref, compile-time cycle) is untested. suggested: Add test_prepare_rejects_invalid_formula building `compute('x', 'undefined_node + 1')`, call `prepare()`, and assert the error mentions 'undefined_node'/'compile' and is not swallowed by caching.

- **EvalWarning enum exhaustiveness** — EvalWarning enum (results.rs:328) — has four variants (DivisionByZero, NaNPropagated, NonFiniteValue, CapitalStructureCashflowIgnored) but only DivisionByZero is explicitly tested (evaluator_engine.rs:155); a new variant could ship untested. (Verifier downgraded from High to Medium.) suggested: Add a parameterized test that triggers each variant and verifies it appears in `results.meta.warnings` with correct node_id and period.

- **Serde stability: result types schema evolution** — ResultsMeta, MonteCarloResults, EvalWarning (results.rs:87-133) — `MonteCarloConfig` has a deny_unknown_fields test (monte_carlo.rs:572-575) but the main result types do not; they rely on `serde(default)`. (Verifier downgraded from High to Low-Medium: `serde(default)` is a safe pattern and persistence across versions is unproven.) suggested: Add test_eval_warning_serde_rejects_unknown_fields deserializing JSON with an unexpected field and assert failure; repeat for ResultsMeta where applicable.

- **Forecast: non-finite base_value across methods** — src/forecast/deterministic.rs:39 growth_pct, src/forecast/override_method.rs:16 apply_override — `is_finite(base_value)` is validated in code (deterministic.rs:51,113; override_method.rs:29-34) but no integration test feeds NaN/Inf through `.value()` + `.forecast()`. suggested: Build a model with `.value('revenue', NaN)` then `.forecast('revenue', ForecastMethod::GrowthPct, ...)` and verify the evaluator returns a non-finite-input error rather than silently propagating NaN.

- **Capital structure: waterfall sweep with zero/negative ECF** — src/capital_structure/waterfall/excess_cash_flow.rs calculate_ecf_sweep — waterfall_tests.rs:15-106 covers only a positive-ECF scenario; ECF=0 (operating outflow) and ECF<0 (sweep floors to zero) corner cases are untested. suggested: Build a capital structure with an ECF sweep spec, set EBITDA/CapEx/Taxes so ECF=0 and verify zero additional prepayment (not NaN, not negative); also test ECF<0 flooring to zero.

- **Forecast: seed overflow and extreme parameter validation** — src/forecast/statistical.rs:106 extract_distribution_params, statistical.rs:60 parse_seed_json — bounds (`f > u64::MAX as f64`) and `fract==0.0` are checked with only one happy inline test (statistical.rs:559); no integration test feeds NaN, overflow (18446744073709551616.0), or fractional seeds through the stack. suggested: Build a model with `ForecastMethod::Normal` and seed=f64::NAN or an overflow seed and verify the evaluator returns an error rather than a forecast with a mangled seed.

- **Validation: registry JSON → validate_metric_definition pipeline** — validate_metric_definition (registry/validation.rs:19-65) — inline tests (validation.rs:67-127) call the function directly, and registry_tests.rs:181-213 partially covers invalid formulas via `load_from_json_str`, but the JSON→deserialize→validation path is not exercised for empty/invalid IDs and empty formulas. suggested: Call `registry.load_from_json_str()` with metrics having (1) empty ID, (2) invalid ID chars ('metric.id'), (3) empty formula, (4) unparseable formula ('a + + b') and assert each returns `Err`.

- **Error handling: From<Error> variant coverage** — From<Error> for finstack_quant_core::Error (error.rs:186-197) — handles 6 variants but only `InvalidInput → Validation` is tested (error.rs:203-207). (Verifier downgraded from High to Medium for incompleteness rather than absence.) suggested: Add tests for CurrencyMismatch → CurrencyMismatch, Serde → Internal, Io → Internal.

- **Type safety: infer_series_value_type mixed scalar/monetary** — infer_series_value_type (types/value.rs:97-135) — inline test (value.rs:186) covers only currency mismatch; the mixed Scalar/Amount paths (lines 119-130) and the 'Mixed scalar and monetary' error are untested. suggested: Call `infer_series_value_type` with `[AmountOrScalar::scalar(100.0), AmountOrScalar::amount(50.0, USD)]` and assert `Err` containing 'Mixed'.

- **Input validation: AdjustmentCap base_mode Reported vs Progressive** — AdjustmentCap base_mode (adjustments/types.rs:90-96) — test_capped_adjustment (engine.rs:258) covers a basic cap but never varies `base_mode`; no test compares Reported vs Progressive output when `cap base_node == target_node`. suggested: Apply two adjustments with identical cap config but differing `base_mode` to the same EBITDA and verify the capped amounts differ as expected.

- **Accrued interest: negative-accrual sign invariant** — src/capital_structure/period_flows.rs calculate_period_flows — audit_accrual.rs:73 exercises the full accrual path (accumulation and reset, lines 131-191) but no test constructs a negative-accrual scenario. (Verifier downgraded to Medium: accrual mechanics are well covered; a dedicated negative-accrual edge case would be nice-to-have.) suggested: Construct a mock `SignedFlowInstrument` with negative accrued interest and verify it is either rejected as invalid or correctly flagged/isolated by the period_flows calculation.

**Low**

- **Serde stability: CapitalStructureSpec deny_unknown_fields** — CapitalStructureSpec (types/model.rs:251-279) — derives `deny_unknown_fields` but is not tested for rejection. suggested: Deserialize CapitalStructureSpec JSON with an unknown field and assert rejection.

- **Serde stability: Adjustment / AdjustmentValue deny_unknown_fields** — Adjustment and AdjustmentValue (adjustments/types.rs:20-57) — derive `deny_unknown_fields` but are not tested against malformed JSON. suggested: Deserialize Adjustment JSON with an unknown field and assert rejection.

- **Forecast: moving-average window vs single historical point** — src/forecast/timeseries.rs:90 timeseries_forecast — `hist_data.len() >= 2` is checked and the window-clamp at line 230 (`window.min(hist_data.len())`) is only reached after the line 222 check passes, so the behavior is sound but the `.min()` reads as confusingly defensive. (Verifier downgraded: logic is correct; no new test strictly required.) suggested: Optionally add a moving_average test with historical=[100.0,110.0] and window=5 to document the error-vs-clamp behavior explicitly.

---

## monte_carlo

Three units (mc-process-rng, mc-discretization, mc-pricing); ~387 inline tests across `src/process`, `src/rng`, `src/discretization`, pricers, payoffs, greeks, and variance reduction.

### Tests to remove

- **[dead]** test_re_exports_work (`finstack-quant/monte_carlo/src/process/correlation.rs:17`) — Trivial smoke test of re-exported `finstack_quant_core` correlation functions; real coverage comes from `test_validate_correlation_matrix_*` (correlation.rs:46-61) and the core `math/linalg` tests. *Action:* Delete; correlation matrix tests belong in `finstack_quant_core/math/linalg` where the implementations live.
- **[unnecessary]** test_relative_stderr (`finstack-quant/monte_carlo/src/estimate.rs:214`) — Trivial getter test that computes `1.0 / 100.0` on a literal `Estimate`; no business logic to validate. *Action:* Remove; `relative_stderr()` is a simple formula with an epsilon guard.

#### Consolidate / strengthen (not a blind delete)

- **[consolidate]** test_multi_ou_drift_diffusion (`finstack-quant/monte_carlo/src/process/multi_ou.rs:127`) — Not a duplicate: the cited `test_multi_brownian_metadata` (brownian.rs:205) tests metadata construction, not drift/diffusion. This is a legitimate test of the κ_i(θ_i − x_i) drift / constant σ_i diffusion formula — just narrow. *Action:* Keep and expand to verify long-run mean reversion (sample a long path, verify empirical drift pulls toward θ), not just the instantaneous formula.
- **[unnecessary]** test_integrated_variance_symmetric (`finstack-quant/monte_carlo/src/discretization/qe_heston.rs:456`) — Not dead code: it covers the distinct symmetric input (v_t == v_next == 0.04), which `test_integrated_variance_various_dt` (qe_heston.rs:476) does not exercise. Low value but a real input. *Action:* Fold the symmetric case into `test_integrated_variance_various_dt` as an added parameter rather than deleting outright.
- **[unnecessary]** test_integrated_variance_bounds (`finstack-quant/monte_carlo/src/discretization/qe_heston.rs:424`) — The bounds assertion (lines 437-443) is subsumed by the exact midpoint-formula check (lines 446-452) that follows immediately; if the exact formula passes, the bounds hold automatically. *Action:* Drop the bounds assertion, keep only the midpoint formula check.
- **[consolidate]** test_milstein_vs_euler (`finstack-quant/monte_carlo/src/discretization/milstein.rs:212`) — Compares Milstein/Euler to a hand-computed exact GBM result with loose (<10%) bands. Note: on a single-path realization (z=0.5, dt=0.05), Milstein is *not* guaranteed closer than Euler, so a strict `milstein_error < euler_error` assertion is unsound without path averaging. *Action:* Keep as a tracking-validation test; if strengthened, require many-path averaging before asserting convergence ordering — do not relocate to exact.rs (which tests `ExactGbm`, not stochastic approximations).

### Coverage holes

#### High

- **process parameter validation** — `SchwartzSmithParams::new()` (schwartz_smith.rs:68) — All five validation branches (lines 75-98: kappa_x, sigma_x, mu_y, sigma_y, rho) are untested; every existing test uses valid params. Corrupt params corrupt downstream pricing. suggested: `test_schwartz_smith_rejects_invalid_params()` covering NaN kappa_x, zero sigma_x, infinite mu_y, negative sigma_y, rho=1.5.
- **process parameter validation** — `SchwartzSmithParams::with_lambda_x()` (schwartz_smith.rs:121) — The finiteness check (lines 122-126) is untested; existing callers pass only valid finite values. suggested: `test_schwartz_smith_rejects_nonfinite_lambda_x()` with NaN and Infinity.
- **process parameter validation** — `CirParams::new()` (cir.rs:47) — All three validation branches (lines 48-62: kappa, theta, sigma) untested. CIR feeds Heston variance dynamics, so corrupt params flow to pricing. suggested: `test_cir_rejects_invalid_params()` with negative kappa, NaN theta, zero sigma.
- **pricer/lsmc.rs public API** — `LsmcConfig::new()` — Validates `num_paths > 0` (line 161) and non-empty `exercise_dates` (line 166), but no test exercises the `num_paths=0` or empty-`exercise_dates` rejection paths. suggested: `LsmcConfig::new(0, vec![1], 2)` should Err, and price should Err when exercise_dates is empty.
- **pricer/path_dependent.rs configuration validation** — `PathDependentPricerConfig::validate()` — The `steps_per_year` check (line 222) has no test for zero, NaN, or negative `steps_per_year`. suggested: `PathDependentPricerConfig::new(1000).with_steps_per_year(0.0).validate()` and `.with_steps_per_year(f64::NAN).validate()` should Err.
- **payoff/barrier.rs BarrierOptionPayoff** — `BarrierOptionPayoff::new()` — Numeric/bridge behavior is well tested (barrier.rs:340-671) but there is no serde round-trip test despite `Serialize`/`Deserialize` derives. suggested: serialize to JSON, deserialize, and verify behavior on a fixed path (including bridge-crossing correction) matches the original.

#### Medium

- **process parameter validation** — `GbmParams::new()` (gbm.rs:147) — All three validation branches (lines 148-162: r, q, sigma) untested; GBM is foundational. suggested: `test_gbm_rejects_invalid_params()` with NaN r, Infinity q, negative sigma.
- **process parameter validation** — `HestonParams::new()` (heston.rs:156) — Only 2 of 7 error branches tested (negative kappa, rho out of range); missing non-finite r, non-finite q, non-positive/non-finite theta, sigma_v, v0 (lines 165-199). suggested: `test_heston_rejects_nonfinite_r()`, `test_heston_rejects_nonfinite_theta()`, `test_heston_rejects_nonpositive_sigma_v()`, `test_heston_rejects_nonpositive_v0()`.
- **greeks/lrm.rs** — `lrm_rho()` — Tested only in isolation with mock payoffs/shocks (lrm.rs:248-260); no integration test against a real pricing run or closed-form/finite-difference comparison. suggested: price a simple GBM option with LRM, compute rho via finite difference, verify it matches `lrm_rho` within tolerance.
- **variance_reduction/ control_variate** — control_variate serialization — Tests (lines 192-314) cover numerical behavior only; no serde round-trip for control-variate metadata. suggested: serialize a `ControlVariateAdjustment` to JSON, deserialize, apply to a path, verify the result matches the original.
- **engine/path_capture with Currency** — `McEngine::price_with_capture()`, `PathCaptureConfig` — All engine tests use only `Currency::USD`; no test pairs a USD process/payoff with a EUR result currency. suggested: create engine with `Currency::USD` payoff, attempt price with `Currency::EUR` result currency, should Err or warn.
- **pricer/path_dependent.rs LRM Greeks edge cases** — `price_with_lrm_greeks()` — Silently returns `(estimate, None)` when `discount_factor <= 0`, `time_to_maturity <= 0`, or `volatility <= 0` (lines 837-841); no test verifies these invalid inputs are rejected or warned. suggested: call with `discount_factor <= 0` (should Err) and with `volatility = 0` (should Err or return None with explicit logging).

#### Low

- **seed derivation determinism** — `derive_seed()` (seed.rs:58) — `test_seed_determinism` and `test_seed_different_instruments` (seed.rs:86) cover same-input determinism and distinctness, but edge cases (empty scenario, very long strings, special characters) are untested. suggested: `test_seed_with_long_strings()` and `test_seed_with_special_chars()`; a systematic collision check over N random IDs is optional given the existing distinctness coverage.
- **RNG uniform mapping** — `PhiloxRng::fill_u01()` (philox.rs:202) — `test_fill_u01_range` (philox.rs:363) checks samples lie in (0,1), but the strict open-interval guarantee (never exactly 0.0 or 1.0) is not explicitly verified. suggested: `test_fill_u01_strictly_open_interval()` filling 100k uniforms, asserting min > 0, max < 1, and no value equal to 0.0 or 1.0.
- **roughVolatility/FBM configuration validation** — `CholeskyFbm::new()` (fbm.rs:130) — Hurst bounds and grid ordering are tested (fbm.rs:510); H=0.5 stability is only implicitly covered by `test_cholesky_h_half_independent_increments`, and minimal-grid handling is missing. suggested: `test_cholesky_numeric_stability_near_h_half()` for H=0.4999/0.5001 and `test_cholesky_minimal_grid()`.
- **discretization/qe_common** — `validate_psi_c` (qe_common.rs:39) — Covered indirectly via `QeHeston::with_psi_c` (`test_with_psi_c_rejects_out_of_band_threshold`, qe_heston.rs:577) exercising 1.0/1.5/3.0/0.5/NaN; a direct isolated boundary unit test (exactly 1.0, exactly 2.0, ±Inf) is still missing. suggested: `test_validate_psi_c_boundaries()` checking the inclusive band edges and NaN/Inf rejection.
- **discretization/qe_heston** — `Discretization::step` for QeHeston, sigma_v degenerate fallback (qe_heston.rs:252, fallback 307-333) — `test_qe_heston_clamps_rho_and_integrated_variance_before_sqrt` (qe_heston.rs:714) uses sigma_v=1e-16 and confirms finiteness, but does not verify the fallback drift formula `(r−q)Δt − ½∫v + √(∫v)·z[0]` numerically. suggested: `test_qe_heston_sigma_v_degenerate_uses_fallback_drift` independently verifying the fallback arithmetic matches x[0].
- **discretization/jump_euler** — `Discretization::step` for JumpEuler (jump_euler.rs:55) — Existing tests (jump_euler.rs:114) all use valid params; NaN-shock propagation through `poisson_from_normal` is untested (defensive, not a Result branch). suggested: `test_jump_euler_handles_nan_shocks` calling step with `z = [0.0, f64::NAN, 0.0]` and asserting x[0] stays finite.
- **discretization/exact_gbm_dividends** — `Discretization::step` for ExactGbmWithDividends (exact_gbm_dividends.rs:64) — Multiple-dividend counts are exercised (exact_gbm_dividends.rs:147), but z-buffer bounds under tightly-sized buffers and edge dividend times are not parametrically tested. suggested: `test_exact_gbm_div_z_buffer_bounds` with various dividend counts verifying z consumption matches sub-interval count with no OOB.
- **discretization/qe_heston** — `QeRegime::exp_moment()` (qe_common.rs:130) — Exercised indirectly via `test_qe_heston_spot_update_uses_k0_star_correction` (qe_heston.rs:588), but the Some→None domain boundary (Quadratic: 2·A·a → 1) is not tested even though the fallback at qe_heston.rs:311 depends on it. suggested: `test_qe_regime_exp_moment_domain_boundary` finding critical A=1/(2a) and verifying Some just below, None just above.
- **barriers/bridge** — `bridge_hit_probability()` (bridge.rs:57) — `test_bridge_hit_probability_definite_hit` (bridge.rs:174) covers the interior case (barrier=100 between 90/110); explicit boundary cases barrier==min_s and barrier==max_s (still 1.0) are not asserted. suggested: `test_bridge_hit_probability_barrier_at_boundary` with barrier=100.0 (lower bound) and 110.0 (upper bound).
- **barriers/corrections** — `gobet_miri_adjusted_barrier()` (corrections.rs:64) — `test_gobet_miri_zero_vol` (corrections.rs:127) covers sigma=0; negative and NaN sigma behavior (shift at line 70 → NaN barrier) is untested, and the f64 signature performs no validation. suggested: `test_gobet_miri_adjusted_barrier_negative_sigma` and `test_gobet_miri_adjusted_barrier_nan_sigma` pinning the intended contract.
- **discretization/rough_bergomi** — `RoughBergomiEuler::step`, work buffer management (rough_bergomi.rs:95, 143) — `test_work_buffer_reset_across_paths` (rough_bergomi.rs:266) verifies correct accumulation after reset, but no negative test confirms a non-zero inherited work[0] corrupts variance (the reset contract is documented but not code-enforced). suggested: `test_rough_bergomi_work_buffer_non_zero_initial` initializing work[0]=0.5 and verifying the inherited accumulation diverges from a fresh path start.
- **payoff/asian.rs** — `geometric_asian_call_closed_form()` (asian.rs:493) — Tested only with static range bounds (price > 0 && price < 10); no Monte Carlo convergence comparison. suggested: price a geometric Asian via `PathDependentPricer` with 100k paths and compare to the closed form within 3 standard errors.

---

## portfolio

Scope: three audit units across the `finstack-quant/portfolio` crate — factor-model risk decomposition, core portfolio/builder/position types, and optimization/sensitivity/margin. ~20 removal candidates and ~28 coverage holes reviewed; below are only the verifier-confirmed and downgraded items.

### Tests to remove

- **[duplicate]** test_builder_dummy_entity_auto_creation (finstack-quant/portfolio/src/builder.rs:420) — *duplicate_of* tests/core_portfolio_and_builder.rs:156. Integration test `builder_required_fields_and_dummy_auto_create` (lines 189-195) builds a portfolio with a position referencing DUMMY_ENTITY_ID and asserts `portfolio.has_dummy_entity()`; the inline test adds no unique assertions. *Action:* Remove the inline test.
- **[duplicate]** test_builder_validation_fails_without_base_ccy (finstack-quant/portfolio/src/builder.rs:454) — *duplicate_of* tests/core_portfolio_and_builder.rs:156. Integration test line 181 asserts the identical `PortfolioBuilder::new("P").as_of(as_of).build().is_err()`; pure duplicate. *Action:* Remove the inline test.
- **[duplicate]** test_builder_validation_fails_without_as_of (finstack-quant/portfolio/src/builder.rs:463) — *duplicate_of* tests/core_portfolio_and_builder.rs:156. Integration test lines 183-186 assert the identical `PortfolioBuilder::new("P").base_ccy(Currency::USD).build().is_err()`; pure duplicate. *Action:* Remove the inline test.
- **[dead]** test_netting_set_margin_creation (src/margin/results.rs:366) — Trivial constructor smoke test; `IM + max(VM, 0) = 6M` is tautological with no edge case or error path. The calculation is covered by the integration aggregation workflows. *Action:* Delete.
- **[duplicate]** test_portfolio_margin_aggregation (src/margin/results.rs:384) — *duplicate_of* tests/margin_aggregation.rs:22. Both build bilateral (5M IM, 1M VM, 10 positions) and cleared (3M IM, 500k VM, 5 positions) sets and assert identical totals (8M IM, 1.5M VM, 15 positions). Integration version is canonical. *Action:* Delete the inline version.
- **[duplicate]** test_currency_mismatch_error (src/margin/results.rs:427) — *duplicate_of* tests/margin_aggregation.rs:59. Both verify adding a EUR netting set to a USD portfolio fails with `CurrencyMismatchError`; the integration test (line 86) additionally validates the message, making it a superset. *Action:* Delete the inline version.
- **[duplicate]** test_add_netting_set_with_fx (src/margin/results.rs:449) — *duplicate_of* tests/margin_aggregation.rs:114. Both add a 1M EUR netting set at FX 1.10 to a USD portfolio and verify conversion (1.1M IM, 220k VM); integration file adds mixed-currency coverage. *Action:* Delete the inline version.

#### Consolidate / strengthen (not a blind delete)

- **[duplicate]** test_builder_basic (finstack-quant/portfolio/src/builder.rs:403) — Verifier downgraded: not a clean duplicate. Unlike integration `builder_required_fields_and_dummy_auto_create`, this test adds tags (line 410) and asserts the `name` field (line 415), which the integration test does not cover. *Action:* Keep the test, or strengthen the integration test to include tags/name before removing.
- **[unnecessary]** test_position_unit_serialization (finstack-quant/portfolio/src/position.rs:628) — Verifier downgraded: a weak smoke test (only checks the JSON contains the substring "notional", no round-trip), but `test_position_spec_roundtrip` in tests/serialization.rs:22 covers serialization more comprehensively. *Action:* Replace/fold into the round-trip test rather than deleting blind.
- **[consolidate]** test_exposure_limit_validation (src/optimization/constraints.rs:236) — Verifier downgraded: the `[0,1]` inclusive range validation is adequate and not redundant, but it omits boundary-adjacent cases (0.9999, 1.0001, -0.0001) and error-message clarity. *Action:* Strengthen with boundary-adjacent cases and error-message assertions; do not delete.

### Coverage holes

#### High

- **Public API validation** — flatten_position_pnls (mod.rs:309) — Zero inline tests for this financial risk-engine helper, which validates n_positions, handles the empty case, and checks scenario-count consistency (lines 309-338). Dimension errors can silently corrupt risk aggregation. suggested: Test with (1) valid `2 x 3` matrix → (Vec, 3); (2) wrong row count → Validation error; (3) inconsistent scenario counts (rows [3,5]) → error naming row 1; (4) empty n_positions=0 → (Vec::new(), 0).
- **Public API validation** — flatten_square_matrix (mod.rs:278) — Tests exist (lines 345-363) but only cover the 2x2 happy path, wrong row count, and a wrong column on row 0; missing empty (n=0), single-element (n=1), and wrong column on rows 1+. suggested: Add (1) valid 3x3 → flat 9-element vec; (2) empty matrix → Ok empty; (3) single element [5.0] → [5.0]; (4) wrong column on row 1 (e.g. `vec![[1,2],[3,4,5]]`) → error naming row 1.
- **Error handling for non-finite inputs** — ParametricDecomposer::validate_factor_axes (parametric.rs:70) — Line 93 checks `!entry.is_finite()` for covariance entries, but no test injects a NaN covariance entry; the existing NaN test (line 374) only covers NaN sensitivities. NaN covariance silently underestimates risk. suggested: Replace covariance[0] with f64::NAN, call decompose(), expect a Validation error naming "finite".
- **Determinism and seed reproducibility** — SimulationDecomposer::new (simulation.rs:176) and decompose (simulation.rs:557) — Tests use fixed seeds (42, 4_242) but none verify that two decomposers with the same seed produce bit-identical results, despite the design comment at line 306 ("RNG stream is exactly the same every run"). suggested: (1) two `SimulationDecomposer(10000, 123)` on identical inputs → all factor_contributions bit-identical; (2) seeds 123 vs 456 → at least one factor contribution differs.
- **Covariance matrix symmetry verification** — ParametricDecomposer::validate_factor_axes (parametric.rs:70) and cholesky (simulation.rs:12) — Both check symmetry with an indexed error message ("...not symmetric at ({i}, {j})", parametric lines 101-104, simulation lines 29-32), but the existing mismatch tests cover factor-ID order, not symmetry violation; the symmetry branch is never exercised. suggested: Construct a covariance matrix with data `[0.04, 0.03, 0.02, 0.09]` (asymmetric), call decompose(), expect an error message containing "(0, 1)".
- **FX conversion and currency handling** — PortfolioValuation (valuation.rs public struct) — `valuation_fx.rs` covers cross-currency happy path and missing-FX errors, but no test verifies `FxConversionPolicy::CashflowDate` vs `ValuationDate` produce different base-ccy rollups, and there is no bit-exact determinism check on repeated calls. suggested: (1) multi-currency portfolio under two policies → assert base-ccy rollups differ; (2) same portfolio/market twice → assert money values are bit-exact equal.
- **optimization** — src/optimization/lp_solver.rs:105-131 (per_position_metric_value with MissingMetricPolicy::Strict) — The Strict error branch (lines 127-129) is never exercised; all optimization tests use Exclude/Zero policy. suggested: Create a portfolio with a metric-less position, set `MissingMetricPolicy::Strict`, optimize with an objective requiring that metric, and verify an error mentioning the missing required metric.
- **margin** — src/margin/results.rs:200-232 (add_netting_set_with_fx FX validation) — Validation at lines 250-254 rejects `fx_rate <= 0` and non-finite rates, but `test_zero_fx_rate` only covers the zero case; negative, NaN, Infinity, and subnormal positive (e.g. 1e-310) rates are untested. suggested: Add cases for -1.10, NaN, Infinity (expect "invalid FX rate" error) and subnormal positive (expect successful aggregation).

#### Medium

- **Serde backward compatibility** — RiskDecomposition (types.rs:22) — Downgraded to Medium: backward-compat tests exist (serialization.rs:99-128, types.rs:180-206, 209-225) covering missing-field default and empty-omit paths, but there is no test of explicit-empty-array deserialization, mixed empty/non-empty roundtrip omission, or unknown-field handling. suggested: (1) old JSON with explicit `"position_residual_contributions": []` deserializes cleanly; (2) mixed roundtrip omits the field only when empty; (3) deserialize unknown field and document accept-or-reject behavior.
- **Risk measure validation** — ParametricDecomposer::scale_for_measure (parametric.rs:195) — Downgraded to Medium: `RiskMeasure::validate` is tested in factor-model (config.rs:442-476), but within portfolio no test exercises `scale_for_measure` propagating a `measure.validate()` failure. suggested: Construct `RiskMeasure::VaR { confidence: 1.5 }`, call scale_for_measure with variance=0.04, expect a Validation error.
- **Zero and near-zero variance edge cases** — ParametricDecomposer::validated_variance (parametric.rs:236) — Downgraded to Medium: zero/negative-variance logic is exercised (parametric.rs:570-590) but not the VARIANCE_TOLERANCE boundary (1e-12). suggested: Test (1) variance 0.0 → 0.0; (2) variance = 1e-12 → 0.0; (3) variance = -5e-13 → 0.0; (4) variance = -2e-12 → Validation error naming "non-negative".
- **Serde stability and schema evolution** — PortfolioSpec (portfolio.rs) and PositionSpec (position.rs) — Downgraded to Medium: `test_portfolio_spec_json_roundtrip` (serialization.rs:147) covers round-trip field preservation, but neither spec derives `deny_unknown_fields`, and field-order invariance and omitted-field defaults are untested. suggested: (1) reject/document unknown fields; (2) serialize with differing field orders, deserialize, assert equality; (3) minimal JSON roundtrips with attributes={} and book_id=None.
- **Book hierarchy validation and cycles** — aggregate_by_book (grouping.rs:192) — Downgraded to Medium: deep-hierarchy correctness is partially covered (book_hierarchy_test.rs:61,219, 3-level), but there is no very-deep valid hierarchy, no memoization-correctness instrumentation, and no visiting-set cleanup test. suggested: (1) valid ~10-level hierarchy → total == sum of all positions; (2) instrument memo to verify a child total is computed once; (3) repeated calls verify no stale visiting-set state.
- **Percentage unit semantics** — PositionUnit::Percentage (position.rs:67) and Position::scale_factor (position.rs:476) — No end-to-end test that a Percentage position (quantity=50, per-unit PV=1000) yields 500 (not 50000); existing tests cover only range rejection. suggested: test_percentage_position_scale_preserves_sign_and_magnitude with quantity 50, 0.01, -50, -100 against known per-unit values, asserting exactly (quantity/100)*value.
- **Attribute-based grouping** — aggregate_by_attribute (grouping.rs:70) — Downgraded to Medium: happy path covered (aggregation_grouping_and_df.rs:19-91) plus one error path (grouping.rs:501), but NaN/Inf position values, currency-consistency assumption, and deterministic iteration order are untested. suggested: (1) NaN value_base → rejection; (2) repeated runs → identical IndexMap order; (3) document/assert caller must supply base-currency values.
- **Valuation fallback and degradation** — PortfolioValuation.degraded_positions (valuation.rs public field) — Downgraded to Medium: valuation_fallback.rs:111 covers one degraded position, but there is no multi-degraded test, no verification that degraded positions are excluded from total_base_ccy, and no reason-message content check. suggested: test_valuation_degrades_unsupported_instrument_and_records_reason with one valid + one unsupported instrument, asserting degraded_positions.len()==1 and contents include the position_id and a message.
- **sensitivity** — src/sensitivity/repricing_engine.rs:61-76 (ScenarioGrid::try_new validation) — ScenarioGrid has no inline tests; `try_new` validation (lines 62-72) for `n_points < 3` and even counts is never exercised, only the panicking `new()`. suggested: call try_new(2) expecting "requires at least 3 points" and try_new(4) expecting "odd number", verifying readable error context.

#### Low

- **Percentage unit edge cases** — Position::new validation (position.rs:219) — Tests cover >100 and <-100 rejection but not exactly 100.0, -100.0, or floating-point boundaries (100.0+epsilon). suggested: test_percentage_quantity_boundary_100_and_negative_100 asserting 100.0 and -100.0 validate, 100.0+1e-15 rejected, 100.0-1e-15 accepted.
- **Error propagation and diagnostics** — PortfolioBuilder.build() (builder.rs public fn) — Downgraded to Low: core_portfolio_and_builder.rs:156 (lines 181-186) asserts only `is_err()`, not message content; no test verifies the error names which field is missing when both base_ccy and as_of are absent. suggested: test_portfolio_builder_validation_error_message_is_helpful, building without both fields and asserting `error.to_string()` mentions at least one clearly.
- **liquidity** — src/liquidity/kyle.rs:26-100 (Kyle lambda units convention) — The per-share (not per-contract) lambda convention is documented at kyle.rs:343-359 but not validated by a test that would fail under wrong units (`estimate_cost_consistent_with_amihud` preserves relative ratios and would still pass). suggested: construct a Kyle model with lambda=0.01, execute a 1000-share trade, verify impact P&L = 10 (= 0.01*1000, not 0.01*1), documenting the unit convention.

---

## statements-analytics

Scope: ECL/credit engine (IFRS 9 + CECL, staging, covenants, adjusted net debt, portfolio aggregation) and corporate DCF/scenario/goal-seek/scorecard extensions. Two units audited; 5 removal candidates and 19 coverage holes triaged.

### Tests to remove

- **[duplicate]** test_compute_ecl_single_stage3_is_discounted_lgd_ead (src/analysis/ecl/engine.rs:974) — duplicate_of tests/analysis_ecl.rs:65; both verify ECL = 0.45 × 1_000_000 / 1.05 with tol 1e-6, buckets.len()==1, marginal_pd==1.0. *Action:* delete inline test; integration test is the canonical public-API verification.
- **[duplicate]** test_stage3_zero_maturity_still_has_allowance (src/analysis/ecl/engine.rs:994) — duplicate_of tests/analysis_ecl.rs:85; both verify ecl > 0 with remaining_maturity=0.0 via the same code path. *Action:* delete inline test; integration test covers the invariant.
- **[duplicate]** test_ead_schedule_reduces_lifetime_ecl (src/analysis/ecl/engine.rs:1022) — duplicate_of tests/analysis_ecl.rs:316; inline covers only IFRS 9 Stage 2 amortization, while the integration test is a superset exercising both IFRS 9 and CECL paths with the same amortizing schedule. *Action:* delete inline test; it is subsumed by integration coverage.
- **[duplicate]** test_invalid_ead_schedule_rejected (src/analysis/ecl/engine.rs:1048) — duplicate_of tests/analysis_ecl.rs:354; identical error cases (non-increasing times, NaN EAD) under Stage 1. *Action:* delete inline test; integration test is authoritative for public-API validation.

#### Consolidate / strengthen (not a blind delete)

- **[unnecessary]** test_ecl_config_builder_valid (src/analysis/ecl/engine.rs:898) — flagged as a smoke test since the valid-build path is implicitly exercised by every `.build().unwrap()`. Verifier downgraded: the test explicitly checks that custom bucket_width(0.5) and scenario count survive the builder, documenting builder-preservation semantics. *Action:* do not delete outright; consolidate with the invalid-weight/invalid-bucket-width cases into one builder validation suite.

### Coverage holes

**High**

- **NaN PD passes `CeclConfig::validate` (real bug, added in manual review)** — `CeclConfig::validate` `historical_annual_pd` check at [ecl/cecl.rs:154](finstack-quant/statements-analytics/src/analysis/ecl/cecl.rs:154) — The guard is `if self.historical_annual_pd < 0.0 || self.historical_annual_pd > 1.0`, the NaN-unsafe idiom: for `historical_annual_pd = f64::NAN` both comparisons are `false`, so a NaN PD is **accepted** and flows into CECL allowance math (→ silent NaN allowance). The sibling fields in the same function (`impaired_time_to_recovery_years`, etc.) correctly use `!x.is_finite() || ...`; this field does not. Not surfaced by the automated pass. suggested: add `!self.historical_annual_pd.is_finite()` to the guard (one-line fix) **and** a test `cecl_config_rejects_nan_historical_pd()` asserting NaN/Inf PD errors; mirror the existing `.is_finite()` pattern used by the neighbouring checks.
- **CECL / IFRS 9 Public API** — compute_ecl_weighted_from_schedules (src/analysis/ecl/engine.rs:704) — exported in mod.rs but never called by any test; the public wrapper accepting raw scenario-weight vectors does not validate weights in its signature, risking corrupted numbers on bad input. suggested: integration test calling it with valid and invalid scenario weights (e.g., summing to 0.8), verifying error rejection.
- **Covenant Analysis** — forecast_covenants, forecast_covenant, to_table (src/analysis/credit/covenants.rs:159-325) — none of the three public functions are tested; integration coverage exists for forecast_breaches but not for projection aggregation or to_table serialization, so a covenant-filtering or period-union bug could corrupt loan monitoring outputs. suggested: integration test calling forecast_covenants with multiple specs of differing tenors, verifying period union and per-spec aggregation.
- **DCF terminal value** — evaluate_dcf_with_market, TerminalValueSpec::HModel (src/analysis/valuation/corporate.rs:319-344) — H-Model NaN guards (high_growth_rate, stable_growth_rate, half_life_years) are untested; only a Gordon Growth NaN test exists, so NaN H-Model parameters could pass through and yield silent NaN valuations. suggested: test calling evaluate_dcf_with_market with TerminalValueSpec::HModel { high_growth_rate: f64::NAN, stable_growth_rate: 0.02, half_life_years: 5.0 } and asserting Err.
- **Scenario management** — ScenarioSet::evaluate_all (src/analysis/scenarios/scenario_set.rs:185) — the empty-set guard ("cannot be empty") and other error paths have no integration coverage; all existing tests supply non-empty sets. suggested: test calling `ScenarioSet { scenarios: IndexMap::new() }.evaluate_all(&model)` and asserting Err with the 'cannot be empty' message.

**Medium**

- **Staging / SICR Triggers** — classify_stage (src/analysis/ecl/staging.rs:229) — downgraded from High: test_stage2_pd_delta (staging.rs:489) exercises classify_stage with default config and a large A→BB delta, but no marginal case verifies the default pd_delta_absolute=0.01 boundary. suggested: verify origination PD 0.002 → current 0.015 triggers Stage 2 at the default 0.01 absolute threshold, while a smaller increase does not.
- **Exposure Validation / Input Sanitization** — Exposure::validate (src/analysis/ecl/types.rs:245-311) — maturity-bound and schedule validation are covered (types.rs:608), but LGD range/finiteness is never exercised in isolation. suggested: inline test_exposure_validate_rejects_invalid_lgd with lgd=-0.1, 1.1, NaN, Inf, verifying all are rejected.
- **RawPdCurve Deserialization Serde Contract** — RawPdCurve serde via TryFrom<RawPdCurveData> (src/analysis/ecl/types.rs:397-410) — test_raw_pd_curve_deserialization_validates (types.rs:567) rejects invalid knots and unknown fields but not non-finite knot values. suggested: inline test_raw_pd_curve_deserialization_rejects_nonfinite with input like {"rating":"BBB","knots":[[0.0,0.0],[1.0,NaN]]}, verifying rejection.
- **Portfolio Aggregation / EAD Mismatch** — PortfolioEclResult::from_results_with_exposures (src/analysis/ecl/portfolio.rs:125) — uses result.ead for aggregation without validating it against the input Exposure's EAD (which may differ via ead_schedule interpolation); test_unmatched_exposure_ids_surfaced covers only unmatched IDs. suggested: inline test passing results with EADs mismatched against input exposures, documenting current (non-validating) behavior.
- **DCF valuation (equity reconciliation)** — CorporateValuationResult (src/analysis/valuation/corporate.rs:27) — downgraded from High: the equity=EV−net_debt identity is covered once (tests/analysis_corporate.rs:589-596) but only for a single capital structure, so a sign error in the net-debt bridge across other structures could go uncaught. suggested: evaluate 3+ models with differing net debt levels, asserting (equity_value − (enterprise_value − net_debt)).abs() < 1e-6 for each.
- **Scenario set serialization** — ScenarioDefinition, ScenarioSet (src/analysis/scenarios/scenario_set.rs:65-102) — both carry deny_unknown_fields (lines 65, 98) but there is no round-trip serde test or unknown-field rejection test. suggested: serialize ScenarioSet to JSON, inject {"unknown_field": true}, and assert serde_json::from_str fails with an unknown-field error.
- **Scenario parent chain** — ScenarioSet::trace (src/analysis/scenarios/scenario_set.rs:352) — cycle-detection code (lines 366-369, 404-407) is never exercised. suggested: build A→B→C→A parent linkage, call trace("A"), and assert Err contains 'Cycle detected'.
- **Goal seek** — goal_seek (src/analysis/goal_seek.rs:321-324) — the inverted-bounds guard (lower >= upper) is never hit; existing tests pass valid bounds only. suggested: call goal_seek with bounds (upper=1.0, lower=10.0) and assert Err with the 'lower bound must be less than upper' message.
- **Scorecard configuration** — CreditScorecardExtension::validate_config (src/extensions/scorecards/mod.rs:262) — downgraded from High: test_scorecard_config_validation_invalid_weights (tests/extensions_scorecards.rs:83-98) exercises the range check via a single oversized weight, but combined weights exceeding 100.0 are untested. suggested: ScorecardConfig with 4 metrics each weight=25.5 (total=102), calling validate_config and asserting Err with 'should be between 0.01 and 100.0'.

**Low**

*None found.*

---

## margin

Scope: SA-CCR/FRTB regulatory engine plus VM/IM (SIMM) calculators, XVA, and CSA/collateral types. Two audit units, ~12 source files surveyed across `regulatory/frtb` and `calculators`/`types`/`xva`.

### Tests to remove

#### Consolidate / strengthen (not a blind delete)

- **duplicate** `simm_calculator_accepts_standalone_marginable_trait_objects` (finstack-quant/margin/tests/marginable_api.rs:55) — Overlaps the inline `public_calculate_matches_full_simm_sensitivities` at simm.rs:1728, which both verify `SimmCalculator::calculate()` on a `Marginable` trait object against `calculate_from_sensitivities`. Verifier downgraded from a clean delete: these are not true duplicates — the inline test exercises 3 risk classes (incl. FX delta) with per-breakdown-entry assertions, while marginable_api.rs only covers 2 classes (IR + equity delta) and checks key presence. *Action:* Do not blind-delete; either keep both or fold FX delta into marginable_api.rs. The file retains independent value for metric-integration coverage (lines 80+).

### Coverage holes

#### High

- **Variation Margin Calculator - Currency Validation** — `VmCalculator::calculate` (finstack-quant/margin/src/calculators/vm.rs:178) — Currency-mismatch checks at lines 186-200 (exposure and posted_collateral vs CSA base currency) have zero tests; both error paths are unexercised, and all VM tests use matching USD currencies. A silent error path that could corrupt margin calls. suggested: `vm_calculator_rejects_exposure_currency_mismatch()` — construct USD CSA, pass EUR exposure, assert `calculate()` errors with "currency mismatch".

- **Serde Serialization Stability - Margin Types** — `VmParameters, ImParameters, CsaSpec` (finstack-quant/margin/src/types/thresholds.rs:44, :290; finstack-quant/margin/src/types/csa.rs:100) — All three carry `#[serde(deny_unknown_fields)]` but have no round-trip or unknown-field rejection tests; a JSON typo or schema drift could silently lose data or reject valid configs at runtime. suggested: `vm_parameters_serde_roundtrip()` (JSON → struct → JSON preserves all fields) plus `vm_parameters_rejects_unknown_fields()` (JSON with `unknown_field` fails to deserialize).

- **Eligibility Collateral Schedule - Serde Stability** — `EligibleCollateralSchedule` (finstack-quant/margin/src/types/collateral.rs:173, :223, :348) — `MaturityConstraints`, `CollateralEligibility`, and `EligibleCollateralSchedule` all use `deny_unknown_fields` without round-trip tests; the registry-loaded schedule could accept typo/version-drift fields silently. suggested: `eligible_collateral_serde_unknown_fields_rejected()` — deserialize JSON containing a spurious `typo_field` and assert it errors.

#### Medium

- **XVA Configuration Validation — NaN/Inf edge cases** — `XvaConfig::validate` ([xva/types.rs:144](finstack-quant/margin/src/xva/types.rs:144)) — *Corrected from the workflow's "High / concrete bug" claim (manual spot-check):* the recovery-rate guard `!(0.0..=1.0).contains(&self.recovery_rate)` **does** reject NaN (NaN is not contained, so the negation fires) — there is no recovery-rate bug. The real gap is (a) a missing test pinning that safe behaviour, and (b) a genuine edge in the `time_grid` strictly-increasing loop (~lines 133-142): a single `+Inf` entry, or `[Inf]` alone, satisfies `cur > prev` and is accepted. Existing tests (lines 638-653) cover only out-of-range (1.5, -0.1). suggested: `xva_config_rejects_nan_recovery()` (assert NaN recovery_rate errors — documents the safe idiom) and `xva_config_rejects_inf_time_grid()` (a `+Inf` time-grid entry should be rejected).

- **SIMM Registry Resolution - Validation Error Handling** — `resolve_simm_params` (finstack-quant/margin/src/calculators/im/simm.rs:192) — Returns a Validation error when the requested `SimmVersion` is absent from the registry; the existing test at line 1400 only confirms present versions (V2_5, V2_6) validate. The missing-version error path is untested and would only surface at calculation time, not construction. suggested: `resolve_simm_params_missing_version_error()` — craft a registry with a version removed and assert the lookup errors.

- **SIMM Sensitivities - Empty Portfolio Handling** — `SimmSensitivities` (finstack-quant/margin/src/types/simm_types.rs:238) — `is_empty()` is tested (line 515) but no test confirms `calculate_from_sensitivities` on a fully empty `SimmSensitivities` returns zero IM (vs. NaN); the empty path skips all risk classes and returns 0.0 at lines 1288-1290 but is never asserted. suggested: `simm_calculator_empty_sensitivities_gives_zero()` — build empty `SimmSensitivities::new(USD)`, assert `total_im == 0.0`.

- **error-handling** — `drc_risk_weight` (finstack-quant/margin/src/regulatory/frtb/drc.rs:164) — Silently falls back to 0.15 (Unrated) for unknown buckets; the fallback and edge cases (bucket 0, bucket > 9) are untested, exercised only indirectly via engine tests using buckets 2 and 4. suggested: call `drc_risk_weight` with bucket=0 and bucket=100, assert both return 0.15, then assert all valid buckets 1-9 against MAR22.24 weights.

- **error-handling** — `drc_lgd` (finstack-quant/margin/src/regulatory/frtb/drc.rs:176) — Silently falls back to 0.75 for unknown seniorities; only `SeniorUnsecured` is exercised indirectly via engine tests (engine.rs:364-432). Verifier downgraded to Medium since `DrcSeniority` is a closed (compile-time) enum, limiting realistic misuse. suggested: directly assert `drc_lgd` returns the correct value for every `DrcSeniority` variant, with a comment noting the fallback is intentional.

- **calculation-correctness** — `drc_charge` (finstack-quant/margin/src/regulatory/frtb/drc.rs:70) — Only indirectly tested via `engine.calculate()` (engine.rs:364-432); no direct unit test for empty list → 0, single long vs single short asymmetry, sign-preserving MAR22.9 JTD floor, or HBR capping when long=0/short=0. Verifier holds at Medium: engine tests cover single-long RW and multi-bucket HBR, and the implementation explicitly returns 0.0 for empty input (lines 71-72) and floors HBR at line 140, so coverage is indirect but reasonably comprehensive. suggested: 4 targeted unit tests in drc.rs — empty → 0; single long → LGD·notional·RW; single short → 0 (HBR=0); underwater long floors at 0.

#### Low

- **calculation-correctness** — `rrao_charge` (finstack-quant/margin/src/regulatory/frtb/rrao.rs:31) — Tested only via `engine.calculate()` (engine.rs:439-476, exotic 1% / non-exotic 0.1%); no direct test for empty list → 0, negative-notional `abs()` handling, or the non-exotic weight. Verifier holds at Low: the implementation is a simple `sum(|notional| * weight)` (uses `.abs()` at line 40, `.sum()` at line 42) so empty input returns 0.0 implicitly and sign handling is correct by design. suggested: 3 unit tests in rrao.rs — empty → 0; positive notional → notional·weight; negative notional → |notional|·weight.

- **parameterization** — `FrtbRevision::Custom` (finstack-quant/margin/src/regulatory/frtb/params/registry.rs:32) — Label generation is tested (registry.rs:475-481), but no test covers round-trip serialization of `Custom` with a non-empty label or its interaction with `FrtbParams::validate()`. Verifier downgraded to Low: `label()` is the critical public API and is covered, and the simple `String` wrapper would round-trip correctly. suggested: build `FrtbParams` with `revision=Custom("test-label")`, serialize→deserialize, assert label matches and `validate()` succeeds.

*Note: the `low_correlation_uses_basel_floor` removal (types.rs:119) and the `NettingSetId` serde hole (netting.rs:103) were rejected by the verifier — the types.rs test uniquely covers the `0.0` floor input, and `NettingSetId` already has round-trip and missing-tag tests (netting.rs:183, :196) — and are excluded here.*

---

## Verification & false positives

The adversarial verify pass rejected **12** removal candidates (tests that looked redundant but cover a distinct input, edge case, or assertion the "duplicate" did not) and **37** coverage holes (behaviors already tested elsewhere in the crate tree, or guaranteed by `thiserror`/derive macros). These are excluded above. Two illustrative margin rejections are noted inline in that section (`low_correlation_uses_basel_floor` and the `NettingSetId` serde hole).

## Suggested sequencing

1. **Clean removals first** (23 tests) — mechanical, low-risk; delete the cited inline duplicates of integration tests and the dead/tautological tests.
2. **High holes that are real bugs, not just gaps** — the NaN-passes-range-check family (margin `XvaConfig` recovery rate; factor-model covariance; MC process constructors) should be fixed in *code* and pinned with a test, not just tested.
3. **High holes — untested error/validation paths** — add `Err`-branch tests for the public APIs flagged High (statements FX/currency-mismatch, covenant missing-metric and untested covenant types, cashflows credit-adjusted PV guards, portfolio dimension/symmetry validators, statements-analytics ECL/DCF/scenario guards, margin VM currency mismatch).
4. **Serde-stability pins** — add reject-unknown-field tests for the `deny_unknown_fields` types listed across analytics, attribution, statements, margin, scenarios, and statements-analytics.
5. **Consolidations & Low holes** — opportunistic, during related refactors.
