# Quant Finance Review — `statements`, `statements-analytics`, and Bindings

**Date:** 2026-06-09
**Scope:** `finstack-quant/statements`, `finstack-quant/statements-analytics` (plus the `finstack-quant/covenants` crate the credit bridge delegates to), `finstack-quant-py/src/bindings/{statements,statements_analytics}`, `finstack-quant-wasm/src/api/{statements,statements_analytics}`, `.pyi` stubs, JS facade, parity contract.
**Method:** Six parallel deep-review passes (evaluator/forecast, capital structure, ECL/IFRS-9/CECL, valuation/scenarios/comps, credit/templates/extensions, bindings parity). All Blocker findings re-verified by direct source inspection. Several evaluator findings were reproduced with throwaway probe tests (outputs quoted inline). Test suites were not run.

---

## Findings

### Blockers

**B1 — Stage 3 ECL is priced with the performing-rating PD curve, not PD = 1** — `finstack-quant/statements-analytics/src/analysis/ecl/engine.rs:418-449`
Stage 3 (credit-impaired) is treated identically to Stage 2: lifetime horizon, PD taken from `exposure.current_rating`'s curve. For a defaulted asset IFRS 9 (5.5.33/B5.5.33) measures the allowance as effectively PD = 1 (ECL ≈ discounted LGD × EAD). A defaulted loan whose rating field still says "BBB" gets ECL ≈ cumPD(BBB) × LGD × EAD — roughly an order of magnitude understated. Worse edge: Stage 3 with `remaining_maturity_years = 0` produces ECL = 0 for a defaulted asset.
**Fix:** branch on `Stage::Stage3` and compute `LGD × EAD × DF(t_recovery)` (or force a cumPD→1 profile), and add the missing Stage-3 sanity test.

**B2 — Off-by-one in the cashflow scale denominator silently inflates interest on on-schedule amortizing loans** — `finstack-quant/statements/src/capital_structure/period_flows.rs:93-104`
End-of-period balances snap at `end − 1 day` (boundary flows excluded), but `scheduled_opening` filters `*d <= period.start` (boundary flows **included**). For the standard alignment where amortization payments land exactly on period boundaries, the stateful opening is pre-payment while the scheduled opening is post-payment, so `scale = opening/scheduled > 1` and every coupon, amort and fee in the period is multiplied up — e.g. 1%/quarter amortizer → +1.01% interest every period, fully silent below the 1.05 warn threshold. This path runs for **all** capital-structure evaluations, waterfall or not.
**Fix:** change the filter to `*d < period.start` to match the snapshot convention; add a regression test with boundary-dated amortization payments asserting per-period interest equals the raw schedule with no `scale_clamped` warning.

**B3 — `project_finance` covenant package: two `MinDSCR` specs share one identity; a distribution-lockup breach triggers an Event of Default** — `finstack-quant/covenants/src/templates.rs:186-211`
Neither spec gets a `.with_label(...)`, so both produce `instance_key() == "min_dscr"` (`engine.rs:118-122`). The lockup report overwrites the primary report in the results `IndexMap`, and `apply_consequences` (`engine.rs:881-887`) resolves a lockup-only breach (e.g. DSCR 1.15 vs lockup 1.25, primary 1.05) to the **first** matching spec — the one carrying `CovenantConsequence::Default` — and sets the instrument into default.
**Fix:** label both specs distinctly (`min_dscr_default` / `min_dscr_lockup`) and reject duplicate `instance_key`s in `CovenantEngine::add_spec`.

**B4 — Comps regression silently misaligns x/y vectors when peers have partially missing data** — `finstack-quant/statements-analytics/src/analysis/comps/scoring.rs:105-121`
`extract_values` filters missing values independently for the y metric and the x metric, then `regression_fair_value` pairs them positionally with `n = min(len)`. A peer missing one metric shifts every subsequent pairing — corrupted slope/intercept/R²/residual with no error, on the most common shape of real comps data.
**Fix:** extract `(x, y)` pairs in a single pass over peers, keeping only rows where both are present.

---

### Major — statements evaluator & forecasting

