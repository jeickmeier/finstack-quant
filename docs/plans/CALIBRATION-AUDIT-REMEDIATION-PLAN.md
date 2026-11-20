# Calibration Audit Remediation Plan (re-baselined 2026-11-19)

Replaces `~/Downloads/implementation_plan.md` (6 slices, 8 Blockers, 13 Majors).
Supersedes the 2026-11-16 version of this file (HEAD `8f0beb709d`), which
reviewed the same draft; see git history for it.
Every finding was re-checked against `master` at `1a2ff4d94c` (98 commits later;
working tree has 16 unrelated dirty valuations files).

## Verdict on the draft

By finding (see the table below): only B6 and the SR3 part of B3 keep the
draft's fix more or less as written. Five more (B3b, M13, M15, M18, M19) are
real and get a narrower or corrected fix. Nine are real symptoms whose proposed
fix contradicts an accepted decision record. M17 is refuted outright, and B2,
B4, M12, M14, M16, M20 and M21 are refuted or already fixed in most of their
parts.
Several proposed fixes call APIs that do not exist (`CurveValidationMode`,
`ctx.foreign_currency()`, `basis_spread_curve_id`, `MAX_HAZARD_RATE`,
`sabr_tolerance`, `swaption_fit_tolerance`, `ResidualUnits::Mixed`).

The draft's four "Confirmed Architectural Decisions" are **not** carried
forward. Each contradicts an accepted record or AGENTS.md (no fallbacks, no
defaulted flags):

| Draft decision | Why not | What this plan does instead |
|---|---|---|
| 1. `upfront_is_clean` defaulted flag, T+1/T+3 from index-name prefix, accrued add-back in the pricer | Changes PV of every stored tranche with an upfront; prefix inference is retired by credit 18.1 | Slice 2.1: quote states clean or cash and its settlement date; the builder converts |
| 2. `foreign_ois_discount_id` with a convention-curve fallback | `BasisSpreadCurve` no longer exists and has no consumer; targets 07.7 retires it | Dropped |
| 3. Copula on `CreditIndexData`, `{index_id}_STUDENT_T_DF` | Duplicates `CopulaSpec` in core; one tranche's fit would change every tranche on the index | Slice 2.2: explicit copula selection on the pricing path |
| 4. LMM reads cube-then-surface, Normal vols, co-terminal least squares | Rate-models 09.1 fits the earliest slice only; 09.3 names a BlackLognormal grid with strict lookup | Slice 2.12 plus decision D3 |

If any of the four is still wanted, amend its record first, then plan it.

## How to read this plan

- **The accepted decision records are the specification** (`D/` =
  `docs/decisions/financial`). A slice names the record items it implements;
  read the full accepted section before coding.
- Evidence tags: **[V]** read directly in source or record during this
  re-baseline, **[R]** reported by a re-baseline check with quoted code (confirm
  on first touch), **[U]** not verified, prove with a failing test first.
- Record items read directly this round: framework 12.2, 12.3; framework 18.2;
  targets 06.4, 06.5; targets 07.1-07.7; targets 14.1, 14.4; targets 15.2,
  15.3, 15.4, 15.7; rate-models 09.1, 09.3.
  All other item numbers are carried from the 16 Nov plan.
- Each slice is a full triplet (Rust, Python, WASM, parity ledger,
  `mise run gen-write`) in one commit, starts with the failing test for its
  symptom, and ends with `mise run slice-gate`. `mise run all` runs once per
  phase. No `#[serde(default)]` on new fields, no fallbacks, no shims.
- Paths are relative to `finstack-quant/` unless they start with
  `finstack-quant-py/` or `finstack-quant-wasm/`. `C/` = `calibration/src`.

## Per-finding verdict

