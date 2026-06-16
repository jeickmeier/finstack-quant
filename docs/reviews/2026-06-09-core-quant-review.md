# Quant Finance Review — `finstack-quant/core` + Python/WASM bindings

- **Date:** 2026-06-09
- **Scope:** `finstack-quant/core` (~83k lines) plus `finstack-quant-py/src/bindings/core/`, `finstack-quant-wasm/src/api/core/`, parity contract, and related tests.
- **Method:** Eight parallel deep-review passes (dates/conventions, money/FX, core math, volatility, expression engine/infra, market data, cashflow/credit, bindings parity), each grounded in the skill's market-standards references. Several findings were verified by independent numerical replication (Python replicas of the rough-Heston algorithm and Dupire extraction, scratch-binary execution of the expr cache/EWM/median paths, hand-traced `add_months` clamping, rust_decimal source inspection).

**TL;DR:** Well-engineered crate on determinism, validation, and documentation, but **4 Blockers** (wrong rough-Heston prices, stale expression-cache results, divergent vol-bump semantics, non-monotone PD curves) and **~29 Majors**. Two systemic themes: **schedule/calendar convention bugs that silently mis-date cashflows** (EOM drift, long-stub bug, FX spot lag, US/UK holiday observance), and **bump/roll/rebuild paths that disagree with their primary counterparts** (hazard CS01 bias, theta-free curve rolls, dropped policy stamps).

---

## Findings

### Blockers

**1. Rough-Heston Fourier pricer is mathematically wrong — two independent bugs.**
`finstack-quant/core/src/math/volatility/rough_heston.rs:119` has the fractional Riccati constant term as `a = ½(iu − u²)`; El Euch & Rosenbaum (2019) Thm 4.1 requires `a = −½(u² + iu)` (the current sign violates the martingale condition F(−i)=0). Separately, `rough_heston.rs:424-453` uses the Lewis (2000) integrand without the `e^{x/2}` factor. Verified by replicating the algorithm in Python against a trusted classical-Heston reference at H≈0.5: **+8% ATM, +37% at K=80, OTM wing prices clamp to exactly 0.0** (so `implied_vol()` returns `None`). The bugs partially cancel ATM, which is why the in-repo 15%-tolerance ATM-only test passes; the only wing test is `#[ignore]`d. With both fixes applied, agreement is 0.02–0.25% across ρ ∈ {0, −0.5, −0.7}. Propagates to `valuations/src/instruments/equity/equity_option/rough_heston_fourier_pricer.rs`; the public `char_func()` (line 328) carries the sign bug too. (The fractional Adams predictor-corrector weights themselves are correct vs Diethelm-Ford-Freed 2004.)

**2. Expression-engine persistent cache returns stale results across evaluations.**
Cache keys are `(dag_node_id, len)` with no input fingerprint (`finstack-quant/core/src/expr/cache.rs:124-149`, `expr/eval.rs:193,264-268,306-318`). Verified empirically: evaluating the same `CompiledExpr` on a second dataset of the same length **returns the first dataset's numbers** (`rolling_std(x,2)+rolling_std(x,2)` on `[1,2,3,4]` then `[10,40,90,160]` returned `[NaN,1,1,1]` instead of `[NaN,30,50,70]`). Since each deduplicated DAG node executes only once per eval, cross-eval reuse is the *only* thing the cache does — exactly the unsound case; concurrent threads sharing an instance also cross-contaminate. The expr README's "build a plan once and reuse it" guidance actively encourages the broken pattern. Fix: drop cross-eval persistence (stop auto-attaching in `with_planning`) or key entries on a data fingerprint/generation token. Blast radius bounded: statements uses its own evaluator; `CompiledExpr` is not exposed via bindings.

**3. `VolBucketPct` is multiplicative with filters, additive without — scenario preview vs execution disagree.**
With filters, `finstack-quant/core/src/market_data/context/ops_bump.rs:169-187` routes to `VolSurface::apply_bucket_bump` (vol × (1+pct/100), `vol_surface.rs:614-647`). With `expiries: None, strikes: None` it rewrites into `BumpSpec{Additive, Percent}` applied as vol + pct/100 **absolute vol points**. pct=10 on a 20% vol: 22% filtered vs **30%** unfiltered. `scenarios/src/adapters/vol.rs:239-249` computes previews multiplicatively but emits a bump the context applies additively when filters are absent. Both behaviors are pinned by different tests (`tests/market_data/context.rs:1101-1120` additive; `:811` multiplicative). Fix: route the unfiltered case through multiplicative semantics and update the pinning test.

**4. PD isotonic regression is not a correct PAV — non-monotone cumulative PDs and negative hazards.**
`finstack-quant/core/src/credit/pd/term_structure.rs:369-399` averages adjacent pairs instead of pooling weighted blocks with forward re-checks. Verified by execution: input `[0.05, 0.03, 0.03]` yields a *decreasing* output `[0.0375, 0.0375, 0.035]`; `[0.06, 0.05, 0.01, 0.02]` → `[0.0394, 0.0394, 0.035, 0.0263]`. `build()` (line 349) then returns a `PdTermStructure` violating its documented monotonicity invariant; `hazard_rate_between` and `marginal_pd` silently go negative. The colocated test (`monotonicity_enforcement`, `pd/tests.rs:288`) covers a single pattern that happens to pass. Fix: true weighted PAV (pool blocks, merge backward, re-check) plus a post-hoc monotonicity assertion in `build()`. True PAV for `[3,1,1]` is `[5/3,5/3,5/3]`, not pairwise averages.

