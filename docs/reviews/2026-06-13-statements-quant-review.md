# Quant Finance Review — `statements`, `statements-analytics`, `covenants`, and Bindings (follow-up)

**Date:** 2026-06-13
**Scope:** `finstack-quant/statements`, `finstack-quant/statements-analytics`, `finstack-quant/covenants`,
`finstack-quant-py/src/bindings/{statements,statements_analytics,covenants}`,
`finstack-quant-wasm/src/api/{statements,statements_analytics,covenants}`, `.pyi` stubs,
JS facades, `parity_contract.toml`.
**Method:** Six parallel deep passes (evaluator/forecast, capital structure, ECL/CECL,
valuation/scenarios/comps, credit/extensions/templates/covenants, bindings). This is a
**follow-up** to `2026-06-09-statements-quant-review.md` and `2026-06-12-covenants-quant-review.md`;
every prior Blocker/Major was re-checked against current source. The two new Blockers and the
covenants NM-ratio divergence were re-verified by direct source inspection (line evidence inline);
several findings were reproduced with throwaway probe tests (since deleted). Test suites not run.

---

## Headline

The **2026-06-09 statements review was substantially remediated** — all four prior Blockers
(B1 ECL Stage-3, B2 period-flows off-by-one, B3 project-finance covenant labels, B4 comps x/y
pairing) and the large majority of prior Majors/Moderates are fixed, with regression tests. That
is genuinely strong follow-through.

Two **new Blockers** were introduced or missed during remediation, plus the **2026-06-12 covenants
review is only partially remediated** (the headline B1 negative-EBITDA convention is still open on
the forward path). New material findings below.

---

## Findings

### Blockers