| ID | Verdict | Evidence | Goes to |
|---|---|---|---|
| B1 | Symptom real only if the quote is clean; fix replaced | Builder turns `upfront_pct` straight into dated cash (`C/build/cds_tranche.rs:103-111`); Student-t fixture says "The quote is cash due at settlement" (`C/targets/student_t.rs:477`). The `start_date` short-circuit is inert. Settlement uses `conv.settlement_days` (isda_na 3, isda_eu 1), not a literal 3. [R] | 2.1 |
| B2 | Already fixed as described | No 0.1Y window: bit-exact match, hard error on a miss (`C/recalibration/hazard.rs:377-396`). [R] Residual: nominal tenors now error, see 2.8 | 2.8 |
| B3 | Real, three parts | Future never discounts (`valuations/rates/src/ir_future/types.rs:735-736` [V]); `get_forward` only (`:457` [R]); SR3 on `USD-SOFR-3M` (`ir_future_conventions.json:12` [V]) | 1.3, 1.4, 2.3 |
| B4 | Mostly refuted | DF > 1 already passes the final build; `BasisSpreadCurve` is gone. Trial and final builders are now the same call (`C/targets/xccy_basis.rs:260-266` [V]). Left: `value > 1.5` (`:294` [V]), unchecked leg orientation, bootstrap with non-local interpolation [R] | 2.4 |
| B5 | Real; fix replaced | Short-circuit and `1e-3` as root tolerance (`C/targets/student_t.rs:236-268`); scalar has no reader; Gaussian hard-wired at 8 non-test sites, the draft fixes 2 [R] | 2.2 |
| B6 | Real; fix right with two corrections | `rmse_w / (2*sqrt(avg_w))`, constant `0.005` (`models/src/volatility/svi.rs:458,599-606`) [R] | 1.1 |
| B7 | Edge targets real; negative-expiry part refuted; fix replaced | No-tolerance bounds check (`C/targets/swaption.rs:329` [V]); month rounding for interpolated nodes (`:702,712` [V]) | 2.11 |
| B8 | HW single-curve real; LMM cannot read the swaption step's output | `get_surface` + BlackLognormal + `tenors[0]` + clamped lookup (`C/lmm.rs:264,285,289,292` [V]); the swaption step writes a `VolCube` only (`C/step_runtime.rs:376` [R]) | 2.12, D2, D3 |
| M9 | Real | `resolve_spot_date` has no pre-roll (`C/build/helpers.rs:16-17` [V]); `add_business_days(0)` returns the date unrolled [R] | D1 |
| M10 | (a)(b)(c) real, (d) refuted | Hazard cap `0.50` is deliberate and test-pinned; post-fit validation ignores `AutoCurrency`; parametric skips validation and has no `CurveValidator` impl; no EUR/CHF/JPY/DKK match exists [R] | 1.7, 2.5 |
| M11 | Real, worse than stated | In-bounds points within `1e-8` of a bound also fail (`C/solver/global.rs:409-411,964` [V]); flat penalty because failure sites pass the clamped copy [R] | 2.6 |
| M12 | Recovery part refuted; weights part refuted; bounds partial | Mismatch error is deliberate and guarded twice (`C/validation/preflight.rs:231-235`, CDS pricer helper); the system is square, weights do not move the root [R] | 2.7 |
| M13 | Real | `[prev, 0.999]` bracket, approximate knots accepted, no contiguity check; the `_CORR` complaint is misdescribed (it is the output ID) [R] | 1.9 |
| M14 | Partly refuted | Existence check and instrument both use `curve_id` (`C/targets/inflation.rs:130-131,144,162` [V]), so the draft's "wrong lookup" is not a pricing bug, but the quote label accepts either `params.index` or `curve_id` (`:121` [V]): two identities remain. `residual_weights` exists (`:464-472` [R]). Left: one identity, bounds, bootstrap with non-local interpolation | 2.9, D4 |
| M15 | Real, both parts | `+0.5` push inside the objective (`C/targets/parametric.rs:147-152` [V]); default unit weights [R] | 2.10 |
| M16 | (a) already fixed, (b) real, (c) refuted | Dividends: `0281bb2d7e`, `C/targets/util.rs:235,256-257` [R]. SABR: `tolerance / min_quote` (`C/targets/vol.rs:157`, `swaption.rs:227` [V]). Vol bounds are identical at both sites [R] | 1.2 |
| M17 | Refuted | Accrual dates are unadjusted; a Saturday-maturity test passes end to end (`C/step_runtime.rs:1203-1243`) [R] | none |
| M18 | Real; one part needs a decision | `validate` never checks `plan.settings`; only `calibrate` does (`C/api/engine.rs:776-779`) [R] | 1.5, D5 |
| M19 | NaN claim refuted; pooling partly real | Ratio fields already exist; `with_plan_aggregation` then overwrites `max_residual`/`rmse` with pooled raw values (`C/report.rs:560-574`) [R] | 1.6 |
| M20 | Already fixed | Recipe stores full `StepParams` and `CalibrationConfig` (`cda46a9a6d`); futures bucket on `period_end`. Left: nearest-pillar `TenorsBp` for rates (`C/recalibration/rates.rs:959` [V]) | 2.8 |
| M21 | Mostly refuted | WASM `validate*` and Python `from_json` already validate. Left: three report getters, a docstring, `PyFxSpotDatum`, parity prefix [R] | 1.5, 1.8, D6 |