- **As-of visibility broken with forecasts** — `finstack-quant/statements/src/evaluator/forecast_eval.rs:51-56, 141-146, 177-192`: actual periods hidden by `as_of` resolve to the Forecast source but are absent from the forecast period set → hard error (`Forecast did not produce value for period ...`); and `determine_base_value` reads raw `node_spec.values` so the *hidden* last actual anchors the forecast — a silent look-ahead leak in the feature that exists to prevent look-ahead. (Both reproduced by probe.) **Fix:** thread the visibility policy into forecast evaluation; treat hidden actuals as forecast periods and resolve the base from the last *visible* actual.
- **Look-ahead in nested aggregates under lagged evaluation** — `finstack-quant/statements/src/evaluator/formula_helpers.rs:84-93`: `collect_historical_values_sorted` does not filter by the context's period, so `lag(rolling_mean(x,4), 2)` windows include data after the lagged evaluation point. Silent bias. **Fix:** include only periods `<= context.period_id` (also in `collect_column_series` in formula_ewm.rs).
- **Seasonal forecast double-shifts when `season_start != 0`** — `finstack-quant/statements/src/forecast/timeseries.rs:439`: the decomposition keys factors by data position, but the forecast adds `season_start` again — following the module's own docs rotates the seasonal pattern (Q4 uplift applied to Q1). **Fix:** `(hist_data.len() + i) % season_length`; add a `season_start = 1` regression test.
- **`ewm_var`/`ewm_std` default bias correction matches no pandas mode** — `finstack-quant/statements/src/evaluator/formula_ewm.rs:121-145`: adjust=False recursion combined with adjust=True correction weights; +9% to +30% overstated EWM variance vs any pandas reference. The unit test `ewm_var_defaults_to_bias_correction` hard-codes the wrong value (0.5625; pandas gives 0.5 for x=[1,2], α=0.5). **Fix:** use `1/(1−Σŵ²)` over the normalized recursion weights, or implement true pandas adjust=True; replace the pinned test value.
- **`lag`/`diff`/`pct_change` with expression args abort evaluation at period boundaries** — `finstack-quant/statements/src/evaluator/formula_timeseries.rs:66-69, 119-127, 159-164`: instead of returning NaN as documented and as the column path does, the expression path errors against an empty context (misdiagnosed as "circular dependency"). **Fix:** return NaN when the target period is absent from history.

### Major — capital structure

- **ECF sweep double-spends cash** — `finstack-quant/statements/src/capital_structure/waterfall/excess_cash_flow.rs:69`, `waterfall/mod.rs:225-241`: scheduled amortization is never deducted from the sweep (fees are, when ranked ahead). Debt paydown overstated by `sweep% × scheduled amort` every period; contradicts the LPA/S&P ECF definition the doc comments cite. **Fix:** deduct total scheduled principal from `remaining_sweep` when Amortization precedes the prepayment priority, mirroring the fees handling.
- **`SCALE_CLAMP_MAX = 1.10` breaks PIK compounding** — `period_flows.rs:115-144`: toggled-PIK balances compound but the schedule doesn't; the clamp binds after ~5 quarters of quarterly 2% PIK, freezing interest at 1.10× the original coupon. Understates distressed-scenario debt and interest. **Fix:** track the PIK-capitalized increment in state and exclude it from the clamp basis, or compute toggled-PIK interest directly as `rate × opening_balance`.
- **Cash-shortfall interest/fees evaporate** — `waterfall/mod.rs:344-388`, `waterfall/cash_distribution.rs:26-42`: the unpaid portion of a cash-capped coupon is recorded nowhere (no accrual, no carry-forward, no warning); negative available cash zeroes the whole coupon. Equity overstated by exactly the unpaid debt service. **Fix:** accumulate per-instrument shortfall into accrued interest (first-priority next period) or capitalize it; at minimum emit a structured warning.
- **Deferred-PIK path lets PIK'd coupons consume cash, then capitalizes only the capped amount** — `waterfall/mod.rs:323-332` vs `450-463`: with priority stacks lacking a prepayment entry, the coupon debits `remaining_cash` and the unfunded part of the PIK disappears. **Fix:** apply the PIK bucket move unconditionally before the cash caps; capitalize the full contractual coupon.
- **Forward-dated instruments report full notional before issuance** — `finstack-quant/statements/src/evaluator/capital_structure_runtime.rs:191-197`, `period_flows.rs:219-228`, `integration.rs:295-300`: pre-issue periods fall back to the first *future* outstanding entry (full notional) → leverage/covenant metrics wrong for refinancing and delayed-draw models. **Fix:** return zero when no entry ≤ period start and issue date > period start.
- **No intra-category seniority** — `waterfall/cash_distribution.rs:50-79`: shortfalls are shared pro-rata across 1L and mezz alike; the engine is single-class pro-rata and nothing documents that. **Fix:** add a seniority rank with sequential allocation across ranks; document the limitation until then.