**N1 — Look-ahead leak in lagged/expanding aggregates over non-column expressions** — `finstack-quant/statements/src/evaluator/formula.rs:263-268` and `:357-363`
The column path was fixed (`collect_historical_values_sorted` now filters `period > context.period_id`, `formula_helpers.rs:78,94`), but the two **expression-path** collectors were missed. `collect_expression_values_sorted` (263-268) and the non-aggregate branch of `collect_expression_window_values` (357-363) both build their period set from the full shared `context.historical_results.keys()` chained with `context.period_id` and **never filter to `<= context.period_id`**. Any aggregate over an *expression* (anything that isn't a bare `Column`/`Literal` — e.g. `rolling_mean(x*1.0, 2)`, `cumsum(a+b)`, `std(a/2)`, `ttm(x*1.0)`) evaluated inside a lagged context pulls in periods *after* the lagged anchor.
**Impact:** Silent forward-looking bias → wrong numbers in any model that wraps a windowed series in arithmetic. Probe-confirmed: `lag(rolling_mean(x*1.0, 2), 2)` returned **55.0** vs the correct **45.0**; `lag(cumsum(x*1.0), 2)` returned **180.0** vs **120.0**. The existing guard test `test_lagged_rolling_mean_has_no_look_ahead` uses only a bare column, so it misses this.
**Fix:** In both functions filter the collected periods to `period <= context.period_id`, mirroring `formula_helpers.rs:78,94`. Cap the `Literal` arms (252-258, 350-353) too for `cumsum/cumprod` of a literal.

**N2 — ECF sweep double-spends cash whenever `Amortization` does not strictly precede the sweep** — `finstack-quant/statements/src/capital_structure/waterfall/mod.rs:217` (+ `waterfall/excess_cash_flow.rs:76-83`)
The 2026-06-09 "ECF sweep double-spends cash" Major was *partially* fixed: a scheduled-principal deduction was added but **gated on cash-cascade ordering** — `deduct_scheduled_principal = amortization_priority < extra_principal_priority`. `priority_index` returns `usize::MAX` when a priority is absent (`payment_stack.rs:10-16`), so when `Amortization` is **omitted** from `priority_of_payments` (`MAX < n` is false) or when **`Sweep` ranks before `Amortization`**, the deduction is skipped — yet scheduled principal is still paid in full from the schedule. ECF is by definition *post*-mandatory-amortization, so the base should never include cash already committed to scheduled principal regardless of cascade position.
**Impact:** Overstates debt paydown by the full scheduled-amortization amount each period and understates leverage — corrupts debt-balance, deleveraging, and credit-metric output in a live deal model. Probe (EBITDA 1000, cash interest 100, scheduled amort 200, sweep 100%) with `[Interest, Sweep, Amortization]` or with `Amortization` omitted: total debt service **1200 vs EBITDA 1000**. Silent — the pure-ECF path (no `available_cash_node`) reports no equity residual, so no cash-conservation check catches it. The same root cause applies to the fee deduction (`mod.rs:278-287`).
**Fix:** Deduct scheduled principal (and fees) from the ECF base unconditionally — `deduct_scheduled_principal = true`, or derive it from whether scheduled principal is actually paid this period, not from cascade order.

**N3 — Forward covenant forecasting ignores the negative-EBITDA (NM-ratio) convention the engine enforces** — `finstack-quant/covenants/src/forward.rs:224-232, 302-311, 363-375`
Carry-over of covenants-review B1 and the 2026-06-09 "negative-EBITDA leverage passes max-leverage covenants" finding: the engine point-in-time path now correctly breaches a `is_ratio_max` covenant whose metric is `< 0` (negative EBITDA → "NM"), at `engine.rs:1093`. **`is_ratio_max()` is called only at `engine.rs:1093` and never in `forward.rs`** (confirmed by grep) — all three forward breach-determination sites still use plain `v > t` (with only a NaN guard added). `forecast_breaches_generic` likewise detects on `headroom < 0` and `headroom_for` returns positive headroom for the negative ratio.
**Impact:** A distressed, negative-EBITDA borrower shows up in a forward max-leverage projection with apparent cushion and **0% breach probability** — the inverse of the true signal, in exactly the distressed regime where forecasting matters most, while the engine flags the same input as breached. Probe (`MaxDebtToEBITDA{5.0}`, metric `-2.0`): forward `headroom=1.4, breach_prob=0, first_breach=None`; engine `passed=false`.
**Fix:** Route both engine and forward through one shared `is_breached(covenant_type, value, threshold)` helper, or replicate `v.is_nan() || (covenant_type.is_ratio_max() && v < 0.0) || …` at all three sites.

**N4 — CECL has no credit-impaired / PD=1 path; defaulted obligors are priced with the performing PD curve** — `finstack-quant/statements-analytics/src/analysis/ecl/cecl.rs:208-252`
The IFRS-9 B1 Stage-3 fix (`engine.rs:513-534`, PD≡1, `LGD×EAD×DF(t_recovery)`) was **not mirrored in CECL**. `compute_cecl` never inspects `days_past_due`, qualitative default flags, or any impairment state — a 120-DPD or bankrupt obligor runs through the same forward PD-curve integration as a performing loan.
**Impact:** CECL allowance on already-defaulted assets is computed from a low performing PD instead of `LGD × EAD` (PD≈1), materially **understating the provision on the worst loans**. An auditor reviewing ASC 326 numbers would challenge this directly. (See OQ — confirm whether impaired-asset treatment is intended to be engine-side, as IFRS-9 is, or caller-side as pooling is.)
**Fix:** Add a PD=1 / impaired branch to `compute_cecl` analogous to `compute_ecl_single`'s Stage-3 path, keyed off DPD ≥ a configurable default threshold and/or a default-evidence flag.

### Major

**N5 — PD-delta SICR trigger hard-errors on a rating downgrade with the only shipped PD source** — `finstack-quant/statements-analytics/src/analysis/ecl/staging.rs:266-273`
The SICR PD-delta compares `pd_source.cumulative_pd(orig_rating, h)` vs `cumulative_pd(curr_rating, h)` against a *single* `pd_source`. The library's only concrete `PdTermStructure`, `RawPdCurve`, is single-rating and **errors** for any other rating (`types.rs:477-481`). Probe: origination AAA / current BBB with a BBB curve → `classify_stage` returns `Err("RawPdCurve is for rating 'BBB', got 'AAA'")`, which propagates out of `process_exposure` and fails the whole exposure.
**Impact:** The rating-migration SICR trigger throws a hard error in exactly the scenario it exists to detect (a downgrade). With the shipped curve type, any exposure whose `origination_rating != current_rating` cannot be staged.
**Fix:** Treat a missing-origination-curve lookup as "no PD-delta trigger" rather than an error, or ship a multi-rating curve type; add a regression test for `orig != curr` with `RawPdCurve`.

**N6 — Unpaid scheduled amortization under an available-cash cap is silently dropped** — `finstack-quant/statements/src/capital_structure/waterfall/mod.rs:446-456, 481-504`
The cash-shortfall accounting added for interest/fees (`mod.rs:481-504`) does **not** cover principal. When the available-cash cap starves scheduled `Amortization`, `scheduled_principal` is reduced to the funded amount with no record, no `interest_shortfall` entry, and no `EvalWarning`. Probe (available cash 100, interest 100, scheduled amort 200): interest paid 100, principal 0, balance unchanged — and **no warning**.
**Impact:** The debt balance stays correct (principal still owed), but a missed mandatory amortization — an event of default / covenant trigger in a live credit book — produces zero signal. Cash-conservation auditing of the priority stack is also incomplete.
**Fix:** Mirror the interest/fee shortfall handling for the `Amortization`/prepayment categories — record unpaid scheduled principal and raise a structured payment-default warning.

**N7 — Persistent breach re-records a fresh cure deadline and re-applies consequences every test date** — `finstack-quant/covenants/src/engine.rs:864-868, 929-934` (covenants-review M2, OPEN)
`evaluate_and_track` dedups on `breach_date == test_date`, so a continuously-breaching covenant tested quarterly creates one record per quarter, each with a new `cure_deadline = test_date + cure_period`; `apply_consequences` dedups on `(covenant_id, breach_date)` and so re-applies the full consequence set per record. Probe: one persistent breach → 2 records → 2 `RateIncrease` applications (e.g. +400 bp for one breach).
**Impact:** A stuck borrower gets a fresh cure window every quarter (the default clock never effectively arrives) and a compounding margin step-up. LSTA-style cure periods run from breach/notice, not from each compliance certificate.
**Fix:** Key the active-breach lookup on the open breach *episode* (same covenant, `!is_cured`, any date); open a new record only when no uncured episode exists; carry the original cure deadline forward.

**N8 — Cure-by-recovery is documented but never implemented; `is_cured` is permanently `false`** — `finstack-quant/covenants/src/engine.rs:15-18, 592, 843-902` (covenants-review M3, OPEN)
The module contract says a breach is neutralized "by the metric recovering before the cure deadline," but no code path sets `is_cured = true`. Probe: after the metric recovers to passing, all prior breach records stay `is_cured=false`.
**Impact:** `find_active_breach` keeps reporting "in cure period"/active for a recovered borrower; `apply_consequences` keeps treating recovered breaches as live once their stale deadline passes (compounds N7). A recovered credit never clears its breach history.
**Fix:** In `evaluate_and_track`, when a covenant passes on a test date ≤ an open breach's cure deadline, mark that breach `is_cured = true`.

### Moderate

**N9 — Covenants crate performs no strict inbound deserialization (`deny_unknown_fields` absent everywhere)** — `finstack-quant/covenants/src/{engine,schedule,report}.rs` (covenants-review M4, OPEN)
Zero `deny_unknown_fields` in the entire crate (vs 26 in statements-analytics), violating the workspace "strict serde on inbound" invariant. Probe: a `CovenantWaiver` with a typo'd `expiry_dat` deserializes to `expiry_date = None`, i.e. a **permanent** waiver; `validate_covenant_engine_json` accepts arbitrary unknown fields.
**Fix:** Add `#[serde(deny_unknown_fields)]` to all inbound covenant types (watch `skip`/`flatten` interactions on `CovenantSpec`/`CovenantEngine`).

**N10 — `CovenantForecast` with NaN/±∞ cannot round-trip JSON; min-headroom summary is NaN-poisoned** — `finstack-quant/covenants/src/forward.rs:234-239, 354-361`
`serde_json` encodes NaN/Inf as `null`; a forecast carrying a NaN `headroom`/`projected_value` (the breach-signal case) and the inactive-springing `+∞` headroom **fail to deserialize**. Separately, the min-headroom scan `headroom[i] < headroom[min_idx]` can't move past a leading NaN, so `min_headroom_value` reads NaN even when later finite periods are worse.
**Fix:** Serialize non-finite floats with an explicit sentinel/`Option<f64>`; skip NaN in the min scan (fall back to NaN only if all non-finite).

**N11 — Negative EIR silently inflates ECL above the undiscounted loss** — `finstack-quant/statements-analytics/src/analysis/ecl/engine.rs:517,565`, `cecl.rs:240`, validation `types.rs:252-257`
`Exposure::validate` only requires `eir > -1`; discounting is `1/(1+eir)^t`, so a negative EIR makes `DF > 1`. Probe: `eir = -0.5` Stage-2 ECL = 402,009 vs 45,000 at `eir = 0` (9× inflation). IFRS 9 B5.5.44 / ASC 326 discount at the (positive) EIR.
**Fix:** Reject `eir < 0` in `validate()` (or clamp `DF ≤ 1`); document the convention.

**N12 — CECL linear reversion derives the forecast hazard from PD-curve values read beyond the R&S horizon** — `finstack-quant/statements-analytics/src/analysis/ecl/cecl.rs:284, 332`
In the reverted region the blend computes a local forecast hazard via `marginal_pd(rating, t1, t2)` where `[t1,t2]` lies past `forecast_horizon_years`. Past the curve's last knot, cumulative PD flat-extrapolates, so `λ_fcst → 0` and the blend under-weights the historical hazard exactly inside the fade window when the forecast curve is shorter than `rs + reversion_years`.
**Impact:** The reasonable-and-supportable → historical reversion can systematically under-provision during the fade window.
**Fix:** Freeze the forecast hazard at the R&S boundary value rather than re-reading the flat-extrapolated curve inside the reversion window.

**N13 — Mid-year convention discounts the annualized terminal flow with a sub-annual shift** — `finstack-quant/valuations/src/instruments/equity/dcf_equity/types.rs:716-728, 829-853`
After the prior TV-frequency fix, the Gordon/H-Model terminal value capitalizes an *annualized* flow but its discount tenor is reduced by `mid_year_shift` = half the *sub-annual* spacing (≈0.125 yr on a quarterly grid) instead of the 0.5 yr appropriate for an annual flow stream.
**Impact:** On a quarterly mid-year DCF the terminal PV uses tenor `t_n−0.125` instead of `t_n−0.5`, **understating** the terminal-value PV ~3.5% (propagates 1:1 into EV/equity).
**Fix:** When `terminal_flow_override` is set, use a fixed 0.5-year mid-year shift for the terminal discount tenor; keep the sub-annual shift for explicit flows.

**N14 — `score_relative_value` returns a different result *shape* in Python vs WASM** — `finstack-quant-py/src/bindings/statements_analytics/comps.rs:384-399` vs `finstack-quant-wasm/src/api/statements_analytics/comps.rs:113-114`
WASM returns the canonical `RelativeValueResult` serde shape `{company_id, composite_score, dimensions:[...], confidence, peer_count}`; Python hand-builds a dict that **omits `company_id`** and replaces the `dimensions` array with a `by_dimension` map keyed by `label` (silently drops a dimension when two share a label).
**Impact:** Same numbers, divergent structure — JS reads `result.dimensions[i]`, Python reads `result["by_dimension"][label]`; porting a model between hosts breaks.
**Fix:** Emit the canonical serde shape (or return the JSON string) from Python.

**N15 — `explain_formula`↔`explainFormula` parity map pairs two different return types** — `finstack-quant-py/.../analysis.rs:764` (dict) vs `finstack-quant-wasm/.../mod.rs:213-229` (string); contract `parity_contract.toml:654`
Python `explain_formula` returns a structured dict (`breakdown[...]`); WASM `explainFormula` returns only `to_string_detailed()` (the equivalent of Python's `explain_formula_text`). A Python user following the structured API who switches to JS per the parity map gets a plain string and hits `TypeError`/`undefined` on `.breakdown`.
**Fix:** Add a WASM `explainFormula` returning structured JSON (and a separate `explainFormulaText`), or relabel the contract so `explain_formula_text → explainFormula` and mark the structured Python `explain_formula` as Python-only.

### Minor

- **season_start silently ignored** in seasonal forecast — `forecast/timeseries.rs:383-507`: after the double-shift fix the param has zero effect; accepted only so it doesn't trip unknown-key validation. Reject it or apply it.
- **`apply_override` silently ignores unmatched override keys** — `forecast/override_method.rs:44-65`: a mistyped/out-of-range period key is dropped with no error → quietly wrong forecast.
- **Stage-3 ECL bypasses curve validation** — `ecl/engine.rs:510-534`: a mis-wired/stale `RawPdCurve` is silently accepted for impaired assets but errors for performing ones; masks config bugs. Optionally validate the exposure-rating curve even on the Stage-3 path.
- **CECL discounting is unconditional, no methodology gate** — `cecl.rs:240`: ASC 326 mandates PV discounting only for the DCF method, not the loss-rate/PD-LGD-EAD method. Add a `discount: bool`/methodology flag.
- **Variance bridge driver-sign economically misleading for cost drivers** — `scenarios/variance.rs:335-338`: a COGS *decrease* shows a negative contribution though it raises EBITDA; total still reconciles via `unexplained`. Document or add per-driver sign tagging.
- **Equity ≠ EV − net_debt identity breaks (undocumented) when DLOM/DLOC discounts apply** — `valuation/corporate.rs:631-639`: `equity_value` is post-discount, `enterprise_value` is pre-discount; the only reconciliation test runs without discounts. Document on the result struct.
- **Goal-seek sign clamp can make a solvable root unreachable** — `analysis/goal_seek.rs:256-263`: a positive `initial_guess` sets `bracket_min = 0`, so a required negative root returns "no sign change". Widen on first failed bracket.
- **Negative scorecard metric weights bypass validation** — `extensions/scorecards/mod.rs:252-258, 557-568`: `validate_config` never rejects individual negative weights; probe (weights +2.0/−1.0) → score 185/100 → AAA. Reject `weight < 0`.
- **`TrendCheck`/`FcfSignCheck` zero-count config flags spuriously; streaks reset across data gaps** — `checks/credit/trend.rs:78`, `fcf_sign.rs:56`. Reject `lookback_periods == 0`/zero thresholds.
- **PIK toggle with `target_instrument_ids = None` capitalizes coupons on non-PIK instruments** — `waterfall/mod.rs:400-408`, `payment_in_kind.rs:45-49`: no PIK-capability gating despite the doc claim. Require explicit targets or an instrument-level capability flag.
- **Sweep silently zeroed when a non-`Sweep` prepayment ranks after `Equity`** — `waterfall/mod.rs:272-276`: `validate()` only checks `Sweep`-before-`Equity`. Extend the ordering check to all prepayment priorities.
- **WASM `runChecks` family lacks the optional pre-computed `results` parameter** — `wasm .../mod.rs:272,290,304` vs `py .../analysis.rs:861,908,953`: WASM always re-evaluates context-free, so a Python caller passing `results` computed under a `MarketContext` can get a different report. Add an optional `results_json` param.
- **`CovenantForecast.covenant_id` holds the Display description, not `instance_key()`** — `covenants/forward.rs:155, 386`: two same-type, distinctly-labeled covenants (project-finance `min_dscr_default`/`min_dscr_lockup`) produce non-joinable forecast ids. Use `instance_key()`.
- **Covenants Python `__all__` not sorted (runtime ≠ stub)** — `finstack-quant-py/src/bindings/covenants/mod.rs:137-146` registration-ordered while `__init__.pyi:5-14` is sorted; binding standards require sorted.
- **No WASM facade tests for the covenants namespace** — `finstack-quant-wasm/tests/facade/` covers only cashflows/core/plain-object; WASM standards require facade tests.
- **`cure_period_days` accepts negative values** — `covenants/engine.rs:99, 878`: a negative cure period produces a deadline before the breach date. Validate.
- **`Exposure.dpd` class docstring marks it required while the constructor defaults `dpd=None`** — `finstack-quant-py/.../ecl.rs:111-114` (cosmetic; stub is correct).

---

## Open Questions / Assumptions

- **N4 (CECL impaired):** Is impaired/defaulted-asset measurement intended to be engine-side (as IFRS-9 Stage 3 is) or caller-side (as CECL pooling explicitly is, `cecl.rs:12-20`)? Even if caller-side, the absence of any guard against silently pricing a defaulted obligor on the performing curve is a weak safeguard. The asymmetry with the IFRS-9 path is the strongest argument it is an oversight.
- **N2 (ECF):** Confirm the intended LPA ECF definition — scheduled (mandatory) amortization should be deducted from the ECF base unconditionally; the cascade-position gating conflates "who is paid first" with "what the ECF base is."
- **N3/N7/N8 (covenants forward + lifecycle):** Confirm `forecast_*_generic` and `evaluate_and_track` are reachable from a production reporting path (vs only the statements bridge). The intended semantics assumed are "one continuous breach = one record, curable by recovery, cure clock anchored at first breach" (the standard LSTA reading).
- The statements engine is f64-based with honest `NumericMode::Float64` stamping — the "Decimal by default" invariant is knowingly deferred there and treated as accepted design, not a finding.
- Test suites were not run; new findings were reproduced with throwaway probes (deleted). Fix verification should use targeted `cargo test -p <crate> --test <file>` (avoiding doctests).

---

## Brief Summary

Remediation quality since 2026-06-09 is high: all four prior Blockers are fixed with regression
tests, the evaluator look-ahead/precedence/determinism cluster is fixed (column path, EWM bias,
seasonal shift, prepared-cache, MC ordering), the capital-structure cash-shortfall/PIK/forward-dating
cluster is fixed, the ECL Stage-3/empty-sources/methodology-validation cluster is fixed, and the
valuation/comps/scenarios convention bugs (TV frequency, residual standardization, sign direction,
discounting basis, zero-baseline, tornado anchor, bridge residual) are fixed. The bindings Majors
(WASM formula-check drop, `.pyi` keyword drift) are fixed and `parity_contract.toml` now carries the
WASM statements subsets.

Residual risk concentrates in three seams: (1) **partial fixes that re-introduced a defect** — the
ECF amortization deduction is gated on the wrong condition (N2, Blocker) and the evaluator's
expression-path collectors were missed by the look-ahead fix (N1, Blocker); (2) **the covenants
forward/lifecycle module still lagging the engine** — negative-EBITDA convention (N3, Blocker),
breach re-recording/consequence-compounding (N7), cure-on-recovery (N8), strict serde (N9), and
non-finite wire format (N10) are all open from 2026-06-12; (3) **CECL trailing IFRS-9** — no
impaired/PD=1 path (N4, Blocker) and an unusable rating-migration SICR trigger (N5).

**Suggested fix order:** N1 and N2 first (silent wrong numbers on common model shapes), then the
covenants forward/lifecycle cluster N3/N7/N8/N9/N10 (one shared `is_breached` helper + episode-keyed
breach tracking + `deny_unknown_fields` resolve most of it), then N4/N5/N11/N12 in ECL/CECL.

## Quant Notes

- **ECF definition:** S&P LCD / LSTA model credit agreements compute Excess Cash Flow *after*
  mandatory (scheduled) debt repayment, then apply the sweep percentage — the deduction must be
  unconditional, not contingent on cascade position.
- **Look-ahead in time-series formulas:** any windowed/expanding aggregate evaluated at a lagged
  anchor must restrict its history to `period <= anchor`; the column path enforces this, the
  expression path must too.
- **Negative-EBITDA "NM" ratio:** rating-agency practice (S&P/Moody's) reports leverage as "NM"
  when EBITDA ≤ 0 and treats it as the worst outcome — the engine is correct; the forecast must
  inherit it.
- **CECL impaired assets:** ASC 326 requires lifetime ECL for all assets; for credit-impaired /
  defaulted obligors the loss is `LGD × EAD` (PD≈1), not a performing-curve PD integration —
  mirroring the IFRS-9 5.5.33 / B5.5.33 Stage-3 treatment already implemented.
- **EIR discounting:** IFRS 9 B5.5.44 / ASC 326 discount expected losses at the (positive) original
  effective interest rate; a sub-zero EIR discounting a future loss *up* is economically wrong.
- **Mid-year convention:** a growth-perpetuity terminal value built from an annualized flow
  represents a year-long stream, so its mid-year benefit is 0.5 years regardless of the explicit
  grid's sub-annual spacing.
