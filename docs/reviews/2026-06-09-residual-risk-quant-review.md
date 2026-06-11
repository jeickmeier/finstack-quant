# Residual-Risk Quant Review — 2026-06-09

Follow-up to the earlier valuations review, covering items listed under "Residual Risk / Not Reviewed". Conducted via parallel deep-review agents; all Blocker-level claims were independently re-verified in source before inclusion.

## Coverage

**All 11 domains reviewed** (two passes; the second pass re-ran the six agents killed by a session usage limit in pass one).

Pass 1 (Part 1 below):

- Swaption LMM/Cheyette-rough pricers + `exotics_shared` HW1F calibration/curve/LSMC/MC internals
- `models/credit/merton.rs` + `calibration/solver/bootstrap.rs`
- Commodity swap/swaption/asian + FX vanna-volga + equity vol-index future/option
- Equity Heston/rough-Bergomi/rough-Heston MC pricers + dcf_equity/pe_fund/real_estate
- Short-rate trees (`short_rate_tree.rs`, `hull_white_tree.rs`) + Heston Fourier (`closed_form/heston.rs`) + COS engine

Pass 2 (Part 2 below):

- Rates: `instruments/rates/repo/` + swaption/IRS/inflation `metrics/` submodules
- Credit: `correlation/copula/multi_factor.rs`, `random_factor_loading.rs`, `student_t.rs`, cds_tranche `recovery01.rs`/`tail_dependence.rs`
- Fixed income: `bond/pricing/engine/merton_mc.rs`, `revolving_credit/` internals (incl. pricer MC stack), `core/src/dates/schedule_gen.rs` + `schedule_iter.rs`
- Securitized: `structured_credit/pricing/stochastic/default/*`, `stochastic/metrics`, `pricing/diversion.rs`, waterfall coverage tests, pool `warf.rs`/`was.rs`
- Models: `calibration/hull_white.rs` (full file)
- Bindings: valuations/correlation bindings (py + wasm) + envelope TypedDict internals

Spot-verified directly in source before inclusion: all four Part 1 Blockers, and in Part 2 the inflation vega scaling, LongFront stub generation, SDA curve shape divergence, HW futures convexity κ→0 limit, and Student-t `integrate_fn` conditioning.

---

# Part 1 — First-pass findings

## Findings

### Blockers

#### B1. Rough-Heston Fourier pricer is wrong twice over — Riccati sign + Lewis contour