### Major — ECL / CECL

- **`CeclEngine` with empty `pd_sources` silently returns ECL = 0** — `finstack-quant/statements-analytics/src/analysis/ecl/cecl.rs:146-201`; the IFRS-9 path rejects this, CECL doesn't. **Fix:** validate non-empty in `CeclEngine::new`.
- **CECL scenario weights never validated** — `cecl.rs:200`: the weights actually used (`pd_sources`) bypass the sum-to-1/non-negativity checks applied to the unused `config.scenarios`. **Fix:** run `validate_scenario_weights` over `pd_sources`.
- **`Warm`/`Vintage` methodologies are silent no-ops** stamped into results as if applied — `cecl.rs:41-49, 165-208`. **Fix:** error on unimplemented methodologies (note: a real WARM must not reuse the EIR discounting).
- **Dead configuration shipping in the policy JSON**: `LgdType` (PointInTime/TTC/Downturn) never read — `engine.rs:102, 259-262`; `rating_downgrade_notches` configurable but the `RatingDowngrade` SICR trigger can never fire — `staging.rs:78-83, 136`. **Fix:** wire or remove.
- **EAD constant over the lifetime horizon** — `engine.rs:445-446`, `cecl.rs:198`: no amortization profile (+30-50% overstated ECL on level-pay term loans) and no CCF/undrawn support (understated for revolvers). **Fix:** accept an optional EAD schedule per exposure and evaluate `EAD(t_mid)` per bucket (per-bucket plumbing already exists).

### Major — valuation, scenarios, comps

- **Gordon/H-Model terminal value capitalizes the last *period* flow with *annual* WACC and g** — `finstack-quant/statements-analytics/src/analysis/valuation/corporate.rs:254-280`, `finstack-quant/valuations/src/instruments/equity/dcf_equity/types.rs:504-532`. With this crate's default quarterly models, TV (typically 60-80% of EV) is understated ~4×. **Fix:** annualize the terminal flow (trailing 12m), or validate the period grid is annual for growth-perpetuity terminal specs.
- **Composite rich/cheap score mixes raw residuals (metric units) with z-scores (unitless)** — `comps/scoring.rs:135`: configured weights are meaningless when a 50bp residual averages with a 1.5 z-score; the docstring claims standardization, the code doesn't. **Fix:** standardize the residual (`residual / σ_residual` across peers).
- **Sign convention flips between scoring paths** — `comps/scoring.rs:133-135`: regression path assumes higher-y-is-cheap (spread-like), univariate `−z` assumes higher-y-is-rich (multiple-like) — the same metric inverts meaning depending on configuration. **Fix:** add a per-dimension direction flag applied consistently to both paths.
- **Mixed discounting bases when a `{CCY}-DISCOUNT` curve happens to exist** — `corporate.rs:349, 377-391`: equity discounts on the market curve, EV/TV on WACC → result envelope no longer satisfies equity = EV − net debt, triggered by a curve-naming coincidence. **Fix:** compute all reported components on one basis; make the synthesized curve ID opt-in.

### Major — credit bridge & extensions

