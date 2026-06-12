# Quant Finance Review — `finstack/attribution` Crate and Bindings

**Date:** 2026-06-12
**Scope:** `finstack/attribution` (~12.7k source lines: parallel/waterfall/Taylor/metrics-based methodologies, credit cascade/factor decomposition, market snapshots, target-currency translation, spec/envelope types), `finstack-py/src/bindings/attribution/`, `finstack-wasm/src/api/attribution/` + JS facade, `parity_contract.toml` attribution sections, and the crate's ~8.8k-line test/bench surface (incl. `quantlib_parity.rs`).
**Method:** Five parallel subsystem reviews (Taylor/metrics-based/model-params; credit + types/spec; engine core parallel/waterfall/snapshot/FX; bindings parity; tests/numerical regression). Every Blocker and Major was independently adversarially verified by separate agents instructed to refute it (Blockers by two agents each, one constructing a numeric counterexample from source). All 22 Blocker/Major claims were confirmed; two were downgraded to Moderate during verification. The `credit_factor.rs` sign inversion was additionally hand-verified.

---

## Findings

### Blockers — wrong P&L decomposition in live code paths

#### B1. Parallel cross-factor interaction P&L has the wrong sign — residual is −2× the true interaction instead of ~0

- **Location:** `finstack/attribution/src/parallel.rs:46-56` (`cross_interaction_pnl`), `:128-138` (`record_cross_pair`), `:693-775` (full-cross path), `:963-1062` (default 6-pair path); consumed by `finstack/attribution/src/types/result.rs:664-669` (`compute_residual`).
- **Issue:** Parallel factor P&Ls are measured from the T1 base: `f_i = V(all-T1) − V(factor_i@T0)`. For two moved factors the exact identity is `total = f_a + f_b − D` where `D = V11 − V(a@T0) − V(b@T0) + V(ab@T0)` is the mixed second difference. The code computes exactly `+D`, stores it in `cross_factor_pnl`, and `compute_residual` **adds** it to the attributed sum — so attributed = `total + 2D` and residual = `−2D`. Verified numerically against the crate's own fixture (`RatesCreditInteractionInstrument`, rates/hazard 1%→2%): total 303.55, factors sum 405.08, cross-as-coded +101.52 → residual −203.05; storing −101.52 gives residual exactly 0. `metrics_based` fills the same field with a T0-base Taylor cross term whose additive sign **is** correct, so the field is also internally inconsistent across methods. No test pins residual size with cross extraction (`cross_factor_attribution_tests.rs` only asserts cross ≠ 0).
- **Impact:** Every parallel attribution with ≥2 active factor families (the common case) publishes a sign-inverted `cross_factor_pnl` and a residual of −2× the true interaction. Interaction direction is mis-stated (a positive rates·credit co-movement gain shows as residual loss) and `residual_within_tolerance` gating fires spuriously.
- **Fix:** Negate at the parallel call sites (`record_cross_pair` should accumulate `val_a + val_b − val_t1 − val_ab`) so `compute_residual`'s additive convention reconciles; leave metrics-based unchanged; decide whether `cross_factor_detail.by_pair` keeps the natural mixed-difference sign (document per-method semantics). Add a regression test asserting parallel residual ≈ 0 with cross extraction on the interaction fixture.

#### B2. `MarketSnapshot::restore_market` silently drops entire market-data families, corrupting every factor reprice

- **Location:** `finstack/attribution/src/factors.rs:216-277` (`extract`), `:291-384` (`restore_market`) vs `finstack/core/src/market_data/context/mod.rs:148-185, 457-477`.
- **Issue:** `MarketContext` holds 9 curve-storage variants plus `credit_indices`, `vol_cubes`, `fx_delta_vol_surfaces`, collateral CSA mappings, and hierarchy. The snapshot copies only 5 curve families (discount/forward/hazard/inflation/base-correlation), the FX matrix, `VolSurface`s, and 4 scalar stores; `restore_market` builds a fresh `MarketContext::new()` and re-inserts only those — Price/VolIndex/BasisSpread/Parametric curves, credit indices, vol cubes, FX-delta vol surfaces, and collateral are dropped. In parallel attribution every restored-factor market lacks them; in waterfall, `build_market_for_factor` replaces `current_market` after the **first** step, so all subsequent cumulative repricings — and the final "T1" state — permanently lose them. Valuations actively reads all of these (swaptions/CMS via vol cubes, commodity instruments via price curves, CDS tranches via credit indices, repo/CSA discounting via collateral). The VOL flag also covers only `VolSurface`, never `VolCube`/`FxDeltaVolSurface`.
- **Impact:** Instruments depending on these families either fail attribution outright (missing-curve on the first factor reprice: swaptions on SABR cubes, commodity forwards/options, CDS tranches, CSA-discounted trades) or are silently mispriced at every factor step, corrupting factor P&Ls and breaking the waterfall residual≈0 contract.
- **Fix:** Rebuild `restore_market` as clone-and-overwrite: start from `current_market.clone()` (lossless — the credit cascade already does this) and replace only flagged families. Extend VOL to cubes + FX-delta surfaces; classify price curves under SCALARS; rebind credit indices after hazard/correlation restores; carry collateral/hierarchy through. Add a round-trip test: `restore(market, extract(market, all()), all())` reproduces every store.