## Decisions needed before the affected slice starts

| # | Decision | Recommendation | Blocks |
|---|---|---|---|
| D1 | **M9.** Zero lag on a closed month-end rolls before the trade date. The draft's unconditional pre-roll also moves T+2 from a Saturday from Tuesday to Wednesday at 8 call sites. Core-conventions 006.6 says a closed start "is not pre-rolled". | Roll Following for zero lag only (`if settlement_days == 0`). No T+N result changes. Add one sentence to 006.6. | Slice 1.10 |
| D2 | **B8a.** Hull-White swaption calibration has one curve. A true multi-curve par strike fails with a misleading "requires ATM swaption quotes" error (`C/step_runtime.rs:195-198`). Framework 18.2 says "expressly shared single-curve evaluator". | Keep single-curve. If two curves are wanted, amend framework 18 and rate-models 03, then add a required `forward_curve_id` mirroring the cap/floor arm (`C/step_runtime.rs:453-475`), not an `Option`. | nothing here |
| D3 | **B8b.** The swaption step publishes only a `VolCube`; LMM calibration reads only a 2-D `VolSurface`, so the repo's own pipeline does not connect. Rate-models 09.3 names an expiry-by-strike BlackLognormal grid. | Amend 09.3 to name the cube at (expiry, co-terminal tenor) as the source, keep earliest-slice matching (09.1). No cube-then-surface fallback. | Slice 2.12 step 2 |
| D4 | **M14.** Should the synthetic inflation swap start at convention spot? | No. `base_cpi` is documented as the reference CPI at the start date (`C/api/schema.rs:874,887`); moving the start without redefining `base_cpi` makes them inconsistent. | nothing here |
| D5 | **M18.** Is "seed placeholder hazard and base-correlation curves in `prior_market`, then recalibrate" the supported credit-index workflow? Examples `05_cdx_base_correlation.json` and `12_full_credit_desk_market.json` rely on it. The draft's "later writes shadow initial IDs" rule rejects both. | Yes, keep it. Do not add the shadowing rule or deferred index materialization. Deferral would also need `BaseCorrelationTarget` to consume `CreditIndexState`, since the index needs the curve that step produces. Then make the dry run agree with execution (1.5 step 4). | Slice 1.5 step 4 |
| D6 | **M21 parity prefix.** Adding `finstack_quant.calibration.` to `PROPERTY_CHECKED_PREFIXES` is not a one-line change: about 12-17 properties are not TypeScript fields, the backlog ceiling is 0, and several compute (`RateQuote.implied_rate`, `CalibrationValidationReport.is_valid`, envelope `market`/`step_ids`) so they need new WASM functions. | Do it as its own slice (1.11) after Phase 1, not folded into the getter slice. | Slice 1.11 |

## Dropped from the draft (no work)