- **NaN metric → 0% breach probability in covenant forecasting** — `finstack-quant/covenants/src/forward.rs:218-225, 293-299, 405-411`: point-in-time evaluation treats NaN as breach (`engine.rs:1020-1028`); the forecast paths treat it as pass. EBITDA collapsing through zero (ratio → NaN) shows a clean 0% breach path. **Fix:** mirror the engine convention (NaN ⇒ breached, probability 1).
- **Volatility units mismatch in MC breach forecasts** — `finstack-quant/statements-analytics/src/analysis/credit/covenants.rs:168-177, 293-304`: per-period `std_dev` from forecast specs is consumed as **annualized** vol (`forward.rs:303-306` scales by √T-years) → breach probabilities understated ~2× on quarterly models, ~3.5× monthly. **Fix:** annualize at the bridge using the model's period cadence.
- **Negative-EBITDA leverage passes max-leverage covenants** — `finstack-quant/covenants/src/engine.rs:1029-1035`: negative ratio satisfies `value <= threshold` with large positive headroom; the in-crate `LeverageRangeCheck` correctly flags the same case as undefined, so the two subsystems disagree. **Fix:** treat negative ratio-type metrics on max covenants as breach/indeterminate.
- **Corkscrew roll-forward breaks never fail the report** — `finstack-quant/statements-analytics/src/extensions/corkscrew/mod.rs:240-292, 402-409`: a failed `opening + Σchanges = closing` identity yields `status: Success` with the failure buried in `data.validations[].is_valid`; `fail_on_error` has no effect on it. **Fix:** push warnings/errors for failed validations and let them drive status.

### Major — bindings

- **WASM `runChecks` silently drops `formula_checks`** — `finstack-quant-wasm/src/api/statements_analytics/mod.rs:255-265` vs `finstack-quant-py/src/bindings/statements_analytics/analysis.rs:869-904`: Python hand-merges formula checks into the suite; WASM calls bare `spec.resolve()` which can't see them (they live in the analytics crate). Same spec JSON → different check reports per host; WASM reports "all passed" without running user-defined validations. **Fix:** one canonical Rust entry point (e.g. `analysis::resolve_check_suite`) that resolves builtins and merges formula checks, called by both bindings.
- **`statements_analytics/__init__.pyi` systematically wrong** — keyword names drift from the actual PyO3 signatures across ~15 functions (`model_json=` vs `model=`, etc.), and `run_checks`/`run_three_statement_checks`/`run_credit_underwriting_checks` are missing their optional `results` parameter. Stub-guided keyword calls raise `TypeError` at runtime; the typed fast path (`FinancialModelSpec | str`) is hidden. **Fix:** regenerate the stub against the binding signatures (contrast: `statements/__init__.pyi` is accurate).

---

### Moderate (grouped)

**Evaluator/forecast**
- `growth_rate` sign-inverted for negative→negative series (improving losses show negative growth) — `formula_timeseries.rs:246-256`. Return NaN for non-positive bases.
- Moving-average forecast carries a half-window lag bias (`bias = slope·(w−1)/2` at every horizon) — `forecast/timeseries.rs:209-230`.
- `evaluate_prepared` reads the evaluator's mutable compiled cache, not the `PreparedEvaluation` — silent wrong-formula reuse across models sharing node ids — `evaluator/engine.rs:367-443, 581-600`. Store the compiled map in `PreparedEvaluation`.
- MC `path_data` row order and `warnings` order nondeterministic under rayon — violates serial ≡ parallel for serialized outputs — `evaluator/monte_carlo.rs:364-391`, `engine.rs:546-563`. Sort in `finish()`.

**Capital structure**
- Configured ECF sweep silently no-ops without a prepayment priority; `validate()` accepts duplicate priorities (duplicate `Interest` double-debits cash) and negative `sweep_percentage` — `waterfall/mod.rs:254-277`, `waterfall_spec.rs:66-91`.
- Revolver re-draw after an off-schedule sweep resurrects the swept balance (`max(scheduled_closing, net_new_funding)`) — `period_flows.rs:249-260`.
- Commitment/facility fees scaled by the *drawn*-balance ratio (backwards; zeroed when swept to zero) — `period_flows.rs:149-167`.
- `AccrualConfig::default()` (frequency `None`) used for accrued-interest snapshots — wrong for ACT/ACT ISMA — `period_flows.rs:266`, `integration.rs:315-319`.
- Sweep residual/capacity overflow assigned to an arbitrary last instrument instead of cascading pro-rata — `waterfall/mod.rs:297-310`.
- Equity is a structural no-op: residual cash never reported, blocking any cash-conservation assertion — `waterfall/mod.rs:385`.