#### B3. Taylor key-rate bump spec breaks both wing buckets — short-end rates P&L understated; whole rates factor silently dropped for curves with >30y knots

- **Location:** `finstack/attribution/src/taylor.rs:634-647` (`key_rate_bump_spec`), used at `:683-693` (`compute_rate_factor`) and `:766-777` (`compute_forward_factor`); failure swallow at `:114-141` (`record_taylor_factor_result`).
- **Issue:** For bucket `i == 0` it passes `prev = 0.0` into `BumpSpec::triangular_key_rate_bp` instead of `triangular_key_rate_first_bp` — the exact anti-pattern documented in `finstack/core/src/market_data/bumps.rs:224-243`: a 1M knot gets weight 1M/3M ≈ 0.33 instead of 1.0 and no other bucket covers the gap, so Σwᵢ(t) < 1 below the first bucket and ~67% of sub-3M rate P&L silently lands in residual (1W/1M/2M pillars are standard on OIS curves). For the last bucket it passes `next = f64::INFINITY`; the runtime path goes through `bump_in_place` (no bucket validation, bypassing the finiteness guard on the copy path), `triangular_weight` computes `(∞−t)/(∞−30) = NaN` for any knot beyond 30y, the interpolator rebuild rejects the NaN series with `Err`, the `?` aborts the **whole curve's** factor, and `record_taylor_factor_result` drops it with only a `tracing::warn` — the entire rates/forward factor for that curve vanishes into residual (40y/50y pillars are routine on EUR/GBP/USD long ends). The canonical `BucketedDv01` calculator uses the correct first/last constructors (`valuations/src/metrics/sensitivities/dv01.rs:501-523`) with a comment warning about exactly this bug.
- **Impact:** Wrong explained rates P&L for any curve with sub-first-bucket knots (breaks the unity-partition standard the rest of the codebase enforces); silent, warn-only loss of the full rates factor for long-end curves.
- **Fix:** Mirror dv01.rs: `i == 0` → `triangular_key_rate_first_bp(target, next, bp)`; last → `triangular_key_rate_last_bp(prev, target, bp)`. Make `bump_in_place` validate bucket finiteness like `apply_bump` (the two paths currently disagree); fix the stale `forward_curve.rs:567-569` doc that recommends the 0.0/∞ sentinels. Add wing-knot fixtures (1W/1M and 40Y) — see also MO-T13.

#### B4. Metrics-based rates convexity formula matches neither producer's units — 100× understated for bonds, ~PV× overstated for swaps

