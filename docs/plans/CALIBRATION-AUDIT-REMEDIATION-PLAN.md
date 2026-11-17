# Calibration audit remediation plan

**Created:** 2026-11-16 at HEAD `8f0beb709d`.
**Replaces:** the 6-slice "Remediate Calibration & Binding Blockers and Major
Defects" draft (8 Blockers, 13 Majors).
**Covers:** the audit findings B1–B8 and M9–M21, re-planned against the
accepted records in
[calibration-framework](../decisions/financial/calibration-framework/README.md),
[calibration-targets](../decisions/financial/calibration-targets/README.md),
[calibration-rate-models](../decisions/financial/calibration-rate-models/README.md)
and
[models-and-valuations-credit](../decisions/financial/models-and-valuations-credit/README.md),
and their entries in
[financial/FOLLOW_UPS.md](../decisions/financial/FOLLOW_UPS.md).

## How this differs from the draft

The draft was checked finding by finding against current source and the
records accepted on 2026-11-15. Three things changed:

- **Seven findings are dropped or narrowed** because the code does not behave
  as described. See [Dropped findings](#dropped-findings).
- **Twelve fixes are replaced.** The symptom is real, but the draft's fix
  adds something an accepted record retires (nearest-tenor matching, clamps
  and penalties, index-prefix inference, fallbacks, defaulted flags). Each
  is now a reconciliation slice for the record that owns it.
- **Four points need an owner decision** before any code. See
  [Owner decisions](#owner-decisions).

## Ground rules

- **No decisions are reopened.** Each slice implements the named items of a
  record's "Accepted decision" section. Where a record is silent, the slice
  waits on the matching row in [Owner decisions](#owner-decisions).
- **No fallbacks, defaults or aliases** (AGENTS.md). A new wire field is
  required unless its record gives it an optional contract. No
  `#[serde(default)]` on new fields.
- **A slice is a full triplet:** Rust, then Python (`src/bindings`, `.pyi`),
  then WASM (`src/api`, `exports/**`), then `parity_contract.toml`, then
  `mise run gen-write`. Regenerated schemas, fixtures, `index.d.ts` and UI
  fixtures land in the same commit as the Rust change that moves them.
- **Every slice carries a regression test for its audit symptom** plus the
  boundary rows its record lists.
- **Gates per slice:** `mise run rust-lint-crate -- <pkg>` and
  `mise run rust-test-crate -- <pkg>` for each touched crate, then
  `mise run gen-write` when a wire type, binding or facade moved, then
  `mise run slice-gate`. Run `mise run all` once per phase.
- **One slice, one commit**, including its FOLLOW_UPS.md edit.
- **Read the record first.** Phase 2 slices list the record items they
  implement. This plan quotes the items that decide the fix; the
  implementer reads the full accepted section before coding, because each
  record is a complete package and a slice must not add anything the same
  record retires elsewhere.

## Phase 1: aligned fixes, ready to implement

These need no decision. Order is by dependency; slices 1.1–1.4 and 1.7–1.8
are independent of each other.

| Slice | Finding | Crates | Record items |
|---|---|---|---|
| 1.1 | B6 SVI gate | models, calibration | targets 12.3 |
| 1.2 | M16b SABR cap | models, calibration | targets 11.5, 14.3 |
| 1.3 | B3c SR3 index | valuations-core | framework 05.3 |
| 1.4 | B3b futures on a discount-only target | calibration | framework 05.4, targets 03.6 |
| 1.5 | M18, M21 dry-run and datum validation | calibration, py | framework 02.3, 02.6 |
| 1.6 | M19 report statistics and absence | calibration, py, wasm | framework 14.1, 14.3, 14.6 |
| 1.7 | M10c parametric validation | calibration | framework 16.7 |
| 1.8 | M21 binding gaps | py | none |
| 1.9 | M13 base correlation | calibration, py, wasm | targets 15.2, 15.3, 15.4, 15.7 |
| 1.10 | M16a dividend payment date | core, calibration | targets 09.4 |

### 1.1 SVI native volatility gate (B6)

`models/src/volatility/svi.rs` converts variance RMSE to vol with
`rmse_w / (2 * sqrt(avg_w))`, which omits the `1/sqrt(T)` factor, and gates
on a hard-coded `0.005`.

- Re-evaluate the fitted slice at the original log-moneyness points and
  compare `sqrt(max(w(k), 0) / T)` with each observed vol.
- Take the cap as an explicit finite non-negative parameter of
  `calibrate_svi_with_solver`. Zero requires exact agreement. Remove the
  internal constant.
- `calibration/src/targets/svi.rs` passes
  `config.vol_surface.validation_tolerance`.
- Record 12.3 says "slice cap" without naming the statistic. Records 11.5
  and 14.3 gate every quote for SABR, so gate the maximum absolute error
  here too. Confirm against
  [svi-fit-and-density](../decisions/financial/models-analytic/svi-fit-and-density.md)
  before coding.
- Tests: the same smile at `T = 0.02` and `T = 5.0` passes or fails the cap
  identically in vol units.

### 1.2 SABR absolute volatility cap (M16b)

`targets/vol.rs:157` and `targets/swaption.rs:227` pass
`tolerance / min_quote` into a relative limit. A zero quote makes it
infinite.

- Make `SabrCalibrator`'s quote-fit limit an absolute annual-volatility
  cap, checked per quote. Records 11.5 and 14.3 retire the conversion, so
  replace the relative limit if these two targets are its only callers;
  do not add a second mode beside it.
- Tests: a high-vol wing and a low-vol wing get the same error budget.

### 1.3 SR3 fixes on the overnight index (B3c)

`ir_future_conventions.json` gives `CME:SR3` the term index `USD-SOFR-3M`,
so daily observations are read from `FIXING:USD-SOFR-3M`.

- Change `index_id` to `USD-SOFR`, matching `CME:SR1`.
- Both indices are USD, ACT/360, modified following, `usny`, so the
  rate-index and day-count lookups do not move.
- `mise run gen-write`: `valuations/src/schema/examples.rs:910` and
  `finstack-quant-py/tests/test_valuations_wasm_parity.py:621` change.
- Check which instruments own the three `FIXING:USD-SOFR-3M` entries in
  `docs-site/src/components/registry-gallery/data.json` before committing.

### 1.4 Reject futures that cannot constrain a discount target (B3b)

A future priced on an external forward curve has no discount sensitivity:
`ir_future/types.rs:713-716` fetches the discount curve and discards it.

- In `DiscountCurveTarget`, reject `RateQuote::Futures` when the projection
  curve is not the curve being fitted. Record 05.4: "The undiscounted live
  mark has no discount-only calibration exposure."
- Single-curve futures are slice 2.3, not this slice.

### 1.5 Dry run validates settings and market data (M18, M21)

`api/validate.rs::validate` never checks `plan.settings`; `calibrate` does,
so a clean dry run can fail at execution. Non-quote data is first checked
when the context is built.

- Call `plan.settings.validate()` in `validate` and report it at
  `/settings`.
- Add Rust `validate()` for the non-quote `MarketDatum` kinds: FX spot
  finite, positive and `from != to`; credit-index recovery and constituent
  count; dividend schedules; inflation fixings. Call it from `validate` and
  from context construction, so there is one check.
- `PyFxSpotDatum::new` calls the new Rust check. This is the binding half
  of M21 and cannot be done before the Rust half.
- Not in this slice: typed read keys. A prior-market object of any kind
  with a matching ID still satisfies a read. That is framework 03.1 and
  belongs to its reconciliation entry (FOLLOW_UPS line 865).

### 1.6 Report statistics and explicit absence (M19)

`report.rs:564-566` pools raw step residuals of different units into one
plan-level max and RMSE.

- Compute plan-level statistics from `q_i = |r_i| / tau_i` only. Remove the
  pooled native plan max and RMS (14.3).
- Change `QuoteQuality::sensitivity` to an explicit optional. Remove the
  `NaN` placeholder at `report.rs:492` and the `0.0` placeholder at
  `solver/global.rs:1143`; both are what 14.1 calls synthetic placeholders.
- The draft's claim that `NaN` breaks `from_json` did not reproduce: no
  stored report carries it. The change is still right under 14.1 and 14.6.
- Wire change: `.pyi`, generated TypeScript, schema and parity ledger move
  in this commit.

### 1.7 Parametric curves pass post-fit validation (M10c)

`step_runtime.rs:612-614` publishes a parametric curve without
`validated_curve_outcome`.

- Route `StepParams::Parametric` through the same validation gate as the
  other curve steps, using the analytic discount evaluator (16.7).
- Do not touch the hazard cap or the negative-rate switch here; that is
  slice 2.5.

### 1.8 Python binding gaps (M21)

- Add the `explanation` getter to `PyCalibrationReport` and its `.pyi`.
- Add `"finstack_quant.calibration."` to `PROPERTY_CHECKED_PREFIXES` in
  `tests/parity/test_parity_ledger.py` and record every calibration
  property in `parity_contract.toml` in the same commit. `backlog_ceiling`
  does not rise.
- Dropped from the draft: WASM `validate*` and Python `from_json` already
  validate.

### 1.9 Base correlation identification and ordering (M13)

`targets/base_correlation.rs` searches `[prev, 0.999]`, accepts unbracketed
knots, never checks attachments, and names its output `{index_id}_CORR`.

- Require the first tranche to attach at zero and each later tranche to
  attach at an exact previously fitted detachment (15.2).
- Add a required ordering election to `BaseCorrelationParams`: none, or
  non-decreasing. Under "none" each detachment searches the full declared
  interval and real drops are kept. Under "non-decreasing" the previous
  correlation is the lower bound (15.4). No default.
- Take correlation bounds from params; remove the `0.999` literal (15.3).
- Return `false` from `allow_approximate_knots`: an unbracketed knot is a
  failed fit (15.3).
- Require an explicit output curve ID; remove both `_CORR` format sites
  (`base_correlation.rs:380`, `api/schema.rs:647`) (15.7).
- Do not add the draft's expected-loss monotonicity check as a substitute
  for ordering. Record 15.4 applies the model's own loss checks on the
  pricing path.
- Left for the record's full reconciliation (FOLLOW_UPS line 909): the
  synthetic second knot and the fixed `1e-3` budget.

### 1.10 Dividends discount from their payment date (M16a)

`targets/util.rs:233` discounts each cash dividend at its ex-date and never
checks its currency. `DividendEvent` has no payment date.

- This starts in core. Add the payment date to `DividendEvent` as
  [dividend-events](../decisions/financial/core-market/dividend-events.md)
  specifies; read that record before choosing the field shape.
- Then compute prepaid spot as `S − Σ d_i D(V, P_i)` and reject a dividend
  whose currency differs from spot (09.4).
- Wire change to a core type: every consumer of `DividendEvent`, the
  schemas and both bindings move in this commit.

## Phase 2: reconciliation slices

Each symptom is real. Each fix is the record's, not the draft's. The
"Not this" column is what the draft proposed and the record rules out.

| Slice | Finding | Build | Not this | Record items |
|---|---|---|---|---|
| 2.1 | B1 | Quote states clean `u` or cash `q` and its settlement date; builder converts `C = N0·u − c·H`; instrument holds actual dated cash | Defaulted `upfront_is_clean` flag on `CdsTranche`; T+1/T+3 from the index-name prefix; an NPV accrued add-back | framework 08.2, 08.4; credit 18.1, 18.5 |
| 2.2 | B5 | Explicit copula selection shared by value, risk, cashflows and calibration; Student-t fit on a bracketed root with an explicit tolerance and an authored scalar key | Copula on `CreditIndexData`; rebinding the index; `{index_id}_STUDENT_T_DF`; a core duplicate of `CopulaSpec` | credit 17.1, 09.1, 09.6; targets 16.3, 16.5, 16.6 |
| 2.3 | B3a | Single-curve futures project from the fitted curve through an explicit link | `.ok()` fallback from forward to discount curve | framework 20.1, 05.3, 05.4 |
| 2.4 | B4 | Admit foreign factors above one; remove the `1.5` cap; one shape rule for trial and final; reject non-local interpolation under bootstrap; reject a calibrated currency that is not the convention base | Either-orientation legs; spline polish; `foreign_ois_discount_id` | targets 07.1, 07.4, 07.5, 07.7 |
| 2.5 | M10a, M10b | Remove the universal `0.50` hazard cap and the generic negative-rate switch from post-fit validation | Raising the cap to `10.0`; wiring validation to `AutoCurrency` | framework 16.3, 16.4, 02.2 |
| 2.6 | M11 | Coordinate transforms for bounded parameters; typed errors in place of penalty residuals; one-sided stencils chosen before pricing | Softer clamp, distance-scaled penalty, backward-difference fallback | framework 12.2, 12.3, 15.3 |
| 2.7 | M12 | Explicit intensity bounds on the logistic map; explicit per-row weights | Silent recovery override; hard-coded `1/RPV01²` | targets 05.1, 05.4; framework 07.1, 12.1 |
| 2.8 | B2, M20 | Replay targets exact quote IDs for rates and hazard; recipe carries effective solver, weighting and acceptance settings; futures match on reference end | Nearest-tenor matching with a `0.25Y` or `0.45Y` cap | framework 19.1, 19.3, 21.3; framework 05.6 |
| 2.9 | M14 | One index identity for the existence check, the currency check and the instrument; explicit CPI bounds | New hard-coded bounds; spline polish | targets 06.4, 06.5 |
| 2.10 | M15 | Elected non-overlapping `tau1` and `tau2` ranges; explicit per-quote weights; remove the `+0.5` repair in `clamp_params` | Smooth penalty; default PV01 weights | targets 08.2, 08.3, 08.4 |
| 2.11 | B7 | Dated references per node; interpolated nodes get a dated forward, not a month-rounded one; reject non-positive `target_expiries` | Snap within 5 business days | targets 13.5, 14.1, 14.4 |
| 2.12 | B8b | Strict lookup in place of `get_surface_vol_clamped` at `lmm.rs:292` | Cube-then-surface fallback; co-terminal least squares; Normal-vol conversion | rate-models 09.1, 09.3 |
| 2.13 | found in passing | Include the first cap coupon when the quote convention includes it (`step_runtime.rs:256` drops it unconditionally) | n/a | rate-models 02.1 |

### Notes that change how a slice is built

- **2.1.** Removing the `start_date` short-circuit is a no-op for
  calibration: calibration tranches always set `start_date`, and the
  returned date is never used for discounting. It goes as part of retiring
  prefix inference (18.1), not as a pricing fix. The existing Student-t
  fixture treats `upfront_pct` as cash ("The quote is cash due at
  settlement"), so the quote's representation must become explicit before
  any conversion is added.
- **2.2.** Today a non-Gaussian copula is reachable only by calling
  `CdsTranchePricer::with_copula` directly. `CdsTranchePricer::new()` is
  hard-wired Gaussian in `base_value`, the cashflow schedule, five metric
  calculators and `BaseCorrelationTarget`. The selection must reach all of
  them in one slice, or value and risk diverge. Do not tighten the root
  tolerance to `1e-10` as an accuracy claim: Student-t uses a fixed
  20-node product rule, and FOLLOW_UPS line 453 tracks qualifying it.
- **2.6 before 2.7.** Hazard bounds use the transform that 2.6 introduces.
- **2.8.** The hazard side already matches pillars bit-exactly and errors
  on a miss. The work there is moving from a year-fraction target to quote
  IDs. The rates side still uses `find_closest_quote` and accumulates
  repeat hits. `QuoteBump::TenorsBp` changes shape for both; plan the
  callers in scenarios and attribution before starting.
- **2.9.** Whether the synthetic swap should start at spot instead of
  `base_date` is not settled by the items read here; see
  [Owner decisions](#owner-decisions).

### Suggested order

1. 2.6, then 2.7 and 2.5 (solver and validation base).
2. 2.1, then 2.2 (tranche contract, then copula).
3. 2.3, 2.4, 2.9, 2.10 (curve targets, independent of each other).
4. 2.8 (replay; widest blast radius outside calibration).
5. 2.11, 2.12, 2.13 (vol and rate models).

## Dropped findings

| Finding | Why |
|---|---|
| B2 as described | There is no `0.1Y` window. `recalibration/hazard.rs` matches bit-exactly and errors on a miss. The real work is slice 2.8. |
| B4 basis-spread curve | `BasisSpreadCurve` no longer exists in the tree, and targets 07.7 retires it. |
| B8a multi-curve Hull-White | Framework 18.2 specifies a shared single-curve evaluator. Supporting two curves is a record change; see Owner decisions. |
| B8b co-terminal fit, Normal vols | Rate-models 09.1 fits the earliest slice only; 09.3 specifies BlackLognormal. |
| M12 recovery override | Targets 05.1 and framework 07.1 require exact recovery agreement. The current error is correct. |
| M16c vol-bound mismatch | Both sites require finite, non-negative vol. |
| M17 cap maturity check | Accrual dates are unadjusted and an in-tree test covers a Saturday maturity. |
| M21 WASM `validate*`, Python `from_json` | Both already validate. |

## Owner decisions

| # | Question | Why it blocks |
|---|---|---|
| 1 | **M9.** `resolve_spot_date` with a zero lag on a closed month-end rolls before the trade date. Reject a closed trade date for zero lag, or roll Following for zero lag only? | Core-conventions 006.6 says a closed start "is not pre-rolled", and no record special-cases zero lag. The draft's pre-roll also moves T+2 from a Saturday from Tuesday to Wednesday. |
| 2 | **B8a.** Should Hull-White swaption calibration support separate projection and discount curves? | It needs an amendment to framework 18 and rate-models 03 first. |
| 3 | **B8b.** The swaption step writes a `VolCube`; LMM calibration reads a 2-D `VolSurface`. Which is the LMM's named source? | Rate-models 09.3 names a BlackLognormal expiry-by-strike grid. Reading the cube needs a decision, not a fallback. |
| 4 | **M14.** Should the synthetic inflation swap start at convention spot? | The schema documents `base_date` as the instrument start on purpose; the record items read for this plan do not settle it. |

## Closing register entries

Each Phase 2 slice edits its FOLLOW_UPS.md entry in the same commit: delete
it if the slice completes the record's package, otherwise rewrite it to
what remains. Phase 1 slices rewrite only the part they close. Slices 1.3,
1.8 and the M9 decision have no existing entry.