**ECL**
- `compute_ecl_weighted_from_schedules` only works for `current_rating == Some("scenario")` (undocumented magic string; Python binding hardcodes it) — `engine.rs:577-609`.
- `EclConfig.scenarios` validated but never used; can diverge from the `pd_sources` actually priced; per-weight bounds unchecked — `engine.rs:125-143`.
- Python binding Stage 3→2 cure default 6 vs core policy default 12 (`ecl_policy.v1.json` `binding_defaults` vs `ifrs9_policies[0].staging`) — cross-surface staging divergence.
- No CECL pooling/collective-assessment path (ASC 326-20-30-2) — `portfolio.rs`/`cecl.rs`.

**Valuation/scenarios**
- Mid-year convention subtracts a flat 0.5 **years** regardless of period frequency and applies to exit-multiple TVs (+~5% at 10% WACC) — `dcf_equity/types.rs:685-692, 581-589`.
- NaN terminal-value parameters bypass validation (`g >= wacc` guards are false for NaN) → silent NaN valuations — `corporate.rs:290-297`, `types.rs:356-446, 509-515`. Write guards fail-closed (`!(wacc > g)`); validate `TerminalValueSpec` finiteness.
- Scenario comparison tables emit `0%` change on zero baselines, contradicting `VarianceRow.pct_var = None` semantics — `scenario_set.rs:605-614`.
- Tornado entries attribute all impact to the upside when the base value isn't in the perturbation grid — `sensitivity.rs:480-486`.
- Bridge decomposition: no residual/unexplained term; cost-driver signs misleading (contributions in driver units) — `variance.rs:286-337`.
- Sensitivity override at a period absent from the model grid is a silent no-op scenario — `sensitivity.rs:287-313`.

**Credit / extensions / templates**
- Relative headroom sign flips for negative thresholds (divide by `threshold.abs()`) — `covenants/engine.rs:1152-1168`.
- `forecast_breaches` builds the period set as the union over all nodes; hard-fails when the metric covers fewer periods — `credit/covenants.rs:238-248`, `forward.rs:203-208`.
- Scorecard: boundary convention resolves to the *worse* grade for lower-is-better metrics — `scorecards/mod.rs:459-481`; NaN/unmatched factors silently score 50 (≈BB+) at full weight — `mod.rs:488-507`; only the final model period is rated — `mod.rs:372-376`; failed metrics silently renormalize while a rating is still emitted — `mod.rs:285-351`.
- `LiquidityRunwayCheck` treats model periods as months (3× understated on quarterly models) — `checks/credit/liquidity.rs:59`.
- Corkscrew: configured `beginning_balance_node` with a missing period value silently falls back to `prev_balance` — `corkscrew/mod.rs:390-399`; articulation checked only at the last period; `fail_on_error` semantics inconsistent.
- `add_roll_forward` has no opening-balance input; first period opens at 0 silently — `templates/roll_forward/mod.rs:30`.
- Rating-scale registry never validates monotonic ordering of user-supplied scales (`determine_rating` depends on it) — `core/src/rating_scales.rs:243-267`.
- Covenant engine: `test_frequency` stored but never enforced; no equity-cure mechanism; LTM convention for covenant metrics neither enforced nor documented — `engine.rs:40, 64-67, 549`.
- `compute_credit_context` silently drops currency-mismatch/zero-denominator periods from min/max stats — `credit/credit_context.rs:151-181`.