- **Location:** `finstack/attribution/src/metrics_based.rs:940-963` (formula), `:929-938` (`Convexity`/`IrConvexity` merged via `.or_else`).
- **Issue:** Computes `convexity_pnl = 0.5 · p0 · convexity · Δy²` (Δy decimal), treating the metric as dimensionless `(∂²P/∂r²)/P`. The bond producer emits **street convexity** `(1/P)·d²P/dy²/100` (`valuations/src/instruments/fixed_income/bond/metrics/convexity.rs:135`, Bloomberg-YAS golden-verified) → formula missing ×100, bond convexity P&L 100× understated. The IRS producer emits the **raw dollar second derivative** `d²PV/dr²` (`valuations/src/instruments/rates/irs/metrics/ir_convexity.rs:97-100`, no P normalization) → multiplying by `p0` is dimensionally wrong (overstates by ~the swap PV; a near-par swap with PV≈0 gets ≈0 convexity despite large gamma).
- **Impact:** `rates_curves_pnl` — a primary output — numerically wrong for two instrument families on the metrics-based path; the error leaks into residual (bonds) or can dominate the attribution (swaps).
- **Fix:** One convention per MetricId, aligned across producer and consumer: bond `Convexity` → `0.5·p0·convexity·100·Δy²`; `IrConvexity` → `0.5·ir_convexity·Δy²` (no `p0`). Stop consuming the two ids identically. Add producer/consumer unit tests (the suite's only convexity test is synthetic — `metrics_based_convexity.rs`).

#### B5. Sign inversion in portfolio-level credit factor attribution (`compute_credit_factor_attribution`) — latent, no production caller yet

- **Location:** `finstack/attribution/src/credit_factor.rs:215, 229, 239` (accumulations), `:9` and `:25-27` (module identity), `:76-79` (input doc); test `tests/attribution/credit_factor_linear.rs:154-175`.
- **Issue:** Every contribution is `−CS01·β·ΔF`. The input doc states the workspace-canonical convention (long credit → **negative** CS01, matching `valuations/src/metrics/sensitivities/cs01.rs:33-50`), under which P&L = `+CS01·ΔS` — exactly what the sibling paths do (`credit_decomposition.rs:144-146` with an explicit "no extra negation" comment; `metrics_based.rs:992-1073`). With the documented convention this function reports a **gain** for a long-credit portfolio when spreads widen (counterexample: report +$45k where truth is −$45k). The unit test feeds negative CS01s and asserts the same inverted identity, so it cannot catch it. Hand-verified independently of the review agents.
- **Impact:** Sign-inverted hierarchy credit P&L (generic + all levels + adder) for any caller following the docs. Mitigant: exported at crate root for the portfolio layer but currently uncalled outside its own test.
- **Fix:** Decide the intended input convention (open question OQ1), then either drop the negation in all three accumulations + module identity, or rewrite the doc to "positive loss-per-bp" and cross-reference the canonical CS01 doc. Re-derive the test against real pricing direction (long credit + widening ⇒ loss).

### Majors

#### M1. Metrics-based rates attribution ignores forward/projection curves entirely

- **Location:** `metrics_based.rs:763-767` (curve ids from `discount_curves` only), `:254-268`, `:591-610`.
- **Issue:** Forward-curve key-rate DV01 present in `measures` is never consumed; the aggregate fallback multiplies a **joint** discount+forward DV01 (`Dv01CalculatorConfig::parallel_combined` for swaps) by a discount-only average shift. The Taylor path handles forward curves explicitly (`taylor.rs:277-310`).
- **Impact:** Rates factor wrong whenever discount and projection curves move differently — i.e. any basis move, a daily occurrence for multi-curve swap books; projection-curve P&L lands in residual or is mis-scaled.
- **Fix:** Include forward curves in curve-id collection, per-tenor shift measurement, and the average-shift preamble, mirroring Taylor.

#### M2. Metrics-based total P&L is price-only while carry is total-return — coupons land in residual with flipped sign

- **Location:** `metrics_based.rs:546-554`.
- **Issue:** `total_pnl = PV₁ − PV₀` with no cashflow adjustment, but carry = `Theta × days` where Theta **includes** period cashflows (`theta.rs:171-178`). On a coupon date PV drops by the coupon, carry predicts ≈0, residual ≈ −coupon, spurious tolerance breach. Taylor/parallel/waterfall fix this via `apply_total_return_carry` (`helpers.rs:166-185`).
- **Fix:** Add `collect_cashflows_in_period` income to metrics-based `total_pnl`, matching the other methods' total-return basis.

#### M3. Theta/CarryTotal are period totals over a configurable `theta_period`; `× time_period_days` double-scales non-default valuations

- **Location:** `metrics_based.rs:671-693, 723-731`; producer horizon at `theta.rs:396-399` (default "1D" via `MetricPricingOverrides`).
- **Issue:** For valuations computed with e.g. `theta_period = "1M"`, attribution multiplies a one-month carry by the day count again; nothing in `ValuationResult` records the horizon so the mismatch is undetectable.
- **Fix:** Stamp the theta horizon into result measures (e.g. `theta_period_days`) and normalize, or hard-assert the 1D contract.

#### M4. `InflationConvexity` has two producers with conflicting units; consumer matches only the swap producer

- **Location:** `metrics_based.rs:1529-1544` (consumer, `½·C·Δi²`, no P₀); ILB producer divides by base PV (`valuations/src/instruments/fixed_income/inflation_linked_bond/metrics/inflation_convexity.rs:56-61`); swap producer is raw `d²PV/dπ²` (matches).
- **Impact:** ILB inflation convexity P&L understated by ~P₀ (≈10⁶ for a $1M bond) — the exact failure the consumer's own unit table (`metrics_based.rs:66-71`) warns about.
- **Fix:** Normalize the ILB calculator to the raw-derivative convention (drop `/ base_pv`); one MetricId = one unit.

#### M5. Scalar-factor credit cascade ignores calibrated issuer betas

- **Location:** `credit_cascade.rs:303-343` (raw moves at `:310`, `:321`); non-scalar path applies betas correctly (`:386`, `:407`); doc contradiction at `:61-63`.
- **Issue:** When market scalar series exist for credit factors (the primary production configuration), step sizes are the raw factor moves with no `β_pc`/`β_k` multiplication, contradicting both the `CreditStepKind` doc (`bp = β×ΔF`) and the calibrated model identity. The unexplained part is silently dumped into the per-issuer adder (`:347`), mislabeled idiosyncratic; reconciliation still closes, masking it. Tests pin the β-free behavior (`:788`).
- **Fix:** Scale scalar steps by issuer betas (`β_pc·Δg`, `β_k·ΔL_k`) and update the pinning tests — or, if the series are quoted in issuer-equivalent bp, fix the doc (OQ2).

#### M6. `translate_to_target_ccy` drops coupon income from `total_pnl`, manufacturing a coupon-sized residual

- **Location:** `target_ccy.rs:109-126` (rebuild from `val_t0 + mark_to_market_pnl`), `:147` (residual recompute); wired in `execution.rs:228-257`.
- **Issue:** Native `total_pnl` is total-return (coupon added by `apply_total_return_carry`); translation rebuilds it as pure MTM while translated `carry` keeps the coupon at T1 FX. Recomputed residual = `(residual_native − coupon)×fx₁`; `total_pnl` silently switches convention vs the `types/result.rs:169-183` doc. Unit tests only cover zero-coupon attributions.
- **Fix:** `total_pnl = translated_mtm + (native_total_pnl − native_mtm)×fx₁`; keep `mark_to_market_pnl = translated_mtm`. Test with nonzero coupon asserting residual ≈ `residual_native×fx₁`.

#### M7. Carry credit/rates split `s/(r+s)` is unbounded near `r ≈ −s` (negative-rates regimes)

- **Location:** `credit_decomposition.rs:291-299` (guard only `|r+s| > 1e-15`).
- **Issue:** With negative rates and small spreads, the credit share explodes (±10²–10⁶ × coupon into `coupon_credit`/`coupon_rates` with opposite signs); invariants still reconcile so the garbage propagates silently into `SourceLine` and `CreditCarryDecomposition`. Since hazard ≥ 0 and R ∈ [0,1] are curve-build-enforced, s ≥ 0 always.
- **Fix:** Clamp the credit share to [0, 1] or take the degenerate branch when `|r+s| < ε·max(|r|, s)`.

#### M8. Carry-input helper silently swallows cashflow-collection errors and uses the panicking `Money::new` on raw metric f64s

- **Location:** `helpers.rs:200-216`; used by parallel (`parallel.rs:447-461`) and waterfall (`waterfall.rs:335-349`).
- **Issue:** (1) `collect_cashflows_in_period(...).unwrap_or(0.0)` converts any error (incl. upstream currency-mismatch) into coupon_income = 0 with no note/tracing/invalid flag — the attribution silently flips from total-return to MTM-only (systematic for cross-currency instruments). (2) `Money::new` at `:203` and `:216` panics on non-finite input; the purpose-built `factor_money_or_invalid` guard exists 80 lines away and is not used (verification note: the panic leg is hard to reach in practice; the swallow is the substantive defect).
- **Fix:** On `Err`, push a meta note and/or set `result_invalid`; route the constructions through `Money::try_new`/`factor_money_or_invalid`.

#### M9. Waterfall `factor_order` accepts duplicates — duplicated factor overwrites its P&L with ~0; duplicated Carry double-counts coupons into `total_pnl`

- **Location:** `waterfall.rs:233-249` (validation: only non-empty + Carry-first), `:319-377` (assignment recording, Carry arm `:333-349`); reachable via the public JSON `AttributionMethod::Waterfall(Vec<...>)` wire; no duplicate test exists.
- **Issue:** Second application of a factor finds the market already rolled, measures ≈0, and **overwrites** the genuine first-pass P&L (real move leaks to residual); a duplicated Carry calls `apply_total_return_carry` twice, double-adding coupon income to `total_pnl`. With default `strict_validation = false` this returns silently in the methodology recommended "for risk reporting where sum must equal total". No warning either for orders omitting factors that actually moved.
- **Fix:** Reject duplicates with `Error::Validation` at entry; optionally note unapplied-but-moved families in `meta.notes`. Add `test_waterfall_rejects_duplicate_factors`.

#### M10. Tuple-keyed detail maps cannot serialize to JSON — latent hard failure on the serde-stable wire, unreachable headline features in both bindings

- **Location:** `types/detail.rs:103, 121, 134, 152` (`IndexMap<(CurveId, String), Money>` / `IndexMap<(Currency, Currency), Money>` with derived `Serialize`); surfaced at `finstack-py/.../entry.rs:92`, `dataframe.rs:34-138`; roundtrip-test gap at `tests/attribution/serialization_roundtrip.rs:351-375`.
- **Issue:** serde_json rejects non-string map keys, so any populated `by_tenor`/`by_pair` makes `AttributionResultEnvelope`/`attribute_pnl` serialization fail at runtime ("key must be a string"); schemars output is also wrong. Today latent — **nothing in the workspace constructs these types** — which simultaneously means per-tenor curve attribution and FX-pair attribution (advertised in `lib.rs:321-323`, the binding docstrings, `.pyi`, and the dataframe row builders) are structurally unavailable through Python and WASM, and all serde roundtrip tests pass only because the fields are always `None`.
- **Fix:** Give the maps a JSON-stable representation (array-of-records `{curve_id, tenor, amount}` or string keys "USD-OIS|5Y"/"EURUSD") in the canonical crate; add a roundtrip test with **every** optional detail populated; until wired, mark the dataframe kinds as not-yet-available.

#### M11. Python `attribute_pnl` rejects the documented bare-string method forms ("Parallel", "MetricsBased")

- **Location:** `finstack-py/src/bindings/module_utils.rs:127-130` (via `attribution/entry.rs:67`).
- **Issue:** `py_to_json_value`'s string branch requires the Python str to already be valid JSON; bare `"Parallel"` raises `ValueError: invalid method JSON: expected value at line 1 column 1`. The docstring, the `.pyi` (`__init__.pyi:344-346`), and the shipped notebook (`examples/notebooks/02_pricing/pnl_attribution.ipynb`, whose stored 2026-04-11 outputs show success) all use the bare form — a post-execution regression. Dict-shaped methods still work, masking it.
- **Fix:** Fall back to `serde_json::Value::String(s)` when `from_str` fails (matching the externally-tagged serde of `AttributionMethod`); add Python tests for `method="Parallel"` and `method={"Waterfall": [...]}`; re-execute the notebook.

#### M12. All Python attribution errors mapped to `ValueError` via `display_to_py` instead of `core_to_py` semantics

- **Location:** `finstack-py/src/bindings/attribution/entry.rs:80, 91, 119-120, 145`; `pnl_attribution.rs:266`.
- **Issue:** The project's mandated taxonomy (missing id → `KeyError`, operational/internal → `RuntimeError`) is bypassed; `spec.execute()` reprices per factor, so `MissingCurve` (a routine production failure) surfaces as `ValueError`, and the error source chain is dropped. Diverges from WASM, which discriminates via `err.kind`.
- **Fix:** Use `core_to_py` wherever the error type is `finstack_core::Error`; add tests asserting `KeyError` for a missing curve and `RuntimeError` for internal failures.

#### M13. FX-forward QuantLib parity test asserts almost nothing per-factor — and the fixture's rate P&L carries a sign error

- **Location:** `tests/attribution/quantlib_parity.rs:643-662` (`let _ = (...)` discards all per-factor expectations); `scripts/generate_quantlib_fixture.py:437-440`.
- **Issue:** Only total P&L, internal reconciliation, and a loose combined FX bound (10% + $5) are asserted; carry and both rate factors are never checked. The fixture computes `usd_rate_pnl = -usd_dv01·10⁴·Δr` although `usd_dv01` is already the signed PV change per +1bp — with the correct sign the first-order residual is −$0.24, with the flip it is $213.25 (matching the committed fixture to 4dp), and a test comment rationalizes the $213 as "structural second-order residual" (false for a 1-day FX forward). Wiring the discarded assertions in against the current fixture would institutionalize the sign error.
- **Fix:** Fix the generator sign, regenerate `fx_forward_1y_eurusd.json`, enable tight per-factor assertions (carry ~$1, usd_rate ~$2, eur_rate ≈ 0), shrink the FX bound to ~1% + $5, delete the misleading comment.

#### M14. IRS attribution parity tolerance ($5,000) cannot detect a carry sign flip, a carry dropout, or a rates dropout

- **Location:** `tests/attribution/quantlib_parity.rs:367, 417-445`.
- **Issue:** Expected carry is $566.23 and rates P&L $4,198.31 (`irs_5y_usd.json`); zeroed carry (diff 566), sign-flipped carry (1,132), and zeroed rates (4,198) all pass under the $5k per-factor tolerance; total uses $15k vs actual $5,011. The schedule-drift justification doesn't carry over: factor P&Ls are first differences and the same valuations file holds DV01 to 5% relative. The IRS test also lacks the Σ factors + residual ≡ total reconciliation the bond/FX tests have.
- **Fix:** Per-component tolerances scaled to expected values (carry: max($100, 25%); rates: max($250, 5%); total: max($1,000, 0.05% notional)); add the reconciliation assertion.

### Moderates

**Engine / methodology**

- **MO-E1. Empty-T0 factor snapshots silently skipped** even when T1 has data (day-one positions, newly marked surfaces/FX): move lands in residual with no note or tracing, unlike the model-params failure paths — `parallel.rs:140-163, 174-191, 843, 939-942`. Push a `meta.notes` diagnostic + `tracing::warn`.
- **MO-E2. Execution policy never stamped into results** (workspace policy-visibility invariant: "numeric mode, parallel flag, rounding context"): `AttributionMeta`/`ResultsMeta` carry no parallel flag; `fx_policy_applied` in `results_meta` also stays `None` when translation stamped `meta.fx_policy` — `types/result.rs:317-355`, `helpers.rs:252-272`, `execution.rs:260`. Add `execution_policy` (serde-additive) and propagate.
- **MO-E3. `restore_market` deep-clones every curve on every factor reprice** despite Arc-based storage (`(**curve).clone()` instead of `Arc::clone`): O(curves × ~10-20 repricings × positions) avoidable allocations inside Rayon workers — `factors.rs:308-338`. No correctness impact.
- **MO-E4. Rate fixings are classified under MarketScalars, not RatesCurves**, splitting a single economic rate move across two factor lines with a cross term, and the carry reprice silently LOCFs missing T1 fixings (`fixings.rs:84-112`) — `factors.rs:257-274`, `parallel.rs:932-961`. Either move `FIXING:`-prefixed series into the RATES family or document the classification + LOCF.
- **MO-E5. Target-ccy translation reprices val_t0 with the T1-parameter instrument** when `model_params_t0` is supplied (methods price val_t0 with T0 params), mis-stating `fx_translation_pnl` by the parameter-induced valuation gap; the currency probe also swallows errors with `.ok()` — `execution.rs:26-29, 228-257`.

**Metrics-based / Taylor / model-params**

- **MO-T1. Key-rate branch silently drops curves without per-tenor data:** if any curve has key-rate DV01, curves with only aggregate DV01 `continue` with zero rates P&L and no note — `metrics_based.rs:778-802`. Per-curve fallback ladder.
- **MO-T2. Inflation attribution iterates all market inflation curves** (`market_t1.curve_ids()`, FxHashMap order) instead of instrument dependencies: unrelated curves contaminate the average Δi; hash-order-dependent float sum — `metrics_based.rs:1487-1505`. Source ids from `market_dependencies()`.
- **MO-T3. Multi-spot Delta/Gamma aggregation inconsistent:** aggregate Delta multiplied by **each** spot's move and summed (~N× overstatement for multi-spot instruments) while Gamma uses the average move once — `metrics_based.rs:1254-1277`.
- **MO-T4. Dividend01 block is dead code carrying a latent 10⁴ unit error:** consumer multiplies by decimal Δq, both producers emit $/bp; never executes today because no instrument overrides `dividend_schedule_id()` — `metrics_based.rs:1466-1483`. Fix units before wiring.
- **MO-T5. Public model-params shift helpers document a 10⁴/10² unit-wrong recipe** (bp/pct-pt shifts paired with per-unit `*01` metrics; producers' own header docs also wrong); internal paths unaffected (they reprice via `with_model_params`) — `model_params.rs:103-106, 140-186, 205-207`, `prepayment01.rs:98-102`, `recovery01.rs:51-55`. *(Downgraded from Major in verification: no in-repo computed path multiplies metric × shift.)*
- **MO-T6. Taylor repricing counts wrong for credit** (records 2, actual ~22 inside `BucketedCs01`), and the `.ok()` on the key-rate CS01 call swallows errors before the fallback — `taylor.rs:322-331, 359-369`.

**Credit / types / serde**

- **MO-C1. Carry-split curve lookups swallow errors:** `zero_rate_on_date(...).unwrap_or(0.0)` / `hazard_rate_on_date(...).unwrap_or(0.0)` make a failed lookup indistinguishable from a zero-spread issuer (whole coupon labeled rates carry); date-overflow collapses tenor to 0y — `credit_decomposition.rs:264-279`.
- **MO-C2. Legacy-JSON USD defaults poison non-USD attributions:** `#[serde(default = "zero_money_usd")]` on `fx_translation_pnl`/`curve_shape_pnl` means an archived EUR attribution deserializes with `USD 0` fields, and re-running `compute_residual()` hard-fails currency validation, setting `result_invalid` on a valid legacy result — `types.rs:18-20`, `result.rs:238`, `detail.rs:61`. Re-currency zero defaults to `total_pnl.currency()` post-deserialize.
- **MO-C3. Single-issuer synthesized decomposition mislabels idiosyncratic risk as level P&L** when no scalar factor series exist (entire issuer ΔS lands in level-0 with unit betas; oscillating components with calibrated betas; the adder-magnitude warning can never fire) — `credit_cascade.rs:365-413`. Route full ΔS to the adder or document degenerate semantics.
- **MO-C4. Linear-path `curve_shape_pnl` also absorbs spread convexity** (single CS01 × Δbp steps), and the "significant non-parallel move" warning fires for perfectly parallel large moves — `credit_decomposition.rs:131-154`, warn at `credit_cascade.rs:626-637`. Rename/document as "non-parallel + higher-order"; suppress the warn on the linear path.

**Bindings**

- **MO-B1. `validate_attribution_json` doesn't check `ATTRIBUTION_SCHEMA_V1`** (Python and WASM): a `"finstack.attribution/99"` envelope passes validation and then fails at execute — `entry.rs:142-146`, `wasm mod.rs:197-202`.
- **MO-B2. Empty detail DataFrames have zero columns**, contradicting the documented "zero rows, schema columns present" contract; the existing test asserts only `len(df)==0` — `pandas_utils.rs:57-64`. Pass the fixed column list for empty frames.
- **MO-B3. Long-format rows stamp the parent aggregate's currency** instead of each row's own `Money` currency; detail maps are never currency-validated, so a mixed-currency payload silently mislabels rows — `dataframe.rs:23-292`. Use `money.currency()` per row.
- **MO-B4. Attribution execution surface untested in WASM entirely** (no `wasm_attribution.rs`, no facade `.mjs`, no `dts_contract.rs` assertions) **and untested in Python beyond DataFrame accessors** — this is how M11 shipped. Add end-to-end tests per surface.

**Tests (coverage gaps)**

- **MO-X1. No coupon-payment-date-inside-window test; `mark_to_market_pnl` never asserted anywhere** — the classic dirty-price/total-return defect class is unguarded (and would catch M2/M6).
- **MO-X2. No negative-rates coverage** (lowest tested rate +0.05%): carry split (M7), funding-cost sign, DF>1 regimes unguarded.
- **MO-X3. No basis-move test** (discount and forward never move differently; `discount_total`/`forward_total` never asserted) — would catch M1.
- **MO-X4. Vol tests only cover flat surfaces with parallel shifts** — smile/skew change semantics unpinned.
- **MO-X5. Analytical self-consistency escape hatch `|| actual_abs < 200.0`** lets a 30-100× collapapsed rates P&L pass all three magnitude pins as long as the sign survives — `analytical_self_consistency.rs:236`. Gate any skip on the **expected** magnitude, never the value under test. *(Downgraded from Major: the invariants.rs magnitude floor partially overlaps.)*

### Minors

- `unreachable!()` panic paths in production match arms of a deny-panic crate — `parallel.rs:647, 889, 958`.
- `PnlAttribution::scale` uses panicking `Money::MulAssign` on a public API — `result.rs:461-568`; guard non-finite factors.
- Result-envelope schema stamped but never validated on read (`AttributionResultEnvelope::new`); `CreditFactorModel.schema_version` also unchecked — `spec.rs:249-257`.
- `build_credit_factor_attribution`: USD fallback on empty steps; shape check is `debug_assert` only (zip truncates in release) — `credit_cascade.rs:548-552`; duplicate model factor ids overwrite last-wins — `:334-336, 566-575`.
- `SourceLine` custom `Deserialize` tolerates unknown fields, unlike the rest of the inbound surface — `detail.rs:254-291`.
- Doc inaccuracies: `detail.rs:363-367` (parallel/waterfall *do* populate `carry_detail`); misleading FX-conversion comments (`parallel.rs:430-433, 849-866` — all factor P&Ls are native-currency identity conversions); stale `CsGamma` unit doc (`ids.rs:752-755`); `forward_curve.rs:567-569` recommends the broken 0.0/∞ sentinels (see B3).
- `num_repricings` undercounts (carry-input metric pricing, execute() probes/translation, credit-detail CS01 reprices) — `helpers.rs:205-216`, `execution.rs:26-29, 235`, `credit_decomposition.rs:111-128`.
- Metrics-based: missing DV01/CS01 yields silent zero rates/credit with no note (carry does note); theta detail linearly extrapolates through expiry inside the window; `Fx01` (joint all-pairs sensitivity) paired with single-pair move — worth note strings.
- Bindings: docstring drift (carry/credit-factor dataframes promise wide schemas but return long rows; `to_dataframe` nullable-float claim is actually object dtype; "pretty-printed" is compact; `full_cross_attribution` undocumented) — `pnl_attribution.rs:288-372`; `__all__` not sorted in registration/stub — `mod.rs:29-39`, `__init__.pyi:5-12`.
- Tests: rounding-context stamping asserted only for Parallel — `rounding_policy.rs:16-61`; proptest uses entropy-derived seeds vs the repo's fixed-seed standard — `invariants.rs:532-533`; `BondExpectedAttribution.residual_first_order` doc claims a check that is dead code — `quantlib_parity.rs:111-115`; no wing-knot fixtures (sub-3M / >30Y) — suite-wide (pairs with B3).

---

## Open Questions

1. **B5:** Is `CreditAttributionInput.cs01` meant to be workspace-canonical `dPV/ds` (then remove the negation) or "positive loss-per-bp" (then fix the doc)? Opposite-signed reports hinge on this.
2. **M5:** Are the `credit::level…` market scalar series quoted in issuer-equivalent bp (β-free path intentional) or factor bp (docs correct, code wrong)? Tests and docs currently disagree.
3. **Coupon window convention:** `collect_cashflows_from_flows` uses half-open `[T0, T1)` (`theta.rs:365-366`). If T1 valuations are ex-payment, coupons paying exactly on T1 drop from both PV and coupon income — needs adjudication against instrument same-day-cashflow conventions. Related: the negative-`Notional`-only exclusion (`theta.rs:367`) means outgoing notional exchanges aren't deducted from "coupon income" while incoming ones are added.
4. **B1 fix shape:** Should `cross_factor_detail.by_pair` keep the trader-intuitive mixed-second-difference sign with only the aggregate negated, given metrics-based/Taylor share the field with a correctly-additive convention? Schema needs per-method documentation either way.
5. **Determinism of error selection:** under `ExecutionPolicy::Parallel`, rayon surfaces a first-completed (not first-in-order) error when multiple factor repricings fail — is deterministic error selection required for serial/parallel triage parity?
6. **`default_waterfall_order`** places Fx after Rates/Credit/Inflation/Correlations and before Vol/ModelParams/Scalars — is this an intentional house methodology (path-dependence makes the FX line absorb FX-rates/FX-credit interactions accumulated to that point)?
7. **Taylor + MC pricers:** `instrument.value()` is called without the CRN seeding `fd_greeks` uses — is the seeding contract of `value()` guaranteed for MC-priced instruments, or are 1bp central differences noise-dominated there?
8. **Grids:** `KEY_RATE_BUCKETS_YEARS` (11 buckets incl. 15y/20y) vs `diff::STANDARD_TENORS` (9, without) — intentional?
9. **Detail-field roadmap:** are `rates_detail`/`credit_detail`/`fx_detail`/`inflation_detail` upcoming? If yes, M10's wire-format decision is hard-blocking now.
10. **Binding naming:** `attribute_pnl`, `attribute_pnl_from_spec`, `validate_attribution_json`, `AttributionParams` have no same-named Rust counterparts — is the JSON-pipeline-helper exemption from the canonical-API rule intended, and should the omitted canonical exports (`simple_pnl_bridge`, Taylor surface, enums) be recorded as `missing` in the parity contract?
11. **`PnlAttribution.from_json`** is lenient (no `deny_unknown_fields`, typo'd keys silently dropped) on a public inbound constructor — accepted exception to the strict-serde invariant?

---

## Verified Correct (spot checks that passed adversarial review)

- **Sign-convention pairing end-to-end on the main paths:** native dPV/dy DV01 × zero-rate-shift bp (`diff.rs:162-178`); hazard CS01 paired with **par CDS spread** moves with the credit-triangle `Δλ = Δs/(1−R)` fallback consistent on both sides (`diff.rs:204-319`, `bumps.rs:600-613`, `credit_cascade.rs:507-518`) — the classic 1/(1−R) overstatement trap is explicitly avoided and tested.
- **Vol unit chain consistent in both paths:** absolute-decimal surface bumps → vol-point conversion → `measure_vol_surface_shift` ×100 → Vega $/vol-pt, Volga $/vol-pt² (`taylor.rs:948-974`, `diff.rs:394-447`); the Vanna-as-CrossGammaSpotVol substitution trap is guarded by producer comments and tests.
- **Taylor coefficients:** central differences, `½·γ·Δx²` own-gamma, theta as full T1-date/T0-market reval + realized cashflows with total-return `total_pnl` — no carry/rates double count (rates measured at constant maturity; roll lives in theta).
- **Parallel/waterfall single-factor isolation direction is consistent** across every factor (`V(all-T1) − V(factor@T0)` via `compute_pnl(reprice, val_t1, ...)`); single-factor cases telescope exactly.
- **Waterfall sum preservation is Decimal-exact** (`Money::checked_add` accumulation, pinned by test); carry-first doctrine enforced at entry; period validation at all four entry points.
- **Serial ≡ parallel determinism:** all rayon fan-outs are order-preserving `par_iter().collect()` over fixed-order slices with serial reduction; pinned bit-exact (incl. `num_repricings` and `by_pair` maps) for parallel and Taylor.
- **No FX double counting in native reporting:** all in-method conversions are same-currency identities; `fx_pnl` is purely pricing-impact; translation only in `translate_to_target_ccy`, which reconciles algebraically for zero-coupon periods.
- **`compute_residual` defensive correctness:** currency validation before summation, non-finite guards, `result_invalid` gating of `residual_within_tolerance`, `RoundingContext`-gated `residual_pct` zero-division.
- **Serde determinism:** BTreeMap/IndexMap keyed maps, Money as Decimal strings, `deny_unknown_fields` on the inbound spec surface, schema gate in `AttributionEnvelope::execute`, deterministic FNV model id.
- **Bindings:** GIL released around `execute`; thin (no business logic; defaults delegate to Rust); Python↔WASM JSON parity by construction (same serde types); WASM panic containment via `catch_unwind` with structured error kinds; parity contract topology matches the live surface (enforced by tests).
- **QuantLib bond fixtures are sound:** file-backed, version-pinned (QL 1.42.1), generated by a committed script, independent of finstack; bond per-factor tolerances (0.005/$100 face) catch sign flips, 100×, and dropouts — the bond leg is a real regression surface (contrast M13/M14).

---

## Brief Summary

The crate is architecturally strong: the five-tier methodology layering is sensible and well-documented, factor isolation direction is consistent, waterfall telescoping is Decimal-exact, serial≡parallel determinism is real and pinned, credit cascade conservation closes exactly, and the serde/spec surface is mostly strict and schema-gated. The defects cluster in three places: (1) **cross-cutting reconciliation conventions** — the parallel cross-factor sign (B1), total-return vs MTM drift between methods and through translation (M2/M6), and the credit-factor sign contract (B5); (2) **producer/consumer unit contracts for pre-computed metrics** — convexity (B4), inflation convexity (M4), theta horizon (M3), model-param recipes (MO-T5): the metrics-based path repeatedly assumes units its producers don't emit, with no shared unit registry to stop it; and (3) **completeness of market-state plumbing** — snapshot families (B2), forward curves (M1), Taylor wing buckets (B3). The test suite is large and rigorous where it pins invariants, but its external-parity legs (FX forward, IRS) are vacuous at the factor level, and the highest-value scenario class — coupon dates, negative rates, basis moves, wing knots — is exactly where the confirmed bugs live. Bindings are thin and parity-clean topologically, but the primary Python entry point has a shipped usability regression (M11) with zero behavioral test coverage (MO-B4).

Residual risk after fixing the Blockers: medium — the unit-contract class (2) will recur as new metrics are added unless producer units are stamped into `MetricId` docs and enforced by producer/consumer tests.

## Quant Notes

- The parallel-method cross-term identity is the standard inclusion-exclusion decomposition (see Meucci, *Risk and Asset Allocation*; mixed second differences): with factor P&Ls measured from the T1 base, interactions must be **subtracted** — B1 is a textbook sign trap.
- Key-rate triangular weights must partition unity (Tuckman & Serrat, *Fixed Income Securities*, ch. on key-rate '01s); the half-triangle wing convention in `dv01.rs` is the correct standard and B3 should converge to it.
- Street convexity scaling (`ΔP/P ≈ −D·Δy + ½·C·100·Δy²` with C in per-100 units) follows Bloomberg YAS conventions already golden-tested in valuations — the consumer must match the producer it cites.
- Carry/total-return doctrine: a P&L explain over a coupon date must satisfy `total = MTM + cash` with carry absorbing the accrual+coupon leg (Christensen, *Fixed Income Attribution*); M2/M6 break this on two of four paths.
- Negative-rates safety for spread/yield decompositions (M7) is table stakes for EUR/JPY books; clamp shares to [0,1] rather than trusting `r+s` denominators.
- The ISDA-standard hazard/par-spread duality handling (re-bootstrap on bump, credit triangle fallback) is genuinely well done and worth protecting with the basis-move and wing tests it currently lacks.