- **B2 fuzzy window, 0.45Y cap.** Matching is already exact. The cap is also inconsistent with the draft's own 30Y figure (30.69Y is 0.69 off).
- **B4 DF > 1 at final build.** `validate_positive_factor_points` only rejects `factor <= 0` (`core/src/market_data/term_structures/common/factor.rs:52`).
- **B4 basis-spread byproduct, `foreign_ois_discount_id`.** No such type or field in Rust, Python or WASM.
- **B7 negative-expiry saturation.** `year_fraction` errors on `start > end` before the cast is reached.
- **B8b co-terminal least squares, Normal conversion.** Out of scope under rate-models 09.1 and 09.3.
- **M10d currency match.** `targets/discount.rs` already uses `effective_rate_bounds(currency)`.
- **M12 recovery override.** Targets 05.1 requires exact agreement; the error is correct. `1/RPV01²` weights change nothing on a square system.
- **M13 `_CORR` "preserve existing ID".** `CreditIndexData` holds the curve by `Arc`, there is no ID field to preserve. The real item is an explicit output ID (1.9).
- **M13 expected-loss check as a substitute for ordering.** Necessary but not sufficient (omits concavity); 15.4 applies the model's own loss checks.
- **M14 index lookup, `residual_weights`.** Consistent and present.
- **M16a dividends.** Fixed in `0281bb2d7e`.
- **M16c vol bounds.** Both sites are finite and `>= 0`.
- **M17 cap maturity tolerance.** Would loosen a correct exact invariant.
- **M19 "NaN breaks `from_json`".** No stored report carries NaN; only the derived `quote_rows()` view does.
- **M20a recipe fields, M20b default config, M20b futures bucket.** Fixed in `cda46a9a6d`.
- **M21 WASM `config.validate()`, Python `wire_methods!(..., validate)`.** Already there (`C/validation/surfaces.rs:278-308`, `finstack-quant-py/src/bindings/calibration/config.rs:284-289,378-384`).
- **M21 config getters.** `sabr_tolerance` and `swaption_fit_tolerance` are not fields anywhere. `to_dict()` already exposes every config field; adding two of the ten missing nested getters would be inconsistent.
- **`StudentTParams` schedule overrides** as drafted. The list adds `roll_rule` (not on `BaseCorrelationParams`) and omits `date_regime`, the one that matters. Folded into 2.2.

---

## Phase 1: ready to implement

Slices 1.1-1.4 and 1.7-1.8 are independent. Run `mise run all` once at the end of the phase.

### 1.1 SVI native volatility gate (B6)

Crates: `finstack-quant-models`, `finstack-quant-calibration`. Record: targets 12.3.

1. In `calibrate_svi_with_solver` (`models/src/volatility/svi.rs`), re-evaluate the fitted slice at the original log-moneyness points and compare `sqrt(max(w(k), 0) / T)` with each observed vol.
2. Take the cap as an explicit finite non-negative parameter. Remove `MAX_VOL_RMSE`. **Keep the non-finite guard** (current `:606`); the draft's diff drops it and would pass a NaN.
3. Gate the maximum absolute error per quote, not RMSE, to match SABR (targets 11.5, 14.3). Confirm against `D/models-analytic/svi-fit-and-density.md` first. [U]
4. `C/targets/svi.rs:169-172` passes `config.vol_surface.validation_tolerance`. That default is `1e-3`, five times tighter than today's `0.005`; expect some SVI fixtures to move or fail and decide per fixture.
5. `calibrate_svi` keeps its signature, so the bindings do not move.

Tests first: the same smile at `T = 0.02` and `T = 5.0` passes or fails the cap identically; a NaN fit is rejected.

### 1.2 SABR absolute volatility cap (M16b)

Crates: `finstack-quant-models`, `finstack-quant-calibration`, bindings. Records: targets 11.5, 14.3.

1. Replace `SabrCalibrator`'s relative quote-fit limit (`models/src/volatility/sabr/calibration.rs:128,153-157`) with one absolute annual-vol cap per quote. Do not add a `ToleranceScale` second mode.
2. `self.tolerance` also drives LM residual acceptance (`:872,1087`), solver tolerances and ATM pinning (`:1198`). Decide each explicitly; the draft leaves them undefined. [R]
3. Remove `/ min_quote` at `C/targets/vol.rs:157` and `C/targets/swaption.rs:227`. Today a zero quote makes the limit infinite.
4. `with_tolerance` is bound in Python (`finstack-quant-py/src/bindings/models/volatility.rs:385-422`): update the binding, `.pyi`, WASM twin and ledger in this commit. Revisit `FittedSlice.max_relative_quote_error`.

Tests first: a high-vol wing and a low-vol wing get the same error budget; a zero quote does not disable the gate.

### 1.3 SR3 fixes on the overnight index (B3c)