**Bindings**
- Python `score_relative_value` re-implements input mapping in the binding (hardcoded `PeriodBasis::Ltm`, invented dimension mini-language, `MetricExtractor::Multiple` unreachable) while WASM takes canonical serde shapes — `finstack-quant-py .../comps.rs:167-349` vs `finstack-quant-wasm .../comps.rs:100-107`.
- Python `peer_stats` renames `count`→`n` and computes `iqr` in the binding (cross-binding drift; logic in binding) — `finstack-quant-py .../comps.rs:84,88`.
- `compute_ecl` patches the PD curve's `(0,0)` anchor in the binding but `compute_ecl_weighted` doesn't — identical curve accepted vs `ValueError` — `finstack-quant-py .../ecl.rs:285-294` vs `:396-426`. Move anchoring into a Rust helper.
- Non-numeric metric values silently skipped in Python comps (`let Ok(v) = val.extract::<f64>() else { continue; }`) — `comps.rs:171-173`. Raise `ValueError`.
- `to_pandas_long` docstring overclaims Decimal precision — `value_money` comes from the f64 mirror (`export.rs:38`), only `to_json()` preserves fixed-point — `finstack-quant-py .../statements/evaluator.rs:106-112`.
- Python `StatementResult` missing `get_money`/`get_scalar` (canonical-API parity gap; no typed monetary accessor) — `evaluator.rs:26-147` vs Rust `results.rs:193, 206`.

---

### Minor