> **FIXED 2026-06-10.** Riccati constant term corrected to `a = −½(u² + iu)`; Lewis phase now `i·(u−i/2)·x` (carries the `e^{x/2}` contour factor); the `.max(0.0)` clamps on call and parity-put were removed. M7 (`v0·I^{1−α}D(T)` terminal term) was fixed in the same slice. New non-circular tests: `F(−i)=0` martingale trajectory check, `φ(−i)=e^{(r−q)T}`, σ→0 Black-Scholes golden values for calls *and* puts across moneyness (the review's 24.59 / 10.45 / 3.25 scenario), and classical-Heston agreement at H=0.499 across strikes at 0.5% (was 15% ATM-only).

`finstack/core/src/math/volatility/rough_heston.rs:119` and `:430`

The fractional Riccati constant term is `a = ½(iu − u²)`; the correct El Euch–Rosenbaum coefficient is `−½(u² + iu)`. Verified: the martingale condition fails — F(−i) evaluates to 1, not 0. Separately, the Lewis inversion drops the `e^{x/2}` contour factor (the phase is `i·u·x` where it must be `i·(u−i/2)·x`), and the result is silently clamped with `.max(0.0)` at line 453 — deep-OTM calls return exactly 0. Benchmarked in the BS limit: 34.22 / 12.66 / 0.00 vs true 24.59 / 10.45 / 3.25.

**Impact:** every rough-Heston Fourier price and implied vol is wrong by 10–40%; the existing tests cannot catch it because the put is *defined* via parity and the H≈0.5 Heston comparison uses 15% tolerance.

**Fix:** `a = -0.5*(u*u + iu)`; use `w = u − i/2` in the exponent phase (`exponent = i*w*x + C + D·v0`); remove or log the `.max(0.0)` clamp.

#### B2. Rough-Bergomi MC returns undiscounted payoff as PV

> **FIXED 2026-06-10.** `mean_pv` is now multiplied by `(-r*t).exp()` and the false "already discounted" comment was removed, matching `heston_mc_pricer.rs` / `rough_heston_mc_pricer.rs`. Regression test `rbergomi_nonzero_rate_price_is_discounted` prices at r=10% in the η→0 BS limit and rejects the undiscounted value.

`finstack/valuations/src/instruments/equity/equity_option/rough_bergomi_mc_pricer.rs:325-331`

The comment claims "simulate_path_fractional already returns discounted PV — do not multiply by discount_factor again." That is false — the payoff module (`finstack/monte_carlo/src/payoff/vanilla.rs`) explicitly documents that vanilla payoffs are undiscounted, and `engine_fractional.rs` applies no discounting (verified).

**Impact:** prices high by `e^{rT}−1` (~3% at r=3%, 1y; worse long-dated). Invisible to in-crate tests because they all use r=q=0.

**Fix:** multiply `mean_pv` by `(-r*t).exp()`, matching `heston_mc_pricer.rs:129` and `rough_heston_mc_pricer.rs:185`.

#### B3. Commodity swaption annuity mis-scaled ~12× vs its own underlying

> **FIXED 2026-06-10.** Decision (Open Question 2): `notional` stays a per-period quantity, as documented. `annuity()` is now `Σ DF_i` and `forward_swap_rate` uses matching `DF_i` weights, consistent with `CommoditySwap`'s unaccrued `quantity × price` per-period payout. The intrinsic branch flows through the same annuity. New test `b3_zero_vol_itm_swaption_matches_underlying_swap_pv` checks a zero-vol monthly ITM swaption against an independently computed `notional × (F−K) × Σ DF`.

`finstack/valuations/src/instruments/commodity/commodity_swaption/types.rs:296-312` (also intrinsic branch `:359-369`)

`annuity()` is `Σ DF·τ` (IR-swaption convention) while `notional` is documented "Notional quantity per period" (`types.rs:104-105`) and the underlying `CommoditySwap` pays `quantity × price` per period with **no** year-fraction accrual (`commodity_swap/types.rs:222-226`, verified). The consistent annuity for a per-period-quantity notional is `Σ DF`.

**Impact:** a monthly-settling swaption (τ≈1/12) is understated ~12×, including its intrinsic branch.

**Fix:** `annuity = Σ DF` (with matching `w_i = DF_i` weights in `forward_swap_rate`), or redefine/redocument `notional` as an annualized rate and prove consistency with `CommoditySwap.quantity`.

#### B4. Hull-White tree drift calibration is off by one step

> **FIXED 2026-06-10.** The solved α is now stored at `alpha[step]` (the index `forward_state_prices` and `backward_induction` read for that interval), with terminal `alpha[N] = alpha[N−1]` for end-of-grid accessors. `test_tree_calibration` tightened to 0.1bp, and `test_tree_calibration_steep_curve` reprices a 1.2%→6% zero curve at every pillar (state prices + backward induction) to <0.1bp. All downstream HW swaption / Bermudan / tree-pricer tests pass unchanged.

`finstack/valuations/src/models/trees/hull_white_tree.rs:290` (vs `:541` and `:829`)

The α solved at iteration `step` (matching `P(0, t_{step+1})` over `[t_step, t_{step+1}]`) is stored in `alpha[step+1]`, while both `forward_state_prices` and `backward_induction` discount that interval with `alpha[step]`. Verified in source. The tree does not reprice the curve except when flat; the in-repo test's 1bp tolerance on a mild curve is almost exactly the size of the bug (~0.8bp).

**Impact:** on steep curves, 20–40bp of ZCB bias feeding HW swaptions (`swaption/hw_pricer.rs:174`, `bermudan/mod.rs:84`) and callable bonds (`bond/pricing/engine/tree/tree_pricer.rs:262`).

**Fix:** store the solved value at `alpha[step]`; set terminal `alpha[N] = alpha[N−1]`; tighten `test_tree_calibration` to ~0.1bp and add a steep-curve case.

### Major

#### Rates / Cheyette-rough

Mitigated in the production registry path by `enforce_calibration: true` (`pricer/exotics.rs:239`), but live for direct construction (`BermudanSwaptionCheyetteRoughPricer::default()`).

- **M1. Look-ahead vol in the Euler step** — `finstack/monte_carlo/src/discretization/cheyette_rough.rs:104-131`. The fBM increment is added *before* σ is computed, so the step's vol sees its own shock; with ρ≠0 this creates a spurious drift of `O(dt^{H−1/2})` that **grows under grid refinement** for H<0.5 (~−300/400bp cumulative drift in the x-state at pricer defaults). Fix: adapted/left-point scheme — compute σ from accumulated `W̃_H(t)` before adding the increment, with compensator `t^{2H}` not `t_next^{2H}`.
  > **FIXED 2026-06-10.** Left-point scheme implemented: σ_t is computed from `work[0]` (accumulated W̃ at *t*) with compensator `t^{2H}`, and the Volterra increment is added to `work[0]` only after the x/y updates. New test verifies E[x_T] drift shrinks under grid refinement at ρ≠0.
- **M2. Rate noise correlated with the fBM increment, not the innovation** — `cheyette_rough.rs:116-128`. fBM increments are autocorrelated (strongly negative at H=0.1), so the rate driver is not Brownian — breaking the Markov bond-reconstruction formula the pricer relies on (`cheyette_rough_pricer.rs:159-172`). The correct tool (`RiemannLiouvilleVolterra`, two normals/step) exists in `rng/volterra.rs` but is not used here.
  > **FIXED 2026-06-10.** Noise source switched to `RiemannLiouvilleVolterra` (BLP hybrid, 2 normals/step); the discretization layout now carries the driving Brownian increment ΔW alongside ΔỸ, and the rate shock is correlated as `ρ·(ΔW/√dt) + √(1−ρ²)·z_indep` — a genuine Brownian rate driver. Pricer generator wiring and `process/cheyette_rough.rs` module docs updated (also resolves the Minor "Volterra fBM doc vs true-fBM" inconsistency); rate-driver increments verified serially uncorrelated.

#### Credit / Merton

- **M3. Jump-diffusion martingale broken** — `finstack/valuations/src/models/credit/merton.rs:969` vs `:1031,:1051`. The compensator uses `κ = e^{μ_J+σ_J²/2}−1` but the simulated jump multiplier is `exp(μ_J − ½σ_J² + σ_J z)` (mean `e^{μ_J}`) — a spurious Itô correction on the jump. E[V_T] ≠ V₀e^{(r−q)T}; ~1.2% drift bias at λ=0.5, σ_J=10%, T=5y. Fix: `(mu_j + sigma_j * jz).exp()` in both branches plus a JD mean-convergence test.
  > **FIXED 2026-06-10.** Jump multiplier is `exp(μ_J + σ_J·z)` in both simulation branches, consistent with the compensator κ. New JD mean-convergence test: `mean(V_T) ≈ V0·e^{(r−q)T}` within MC tolerance at λ=0.5, σ_J=0.1, T=5.
- **M4. Black-Cox first-passage wrong for growing barriers** — `merton.rs:279-307`. For `barrier_growth_rate ≠ 0` the reflection term uses drift μ+g instead of ν = μ−g and the wrong exponent (`−2μ(x₀−gT)/σ²` instead of `−2νx₀/σ²`). All closed-form regression tests use g=0; the g≠0 test only checks monotonicity, which the wrong formula also satisfies. Fix: reduce to BM with drift ν = μ−g started at x₀ = ln(V/B); add a g≠0 Black-Cox (1976) regression test.
  > **FIXED 2026-06-10.** `FirstPassage` branch rewritten as BM with drift ν = μ − g from x₀ = ln(V/B): `PD = N(−(x₀+νT)/(σ√T)) + e^{−2νx₀/σ²}·N((−x₀+νT)/(σ√T))`, keeping the log-space/NaN guards. Hand-computed g≠0 Black-Cox regression test added; g=0 results verified unchanged.

#### Trees

- **M5. "Black-Karasinski" mean reversion is inoperative** — `finstack/valuations/src/models/trees/short_rate_tree.rs:852-860`. κ only shrinks per-step spacing by `(1−κΔt/2)`; terminal dispersion → `σ√T` regardless of κ as Δt→0 (true BK: `σ√((1−e^{−2κT})/2κ)` — 13% lower at κ=0.03, T=10y). Constant-spacing recombining binomial lattices cannot represent BK's state-dependent drift. The Bloomberg-OAS1-parity doc claims (`:832`, `:346`) are not supportable. Fix: reject κ≠0 (as Ho-Lee does) or implement a real trinomial BK lattice in ln r.
  > **FIXED 2026-06-10.** Real trinomial Black-Karasinski lattice implemented in x = ln r: Hull-White geometry (spacing σ√(3Δt), `j_max` cap with edge branch switching, per-node mean-reverting probabilities — reusing `HullWhiteTree::compute_probabilities`) with the per-step additive x-shift Brent-solved by Arrow-Debreu forward induction. `ShortRateModel::BlackDermanToy` with κ≠0 routes to the new lattice (κ=0 stays binomial BDT); the bogus `(1−e^{−2κΔt})/2κ` spacing tweak was deleted and the Bloomberg-OAS1 doc claims corrected. Tests: terminal log-rate dispersion matches `σ√((1−e^{−2κT})/2κ)` within 2% (and sits materially below σ√T); curve repricing <0.1bp via both state prices and backward induction; κ→0 converges to BDT (dispersion → σ√T within 1%, rate-call price within 5%); all existing callable-bond tree tests pass.
- **M6. Ho-Lee calibration/pricing compounding mismatch** — `short_rate_tree.rs:686,:719,:753` vs `:1245`. Calibration hard-codes continuous discounting while `price()` honors `config.compounding` — a Simple-compounding Ho-Lee tree silently fails to reprice the curve (~10–20bp on 5y ZCB). Currently masked only because `tree_pricer.rs:299-305` accidentally drops `tree_compounding` from the documented `production_ho_lee` preset (`config.rs:375`). Fix: use `comp.df()` throughout `calibrate_ho_lee` (or reject non-Continuous), and make `tree_pricer.rs` pass `tree_compounding` through.
  > **FIXED 2026-06-10.** `calibrate_ho_lee` now uses `comp.df()` throughout (including the error-measurement pass and the extreme-node guard); r₀ inverts the configured convention via a new `TreeCompounding::rate_from_df`, and θ is root-found under non-continuous compounding (closed form retained for Continuous, where it is exact). The Ho-Lee arm of `tree_pricer.rs` passes `tree_compounding` through. Test: Simple/SemiAnnual/Quarterly/Monthly Ho-Lee trees reprice the curve to <0.1bp; `rate_from_df` round-trip test added.

#### Equity exotics / rough vol

- **M7. Rough-Heston CF v₀ term uses D(T) instead of I^{1−α}D(T)** — `rough_heston.rs:334-336,:428-430`. EER (2019) Thm 4.1 requires `v0·I^{1−α}h`; `v0·h(T)` is correct only at α=1. With B1 fixed: 0.3% ATM growing to 2.5% in the wings (H=0.1, σ=0.3). Fix: product-integrate `I^{1−α}D` at T over the existing `d_trajectory`.
  > **FIXED 2026-06-10** (in the B1 slice). New `FractionalRiccatiSolver::fractional_integral_d` evaluates `I^{1−α}D(T)` by product integration (piecewise-linear D, exact `τ^{−α}` kernel moments); `char_func` and the Lewis integrand both use it. Exactness verified on a constant trajectory (`c·T^{1−α}/Γ(2−α)`) and via the tightened H→0.5 classical-Heston comparison.
- **M8. Rough-Heston MC kernel underweights the singular near-field ~41% at H=0.1** — `finstack/monte_carlo/src/discretization/rough_heston.rs:188-199`. Midpoint rule on the last interval vs the exact stochastic weight `√(dt^{2α−1}/(2α−1))` — the naive-Riemann O(n^{−H}) bias class. The "exact (no binning approximation)" doc claim is wrong and `RoughHestonHybrid` is not the BLP hybrid scheme. Fix: exact per-interval kernel integrals (drift) + exact-covariance near-field (noise), or document/rename.
  > **FIXED 2026-06-10.** Drift convolution now uses exact per-interval kernel integrals `∫(t_next−s)^{α−1}ds/Δt` and the last-interval noise term uses the exact stochastic weight `√(Δt^{2α−1}/(2α−1))` (BLP-style near-field); the stored integrand was split into drift and noise components. Docs corrected — `RoughHestonHybrid` is now a genuine hybrid near-field scheme. Tests: E[V_T] vs an independent product-integration solution of the fractional mean ODE; MC ATM price vs the (post-B1/M7) Fourier pricer at H=0.1 (slow suite).

#### PE fund waterfall/metrics

- **M9. Promote-tier hurdles are decorative** — `pe_fund/waterfall.rs:832-855`. The hurdle only appears in the tranche display name; the tier splits all remaining cash, so multi-tier promotes never trigger past tier one and `Hurdle01` measures noise. Fix: gate each promote tier on LP IRR reaching `hurdle` (same solver as PreferredIrr).
  > **FIXED 2026-06-10.** Each `Tranche::PromoteTier` is now gated on LP IRR reaching its `hurdle.rate` via the same incremental Brent solve as the preferred tier (100% to LP until the hurdle is met, then split at `lp_share/gp_share`, cascading to the next tier). `Hurdle01` is now meaningful. The bracketing in `calculate_preferred_amount` was also hardened (explicit `[0, hi]` bracket with doubling upper bound).
- **M10. Preferred-return solve and LP IRR use gross fund events** — `waterfall.rs:894-900,:1016-1029`. Distribution/Proceeds counted at full face value, crediting LPs with GP carry; pref under-allocates and LP IRR is overstated after any carry. Fix: build LP history from the ledger `to_lp` rows (as `lp_cashflows()` does).
  > **FIXED 2026-06-10.** `calculate_preferred_amount` and `calculate_lp_irr_to_date` build LP history from in-progress ledger `to_lp` rows plus contributions (the `lp_cashflows()` shape), threaded through the allocation loop, instead of gross `all_events`.
- **M11. `GpIrr` returns total carry in dollars** — `pe_fund/metrics.rs:72-76`. Registered as an IRR; consumers reading a rate get garbage. Fix: compute an actual GP IRR or rename `GpCarryTotal`.
  > **FIXED 2026-06-10.** Renamed to what it computes: `MetricId::GpCarryTotal` / `GpCarryTotalCalculator`; `GpIrr` registration dropped and the `pricing_fundamentals.ipynb` reference updated.
- **M12. `MoicLp` includes GP carry** — `metrics.rs:97-114`. Use ledger `to_lp` as `DpiLpCalculator` correctly does (`:136-141`).
  > **FIXED 2026-06-10.** `MoicLpCalculator` sums ledger `to_lp` rows with `date ≤ as_of` (the `DpiLpCalculator` pattern), excluding GP carry.
- **M13. Fund PV discounts the entire historical flow set; metrics double-count it** — `pe_fund/pricer.rs:31-39` + `core/src/cashflow/discounting.rs:252-267`. No `d <= as_of` holder-view filter, so `base_value` is lifetime NPV, then `LpIrr`/`TvpiLp` re-add it as terminal "Residual NAV". A fully-realized fund won't show residual ≈ 0. Fix: holder-view filter for PV; model unrealized NAV explicitly.
  > **FIXED 2026-06-10.** Holder-view PV: `compute_pv` filters LP flows to `d > as_of` and adds a new explicit `unrealized_nav: Option<Money>` input on `PrivateMarketsFund` (builder + serde + schema), stated as of the valuation date. Tests: fully-realized fund prices to ≈0, NAV adds to PV, currency mismatch errors.

#### DCF / Real estate

- **M14. Silent discounting-regime switch to risk-free curve** — `dcf_equity/pricer.rs:48-72`, `real_estate/pricer.rs:38-52`. If the named OIS curve is loaded, risky FCF/NOI and terminal value discount at risk-free instead of WACC / property rate — same instrument, wildly different PVs depending on MarketContext contents, no spread adjustment, no policy stamp. Fix: curve + calibrated spread (matching WACC basis at base curve), or compute DV01 by bumping a risk-free component inside WACC.
  > **FIXED 2026-06-10.** The curve-discounting branch was removed from both pricers: explicit flows and terminal value always discount at WACC (DCF) / property `discount_rate` (RE); `discount_curve_id` is documented as risk-attribution-only. Rate sensitivity reimplemented via a new `RfComponentPriced` trait + `RfComponentDv01Calculator` (parallel and bucketed) that bumps the risk-free component *inside* the rate. Tests: PV invariant to whether the OIS curve is loaded; Dv01 matches the analytic ∂PV/∂rf; bucketed DV01 sums to parallel.

#### Commodity

- **M15. No realized-fixings mechanism on `CommoditySwap`** — `commodity_swap/types.rs:327-341`. Completed averaging periods mark at *today's spot*; the live averaging period propagates a hard error by design. Unusable for daily seasoned marking. Fix: add a realized-fixings store mirroring `CommodityAsianOption.realized_fixings`.
  > **FIXED 2026-06-10.** `realized_fixings: Vec<(Date, f64)>` added to `CommoditySwap` (builder + serde + schema; not bound in Python/WASM). In `expected_period_price`, observation dates strictly before `as_of` read from the store with `Error::Validation` naming any missing date (W-11 no-silent-substitution policy, replacing the `obs_end <= curve_base → spot` branch); duplicates are rejected; dates ≥ as_of keep curve projection. Tests: seasoned swap marks to ~0 on flat fixings/curve; missing past fixing errors with the date; duplicate fixing errors.
- **M16. Geometric Asian drops the Kemna-Vorst drift correction** — `commodity_asian_option/pricer.rs:187-221` (seasoned variant `:288-300`). Forward for E[G] overstated by `exp((σ²/2)(Σt/m − ΣΣmin/m²))` ≈ +0.75% for monthly/1y/σ=30% — several % of an ATM premium. The equity seasoned-geometric control variate (`exotics/asian_option/pricer.rs:213-258`) does it correctly; the commodity test validates against the same wrong law. Fix: Black-76 forward = `geo_mean·exp(v/2 − (σ²/2m)Σt_i)`.
  > **FIXED 2026-06-10.** Kemna-Vorst drift carried in both `price_geometric_kv_commodity` and `price_seasoned_geometric_commodity`: the Black-76 forward is `F_G = geo_mean·exp(½σ_G² − (σ²/2m)Σt_i)` (and the seasoned `ln G_fut` mean carries the same correction before forming E[X]). Test expectations rebuilt from the corrected law plus an MC cross-check.
- **M17. Missing past fixings silently distort the effective strike** — `commodity_asian_option/types.rs:158-181` + `pricer.rs:345-374`. No completeness check `hist_count + future_count == total_fixings`; a missing/date-mismatched fixing inflates `k_eff` silently. Fix: error when any fixing date ≤ as_of lacks a realized value; reject duplicates.
  > **FIXED 2026-06-10.** Completeness validated on the pricer entry path: every fixing date ≤ as_of must have a realized fixing (`Error::Validation` naming the missing date), duplicate fixing dates are rejected, and `hist_count + future_count == fixing_dates.len()` is asserted before `k_eff`. Tests: missing past fixing errors; duplicate errors; complete seasoned option unchanged.

### Moderate

- **Credit bootstrap approximate-knot gate bypass** — `calibration/solver/bootstrap.rs:538-539` + `helpers.rs:197-277`. When the no-bracket secant fallback converges below *solver* tolerance, the knot commits with only a metadata flag — `target.allow_approximate_knots()` is never consulted on the `Some` path. Identical economics get opposite outcomes depending on tolerance ordering. Fix: route through the same gate as `resolve_no_bracket`.
- **HW1F default-param fallback warn-only** — `exotics_shared/hw1f_calibration.rs:283-291`. Uncalibrated κ=3%/σ=1% defaults proceed with only `tracing::warn!`; no stamp in result metadata. Partial overrides (one of κ/σ) silently discarded (`:239,:271`). Fix: error or stamp `hw1f_param_source`; reject partial overrides.
- **Vol-surface axis/quote-type convention implicit and inconsistent** — `hw1f_calibration.rs:197-214` reads (expiry × tenor, normal); LMM (`lmm_pricer.rs:258-261`) and Cheyette (`cheyette_rough_pricer.rs:322-332`) read the same `vol_surface_id` channel as (expiry × strike, Black). `VolSurface` has no quote-type metadata. Fix: document at minimum; ideally tag surfaces.
- **LSMC rollback only correct for bullet-cashflow products** — `hw1f_lsmc.rs:205,:264,:299-301`. Products paying coupons before exercise dates get biased call decisions and dropped pre-call coupons. Fine for the sole current consumer (callable range accrual); undocumented for the advertised PRDC/snowball use (`bermudan_call.rs:5-9`). Fix: document the contract restriction or regress only post-exercise cashflows.
- **LMM first-alive forward selection has no tolerance** — `lmm_bermudan.rs:241,:519`. `partition_point(tenor < t_exercise)` vs day-count year fractions: 1e-12 noise silently drops the first period from intrinsic and numeraire. Fix: snap to nearest tenor within ~1e-8.
- **LMM base-vol calibration inconsistencies** — `lmm_pricer.rs:145-151,:253-261`. ATM lookup uses a single-period forward, not the co-terminal swap rate; displaced-diffusion mapping omits the ≈S/(S+d) rescaling exactly in the low-rate regime the shift targets; silent fallback chain (raw surface vol → 12%).
- **Cheyette grid snapping with silent corruption on collision** — `cheyette_rough_pricer.rs:357-386,:461-473`. 100 uniform steps move exercise dates up to ~12 days; the event match is `if` not `while`, so two dates on one node silently record end-of-path state as exercise data. Fix: exercise-aligned grid + strictly-increasing index validation.
- **Cheyette η convention mismatch** — `monte_carlo/src/process/cheyette_rough.rs:16,:29`. σ-lognormal (not variance-lognormal) compensation: with the rBergomi-scale default η=1.5, E[σ²] ≈ 22× σ₀² at 5y. Fix: compensate variance (rBergomi semantics) or rescale/document η.
- **Commodity swaption forward at payment date vs period average** — `commodity_swaption/types.rs:261-267` vs `commodity_swap/types.rs:343-366`. The underlying floats on the business-day average; sampling F(payment_date) moves the swaption ~half a period of carry per period on sloped curves.
- **Commodity swaption silent spot fallback** — `types.rs:265-267`. `unwrap_or_else(spot)` when the curve doesn't cover a payment date — the same failure mode made a hard error in the swap. Propagate the error.
- **Commodity swap averaging windows double-count boundary dates** — `commodity_swap/types.rs:243-264,:351-357`. Both-ends-inclusive loops observe each payment date in two adjacent periods. Use half-open windows.
- **Vanna-volga base leg at strike vol, correction at ATM vol** — `fx_barrier_option/pricer.rs:618-638` + `vanna_volga.rs:281-285`. Castagna-Mercurio requires the base BS leg at σ_ATM; non-flat ambient surface double-counts the smile. Fix: rebuild the BS barrier leg at `quotes.vol_atm` inside the VV path.
- **Vanna-volga symmetric KO/KI survival weighting breaks in–out parity** — `vanna_volga.rs:58-83,:376-382`. `VV(KO)+VV(KI) ≠ VV(vanilla)`. Market practice (Wystup 2006): attenuate KO only, price KI via parity.
- **Vol-index option expiry-edge delta** — `vol_index_option/pricer.rs:126-133`. At t≤0 returns +1 for any ITM option (puts should be −1) and omits multiplier/contracts/df scaling, unlike the t>0 branch. Feeds the registered Delta metric.
- **Heston Gil-Pelaez underflow misclassified as corruption** — `closed_form/heston.rs:459-461,:939` (threshold `:420`). Legitimate exp-underflow of ψ (Re < −745) is indistinguishable from the overflow sentinel `Complex::ZERO`; >5% underflowed nodes silently swaps to BS — from T≈15y at κθ=0.27, σ_v=0.2. Fix: distinguish overflow from underflow via an enum/flag from the CF.
- **Heston BS fallbacks use √v₀, ignoring mean reversion** — `heston.rs:540-548` et al. Correct deterministic limit is BS at `v̄(T) = θ + (v0−θ)(1−e^{−κT})/(κT)` — 23.5% vs 10% in a v₀=0.01/θ=0.09/κ=2/T=1 example. One-line fix.
- **HW tree uniform dt grid with nearest-step rounding** — `hull_white_tree.rs:861-868`. Exercise/coupon dates land up to dt/2 off (±18 days at 100 steps/10y); no mechanism to align the grid with mandatory dates. Fix: construct the grid through mandatory dates (probabilities already support per-step dt).
- **QE-Heston "martingale-corrected QE-M" label wrong** — `monte_carlo/src/discretization/qe_heston.rs:234-248`. Scheme is faithful plain Andersen (2008) with γ1=γ2=½; the K0* correction (Andersen §4.2) is absent. Fix docs or implement K0*.
- **rBergomi silent default parameters** — `rough_bergomi_mc_pricer.rs:223-225`. η=1.9/H=0.07/ρ=−0.9 via `get_unitless_scalar` when scalars missing, while Heston/rough-Heston MC were deliberately converted to strict resolvers. Fix: strict resolver for `RBERGOMI_*`.
- **rBergomi flat ξ₀ from strike-specific implied vol** — `rough_bergomi_mc_pricer.rs:230-233` + `equity_option/pricer.rs:305-311`. Double-counts the smile; should use ATM/variance-swap term structure (`ForwardVarianceCurve` exists).
- **DCF mid-year convention shifts ExitMultiple TV to t−0.5** — `dcf_equity/pricer.rs:59-65` + `types.rs:685-692`. Point-in-time exit TV should discount at full t_n; ~5% of TV overvalued at 10% WACC.
- **DCF curve queried with hardcoded ACT/365.25** — `dcf_equity/types.rs:676-692` + `pricer.rs:53-61`. Violates the workspace two-clock principle; ~0.4-0.5% PV error on a 10y flow with an ACT/360 curve.
- **PE clawback assumes full-catch-up economics** — `pe_fund/waterfall.rs:1044-1052,:1068-1075`. With partial/no catch-up, settlement releases too much to the GP.
- **Rough-Heston test suite structurally unable to catch B1/M7** — `rough_heston.rs:459,:642-660,:701-735`. Put defined via parity (circular), 15% tolerance at the point of smallest error, no golden fixture at rough H. Fix: tight (≤0.1%) classical-Heston parity at H→0.5 across moneyness + a published rough-H golden value.
  > **ADDRESSED 2026-06-10** with the B1/M7 fix: σ→0 Black-Scholes golden values for calls and puts across moneyness at rough H=0.1 (analytic, non-circular), martingale checks at the Riccati and CF level, and classical-Heston agreement at H=0.499 across five strikes at 0.5% tolerance. A published rough-H (finite-σ) golden fixture remains a worthwhile future addition.

### Minor

- Antithetic legs treated as i.i.d. in stderr (`hw1f_mc.rs:149-157`, `hw1f_lsmc.rs:322-333`, `lmm_bermudan.rs:372-380`) — pairs should average into one sample.
- In-sample LSMC without OOS option in LMM/Cheyette (HW1F has `oos_lsmc`); inconsistent regression-failure policy (silent skip vs propagate).
- Dead `call_prices`/`notional` config fields on `RateExoticHw1fLsmcPricer` (`hw1f_lsmc.rs:79-86,:365-371`) — can diverge from the payoff's own values.
- Scattered silent numeric fallbacks without warns: forward→3% (`lmm_pricer.rs:127-131`, `cheyette_rough_pricer.rs:123-130`), base vol→0.5% (`:332`), 12% vol fallback (`lmm_pricer.rs:290-293`).
- θ sampled at step start across θ-knot boundaries in event-aligned HW1F grids (`exact_hw1f.rs:71-76` + `hw1f_mc.rs:195-218`) — O(dt) local bias.
- `lmm_bermudan.rs:147` doc claims fixing-date grid alignment that isn't implemented.
- Cheyette "Volterra fBM" doc vs true-fBM implementation (`process/cheyette_rough.rs:7,:24`) — different autocovariance.
- `implied_spread` accepts horizon ≤ 0 → NaN; recovery unbounded (`merton.rs:323-327`, `:550-552`).
- `simulate_paths`: `num_steps = 0` → NaN grid; CreditGrades paths simulate plain GBM without the stochastic barrier (`merton.rs:946,:958-961`) — undocumented.
- `HazardCurveParams::interpolation` silently ignored (`calibration/targets/hazard.rs:484-498`).
- Bootstrap docs describe Brent+Newton; code is scan/bisection/false-position (`bootstrap.rs:341-345`).
- Genuinely converged roots falsely flagged `approximate_knots` (`helpers.rs:143-147,:175-181`).
- Asymmetric FD fallback reports zero sensitivity on up-bump failure (`bootstrap.rs:614-617`).
- Initial guess evaluated twice per knot (~1 full CDS repricing wasted per pillar).
- `piecewise_gbm` interval lookup off-by-one at exact boundaries (`piecewise_gbm.rs:42-46`) — `<` should be `<=`.
- Vol-index future Dv01 metric identically zero yet `discount_curve_id` required (`vol_index_future/pricer.rs:17-29`).
- Vol-index docs overstate the no-convexity claim (true only for directly-quoted vol-level curves) and misstate VIX settlement mechanics (`vol_index_future/types.rs:31-32,:99`).
- VV fixed FD bumps can cross the barrier or push vol negative; rebate leg excluded from matched greeks (`vanna_volga.rs:171-235`).
- Commodity `index_lag_days` in calendar days; `bdc` inert without `calendar_id` (`commodity_swap/types.rs:304-306,:374-385`).
- HW trinomial doc comments have wrong sign convention vs (correct) code; "0.184 margin" rationale wrong (`hull_white_tree.rs:230-240,:331-333`).
- `backward_induction` doesn't validate `terminal_values.len()` (`hull_white_tree.rs:804-811`).
- `bond_price` silently substitutes zero rate for forward rate on `instantaneous_forward` failure (`hull_white_tree.rs:677-682`).
- Heston `u_max` chosen from T only; real truncation risk for small v₀·T (`heston.rs:350-388`).
- Terminal-row tree rates carry no final-step drift calibration (documented for Ho-Lee, not BDT).
- Default tree Greeks vol bump 0.01 is a 100% relative bump at default σ=1% (`short_rate_tree.rs:1310-1311`).
- rBergomi pricer reports no `mc_stderr` (`rough_bergomi_mc_pricer.rs:103-135,:327`).
- PE IRR plumbing: `current_irr` errors → 0.0, day-count failures → t=0 silently, INFINITY-hostile Brent objective, no multiple-root guard.
- `dcf_equity/metrics/mod.rs:133` doc claims Theta is registered; it is not.
- pe_fund "01" metric docstrings promise per-1bp/per-1% but return raw derivatives (consistent with workspace dPV/dy convention; comments misleading).

---

## Open Questions (Part 1)

1. **DCF/RE risk-free discounting (M14):** intentional "rates-sensitivity mode" or a defect? If intentional it needs a policy stamp in result metadata; if not, it is Blocker-class whenever the curve loads.
   > **RESOLVED 2026-06-10:** a defect. Discounting always uses WACC / property discount rate; rate DV01 bumps the risk-free component inside the rate (see M14).
2. **Commodity swaption notional semantics (B3):** per-period quantity (as documented) or annualized rate? Decides the fix direction.
   > **RESOLVED 2026-06-10:** per-period quantity, as documented. Annuity and forward-swap-rate weights changed to `Σ DF` / `DF_i` accordingly (see B3).
3. **PE fund PV semantics (M13):** residual position value (needs holder-view filter + explicit NAV input) or lifetime NPV (then metrics must stop re-adding residual NAV)?
   > **RESOLVED 2026-06-10:** residual position value (holder view). Implemented with `d > as_of` filter + explicit `unrealized_nav` input (see M13).
4. **Cheyette-rough:** research prototype indefinitely (registry refusal is the only guard), or fix the discretization via the existing `RiemannLiouvilleVolterra`? Decide the η convention (variance vs σ lognormal) at the same time.
   > **PARTIALLY RESOLVED 2026-06-10:** the discretization was fixed via `RiemannLiouvilleVolterra` (M1+M2). The η convention question (Moderate finding) remains open.
5. **Heston Fourier proliferation:** three implementations (`closed_form/heston.rs` Gil-Pelaez, `models/volatility/heston.rs`, COS infra without a Heston CF), cross-validated at one parameter point only. Consolidation planned? Is a `HestonCf` for the COS path planned?
6. **Bloomberg BK parity (M5):** if OAS1 parity is a requirement, the BDT-κ variance tweak cannot deliver it — a real BK lattice is needed.
   > **RESOLVED 2026-06-10:** a real trinomial BK lattice in ln r was implemented; κ≠0 routes to it (see M5).
7. **Vol-surface typing:** project-wide convention for which `vol_surface_id`s are (expiry × tenor, normal) vs (expiry × strike, Black)?
8. **`implied_spread` convention:** reduced-form with exogenous recovery (current, documented) vs canonical Merton endogenous-recovery spread — users comparing to textbook values will differ.
9. **MC step defaults for rough models:** 100 steps with O(n^{−H}) convergence at H≈0.07–0.1 — has a step-refinement study validated this? The construction-time warning triggers only *above* 200 steps, backwards relative to the accuracy risk.

## Brief Summary (Part 1)

The reviewed surface splits sharply. The mature paths are genuinely strong: LMM terminal-measure drift/deflator algebra, HW1F θ-fit and bank-account discipline, the credit bootstrap's no-silent-skip design, QE-Heston variance stepping, the rBergomi noise machinery (BLP hybrid, exact near-field), the COS engine, ISDA-style CDS accrual-on-default, and Philox determinism all verified cleanly. Defects cluster in (a) the newest research-grade pricers — rough-Heston Fourier is unusable as written, rough-vol discretizations carry refinement-divergent bias, Cheyette-rough has measure-level problems; (b) instruments whose tests cannot see the bug — HW tree's flat-curve 1bp tolerance, rBergomi's r=0 tests, the geometric Asian validating against its own wrong law, rough-Heston's circular parity test; and (c) the cash-economics layer (PE waterfall, commodity seasoning) where conventions, not stochastics, are wrong.

**Highest-leverage process fix:** external golden fixtures for every Fourier/rough pricer — four of the five worst findings survived only because all tests were self-referential.

---

# Part 2 — Second-pass findings

No Blockers in this pass. Several Majors are blocker-adjacent depending on configuration reachability (noted inline).

## Major

### Rates metrics

- **M2.1. Inflation cap/floor vega 100× overstated** — `inflation_cap_floor/metrics/vega.rs:50` (verified). Returns `(pv_up − pv_down)/(2·bump)` = per **unit** absolute vol, while every other vega in the workspace is per **vol point** (swaption `/VOL_PCT_SCALE` at `swaption/metrics/vega.rs:90`, nominal cap/floor, CMS, fd_greeks). Its own registry doc claims "per 1% shift". Cross-asset vega aggregation/hedging wrong. Fix: divide by `VOL_POINTS_PER_ABSOLUTE_VOL` (equivalently `(pv_up−pv_down)/2`).
- **M2.2. Swaption implied vol fabricates a bound endpoint on solver failure** — `swaption/metrics/implied_vol.rs:88-102`. On Brent failure it silently returns 1e-6 or 3.0 ("pick the closer"). Guaranteed to trigger for Normal-model swaptions with non-positive forward/strike (the negative-rate guard only covers the Black branch) and whenever target PV < discounted intrinsic. Risk systems receive a fabricated 0.0001%/300% vol indistinguishable from a real solution. Fix: return `Err` on solver failure or `|f(root)| > tol`.

### Credit copulas / tranche metrics

- **M2.3. Student-t semi-analytic tranche engine prices a different model than documented** — `correlation/copula/student_t.rs:383-405` (verified). `integrate_fn` collapses (Z, W) to the single t-variate `m = z/√w` and the pricer treats names as conditionally independent given M alone. True shared-W t-copula conditional independence holds only given (Z, W) — the code documents this itself for the MC path ("sigma-algebra mismatch") and implements the correct (Z,W) conditional at `:343-381`. Quadrature engine understates joint-default clustering → senior EL biased low, equity high. One `CopulaSpec::StudentT` now means three claims/two models across quadrature, per-name MC, and docs; `tail_dependence()` reports λ of the model NOT priced; df calibrated through this engine is an effective parameter of the wrong model when reused in MC. Fix: `num_factors() = 2`, pass `[z, w]`, dispatch the 2-factor conditional already implemented.
- **M2.4. Trait-default mixing conditional breaks every multi-slot copula on the LHP fast path** — `correlation/copula/mod.rs:141-150`. Default forwards `&[systematic]` (len 1) to `conditional_default_prob`. Via the production-reachable `PerNameCopulaDefault` path: `MultiFactorCopula` → **debug panic / release prices with zero effective correlation** (`multi_factor.rs:423-437`); `RandomFactorLoadingCopula` → debug panic / release effective correlation ρ−σ²_β biased low plus a per-call warn flood (no once-guard, `random_factor_loading.rs:288-305`). Fix: override per copula or reject multi-slot copulas in `PerNameCopulaDefault::new`.
- **M2.5. RFL finite-pool MC silently degenerates to a plain Gaussian copula** — `random_factor_loading.rs` has no `latent_variable`/`sample_mixing` overrides, so `simulate_period` never draws the loading shock η. One `CopulaSpec::RandomFactorLoading` yields three different effective models (quadrature = stochastic loading; per-name MC = Gaussian(ρ); LHP = Gaussian(ρ−σ²_β)), breaking the documented per-name ↔ LHP convergence contract. Fix: implement `sample_mixing` (η shared per period) + `latent_variable = β(η)Z + √(1−β(η)²)ε`, or fail fast on RFL.

### Fixed income / schedules

- **M2.6. `StubKind::LongFront` never produces a long stub** — `core/src/dates/schedule_gen.rs:201-221` (verified). Output is identical to ShortFront: the lowest anchor is kept instead of merged. The pinning test (`core/tests/dates/schedule.rs:202-225`) codifies the wrong behavior — its comment calls a 1-month period on a quarterly schedule "long". Every LongFront consumer (revolver `stub`, bonds) silently gets short-front accrual. Fix: skip `a_min` when `a_min > start`; update the test.
- **M2.7. Roll-day drift through short months in schedule generation** — `schedule_gen.rs:131-145,154-160,172-179`. Each next date is computed from the previous *clamped* date, not the anchor: backward semi-annual Aug 31 → Feb 28 → **Aug 28**; forward monthly Jan 31 → Feb 28 → **Mar 28** → … (eom=false). Also makes `StubKind::None` spuriously error for valid aligned schedules (Jan 31 → Jul 31 monthly). Payment/accrual dates wrong by 1–3 days per period for 29/30/31 anchors — matches no market convention. Fix: generate anchors as `anchor + i·tenor` from the fixed anchor (QuantLib-style), clamping independently.
- **M2.8. Revolver commitment-date draw event double-counts principal** — `revolving_credit/cashflow_engine.rs:206-222` vs `:266-280,:532-552`. The dedup that supports encoding the initial draw as a commitment-date event removes the duplicated outflow but the period replay and terminal balance still add the event on top of `drawn_amount`: outflow −X, interest accrues on 2X, terminal repayment +2X. No test covers a commitment-date event. Fix: one canonical semantic (reject events dated on commitment_date, or make period 0 replay match periods i>0 and drop the dedup).
- **M2.9. Revolver Sobol QMC mode is statistically invalid** — `revolving_credit/pricer/path_generator.rs:173-236`. A 3-dimensional Sobol sequence is consumed once per time step per path, violating the documented contract (`monte_carlo/src/rng/sobol.rs:3-7`: dimension must be `num_steps × num_factors`). Consecutive Sobol coordinates are van-der-Corput anti-correlated → biased path dynamics, not just inefficiency. Fix: proper dimensioning or reject `use_sobol_qmc`.
- **M2.10. Revolver survival weighting not conditioned on survival to as_of** — `revolving_credit/pricer/unified.rs:126-157,:473-528`. Both static and dynamic branches use unconditional survival from commitment/curve-base; seasoned facilities understate PV by the factor S(→as_of) (~4% for 200bp spread, 2y seasoned). The bond hazard engine establishes the correct in-repo convention (`bond/pricing/engine/hazard.rs:226-250`). Fix: divide weights and recovery-leg ΔSP by S(as_of).
- **M2.11. Amortizing bonds silently priced as bullets by MertonMc** — `bond/types/traits.rs:197-204`. `CashflowSpec::Amortizing` extracts only base coupon rate/frequency; the schedule is discarded and the engine simulates constant notional with full redemption at maturity, no error. Fix: reject Amortizing in `price_merton_mc` or model the notional schedule.
- **M2.12. Merton MC synthetic coupon grid: horizon rounding + seasoned timing errors** — `merton_mc.rs:467,:475-478,:656-657,:739,:934-959`. (a) `num_coupons = round(maturity·freq)` can place the last coupon beyond the simulated horizon — included in the risk-free PV but unreachable in MC → expected_loss overstated for stub maturities; (b) step rounding lets first-passage defaults trigger after maturity; (c) coupons anchored at `i/freq` from as_of instead of contractual dates → stub coupon dropped, clean ≡ dirty with zero accrued. Fix: `dt = maturity/ceil(maturity·steps_per_year)`, floor+stub coupon count, pass actual schedule times.
- *(Also independently re-found the Black-Cox growing-barrier defect — same root cause as Part 1 M4, two agents agree on the fix: drift ν = μ−g with x₀ = ln(V/B).)*

### Securitized

- **M2.13. Systematic-factor sign conventions inconsistent across default models, recovery presets, and tree-vs-MC** — copula stress is **Z<0** (`copula/gaussian.rs:207-210`) while intensity/hazard/factor-correlated and tree-mode stress is **Z>0** (positive β presets, `intensity_process.rs:138-140`, `tree/tree.rs:347-378`). MC mode shares one monthly z across default/prepay/recovery: the shipped configs pair the copula default spec with recovery `factor_correlation = −0.50/−0.40`, so defaults and recoveries co-move **positively** — inverting the Altman PD-LGD relationship (cited in the repo's own docs) and understating stress-path loss ~3× (13.5% PD × 30% LGD instead of × 90%). `loss_factor_correlation` flips sign between tree and MC pricing of the same config. Fix: pick one canonical convention, flip β/ρ_R signs accordingly, add a cross-model sign test (corr(defaults, recovery) < 0 per path).
- **M2.14. Two of three SDA implementations omit the 30–60 plateau and end the decline at month 60 instead of 120** — `metrics/pool/characteristics.rs:166-185` and `types/mod.rs:820-836` (verified; the latter feeds actual cashflow projection via `sda_speed_multiplier`). Month 45: 0.315% vs standard 0.60%; month 90: 0.03% vs 0.315% (~10× low). The third implementation (`stochastic/default/copula_based.rs:85-116`) is correct — the codebase disagrees with itself. Fix: replicate the copula_based shape or centralize.
- **M2.15. Student-t dof ≤ 2 silently falls back to Gaussian while thresholds keep the t-quantile** — `copula_based.rs:194-219`. A config with dof = 2.0 deserializes, warns, prices Gaussian-conditionally against t₂⁻¹(PD) ≈ −4.85 thresholds → conditional PDs collapse by orders of magnitude, silently. The per_name path correctly propagates the error — the two paths disagree. Fix: make `CopulaBasedDefault::new` fallible.
- **M2.16. Systematic factor has zero temporal persistence; `mean_reversion` parameters are dead** — fresh i.i.d. monthly normals (`pricer/engine.rs:224-310,:592-639`); `LatentFactorSpec::mean_reversion` and `IntensityProcessDefault.mean_reversion` never consumed (grep-verified) despite documented OU dynamics. Annual-calibrated copula ρ applied to i.i.d. monthly factors time-diversifies systematic risk (~ρ/12 dilution) — thin loss tails for mezz/senior tranches. Fix: AR(1)/OU evolution using the configured parameter; document the effective correlation horizon.

### Hull-White calibration

- **M2.17. `hw1f_convexity_adjustment` is mathematically wrong vs the Hull reference it cites** — `calibration/hull_white.rs:1594-1598` (verified). Code: `½σ²B(0,T₁)B(T₁,T₂)`. κ→0 limit gives ½σ²T₁(T₂−T₁) instead of Hull's ½σ²T₁T₂ — missing the ½σ²T₁² term. At T₁=5y (κ=0.03, σ=0.01): ~0.58bp vs correct ~11.3bp, ~20× understatement. Currently latent (curve targets consume a pre-computed adjustment; no internal callers) but public and documented as Hull's formula. Fix: full HW futures-forward CA with κ→0 Taylor branch + Ho-Lee-limit test.
- **M2.18. Cap/floor calibration uses payment time as option expiry and includes the first caplet** — `hull_white.rs:1379,:1437,:1441,:1506-1519`. Both market and model legs accrue vol to `t_end` instead of the fixing time `t_start`, and the spot-start caplet (fixing known) is priced with full time value. The error partially cancels between legs but biases calibrated σ ~1–3% (worse for short/quarterly caps), and the σ lands in convention-correct consumers (trees/MC). The swaption path is correct. Fix: expiry = `t_start` on both legs; exclude/intrinsic the t_start=0 caplet.
- **M2.19. `forward_rate_from_df` silently absorbs NaN discount factors, defeating the non-finite-price error contract** — `hull_white.rs:1521-1526`. `df.max(1e-12)` ignores NaN (f64::max semantics) → forward = 0 or ≈ −1/τ, all finite, so the `fill_penalty` protection never fires; `step_runtime.rs:385-389` deliberately maps curve errors to NaN expecting propagation. The file documents this exact NaN-absorbing-max hazard for prices 500 lines later. Fix: validate `is_finite() && > 0`, propagate NaN/error.

### Bindings

- **M2.20. Rust panic reaches Python as `PanicException`** — `finstack-py/src/bindings/valuations/correlation/mod.rs:647-650`. `generate_correlated_factors` forwards to a Rust `assert_eq!` on input length; a wrong-length list from Python raises `pyo3_runtime.PanicException` (a `BaseException` — escapes `except Exception`). Fix: length-check in the binding → `ValueError`.
- **M2.21. `index.d.ts` declares `number[]` where `Float64Array` is returned** — `finstack-wasm/index.d.ts:760,:761,:777` (`correlationBounds`, `jointProbabilities`, `nearestCorrelation`). TS users calling `.push()` type-check clean and throw at runtime; `dts_contract.rs` doesn't pin this namespace. Fix: declare `Float64Array`.
- **M2.22. Python class `LatentFactor` renames Rust `LatentFactorKind`** — `bindings/valuations/correlation/mod.rs:395-403`. Violates the Rust-canonical naming rule (concrete enum, not a trait-object wrapper); no documented exception, and the namespace has no symbol pinning in `parity_contract.toml` to record one. Fix: rename or record an explicit contract exception.

## Moderate

- **Inflation discounting without curve rebasing**: YoY swap `npv_raw` and `InflationCapFloor::npv_with_model` use `disc.df(yf(as_of, pay))` assuming curve base == as_of; the ZC swap/repo/IRS/swaption paths all use relative-DF helpers (`inflation_swap/types.rs:727-731`, `inflation_cap_floor/types.rs:412-415`). All their metrics inherit the mis-discounting.
- **Inflation cap/floor CPI fallback ignores the curve's anchor convention** (`inflation_cap_floor/types.rs:276-284`): always `Act365F(as_of → lagged)` against `curve.cpi(t)`, vs the inflation swap's anchor-aware branch — wrong abscissa for epoch-anchored or rebased curves.
- **Realized CPI fixings consulted without a `≤ as_of` gate** in cap/floor and YoY paths (`types.rs:263-268`, `inflation_swap/types.rs:640-644`) — look-ahead bias if the fixing series extends past as_of; also zeroes those periods' Inflation01.
- **Inflation convexity/gamma divide Money-rounded PVs by h² = 1e-8** (`inflation_convexity.rs:49-78`, `gamma.rs:40-69`) — up to ±2e6 rounding noise; IRS analogue correctly uses `value_raw`. Unit docs also claim per-bp² while code yields per-unit² (1e8×).
- **Repo `CollateralPrice01` is identically zero** (`repo/metrics/collateral_price01.rs`) — collateral price never enters `Repo::pv`; docs claim a real sensitivity (Haircut01 has the honest limitation header). Units comment also wrong.
- **Repo `FundingRisk` returns −ΔPV** (`funding_risk.rs:23`), contradicting its own doc and the workspace dPV/dy convention used by Dv01 on the same instrument; test locks in the inverted sign.
- **Bermudan swaption gamma: ±1bp central difference on a 50-step recalibrated HW tree** (`bermudan_greeks.rs:385-412`) — exercise-boundary/discretization noise divided by 1e-8; use ≥10bp and/or freeze the grid. Related: `expected_exercise_time` drops surviving probability mass (E[τ·1{exercised}], biased toward 0 for OTM; `:454-456`), and Bermudan "Vega" is per HW short-rate σ under the same `MetricId::Vega` as Black-vol European vega.
- **TailDependence tranche metric reads a default-constructed pricer** (`cds_tranche/metrics/tail_dependence.rs:60-61`) — always `CopulaSpec::Gaussian` → constant 0.0 through the registry; RFL arm uses a third inconsistent heuristic (`mean_loading = √ρ` vs the copula's β̄ = √(ρ−σ²_β) vs the trait's NaN contract). Delegate to `copula_spec.build().tail_dependence()`.
- **`smooth_correlation_boundary` is discontinuous at its own seams** (`cds_tranche/pricer/sensitivities.rs:19-38`): 2.5e-3 jump at the upper seam (25% of the correlation01 bump) — kinks exactly where smoothing was intended; hazard for base-correlation bootstrapping near bounds.
- **recovery01 per-leg silent recalibration fallback** (`cds_tranche/metrics/recovery01.rs:90-99`): if one bump leg's re-bootstrap fails, the central difference mixes a frozen-curve PV with a recalibrated PV — neither partial nor full sensitivity, no log. Detect asymmetry, recompute both legs frozen, warn.
- **RFL mislabeled Andersen-Sidenius** (`random_factor_loading.rs:1-27`): β = β̄ + σ_β·η with η ⊥ Z is a stochastic-correlation mixture (Burtschell-Gregory-Laurent), not AS state-dependent a(Z); the "higher correlation in stress" doc claim is not a property of the implemented model (internally consistent otherwise — no m/v adjustment needed, verified).
- **Revolver MC stack** (`pricer/`): zero utilization vol freezes all three factors including rates/spreads (`path_generator.rs:81-83`); deterministic-forward overwrite reads the curve on path time with negative time offsets clamped to 0 (`monte_carlo_process.rs:173-176`) — wrong forwards whenever commitment ≠ curve base; `McConfig.util_credit_corr` documented but never consumed; draw/repay limit validation skipped for boundary-dated events (balance can exceed commitment → negative fees); antithetic SE treated as i.i.d. (both this and Merton MC); seasoned facilities simulate from commitment with today's utilization as the t=0 state (overstated dispersion); paths discounted on the static curve (no pathwise numeraire) — rate correlations can't affect PV through discounting.
- **Merton MC calibration returns Ok on non-convergence** (`merton_mc.rs:1262-1295`) — last bisection midpoint with whatever residual; `price_raw_dyn` discards the stamped residual. Also `solve_effective_spread` solves on a flat-rate basis against a term-structure PV (`:977-1015`) — curve shape leaks into the spread, zero-floor can bind spuriously.
- **Schedule post-adjustment dedup can silently drop the maturity date** (`schedule_iter.rs:981-989`) when two adjacent dates adjust to the same business day (Preceding/MF month-end collisions).
- **Securitized exp-shocks missing the −½β²σ² compensator** (`intensity_process.rs:138-140`, `hazard_curve_adapter.rs:145-147`): +5.3% mean-hazard bias at the CLO preset; `expected_mdr` reports the uncorrected base so "expected" ≠ simulated mean.
- **HazardCurveDefault indexes the curve by loan seasoning instead of time-from-valuation** (`hazard_curve_adapter.rs:159-177`) — seasoned pools read the hazard at t ≈ seasoning years from month one.
- **Tree TwoFactor mode: credit factor computed but never consumed** (`tree/tree.rs:304-378`) — defaults and recovery read `factors.first()` (the prepay factor); configured prepay/credit correlation and credit_vol are dead.
- **DiversionEngine duplicate-source rules: lowest priority silently wins** (`diversion.rs:232-245`) — `active.insert` overwrites; the canonical OC+IC-on-one-tier case resolves wrong. (Live waterfall uses `tier.divertible`, so currently validation-only — see open questions.)
- **OC test numerator includes interest collections** (`waterfall.rs:820-833` passing full `available_cash` with `include_cash: true`) vs standard CLO par-OC; W-22 cure formula also wrong if `include_cash=false` is constructed. Phantom `DiversionRecord`s emitted with cure amounts when zero cash moved (`waterfall.rs:311-348`).
- **WARF/WAS include defaulted assets; WAS counts fixed-rate all-in coupon as spread** (`metrics/pool/warf.rs:20-30`, `types/pool.rs:804-821` with `spread_bps()` fallback `rate × 10000`) — one-sided WAS inflation with fixed buckets.
- **HW caplet normal vol omits the (1+τF) factor** (`hull_white.rs:1483-1504`) — model caplet vol understated ~τF; cleaner fix prices caplets exactly as ZCB puts reusing the formulas already in the file.
- **HW silent per-quote schedule fallback** (`hull_white.rs:780-787`): malformed real schedules silently revert to synthetic constant-dt while metadata still stamps `schedule_source = "real_day_count"`. Fixed-κ path has weaker guardrails (no κ band, σ search capped 1.0 vs SIGMA_MAX 2.0, no at-bound error, price-unit tolerance applied to vol-scale residuals).
- **Bindings**: WASM missing 5 trivial members present in Python on shared classes (`isRfl`, `isMultiFactor`, `marketStandardStochastic`, `conditionalLgd`, `recoveryVolatility`) — contract pins export names only so method drift is invisible; `expected_recovery` docstrings claim "unconditional" vs Rust's location-parameter semantics (Jensen correction); `cholesky_decompose` doc claims lower-triangular factor but pivoting can produce above-diagonal entries, and rank info is dropped; stub examples numerically wrong (`model_name`, conditional PD shows the unconditional value); concrete copulas/recoveries and several trait methods unreachable from both hosts with no documented exclusions (`stress_correlation_proxy` is unreachable exactly where the NaN contract directs users to it).

## Minor

Repo module doc has both PV signs flipped vs implementation; swaption implied-vol time uses instrument day count vs pricer ACT/365F; ZC inflation `par_rate` projects at curve base not as_of; haircut01 down-bump clamp without divisor adjustment; repo coverage metrics fetch dependencies with silent defaults (`unwrap_or`) and return INFINITY on zero collateral; cash-settled ParYield swaption analytic Greeks freeze the cash annuity (A′ term dropped, undocumented); `register_bermudan_swaption_metrics` is dead code (`#[allow(dead_code)]`, no caller found); InflationSwap registers Npv01 and Dv01 with identical configs. Gaussian copula `MIN_CORRELATION` early-return discontinuity; `MultiFactorCopula::with_loadings` loadings dead for pricing and `quadrature_order` ignored by the tranche engine; `select_quadrature` silently substitutes order 20 and caches under the requested key; stale GH-table normalization comment. Revolver `ThreeFactorPathData` unvalidated (panics on short vectors); PIK `Stepped` sortedness unvalidated; `num_paths=0/1` → NaN price/SE; IMM mode returns empty schedule silently; `StubKind::None` doc contradiction; `eom=true` snaps user start/end dates to month-end (moves economic dates); `MertonModel` accepts V ≤ B. Securitized: Ba3 factor 1760 vs Moody's 1766, CC=9550 non-standard; `CustomExpression` diversion conditions silently false; `correlation()` returns the loading for FactorCorrelated; expected_mdr discrete vs conditional continuous compounding mismatch; dead inconsistent `default_distribution` API; `prepay_factor_01` hardcoded 0.0; `loss_severity` doc vs LGD; coverage-test id truncation (`new_oc(1.15)` → `oc_test_114`); currency-mismatch checked_add failures silently zero in OC tests; MC recovery uses hard-clamped affine rule not the Jensen-corrected logistic. HW: dead `forward_analytic` hook (error provably cancels in Jamshidian strikes); no negative-rate monotonicity guard in the Jamshidian solve (assert `g'(r*) < 0`); quote types lack `deny_unknown_fields` and deserialize-time validation; "ATM (or off-ATM)" doc vs strike-less `SwaptionQuote`. Bindings: `nearest_correlation` defaults duplicated in three places; `.d.ts` namespace doc claims "factor models"; `tail_dependence` NaN contract undocumented in all hosts; facade interfaces omit `free()`; prior-market TypedDict "open dict" claim inaccurate for literal construction.

---

## Open Questions (Part 2)

1. **Canonical systematic-factor sign convention** (M2.13): Z>0 = good economy (copula) or Z>0 = stress (intensity/tree)? The recovery presets' ρ_R sign must follow the decision; nothing else can be fixed first.
2. **Student-t semi-analytic engine** (M2.3): intentional "effective model"? If so it needs a documented caveat — df calibrated through it is inconsistent when reused in the (correct) MC engine.
3. **`DiversionEngine` execution intent**: live waterfall uses `tier.divertible` only; if the engine stays validation-only, its findings drop in severity and `CustomExpression` should be removed.
4. **Cap quote convention** (M2.18): are the `CapFloorVol` quotes standard market quotes (first caplet excluded, fixing-date expiry)? If RFR in-arrears, the right fix is Lyashenko-Mercurio decaying vol, not `t_start`.
5. **Recovery-of-par in the Merton MC**: payout R·N can exceed firm value at default — intentional `DynamicRecoverySpec` semantics, or should recovery cap at V_τ?
6. **Pcg64 streams in Merton MC** vs the workspace Philox standard: the "non-overlapping for 2^64 samples" claim for distinct increments is unsubstantiated.
7. **Zero factor persistence** (M2.16): accepted approximation (then document and remove dead params) or gap?
8. **Defaulted assets in WARF/WAS**: captured via OC haircuts deliberately, or should pool metrics filter to performing par?
9. **Pathwise discounting in the stochastic revolver**: static-curve discounting of HW-rate-driven coupons is an undocumented approximation — intended?
10. **Bermudan metrics registration**: `register_bermudan_swaption_metrics` has no caller — if nothing wires it, the Bermudan-Greeks findings are latent.
11. **`finstack.valuations.correlation` symbol pinning**: the namespace has module-existence-only entries in `parity_contract.toml`, so all Python/WASM surface drift there is untested.

## Brief Summary (Part 2)

Same split as Part 1, sharper. The core single-factor Gaussian copula machinery, the Jamshidian swaption decomposition, the waterfall cure algebra, per-name MC determinism, repo cashflow mechanics, IRS DV01/par-rate conventions, and the envelope TypedDict field-name parity all verified cleanly — many with exact formula checks. The damage concentrates in (a) **multi-model consistency**: one spec means different models per engine (Student-t quadrature vs MC, RFL three ways, SDA three implementations/two shapes, sign conventions flipping between copula and intensity models — these are the worst findings in the pass because they're silent and config-dependent); (b) **convention details in derived quantities**: metrics layers (inflation vega scaling, FundingRisk sign, Money-rounded second differences) drift from the conventions their own pricers establish; (c) **schedule generation**, where LongFront and roll-day drift are wrong for any misaligned anchor and the tests pin the wrong behavior. The bindings layer is structurally sound with honest error mapping; its defects are documentation lies and a panic path, not numerics. Recurring root cause across both passes: components validated only against themselves — the LongFront test, the SDA self-consistency, the t-copula df calibration, and the cap/floor two-sided convention error all need external/golden anchors to become visible.

---

## Quant Notes

References for the Part 1 fixes: El Euch & Rosenbaum (2019) for the fractional Riccati and the I^{1−α} terminal term; Lewis (2000)/Lipton for the contour-integral prefactor; Bennedsen-Lunde-Pakkanen (2017) for the hybrid kernel near-field; Andersen (2008) §4.2 for the QE K0* correction; Hull-White (1994) forward-induction α placement (QuantLib's `ShortRateTree` is a good external cross-check on a steep curve); Black-Cox (1976) for the growing-barrier reflection term; Kemna-Vorst (1990) for the geometric-average drift; Castagna-Mercurio (2007) and Wystup (2006) for vanna-volga base-leg vol and KI-by-parity conventions; Diethelm-Ford-Freed (2004) Adams weights (verified correct in-repo); Longstaff-Schwartz (2001) / Glasserman (2003) Ch. 8 for the OOS-LSMC foresight-bias discussion.

References for the Part 2 fixes: PSA/BMA Standard Default Assumption (ramp 1–30, plateau 30–60, decline 61–120, terminal 121+) for the SDA curve shape; Moody's idealized rating-factor table for WARF golden values; Hull, Technical Note #1 / Kirikos-Novak (1997) for the HW futures-forward convexity adjustment (Ho-Lee limit ½σ²T₁T₂); Demarta-McNeil (2005) for the t-copula and its tail-dependence coefficient; Andersen-Sidenius (2005) for true state-dependent RFL vs Burtschell-Gregory-Laurent (2007) stochastic-correlation mixtures; Li (2000) for the copula default-time horizon convention; Brigo-Mercurio Ch. 3 (3.39–3.41) for HW bond-option/Jamshidian formulas (verified correct in-repo); Altman et al. (2005) for the PD-LGD co-movement sign; Joe-Kuo/Sobol dimension-assignment discipline for QMC path generation.