Crate: `finstack-quant-valuations-core`. Record: framework 05.3.

1. `valuations/core/data/conventions/ir_future_conventions.json:12`: `USD-SOFR-3M` to `USD-SOFR`. Both are USD, ACT/360, modified following, `usny`, so only the fixing series ID moves.
2. Drop the draft's "resolve fixings through a convention alias" bullet; unnecessary once the data is right.
3. `USD-SOFR-3M` is also a forward-curve ID in many tests and docs; those do not move. Only `FIXING:USD-SOFR-3M` series owned by SR3 do. Check the three such entries in `docs-site/src/components/registry-gallery/data.json`, plus examples and notebooks, before committing.
4. `mise run gen-write`. Expect `valuations/src/schema/examples.rs`, `finstack-quant-py/tests/test_valuations_wasm_parity.py` and `finstack-quant-py/tests/data/calibration_wasm_parity.json` to move.

### 1.4 Reject quotes that cannot constrain a discount target (B3b)

Crate: `finstack-quant-calibration`. Records: framework 05.4, targets 03.6.

1. In `DiscountCurveTarget` (after `prepare_rate_calibration_quotes`, `C/targets/discount.rs:310`), reject `RateQuote::Futures` when the projection curve is not the curve being fitted.
2. Same rejection for `RateQuote::Fra` with an external forward: PV is `N·τ·(F−K)·DF` with `F` fixed, so there is no root unless `F = K`. Not in the draft. [R]
3. The same wiring is used by `C/targets/xccy_basis.rs:172-175` and `C/targets/parametric.rs:171-174`; apply the same rule there.
4. `replay_recipe_preserves_supported_mixed_quote_shapes_losslessly` (`C/targets/discount.rs:1192`) builds FRA and futures in a discount recipe. Check whether its projection curve is external; if so it moves to the forward target. [U]
5. Single-curve futures are slice 2.3. First add a test of what a single-curve discount step with a future does today; by reading it errors with `WrongCurveType`. [U]

### 1.5 Dry run validates settings and market data (M18, M21)

Crates: `finstack-quant-calibration`, `finstack-quant-py`. Records: framework 02.3, 02.6.

1. Move `plan.settings.validate()` into `validate` (`C/api/validate.rs:91`), reported at `/plan/settings` (the field is `envelope.plan.settings`, not `envelope.settings`). Delete the separate call in `calibrate` (`C/api/engine.rs:776-779`) so there is one path.
2. Add Rust `validate()` for the non-quote `MarketDatum` kinds (FX spot finite, positive, `from != to`; credit-index recovery and constituent count; dividend schedules; inflation fixings). Call it from `validate` and from context construction. Core's `validate_stored_quote`/`validate_fx_rate` are `pub(crate)`; expose one public entry point rather than duplicating.
3. `PyFxSpotDatum` (`finstack-quant-py/src/bindings/calibration/quotes.rs:989-998,1029`) calls the Rust check and switches to `wire_methods!(..., validate)`. No rule in the binding.
4. Credit-index datum (needs D5). Today a plan whose hazard curve comes from an earlier step passes `check_dependencies` but fails at the Context stage, because `build_initial_context` needs both curves up front (`C/api/context_builder.rs:166-168`). Under D5, `validate` requires a `CreditIndex` datum's hazard and base-correlation curves in `prior_market`, and `build_nodes` (`C/api/validate.rs:361-368`) records `base_correlation_curve_id` as a read, which it omits today. [R]
5. Not here: typed read keys (framework 03.1).

Tests first: `dry_run` rejects an invalid `rate_bounds`; a zero FX rate fails in `validate`, not at the Context stage; a credit-index datum without its prior curves fails in `validate`.

### 1.6 Report statistics and explicit absence (M19)

Crates: `finstack-quant-calibration`, bindings. Records: framework 14.1, 14.3, 14.6.