- Strict-serde gaps: `MonteCarloConfig` (`evaluator/monte_carlo.rs:38-50`), `CashflowBreakdown`/`CapitalStructureCashflows` (`capital_structure/cashflows.rs:55, 82`), and all inbound ECL types (`Exposure`, `EclConfig`, `StagingConfig`, `MacroScenario`, `CeclConfig`, `RawPdCurve`) lack `deny_unknown_fields`; deserialized `RawPdCurve` bypasses `new()` validation.
- Forecast `params` typos silently default (`growth`/`season_start` via `.unwrap_or`) — `forecast/timeseries.rs:415-421`.
- `normalize_percentiles` silently clamps out-of-range percentiles (user `5` meaning 5% → max) — `monte_carlo.rs:185-195`.
- `pct_change(x, 0)` returns 0.0 for NaN input (`diff` propagates NaN) — `formula_timeseries.rs:148-150`.
- EWM functions don't skip non-finite values, unlike every other aggregate — `formula_ewm.rs:18-30`.
- Single-run mode: two stochastic nodes sharing a `seed` get identical shock paths (MC mode mixes node hash; single-run doesn't) — `forecast/mod.rs:135-164`.
- MC path decorrelation relies solely on PCG64 stream id with identical state seed (statistical-quality hardening) — `engine.rs:508-511`.
- TTM/rolling windows are entry-count-based, not calendar-bound (defensive only; history is dense today) — `formula_helpers.rs:106-121`.
- Garbled error message in precedence resolution ("has no visible under the active as_of policy value") — `precedence.rs:61-69`.
- `waterfall_currency` defaults to USD for empty instrument sets — `waterfall/payment_stack.rs:43`.
- Misleading comment "PIK accrues on the post-sweep balance" (it doesn't) — `waterfall/mod.rs:320-322`.
- Negative category values silently zeroed by cash caps — `cash_distribution.rs:33-41`.
- SICR PD-delta horizon clamps at min 1y even for sub-1y exposures — `staging.rs:247`.
- Portfolio EAD aggregation reads first-bucket EAD and silently zero-fills unmatched ids (inflates coverage ratio) — `ecl/portfolio.rs:73-79, 126-139`.
- `compute_waterfall` hardcodes `write_offs: 0.0`, folding write-offs into derecognitions (IFRS 7.35I mislabeling) — `portfolio.rs:324`.
- Unbounded `remaining_maturity_years` → unbounded bucket allocation — `engine.rs:425-426`, `cecl.rs:170-171`.
- `PLSummaryReport` prints `0.00` for missing line items — `reports.rs:376`.
- Orchestrator silently drops non-positive EV as LTV reference — `valuation/orchestrator.rs:381-384`.
- Comps `peer_stats`/`z_score` not NaN-hardened (NaNs poison mean/std) — `comps/stats.rs:35-95`.
- MC shock model ignores denominator inversion (Jensen term sign for leverage ratios; second-order) — `credit/covenants.rs:260-291`.
- Vintage curve coefficients truncated to 6 decimals in generated formulas — `templates/vintage/mod.rs:40-47`.
- Scorecard `execute` never calls `validate_config`; unknown scale silently falls back to S&P — `scorecards/mod.rs:94-102`.
- Renewal free-rent attribution books full contractual rent as `free_rent` instead of probability-weighting (identity still holds) — `templates/real_estate/mod.rs:894-905`.
- Adjusted net debt: no restricted-cash haircut, gross (untaxed) pension, no lease-capitalization helper — documented-workaround gaps.
- Real-estate template gaps: no NNN recoveries, percentage rent, TI/LC, or cap-rate/DSCR/debt-yield/LTV node helpers (the CRE covenant template expects metric nodes nothing here generates).
- Corkscrew: tolerance doc error ("0.01 ... 1 basis point"); change-sign convention undocumented (reductions must be stored negative); single-period run vacuously valid.
- WASM comps: `null` vs `undefined` inconsistency contradicting `index.d.ts` `| null` declarations — `finstack-quant-wasm .../comps.rs`.
- WASM `goalSeek` silently ignores a half-specified bound (unbounded Newton instead of bisection) — `mod.rs:150-153`.
- `values_scalar` alias on Python `MixedNodeBuilder` (forbidden alias) — `builder.rs:111-114`.
- Parity contract lacks `[wasm_statements_subset]`/`[wasm_statements_analytics_subset]` sections — structural drift invisible to parity tests.
- `goal_seek` no-update return shape differs (Python `""` vs WASM omitted field).
- Test gaps: waterfall integration tests bypass the scale pipeline (would have caught B2); ECL has no integration/golden tests (would have caught B1); EWM unit test pins a wrong value.

---

## Open Questions / Assumptions

- Test suites were not run; the evaluator findings F1/F4 and others were reproduced with throwaway probe tests. Fix verification should use targeted `mise run rust-test` invocations per crate.
- The statements engine is f64-based with honest `NumericMode::Float64` stamping — the workspace "Decimal by default" invariant is knowingly deferred there, treated as accepted design, not a finding.
- B3 lives in `finstack-quant-covenants`, which the statements-analytics credit bridge re-exports/delegates to; treated as in scope since the exposed credit workflow routes through it.
- The ECF sweep amortization treatment could be a deliberate definition choice, but the asymmetry with the fees deduction strongly suggests an oversight — confirm against the intended LPA definition.

## Brief Summary

Architectural hygiene is strong: precedence (Value > Forecast > Formula), DAG cycle detection, survival-adjusted marginal PDs, EIR discounting, Gordon T+1 flow, g<WACC validation, breach-direction conventions, currency-safety enforcement, IndexMap-everywhere determinism, and no-panic FFI were all explicitly checked and held up. Defects cluster in three places: (1) **boundary/convention seams** — half-open-period off-by-one (B2), as-of visibility, `season_start`, mid-year-on-quarterly, per-period-vs-annual vol; (2) **silent degradation paths** — cash shortfalls, CECL empty sources, dead config knobs, corkscrew "Success", NaN-passes-forecast; (3) **identity/alignment bugs** — covenant instance keys, comps x/y pairing. Test coverage is the residual risk.

**Suggested fix order:** B1–B4 first (materially wrong numbers or wrongful default, no error surfaced), then the silent-cash-leak cluster in the waterfall (shortfalls, ECF, PIK clamp), then the look-ahead pair in the evaluator, then the WASM formula-check gap and `.pyi` regeneration.

## Quant Notes

- **ECL Stage 3:** IFRS 9 5.5.33/B5.5.33 — allowance = gross carrying amount − PV(expected recoveries at EIR), i.e. PD ≡ 1; a configurable time-to-recovery for discounting is the common practical parameter.
- **ECF definitions:** S&P LCD / LSTA model credit agreements deduct *scheduled* debt repayment from ECF before applying the sweep percentage — matching the fees treatment already in the code.
- **EWM variance:** pandas `adjust=False, bias=False` correction is `1/(1−Σŵ²)` over the normalized recursion weights (RiskMetrics TD4 convention); the current hybrid matches neither pandas mode.
- **Terminal value frequency:** Koller et al. (*Valuation*) and Damodaran define the continuing-value formula on annualized FCF; for sub-annual grids standard practice is trailing-twelve-month aggregation of the final explicit year.
- **Comps scoring:** standard rich/cheap composites z-score the regression residual against the peer residual distribution (residual/σ_residual), fixing both unit-mixing and weight meaningfulness.