### Major — schedules, calendars, day counts

- **Month-end schedule drift / broken regular schedules** — `finstack-quant/core/src/dates/schedule_gen.rs:131-163` steps cumulatively from the previous (clamped) date instead of `anchor + k×freq`. Monthly-from-Jan-31 gives …Feb 28, Mar 28, Apr 28… instead of the market-standard roll-day-31 sequence, and `StubKind::None` *errors* (`NonIntegerScheduleTenor`) on a perfectly regular Jan-31→Jul-31 monthly schedule. Backward generation drifts too (ShortFront from May 31: …Feb 28, **Mar 30**, Apr 30, May 31). Fix: QuantLib-style k-multiples from the unadjusted seed.
- **`StubKind::LongFront` never merges — produces a short-front schedule** — `schedule_gen.rs:201-221`; output is byte-identical to `ShortFront` for the same inputs, and `tests/dates/schedule.rs:203-225` codifies the bug. Long-first-coupon instruments get an extra coupon period. Fix: drop the lowest regular anchor when `prev < start` (mirror of `gen_long_back`).
- **EOM flag snaps effective and maturity dates to month-end** — `schedule_gen.rs:19-30,147-153`. `end_of_month(true)` with start Jan 15/end Apr 15 yields Jan 31…Apr 30: maturity silently lengthened. ISDA/QuantLib EOM only rolls intermediate anchors when the anchor is month-end; never moves start/end.
- **ACT/ACT ICMA frequency-only path mis-prices EOM coupons** — `finstack-quant/core/src/dates/daycount.rs:1086-1110,1140-1147`. The quasi-coupon grid is anchored at `start.add_months(-freq)` and clamping means it doesn't return to a month-end start: regular EOM semi-annual period [2025-08-31, 2026-02-28) gives 181/184 × 0.5 ≈ 0.49185 instead of exactly 0.5 (~0.8% of a coupon of accrued error). Golden ICMA fixtures (`tests/golden/data/daycount_quantlib.json`) only use 1st/15th anchors. Fix: anchor the grid on `start` with EOM-consistent rolling.
- **FX spot dates one day late for USD pairs around US holidays** — `finstack-quant/core/src/dates/fx.rs:219-269`. `add_joint_business_days` requires intermediate days good in *both* calendars; market convention: a US holiday at T+1 does not delay EUR/USD spot (only the final value date must be good in both, plus USD). EUR/USD traded Thu 2025-07-03 → market spot Mon 07-07; finstack returns Tue 07-08.
- **USNY calendar applies Saturday→Friday observance** — `finstack-quant/core/data/calendars/usny.json:6-16` uses `fri_if_sat_mon_if_sun` (OPM rule). Fed convention: Sunday→Monday only; banks open the Friday before a Saturday holiday. E.g., Fri 2026-07-03, Fri 2027-12-24, Fri 2028-11-10 wrongly marked holidays — wrong SOFR/Fedwire business days. Needs a `MonIfSun` observance variant.
- **GBLO Christmas/Boxing Day substitute stacking wrong in collision years** — `gblo.json:6-8` + `src/dates/calendar/rule.rs:571-591`. Independent `NextMonday` mapping collapses both holidays onto one day (Dec-25-on-Sunday: 2022, 2033 — Tue Dec 27 treated as business day) or misses Dec 28 (Dec-25-on-Saturday: 2021, 2027). Wrong GBP business days ~2 of 7 years incl. historical 2021/2022. Also missing UK one-offs: 2012/2022 Jubilees, 2020 VE Day move, 2022-09-19 funeral, 2023-05-08 Coronation.

### Major — money & FX