1. Stop overwriting plan-level `max_residual`/`rmse` with raw values pooled across steps (`C/report.rs:560-574`). Plan statistics come from `q_i = |r_i| / tau_i`; the report is already tagged `AbsoluteResidualOverStepTolerance` (`C/api/engine.rs:510`). There is no `ResidualUnits::Mixed`. Drop whichever of `max_residual`/`max_residual_ratio` becomes a duplicate.
2. Check consumers of the raw plan number before removing it: logging at `C/api/engine.rs:814`, tests, Python/WASM/UI fixtures. [U]
3. `QuoteQuality::sensitivity` becomes an explicit optional. Remove the `NaN` at `C/report.rs:492` and the `0.0` placeholders at `C/solver/global.rs:1139-1143` and in `C/solver/bootstrap.rs` (~`:650-660`).
4. Wire change: `.pyi`, generated TypeScript, schema and ledger move in this commit.

### 1.7 Parametric curves pass post-fit validation (M10c)

Crate: `finstack-quant-calibration`. Record: framework 16.7.

1. Write the `CurveValidator` impl for `ParametricCurve`; none exists (impls are Discount, Forward, Hazard, Inflation, BaseCorrelation in `C/validation/curves.rs`). The draft assumes it does.
2. Route `StepParams::Parametric` (`C/step_runtime.rs:612-619`) through `validated_curve_outcome`.

### 1.8 Python report getters and one docstring (M21)

Crate: `finstack-quant-py`.

1. Add `explanation`, `results_meta` and `solver_config` getters to `PyCalibrationReport` and `_report.pyi`. `solver_config` is non-optional in Rust; the draft's `SolverConfig | None` is wrong. All three are TypeScript field names, so they need no ledger entry.
2. `calibrate_bermudan_lmm_base_vol` docstring (`finstack-quant-py/finstack_quant/calibration/__init__.pyi:243-247`) says `Swaption | str`; the accepted types are `BermudanSwaption | dict[str, Any] | str`. Do not adopt the draft's JSDoc rewrite; it describes behaviour that does not exist.

### 1.9 Base correlation identification and ordering (M13)

Crates: `finstack-quant-calibration`, bindings. Records: targets 15.2, 15.3, 15.4, 15.7 [V].

1. Require the first tranche to attach at zero and each later tranche to attach at an exact previously fitted detachment (15.2). No contiguity check exists in `C/quotes/cds_tranche.rs`, `C/validation/preflight.rs` or `C/api/validate.rs`.
2. Add a required ordering election to `BaseCorrelationParams`: none, or non-decreasing (15.4). No default. The monotonicity policy is enforced in three places (`C/targets/base_correlation.rs:387-393,411-427`, `C/validation/curves.rs:545-551`) and pinned by `build_curve_final_rejects_non_arbitrage_free_shape`; all move together.
3. Bounds from params; remove the `0.999` literal (15.3).
4. `allow_approximate_knots` returns `false` (15.3). Also close the unbracketed `Some(root)` accepted without a residual gate at `C/solver/bootstrap.rs:557`. [R]
5. Explicit output curve ID; remove both `_CORR` format sites (`C/targets/base_correlation.rs:380`, `C/api/schema.rs:659`) (15.7).

### 1.10 Zero-lag spot date (M9)

Crate: `finstack-quant-calibration`. Needs D1.

Apply D1 in `resolve_spot_date` (`C/build/helpers.rs`). Reachable today: `rate_index_conventions.json:193-194` has `market_settlement_days: 0` with `modified_following`. Test: Saturday 31 May, T+0, Modified Following gives a date `>= as_of`; Saturday T+2 is unchanged.

### 1.11 Calibration property parity (M21)

Crates: `finstack-quant-py`, `finstack-quant-wasm`. Needs D6.

Add the prefix at `finstack-quant-py/tests/parity/test_parity_ledger.py:386`, run the test to get the exact list, then per property: `literal` only where it views plain data, otherwise a WASM function plus facade and `gen-write`. `backlog_ceiling` stays 0.

### 1.12 Remove the unread `allow_non_monotonic_final` setting

Crates: `finstack-quant-calibration`, schemas, fixtures.

`DiscountCurveSolveConfig::allow_non_monotonic_final` (`C/config.rs:285,314`) has no Rust reader [V], but it is a wire field: it appears in `calibration.schema.json`, `docs-site/.../registry-gallery/data.json`, the credit and canonical calibration fixtures, an attribution fixture and every file under `benchmarks/credit-calibration-evidence/`. Removing it under `deny_unknown_fields` means stripping the key from all of them, then `gen-write`. Check the benchmark evidence files are regenerable before touching them. [U]