- **`Money` `Display` truncates instead of rounding** — `finstack-quant/core/src/money/types.rs:566-579`. rust_decimal precision-spec `Display` truncates (verified vs 1.42.0 source + empirically: `{:.2}` of `10.006` → `"10.00"`); `format(2, true)` returns `"10.01"`. `Money::new(99.9, JPY)` displays `JPY 99`. Fix: route `Display` through `format_with` (banker's rounding).
- **FX triangulation ignores pinned quotes** — `finstack-quant/core/src/money/fx/matrix.rs:773-811`. `rate()` honors authoritative pinned fixings; `triangulate_rate` → `get_or_fetch` skips them — internal triangular arbitrage within one matrix on the same date/policy, and triangulation fails when only a pinned leg exists. Fix: same precedence (explicit → pinned → observed → provider) in `get_or_fetch`.
- **`triangulated` metadata flag is cache-state-dependent** — `matrix.rs:216-247,679`. First lookup of a cross returns `triangulated: true` and caches the derived rate; subsequent identical queries return `false`; flips back after LRU eviction (cap 256). Stamped metadata depends on call history/thread timing — violates serial≡parallel. Fix: cache `(rate, triangulated)` together.

### Major — core math

- **`minimize()` silently returns best-guess on non-convergence** — `finstack-quant/core/src/math/solver_multi.rs:552-671,318-357`. `MaxIterations`/`StepTooSmall` terminations return `Ok`, stats discarded (`?.params`). Live in SABR calibration (`valuations/src/models/volatility/sabr/calibration.rs:294,625`) → uncalibrated parameters with no signal. Fix: return `LmSolution` or error on non-convergent termination reasons.
- **Scalar-objective "LM" is a root-finder for f(x)=0, not a minimizer** — `solver_multi.rs:327-357`. Step is δ = −f·∇f/(|∇f|²+λ), targeting f=0 not ∇f=0; acceptance compares `|f_new| < |f_old|`. Objectives with positive minima (vega-weighted SSE) misbehave near optimum; negative-capable objectives have improving steps rejected. Fix: genuine LM over residual vectors or a documented non-negativity precondition.
- **`transform_pca_to_assets` scrambles the asset axis** — `finstack-quant/core/src/math/random/sobol_pca.rs:136-162`. `z_temp = Q_sorted · z_pca` is already asset-ordered; the subsequent `z_out[permutation[i]] = z_temp[i]` applies the eigen-sort permutation to the asset axis: Cov(z_out) = P·ρ·Pᵀ ≠ ρ. Colocated test locks in the bug. No external callers today but exported. Fix: drop the permutation step.
- **Inverse normal/t CDF panic on out-of-domain p (incl. NaN)** — `finstack-quant/core/src/math/special_functions.rs:336-340,436-453`. statrs 0.18 panics; wrappers add only `debug_assert!` (compiled out in release). Reachable from user-supplied `alpha` in `OnlineStats::confidence_interval`/`required_samples` (`stats.rs:756-797`). Add range/NaN guards returning `Error::Validation`.

### Major — volatility

- **Dupire local-vol extraction biased low whenever r≠0** — `finstack-quant/core/src/math/volatility/local_vol.rs:122-127,218-225`. Forward-measure Dupire formula applied to *discounted* call prices; ∂C/∂T picks up a spurious −rC̃ term never added back. Verified numerically: flat 20% surface with r=3% extracts **18.9% ATM / 16.4% at K=80** at T=1. One-line fix: drop `df` (cancels in the strike derivatives). Colocated flat-surface test uses `rate = 0.0`.
- **No external golden tests for any vol model** — `finstack-quant/core/tests/golden/` contains only QuantLib day-count fixtures. Heston/SABR/SVI/Black/Bachelier/implied-vol tested by self-consistency only; precisely how the rough-Heston Blocker survived (the one cross-model check had 15% tolerance, ATM-only). Pin Heston to Albrecher et al. (2007)/QuantLib, SABR to Hagan (2002)/QuantLib `sabrVolatility`, Black-76/BSM to textbook values, rough Heston to El Euch-Rosenbaum/Gatheral-Radoicic smiles.

### Major — expression engine

- **LRU memory accounting double-subtracts → usize underflow panic** — `finstack-quant/core/src/expr/cache.rs:152-191`. `lru 0.16` `put()` returns `Some(old)` only on same-key replacement; capacity eviction silently drops. Verified panic ("attempt to subtract with overflow", cache.rs:180) reachable from normal public API use (re-eval with different row count); true capacity evictions over-count `current_memory`. Fix: use `push()` and subtract exactly once.
- **Result envelopes always stamped with `FinstackConfig::default()`** — `finstack-quant/core/src/expr/eval.rs:352-358`. `plan.meta` (the caller's `ResultsMeta` from `with_planning`) is never read — verified an `AwayFromZero` rounding context stamps `Bankers`. Fix: stamp `plan_to_use.meta.clone()`.
- **`ewm_var`/`ewm_std` silently return 0.0 on leading NaN** — `finstack-quant/core/src/expr/eval_functions.rs:863-901`. Seeds from `base[0]` unconditionally; NaN poisons EMA and `.max(0.0)` converts NaN→0. Verified: `ewm_std([NaN,1,5,9,2], α=0.5)` → all zeros. Zero vol from missing data is the worst silent failure for risk. Fix: seed from first non-NaN, emit NaN until then, guard the clamp.

### Major — market data

- **`DiscountCurve::roll_forward` doesn't renormalize by DF(dt)** — `finstack-quant/core/src/market_data/term_structures/discount_curve.rs:989-1052` (+ `common/knot_ops.rs roll_knots`, `context/ops_roll.rs`). Rolled curves preserve DF at calendar dates → implies 0% rate over the elapsed period; flat 5% cc curve rolled 1Y reads ~10% at the 1Y point; discounting theta identically zero (no carry, no roll-down) despite convertible-pricer comments (`valuations/.../convertible/pricer.rs:1202-1216`) claiming roll-down capture. `InflationCurve::roll_forward` rebases correctly (`inflation.rs:463-486`). Fix: divide rolled DFs by DF_old(dt) (realized-forward) or keep knots in tenor space (constant-tenor); document the choice.
- **Hazard `rebuild_interp` (bump/CS01 path) uses a different λ-segment convention than `build()` for zero-anchored curves** — `finstack-quant/core/src/market_data/term_structures/hazard_curve.rs:486-515 vs 948-969`. Bumping re-attributes base hazards → spurious (λ₁−λ₀)·t₁-type CS01 component. Doc examples use zero-anchored curves (`context/mod.rs:1577`). Also `sp()` vs `hazard_rate()` disagree beyond the last knot for zero-anchored curves. Fix: one shared accumulation function.
- **`deny_unknown_fields` silently inert on all flattened `Raw*` curve states** — `discount_curve.rs:213-246`, `forward_curve.rs:178-201`, `hazard_curve.rs:141-168`, and Raw{Inflation,Price,VolatilityIndex,BasisSpread} siblings. serde does not support it with `#[serde(flatten)]`; typo'd inbound curve JSON deserializes cleanly. Also missing entirely on `FxDeltaVolSurface` (derived `Deserialize` bypasses `validate()`), `ForwardVarianceCurve`, `DieboldLi`, scalars. Fix: inline the flattened fields or custom rejecting deserializers; route FxDeltaVolSurface through validating `TryFrom`.
- **Rebuild-style bumps/rolls drop `fx_policy` and hazard metadata** — `discount_curve.rs:840-865,934-987,1029-1052`; `forward_curve.rs:581-612,676-703,745-766`; `bumps.rs:570-633`. `with_parallel_bump`, key-rate rebuilds, `roll_forward`, `Bumpable::apply_bump` rebuild without re-threading `fx_policy` (hazard path also drops issuer/seniority/currency/par_interp and reports stale `cds_quote_bp`). Risk built via these paths silently loses the FX policy stamp; context in-place path preserves it. Fix: thread full metadata like `to_builder_with_id`.

### Major — credit

- **Altman Z'' mixes the EM +3.25 constant with non-EM 2.60/1.10 cutoffs** — `finstack-quant/core/src/credit/scoring/altman.rs:291-298`. All-zero ratios (deep distress) score 3.25 → "Safe" (implied PD ≈ 0.9%); distressed non-manufacturers shift ~2 zones optimistic; `z_score_implied_pd` inherits. Fix: drop constant with 2.60/1.10, or keep constant with EM cutoffs 5.85/4.35.
- **`central_tendency` uses geometric mean while claiming "the standard regulatory approach"** — `finstack-quant/core/src/credit/pd/calibration.rs:96-137`. Basel IRB / EBA GL/2017/16 define the long-run average default rate as *arithmetic*; GM systematically understates (1.5% vs 2.5% for `[0.5%, 4.5%]`). Switch or re-document as a non-regulatory house choice.

### Major — bindings

- **ForwardCurve bindings hard-code Act/360, defeating Rust curve-ID inference** — `finstack-quant-py/src/bindings/core/market_data/curves/forward.rs:60`, `finstack-quant-wasm/src/api/core/market_data.rs:197-200`. Rust infers day-count/reset-lag from the ID (`infer_forward_curve_defaults`, `forward_curve.rs:255` — GBP-SONIA → Act365F); both bindings always call `.day_count(act_360 default)`. ~1.4% systematic accrual error class for non-Act360 curves built from Python/JS. Fix: only call `.day_count()` when supplied (DiscountCurve binding does this correctly).
- **Python `ScheduleBuilder` defaults Quarterly; Rust defaults Monthly** — `finstack-quant-py/src/bindings/core/dates/schedule.rs:260` vs `schedule_iter.rs:629-646`. Silently different schedules cross-language; no Python test covers ScheduleBuilder.
- **Error mapping collapses everything to `ValueError`** — `finstack-quant-py/src/errors.rs:73-75`. KeyError (missing id) / RuntimeError (operational) policy unimplemented despite structured core variants (`error/inputs.rs:165-199`); credit modules bypass `core_to_py` via `display_to_py`, dropping the source chain. Fix: match `MissingCurve | NotFound | CalendarNotFound` → `PyKeyError`; route credit through `core_to_py`.
- **`MarketContext.insert_price` panics on NaN** — `finstack-quant-py/src/bindings/core/market_data/context.rs:126` uses panicking `Money::new` on user input → `PanicException` instead of `ValueError`. Fix: `Money::try_new(...).map_err(core_to_py)?`. (WASM correctly uses `try_new`.)

### Moderate (condensed, by domain)

**Dates**
- CDS schedules roll start *forward* to the next IMM roll, dropping the standard front accrual period (post-Big-Bang accrues from prior roll) — `schedule_iter.rs:682-691,947-965`.
- Brazil Nov-20 (Consciência Negra) not year-gated from 2024 — phantom holiday corrupts pre-2024 BUS/252 — `data/calendars/brbd.json:11`.
- `CompositeCalendar` ignores sub-calendar weekend rules (inherits hardcoded Sat/Sun) — `calendar/composite.rs:95-110`.
- Convention gaps: no 30E/360 ISDA, no NL/365; `act_365afb` alias conflates Act/365L with ACT/ACT AFB; `Thirty360Convention::Isda` unreachable dead public API — `daycount.rs:241-576,927,1244`.
- Act/365L boundary semantics deviate from ICMA Rule 251 ([start,end) vs (start,end]; frequency rule ignored) — `daycount.rs:1163-1194`.
- `DayCountContextState` silently drops `coupon_period` on serialization → exact ICMA downgraded to drifting frequency path — `daycount.rs:163-204`.
- Fixed-holiday observance cannot cross year boundary; `applies()` vs `materialize_year()` diverge — `calendar/rule.rs:642-651`.

**Money/FX**
- `from_f64_retain` embeds IEEE noise in the Decimal store (`0.1` → 28-digit value; breaks `PartialEq`, pollutes serde wire/golden files) — `money/types.rs:297`, `rounding.rs:90,123,145`.
- `with_bumped_rate` flattens the FX term structure for date-aware providers (constant rate for every date + pair-global pin) — `matrix.rs:498-554`, `providers.rs:241-276`.
- Reciprocal rates not re-validated; `FxMatrix::rate` can return `+inf` — `provider.rs:11-34`, `matrix.rs:195-228`.
- Persistence drops pinned fixings; `FxMatrixState` lacks `deny_unknown_fields` — `matrix.rs:428-455`, `fx/types.rs:175-181`.
- `NumericMode::F64` stamped on Decimal-backed money results — `config.rs:487,392-412`.

**Math**
- Newton convergence criteria purely absolute (1e-12) — unattainable for dollar-scale residuals, premature for tiny scales — `solver.rs:320-330,508,564`.
- Adaptive Gauss-Legendre silently returns at max_depth (vs adaptive_simpson which errors); GH adaptive returns inconsistent-order estimates — `integration.rs:909-936,376-445`.
- Mittag-Leffler raw Taylor only; garbage for large negative real z (the advertised rough-Heston regime) — `fractional.rs:192-232`.
- `cholesky_correlation` absorbs NaN inputs (NaN pivot via `total_cmp`) → `Ok(NaN factor)` — `linalg.rs:311-341`.
- Poisson sampler: plain N(λ,λ) at λ≥30 without continuity correction; small-λ truncated at k=200 silently — `random/poisson.rs:23-53` (live in jump_euler/merton).
- Yang-Zhang RS term biased by (n−1)/n; k uses bar count vs return count — `stats.rs:593-638`.
- Inverse-CDF doc cites Wichura AS241 but statrs uses Boost-style erfc_inv; tail tests ~12 orders looser than delivered precision (regressions invisible) — `special_functions.rs:85-88` + tests 517-607.
- LM FD step 1e-8 in central differences (optimal ~6e-6·scale); Levenberg +λI not Marquardt λ·diag(JᵀJ) — `solver_multi.rs:266-276,446-498`.
- Hagan-West positivity projection sequential, can re-violate previous segment; no fixpoint pass/collar — `interp/strategies.rs:1096-1153`.

**Volatility**
- Heston σ_v→0 fallback uses √v0 instead of time-averaged deterministic variance (10% vs ~23.5% in worked example); discontinuity at 1e-10 threshold — `heston.rs:438-440,504-508,1121`.
- `VOL_CEIL_BACH = 10.0` in price units; Bachelier implied vol fails (loudly) for price-quoted underlyings — `implied.rs:52,379`. Ceiling should scale with `max(|F|,|K|)`.
- VG cumulant c1 missing `+θt` (Fang & Oosterlee Table 2) → COS truncation mis-centered for skewed VG — `characteristic_function/variance_gamma.rs:129-131`.
- SABR silently prices cross-zero (f·k≤0) β>0 inputs with an arbitrary internal shift — `sabr.rs:319-327`. Should error or require explicit `with_shift`.
- Vol model param structs (`SabrParams`/`HestonParams`/`RoughHestonFourierParams`/`SviParams`) deserialize without validation or `deny_unknown_fields` — `sabr.rs:76-90`, `heston.rs:72-84`, `rough_heston.rs:219-239`, `svi.rs:62-74`.

**Market data**
- `apply_bucket_bump` strike match tolerance 0.01 *absolute* (±100bp for IR decimal strikes); expiry tolerance 0.01y collides sub-weekly expiries — `vol_surface.rs:630-635`.
- Hazard bump negative-clamp semantics differ between paths (`apply_bump` clamps+warns; `bump_in_place` errors); `bump_in_place` lacks recovery<1 guard (recovery=1.0 → λ=inf silently) — `bumps.rs:577-623` vs `hazard_curve.rs:546-557`.
- Day-count basis silently inferred from curve-ID substrings; renamed ID → ~1.4%·t time error with no diagnostic — `common/conventions.rs infer_discount_curve_day_count`.
- Inflation `cpi_with_lag` is continuous t−months/12 shift, not the documented Canadian-model monthly reference-index interpolation; no seasonality — `inflation.rs:253-269,83-89`.
- FX delta surface docs claim spot-delta GK but implementation is forward delta; BF treated as smile strangle (not market strangle); expiry interp linear in vol not total variance — `delta_vol_surface.rs:16-31`, `surfaces/mod.rs:70-89`.
- Merged global strike grid flattens short-expiry FX smiles (flat-extrapolated wings dominate) — `delta_vol_surface.rs:295-360`.

**Credit / cashflow / types**
- `npv` has no valuation-date cutoff; past flows silently future-valued (curve-extrapolation-dependent) — `cashflow/discounting.rs:216-268`.
- XIRR Newton acceptance uses absolute 1e-6 currency-unit tolerance → scale-dependent; large notionals lose Newton roots, fall to Brent bounded (−0.99, 10.0); >1000% IRRs lost — `cashflow/xirr.rs:427,411-448`. Normalize flows first.
- Downturn LGD misattributed to Frye-Jacobs (2012) (actual: ad-hoc mean+multiple-of-Bernoulli-stdev); `downturn_lgd_frye_jacobs` hardcodes sensitivity 1.0 vs documented 0.3-0.5 — `lgd/downturn.rs:17-34,147-159`, `lgd/mod.rs:117-123`.
- WARF table: Ba3=1760 vs published Moody's 1766; CC=9550 unpublished — `data/credit/credit_assumptions.v1.json` via `types/ratings.rs:520-534`.
- `from_transition_matrix` rounds tenors silently and ignores `tm.horizon()` — `credit/pd/term_structure.rs:256-301`.
- KS generator regularization changes economics without stamping; 1e-2 inf-norm default tolerance is 100bp of row probability (IG rows distorted invisibly) — `migration/generator.rs:101-138,273-297`.
- `Bps` f64 conversions asymmetric: `From<Bps> for f64` returns decimal, `TryFrom<f64>` reads bp count — round-trip turns 25bp into 0bp — `types/rates.rs:402-410,517-521`.
- Ohlson zone cutoffs (raw O ∈ {0.38, 0.50}) label PD≈59% firms "Safe"; unrelated to Ohlson's P*=0.038 — `scoring/ohlson.rs:84-87,163-170`.

**Bindings**
- Schedule `build()` fails closed on warnings → `MISSING_CALENDAR_WARNING` policy selectable but dead; `error_policy` toggling order-dependent — `finstack-quant-py/.../dates/schedule.rs:306-336`.
- DayCountContext silently drops unknown calendar IDs to `None` (both bindings); `coupon_period` unreachable from Python/JS — `finstack-quant-py/.../dates/daycount.rs:186-198`, `finstack-quant-wasm/.../dates.rs:24-37`.
- Systematic naming drift vs canonical Rust: `survival` vs `sp`, `forward_rate` vs `forward`, `check_*` vs `*_grid`, `get_*` prefixes, abbreviated scoring params — parity contract pins topology, not method names.
- `zmijewski_score` drops the `zone` field returned by Rust — `credit/scoring.rs:175-183`.
- No GIL release anywhere in finstack-quant-py (migration MC, Cholesky, Diebold-Li/PCA) despite the standard — zero `allow_threads` hits.
- WASM `FxDeltaVolSurface` exported by facade + contract but absent from `index.d.ts`; no core market-data dts assertions — `exports/core.js:19`.
- WASM `FxRateResult` invents a binding-side `policy` field and uses `getX()` methods vs Python property getters — `finstack-quant-wasm/.../market_data.rs:310-336`.
- Money Decimal fidelity one-way: lossless ingestion (Python) but f64-only read-back; WASM has no Decimal/string path at all. Needs core `Money::amount_decimal()` mirrored in bindings.
- No Node `*.test.mjs` facade tests exist despite the wasm standards requiring them.

### Minor (selected)

- Bump-ID formatting collides: `{:.0}` makes 1.5% → `_bump_2pct`; ±0.4bp both → `_bump_0bp` — `bumps.rs:433-445`.
- Vol cube/surface silent 0.001 vol floor on non-finite/non-positive SABR expansions; no normal-vol cube output — `vol_cube.rs:373-381`, `vol_surface.rs:1055-1082`.
- Fixings LOCF without staleness bound (`value_on_or_before(max_staleness_days)` exists, unused) — `fixings.rs:90-109`.
- `[workspace.lints]` is dead config: no crate has `[lints] workspace = true`; `indexing_slicing`/`unreachable` enforced nowhere — root `Cargo.toml:113+`.
- `median`/`rolling_median` count NaN as largest value (verified `median([1,2,3,NaN])` = 2.5); three conflicting NaN policies across expr reducers; `ewm_mean` NaN-poisons while `ewm_var` skips — `eval_functions.rs:472-498,620-659,361-393`.
- `rolling_var_incremental` uses E[X²]−E[X]² (cancellation); window args read only element [0] of arg series — `eval_functions.rs:589-611,33-39`.
- Sobol: no golden test vs Joe-Kuo reference (dims 21–40 provenance unverified); `next_point` can emit exactly 0.0 — `random/sobol.rs`.
- `d1_black76` returns 0.0 (⇒N(d1)=0.5) for degenerate inputs, inconsistent with delta digital-limit convention — `black.rs:374-381`.
- Heston fixed 128-node quadrature: deep-wing strikes under-resolved when upper limit clamps to 500 — `heston.rs:643-682`.
- `implied.rs` docs cite Jäckel (2017) but implement bisection+Halley (~148 evals vs ~2) — `implied.rs:20,64`.
- `CashFlow::validate` rejects zero amounts (floored coupons can't pass) — `cashflow/primitives.rs:388-394`.
- `BetaRecovery::sample`/`quantile` silently fall back to mean on internal errors — `lgd/seniority.rs:171-202`.
- `Rate`/`Percentage` serde bypasses finiteness gate for non-self-describing formats — `types/rates.rs:153-155`.
- Currency table: CLF (4dp), SSP, XXX/XAU/XAG/XDR absent; `decimals()` silently falls back to 2 — `data/iso_4217.csv`.
- Money `From<(i64|u64, Currency)>` casts through f64 (corrupts >2^53) — `money/types.rs:649-660`.
- `nth_weekday_of_month` with n>occurrences silently returns next-month date — `calendar/generated.rs:14-42`.
- `enforce_monotonic_and_dedup` silently drops coupon dates that collide after adjustment — `schedule_gen.rs:84-98`.
- Python money.pyi declares `__iadd__` etc. the frozen class doesn't define; WASM `Rate.fromPercent` re-implements pct/100; `fromBps(f64)` rounds vs Rust i32.
- Expr README stale/non-compiling; `missing_curve_error` available-IDs list in FxHashMap order — `context/mod.rs:905-908`.

---

## Open Questions or Assumptions

1. **`VolBucketPct` semantics** — multiplicative (per docs) or additive? Tests pin both; fixing requires a decision + updating `tests/market_data/context.rs:1119`.
2. **Curve roll convention** — is "DF preserved at calendar dates" deliberate? It zeroes discounting theta; convertible-theta comments expect roll-down. Choose realized-forward vs constant-tenor and document.
3. **Is the rough-Heston Fourier pricer in a live valuation path** or experimental? Determines hotfix urgency.
4. **`from_f64_retain` and the 28-digit wire strings** — do existing golden files depend on them?
5. **Geometric-mean central tendency and Z''+3.25/2.60** — deliberate house methodology needing re-documentation, or transcription errors? Same for WARF Ba3=1760/CC=9550 provenance.
6. **Binding divergences that read intentional** (Quarterly default, warnings-fail-closed) — fix to match Rust, or document in the parity contract's `method_gaps`?
7. `usny` scope (Fed vs SIFMA vs OPM) and `brbd` scope (ANBIMA vs B3) are undocumented; the calendar findings assume Fed and B3.
8. Should `Money::amount_decimal()` be added to canonical Rust so Python/WASM can expose lossless Decimal read-back without violating the Rust-canonical rule?
9. Is cross-eval expression caching a requirement at all? Deleting `CacheManager` + auto-attach removes both expr cache findings wholesale.
10. MonotoneConvex rejects DF>1 (negative-rate) curves by policy — conflicts with the workspace's own algorithm-standards checklist; which policy wins?

---

## Brief Summary

A genuinely strong codebase by desk-library standards: currency safety is real (no raw `Add` on Money, typed mismatch errors), determinism discipline is consistent (FxHash, `total_cmp`, sorted snapshot serde, seeded RNG with state round-trips), validation posture is fail-closed almost everywhere, and citation/provenance quality (Padé coefficients digit-perfect vs Higham 2005, ISDA section references, documented rejected-review decisions) is unusually good. Verified correct against canonical formulations: key-rate bump methodology (partition-of-unity tested), arbitrage checks (Durrleman g(k), total-variance calendar), Heston little-trap CF, SABR Hagan 2.17a/b terms, Black-76/BSM/Bachelier pricers, Nelson-Siegel/Svensson, ACT/ACT ISDA & 30/360 US day counts, Altman 1968/Z' & Ohlson & Zmijewski coefficients, Vasicek PiT/TtC inverse.

Residual risk concentrates in three places:
1. **Secondary paths diverging from primary paths** — bump-vs-build hazard conventions, filtered-vs-unfiltered vol bumps, rebuild paths dropping policy stamps, binding defaults overriding Rust inference. The classic source of risk-vs-pricing inconsistency.
2. **Boring date plumbing** — EOM schedules, long stubs, FX spot lags, holiday observance. Silently mis-dates cashflows; will show up as small unexplained P&L vs counterparties.
3. **Missing external benchmarks for vol models** — allowed a sign error in rough Heston to survive its own test suite.

## Quant Notes

- Highest-leverage single action: add **golden parity fixtures** (QuantLib/published values) for Heston, SABR, Black-76, Bachelier, and schedule generation — the pattern already proven for day counts; would have caught the Blocker and three Majors.
- The schedule fixes (anchor-multiple generation, LongFront merge, EOM intermediate-only) should land together with regenerated test expectations — several existing tests codify the buggy behavior, so the test updates are the spec decision, not collateral churn.
- References used in verification: El Euch & Rosenbaum (2019) Thm 4.1; Lewis (2000); Albrecher et al. (2007) little trap; Hagan et al. (2002) §2.17; Diethelm-Ford-Freed (2004); Fang & Oosterlee (2008); Frye & Jacobs (2012); Altman (1968, 1995); Ohlson (1980); Zmijewski (1984); EBA GL/2017/16; ISDA 2006 Definitions §4.12/4.16; ICMA Rule 251; Fed/UK holiday observance rules; Yang-Zhang (2000); Higham (2005); Joe-Kuo Sobol tables (dims 21–40 provenance still unverified — worth a golden test).

---

# Implementation Status — reconciled 2026-06-11

Full audit of every finding against the working tree (three independent verification passes, evidence at file:line confirmed for each item).

## Scorecard

| Severity | Total | Fixed | Doc-fix (as prescribed) | Superseded (fixed differently) | Deferred (recorded) | Outstanding |
|---|---|---|---|---|---|---|
| Blocker | 4 | 4 | — | — | — | 0 |
| Major | 29 | 28 | — | — | — | 1 partial |
| Moderate | ~30 | 26 | 3 | — | 1 | 0 |
| Minor | ~19 | 14 | 2 | 2 | 1 | 1 sub-item |

**Open Questions: 10 of 10 resolved** (decisions recorded inline in code/tests: VolBucketPct multiplicative; roll_forward realized-forward; from_f64; Z''/central-tendency/WARF to published values; binding renames to canonical Rust; usny=Fed K.8 / brbd=B3 documented; amount_decimal added; expr cache deleted; MonotoneConvex negative-rate support — see Remaining item 5).

## Notable: golden tests found real bugs after the review

- The Sobol Joe-Kuo golden test (added per the Minor finding) caught **wrong direction numbers for dims 18–39 and a missing dim 40** (out-of-bounds panic) — corrected to the published table (commit 767b3933f). QMC sequences for dims ≥ 18 changed (now correct).
- The Parlett recurrence cross-term sign error in the generator-matrix logarithm was found and fixed while implementing the KS-regularization stamping (verified against scipy.linalg.logm).

## Remaining items (deliberate)

1. **External QuantLib/published golden fixtures for vol models** (the partial Major): **RESOLVED 2026-06-11** — `tests/golden/data/vol_models_quantlib.json` (QuantLib 1.42.1 via `gen_vol_golden.py`, 122 cases) pins Black-76, BSM, Bachelier (incl. negative rates), implied-vol round-trips, SABR Hagan lognormal/normal/shifted, Heston (little-trap), SVI (50-digit mpmath), and the rough-Heston H=0.499 classical limit; found two genuine accuracy limits (norm_cdf ~1e-9 in price space; Heston T=1 quadrature up to ~7.5e-5 abs at vov 0.5/ρ −0.9 — 4 cases pinned but skipped, documented in the fixture).
2. **FX delta surface merged-strike-grid redesign** (Moderate): **RESOLVED 2026-06-11** — per-expiry smiles are now the single source of truth (`fx_smile_pillars` shared by `FxDeltaVolSurface::implied_vol` and the builder); the rectangular materialization samples each expiry's own smile on the union strike axis (regression-tested pillar-exact in `tests/market_data/surfaces/fx_delta_vol_tests.rs`), and the residual off-pillar fixed-strike blending of the materialized grid is documented with `implied_vol` as the smile-faithful query path.
3. **Workspace lint inheritance** (Minor): **RESOLVED 2026-06-11** — every workspace member now opts in via `[lints] workspace = true`, with a documented split: the root table holds all-target-safe lints (deny: `unsafe_code`, `missing_docs`, `dbg_macro`/`todo`/`unimplemented`, `lossy_float_literal`, `match_wild_err_arm`, `redundant_field_names`, `match_like_matches_macro`, the `manual_*` family, `boxed_local`; warn pending legacy cleanup: `manual_let_else`, `match_same_arms`, `redundant_clone`, `clone_on_ref_ptr`, `match_wildcard_for_single_variants`), while lib-only strictness (`panic`/`unwrap_used`/`expect_used`/`unreachable`/`indexing_slicing`/`float_cmp`) stays as lib.rs attributes so tests and benches keep idiomatic panics. Enabling inheritance surfaced and fixed ~140 real violations (redundant clones, let-else rewrites, missing docs on wasm bindings and test crates).
4. **Normal-vol output from the swaption cube** (Minor sub-item): **RESOLVED 2026-06-11** — `VolCube::vol_normal`/`vol_normal_clamped` and `materialize_{tenor,expiry}_slice_normal` (tagged `VolQuoteType::Normal`) route through SABR's Hagan normal expansion with shift support and a normal-units floor (`1e-8·max(|F|,1)`); price-space cross-checks vs the lognormal quote in `tests/market_data/surfaces/vol_cube_tests.rs`. Binding exposure deferred to a parity pass.
5. **Open Question 10 — MonotoneConvex rejects DF > 1 (negative-rate) curves**: **RESOLVED 2026-06-11** (user decision: support negative-rate curves). The `validate_monotone_nonincreasing` rejection was removed from `MonotoneConvexStrategy` (DF must still be > 0 and finite), and the Hagan-West positivity projection is now auto-conditional: applied only when all discrete forwards are non-negative, skipped for genuine negative-rate curves so negative forwards interpolate faithfully; `DiscountCurve` builds DF > 1 MonotoneConvex curves under `ValidationMode::NegativeRateFriendly`/`Raw`, and the obsolete bootstrap guard in `valuations/calibration/targets/discount.rs` was removed.

All decisions that changed semantics (NPV cutoff/holder-view deposits, Money from_f64 wire format, binding renames, CDS front accrual, hazard bump errors, USNY/GBLO observance, Sobol dims ≥ 18) carry inline comments citing this review, with regenerated goldens documenting provenance.