---

## Phase 2: reconciliation slices

The symptom is real; the fix is the record's, not the draft's. Unchanged from the 16 Nov plan unless a status note says otherwise.

| Slice | Finding | Build | Not this | Record items |
|---|---|---|---|---|
| 2.1 | B1 | Quote states clean `u` or cash `q` and its settlement date; builder converts `C = N0·u − c·H`; instrument holds actual dated cash | Defaulted `upfront_is_clean`; T+1/T+3 from the index-name prefix; an NPV add-back | framework 08.2, 08.4; credit 18.1, 18.5 |
| 2.2 | B5 | Explicit copula selection shared by value, risk, cashflows and calibration; Student-t fit on a bracketed root with an explicit tolerance and an authored scalar key | Copula on `CreditIndexData`; rebinding the index; `{index_id}_STUDENT_T_DF`; a core duplicate of `CopulaSpec` | credit 17.1, 09.1, 09.6; targets 16.3, 16.5, 16.6 |
| 2.3 | B3a | Single-curve futures project from the fitted curve, gated on the structural predicate `forward_curve_id == discount_curve_id` as IRS does (`valuations/rates/src/irs/pricer.rs:41-48`) | "If absent, fall back to the discount curve" | framework 20.1, 05.3, 05.4 |
| 2.4 | B4 | Replace the `1.5` cap with explicit annual zero-rate bounds mapped to factor bounds (07.5 [V]); reject non-local interpolation under bootstrap (07.4 [V]); reject a calibrated currency that is not the convention base (07.1 [V]) | Either-orientation legs; spline polish; `foreign_ois_discount_id` | targets 07.1, 07.4, 07.5, 07.7 |
| 2.5 | M10a, M10b | Remove the universal `0.50` hazard cap and the generic negative-rate switch from post-fit validation | Raising the cap to `10.0`; `effective_validation_config` | framework 16.3, 16.4, 02.2 |
| 2.6 | M11 | Coordinate transforms for bounded parameters; typed errors in place of penalty residuals; one-sided stencils chosen before pricing | Either of the draft's two penalty designs; distance-scaled penalty; backward-difference fallback | framework 12.2, 12.3 [V], 15.3 |
| 2.7 | M12 | Explicit intensity bounds on the logistic map; explicit per-row weights | Silent recovery override; hard-coded `1/RPV01²` | targets 05.1, 05.4; framework 07.1, 12.1 |
| 2.8 | B2, M20 | Replay targets exact quote IDs for hazard and rates | Nearest-tenor matching with a `0.25Y` or `0.45Y` cap | framework 19.1, 19.3, 21.3, 05.6 |
| 2.9 | M14 | One index identity for the quote label, the existence check and the instrument (which of `params.index` and `curve_id` survives is the record's call [U]); explicit positive CPI bounds (06.5 [V]); reject bootstrap with non-local interpolation (06.4 [V]) | New hard-coded bounds; spline polish | targets 06.4, 06.5 |
| 2.10 | M15 | Elected non-overlapping `tau1` and `tau2` ranges; explicit per-quote weights; remove the `+0.5` repair | Smooth penalty; default PV01 weights | targets 08.2, 08.3, 08.4 |
| 2.11 | B7 | Dated references per node; interpolated nodes get a dated forward, not a month-rounded one; out-of-range targets stay rejected | Snap within 5 business days; total-variance interpolation | targets 13.5, 14.1, 14.4 [V] |
| 2.12 | B8b | Strict lookup in place of `get_surface_vol_clamped` (`C/lmm.rs:292` [V]); then the source named by D3 | Cube-then-surface fallback; co-terminal least squares | rate-models 09.1, 09.3 [V] |
| 2.13 | found on 16 Nov | Include the first cap coupon when the quote convention includes it; `C/step_runtime.rs:256` still does `.skip(1)` unconditionally [V] | n/a | rate-models 02.1 |

### Status notes since 16 Nov

- **2.1.** The draft's test expectation is inconsistent: Jun 20 to Sep 15 is 87 days exclusive, the single-name convention it claims to mirror (`valuations/credit/src/cds/pricing/metrics.rs:221-222`, `as_of + 1` day) gives 88/360, and its own "to T+1 settlement" rule gives 88 or 90. Fix the convention in the record text before writing the test. Whether CDX tranches settle T+1 is a market fact not verified here. [U] The pricer still infers it from the name prefix (`valuations/credit/src/cds_tranche/pricing/engine.rs:366-368`).
- **2.2.** The Gaussian sites are `cds_tranche/types.rs:392,424`, `cds_tranche/metrics/mod.rs:60,74,88,101,115` and `C/targets/base_correlation.rs:140`. All eight move in one slice or value and risk diverge. The published scalar is keyed by tranche ID (`C/step_runtime.rs:381-391`). Keep `student_t_loose_solver_tolerance_does_not_relax_fit_acceptance` (`C/targets/student_t.rs:688`). Do not claim `1e-10`: the Student-t integrator is a fixed product Gauss rule. The Student-t target also rebuilds the tranche under `CdsDateRegime::Standard` regardless of how base correlation built it; carry `date_regime` here.
- **2.4.** Only one convention is registered (EUR/USD, base EUR), so nothing is mis-oriented today; the check is a guard for the next one. Stale doc at `C/targets/xccy_basis.rs:92` ("basis-spread curve") goes in this slice. [V]
- **2.5.** `tests/hazard_curve.rs` (~`:727-731`) asserts "default hazard cap must be respected"; it changes with the slice. `validate_bounds` uses `min_forward_rate * 5.0` as the zero-rate floor (`C/validation/curves.rs:216`); revisit when the switch goes.
- **2.6.** Core LM has no bounds or projection, and `C/solver/traits.rs:239-249` documents "projected LM", which is false; fix the doc here. The existing test `fill_penalty_yields_nonzero_jacobian_column_for_each_parameter` exercises a path production never takes. 2.6 goes before 2.7.
- **2.8.** Rates are mostly done: replay rebuilds from the stored step and config, and key-rate paths use `RateQuoteShock::Exact`. Left for rates: the nearest-pillar `TenorsBp` arm at `C/recalibration/rates.rs:959`, which has no production caller; make it exact or remove it. Left for hazard: targets are year fractions, and `valuations/core/src/metrics/risk/market_history.rs:164` passes the raw `CreditSpread.tenor_years`, so a nominal 5.0 against an IMM curve (pillar 4.82-5.33) now fails loudly. Add that as the failing test. Plan the scenarios and attribution callers first.
- **2.9.** The hazard knobs for bounds, when 2.7 lands, are `config.hazard_curve.hazard_hard_min`/`hazard_hard_max`, not `config.validation.max_hazard_rate`.
- **Fold into the owning slice, low value alone:** `quote_annuity_proxy` uses cumulative maturity for FRA and futures and its doc is wrong (`C/targets/util.rs:408-427`); fix when weights become explicit (2.7/2.10). It only affects conditioning on over-determined fits and is pinned by `C/targets/forward.rs:645-662`.

### Order

1. 2.6, then 2.7 and 2.5.
2. 2.1, then 2.2.
3. 2.3, 2.4, 2.9, 2.10.
4. 2.8.
5. 2.11, 2.12, 2.13.

## Still unverified (resolve inside the named slice)

- Whether a single-curve discount step with a future errors or solves today (1.4, 2.3).
- Which SVI fixtures fail at a `1e-3` cap (1.1).
- Consumers of the raw plan-level `max_residual` (1.6).
- Whether any golden uses a weekend or holiday `as_of` (1.10).
- The exact property list for calibration parity (1.11).
- CDX tranche cash settlement lag as a market convention (2.1).
- Whether a gapped tranche chain corrupts results or uses an extrapolated attachment correlation (1.9).
- M16a follow-ups: with `spot_override` the quote currency is `None`, so the cash-dividend path at `C/targets/util.rs:235` may always error; and whether the discount curve's currency is checked against spot.
- Record item numbers not marked [V] above.
- Nothing was built or run for this re-baseline.

## Closing register entries

Each Phase 2 slice edits its `D/FOLLOW_UPS.md` entry in the same commit: delete it if the slice completes the record's package, otherwise rewrite it to what remains.
