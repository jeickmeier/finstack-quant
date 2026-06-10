# Residual-Risk Quant Review — 2026-06-09

Follow-up to the earlier valuations review, covering items listed under "Residual Risk / Not Reviewed". Conducted via parallel deep-review agents; all Blocker-level claims were independently re-verified in source before inclusion.

## Coverage

**Reviewed (5 of 11 domains):**

- Swaption LMM/Cheyette-rough pricers + `exotics_shared` HW1F calibration/curve/LSMC/MC internals
- `models/credit/merton.rs` + `calibration/solver/bootstrap.rs`
- Commodity swap/swaption/asian + FX vanna-volga + equity vol-index future/option
- Equity Heston/rough-Bergomi/rough-Heston MC pricers + dcf_equity/pe_fund/real_estate
- Short-rate trees (`short_rate_tree.rs`, `hull_white_tree.rs`) + Heston Fourier (`closed_form/heston.rs`) + COS engine

**NOT reviewed (review agents killed by session usage limit — still residual risk):**

- Rates: `instruments/rates/repo/` + swaption/IRS/inflation `metrics/` submodules
- Credit: `correlation/copula/multi_factor.rs`, `random_factor_loading.rs`, cds_tranche `recovery01.rs`/`tail_dependence.rs`
- Fixed income: `bond/pricing/engine/merton_mc.rs`, `revolving_credit/` internals, `core/src/dates/schedule_gen.rs` + `schedule_iter.rs`
- Securitized: `structured_credit/pricing/stochastic/default/*`, `stochastic/metrics`, `pricing/diversion.rs`, pool `warf.rs`/`was.rs`
- Models: `calibration/hull_white.rs` (102KB)
- Bindings: valuations/correlation bindings (py + wasm) + envelope TypedDict internals

---

## Findings

### Blockers

#### B1. Rough-Heston Fourier pricer is wrong twice over — Riccati sign + Lewis contour

`finstack/core/src/math/volatility/rough_heston.rs:119` and `:430`

The fractional Riccati constant term is `a = ½(iu − u²)`; the correct El Euch–Rosenbaum coefficient is `−½(u² + iu)`. Verified: the martingale condition fails — F(−i) evaluates to 1, not 0. Separately, the Lewis inversion drops the `e^{x/2}` contour factor (the phase is `i·u·x` where it must be `i·(u−i/2)·x`), and the result is silently clamped with `.max(0.0)` at line 453 — deep-OTM calls return exactly 0. Benchmarked in the BS limit: 34.22 / 12.66 / 0.00 vs true 24.59 / 10.45 / 3.25.

**Impact:** every rough-Heston Fourier price and implied vol is wrong by 10–40%; the existing tests cannot catch it because the put is *defined* via parity and the H≈0.5 Heston comparison uses 15% tolerance.

**Fix:** `a = -0.5*(u*u + iu)`; use `w = u − i/2` in the exponent phase (`exponent = i*w*x + C + D·v0`); remove or log the `.max(0.0)` clamp.

#### B2. Rough-Bergomi MC returns undiscounted payoff as PV

`finstack/valuations/src/instruments/equity/equity_option/rough_bergomi_mc_pricer.rs:325-331`

The comment claims "simulate_path_fractional already returns discounted PV — do not multiply by discount_factor again." That is false — the payoff module (`finstack/monte_carlo/src/payoff/vanilla.rs`) explicitly documents that vanilla payoffs are undiscounted, and `engine_fractional.rs` applies no discounting (verified).

**Impact:** prices high by `e^{rT}−1` (~3% at r=3%, 1y; worse long-dated). Invisible to in-crate tests because they all use r=q=0.

**Fix:** multiply `mean_pv` by `(-r*t).exp()`, matching `heston_mc_pricer.rs:129` and `rough_heston_mc_pricer.rs:185`.

#### B3. Commodity swaption annuity mis-scaled ~12× vs its own underlying

`finstack/valuations/src/instruments/commodity/commodity_swaption/types.rs:296-312` (also intrinsic branch `:359-369`)

`annuity()` is `Σ DF·τ` (IR-swaption convention) while `notional` is documented "Notional quantity per period" (`types.rs:104-105`) and the underlying `CommoditySwap` pays `quantity × price` per period with **no** year-fraction accrual (`commodity_swap/types.rs:222-226`, verified). The consistent annuity for a per-period-quantity notional is `Σ DF`.

**Impact:** a monthly-settling swaption (τ≈1/12) is understated ~12×, including its intrinsic branch.

**Fix:** `annuity = Σ DF` (with matching `w_i = DF_i` weights in `forward_swap_rate`), or redefine/redocument `notional` as an annualized rate and prove consistency with `CommoditySwap.quantity`.

#### B4. Hull-White tree drift calibration is off by one step

`finstack/valuations/src/models/trees/hull_white_tree.rs:290` (vs `:541` and `:829`)

The α solved at iteration `step` (matching `P(0, t_{step+1})` over `[t_step, t_{step+1}]`) is stored in `alpha[step+1]`, while both `forward_state_prices` and `backward_induction` discount that interval with `alpha[step]`. Verified in source. The tree does not reprice the curve except when flat; the in-repo test's 1bp tolerance on a mild curve is almost exactly the size of the bug (~0.8bp).

**Impact:** on steep curves, 20–40bp of ZCB bias feeding HW swaptions (`swaption/hw_pricer.rs:174`, `bermudan/mod.rs:84`) and callable bonds (`bond/pricing/engine/tree/tree_pricer.rs:262`).

**Fix:** store the solved value at `alpha[step]`; set terminal `alpha[N] = alpha[N−1]`; tighten `test_tree_calibration` to ~0.1bp and add a steep-curve case.

### Major

#### Rates / Cheyette-rough

Mitigated in the production registry path by `enforce_calibration: true` (`pricer/exotics.rs:239`), but live for direct construction (`BermudanSwaptionCheyetteRoughPricer::default()`).

- **M1. Look-ahead vol in the Euler step** — `finstack/monte_carlo/src/discretization/cheyette_rough.rs:104-131`. The fBM increment is added *before* σ is computed, so the step's vol sees its own shock; with ρ≠0 this creates a spurious drift of `O(dt^{H−1/2})` that **grows under grid refinement** for H<0.5 (~−300/400bp cumulative drift in the x-state at pricer defaults). Fix: adapted/left-point scheme — compute σ from accumulated `W̃_H(t)` before adding the increment, with compensator `t^{2H}` not `t_next^{2H}`.
- **M2. Rate noise correlated with the fBM increment, not the innovation** — `cheyette_rough.rs:116-128`. fBM increments are autocorrelated (strongly negative at H=0.1), so the rate driver is not Brownian — breaking the Markov bond-reconstruction formula the pricer relies on (`cheyette_rough_pricer.rs:159-172`). The correct tool (`RiemannLiouvilleVolterra`, two normals/step) exists in `rng/volterra.rs` but is not used here.

#### Credit / Merton

- **M3. Jump-diffusion martingale broken** — `finstack/valuations/src/models/credit/merton.rs:969` vs `:1031,:1051`. The compensator uses `κ = e^{μ_J+σ_J²/2}−1` but the simulated jump multiplier is `exp(μ_J − ½σ_J² + σ_J z)` (mean `e^{μ_J}`) — a spurious Itô correction on the jump. E[V_T] ≠ V₀e^{(r−q)T}; ~1.2% drift bias at λ=0.5, σ_J=10%, T=5y. Fix: `(mu_j + sigma_j * jz).exp()` in both branches plus a JD mean-convergence test.
- **M4. Black-Cox first-passage wrong for growing barriers** — `merton.rs:279-307`. For `barrier_growth_rate ≠ 0` the reflection term uses drift μ+g instead of ν = μ−g and the wrong exponent (`−2μ(x₀−gT)/σ²` instead of `−2νx₀/σ²`). All closed-form regression tests use g=0; the g≠0 test only checks monotonicity, which the wrong formula also satisfies. Fix: reduce to BM with drift ν = μ−g started at x₀ = ln(V/B); add a g≠0 Black-Cox (1976) regression test.

#### Trees

- **M5. "Black-Karasinski" mean reversion is inoperative** — `finstack/valuations/src/models/trees/short_rate_tree.rs:852-860`. κ only shrinks per-step spacing by `(1−κΔt/2)`; terminal dispersion → `σ√T` regardless of κ as Δt→0 (true BK: `σ√((1−e^{−2κT})/2κ)` — 13% lower at κ=0.03, T=10y). Constant-spacing recombining binomial lattices cannot represent BK's state-dependent drift. The Bloomberg-OAS1-parity doc claims (`:832`, `:346`) are not supportable. Fix: reject κ≠0 (as Ho-Lee does) or implement a real trinomial BK lattice in ln r.
- **M6. Ho-Lee calibration/pricing compounding mismatch** — `short_rate_tree.rs:686,:719,:753` vs `:1245`. Calibration hard-codes continuous discounting while `price()` honors `config.compounding` — a Simple-compounding Ho-Lee tree silently fails to reprice the curve (~10–20bp on 5y ZCB). Currently masked only because `tree_pricer.rs:299-305` accidentally drops `tree_compounding` from the documented `production_ho_lee` preset (`config.rs:375`). Fix: use `comp.df()` throughout `calibrate_ho_lee` (or reject non-Continuous), and make `tree_pricer.rs` pass `tree_compounding` through.

#### Equity exotics / rough vol

- **M7. Rough-Heston CF v₀ term uses D(T) instead of I^{1−α}D(T)** — `rough_heston.rs:334-336,:428-430`. EER (2019) Thm 4.1 requires `v0·I^{1−α}h`; `v0·h(T)` is correct only at α=1. With B1 fixed: 0.3% ATM growing to 2.5% in the wings (H=0.1, σ=0.3). Fix: product-integrate `I^{1−α}D` at T over the existing `d_trajectory`.
- **M8. Rough-Heston MC kernel underweights the singular near-field ~41% at H=0.1** — `finstack/monte_carlo/src/discretization/rough_heston.rs:188-199`. Midpoint rule on the last interval vs the exact stochastic weight `√(dt^{2α−1}/(2α−1))` — the naive-Riemann O(n^{−H}) bias class. The "exact (no binning approximation)" doc claim is wrong and `RoughHestonHybrid` is not the BLP hybrid scheme. Fix: exact per-interval kernel integrals (drift) + exact-covariance near-field (noise), or document/rename.

#### PE fund waterfall/metrics

- **M9. Promote-tier hurdles are decorative** — `pe_fund/waterfall.rs:832-855`. The hurdle only appears in the tranche display name; the tier splits all remaining cash, so multi-tier promotes never trigger past tier one and `Hurdle01` measures noise. Fix: gate each promote tier on LP IRR reaching `hurdle` (same solver as PreferredIrr).
- **M10. Preferred-return solve and LP IRR use gross fund events** — `waterfall.rs:894-900,:1016-1029`. Distribution/Proceeds counted at full face value, crediting LPs with GP carry; pref under-allocates and LP IRR is overstated after any carry. Fix: build LP history from the ledger `to_lp` rows (as `lp_cashflows()` does).
- **M11. `GpIrr` returns total carry in dollars** — `pe_fund/metrics.rs:72-76`. Registered as an IRR; consumers reading a rate get garbage. Fix: compute an actual GP IRR or rename `GpCarryTotal`.
- **M12. `MoicLp` includes GP carry** — `metrics.rs:97-114`. Use ledger `to_lp` as `DpiLpCalculator` correctly does (`:136-141`).
- **M13. Fund PV discounts the entire historical flow set; metrics double-count it** — `pe_fund/pricer.rs:31-39` + `core/src/cashflow/discounting.rs:252-267`. No `d <= as_of` holder-view filter, so `base_value` is lifetime NPV, then `LpIrr`/`TvpiLp` re-add it as terminal "Residual NAV". A fully-realized fund won't show residual ≈ 0. Fix: holder-view filter for PV; model unrealized NAV explicitly.

#### DCF / Real estate

- **M14. Silent discounting-regime switch to risk-free curve** — `dcf_equity/pricer.rs:48-72`, `real_estate/pricer.rs:38-52`. If the named OIS curve is loaded, risky FCF/NOI and terminal value discount at risk-free instead of WACC / property rate — same instrument, wildly different PVs depending on MarketContext contents, no spread adjustment, no policy stamp. Fix: curve + calibrated spread (matching WACC basis at base curve), or compute DV01 by bumping a risk-free component inside WACC.

#### Commodity

- **M15. No realized-fixings mechanism on `CommoditySwap`** — `commodity_swap/types.rs:327-341`. Completed averaging periods mark at *today's spot*; the live averaging period propagates a hard error by design. Unusable for daily seasoned marking. Fix: add a realized-fixings store mirroring `CommodityAsianOption.realized_fixings`.
- **M16. Geometric Asian drops the Kemna-Vorst drift correction** — `commodity_asian_option/pricer.rs:187-221` (seasoned variant `:288-300`). Forward for E[G] overstated by `exp((σ²/2)(Σt/m − ΣΣmin/m²))` ≈ +0.75% for monthly/1y/σ=30% — several % of an ATM premium. The equity seasoned-geometric control variate (`exotics/asian_option/pricer.rs:213-258`) does it correctly; the commodity test validates against the same wrong law. Fix: Black-76 forward = `geo_mean·exp(v/2 − (σ²/2m)Σt_i)`.
- **M17. Missing past fixings silently distort the effective strike** — `commodity_asian_option/types.rs:158-181` + `pricer.rs:345-374`. No completeness check `hist_count + future_count == total_fixings`; a missing/date-mismatched fixing inflates `k_eff` silently. Fix: error when any fixing date ≤ as_of lacks a realized value; reject duplicates.

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

## Open Questions

1. **DCF/RE risk-free discounting (M14):** intentional "rates-sensitivity mode" or a defect? If intentional it needs a policy stamp in result metadata; if not, it is Blocker-class whenever the curve loads.
2. **Commodity swaption notional semantics (B3):** per-period quantity (as documented) or annualized rate? Decides the fix direction.
3. **PE fund PV semantics (M13):** residual position value (needs holder-view filter + explicit NAV input) or lifetime NPV (then metrics must stop re-adding residual NAV)?
4. **Cheyette-rough:** research prototype indefinitely (registry refusal is the only guard), or fix the discretization via the existing `RiemannLiouvilleVolterra`? Decide the η convention (variance vs σ lognormal) at the same time.
5. **Heston Fourier proliferation:** three implementations (`closed_form/heston.rs` Gil-Pelaez, `models/volatility/heston.rs`, COS infra without a Heston CF), cross-validated at one parameter point only. Consolidation planned? Is a `HestonCf` for the COS path planned?
6. **Bloomberg BK parity (M5):** if OAS1 parity is a requirement, the BDT-κ variance tweak cannot deliver it — a real BK lattice is needed.
7. **Vol-surface typing:** project-wide convention for which `vol_surface_id`s are (expiry × tenor, normal) vs (expiry × strike, Black)?
8. **`implied_spread` convention:** reduced-form with exogenous recovery (current, documented) vs canonical Merton endogenous-recovery spread — users comparing to textbook values will differ.
9. **MC step defaults for rough models:** 100 steps with O(n^{−H}) convergence at H≈0.07–0.1 — has a step-refinement study validated this? The construction-time warning triggers only *above* 200 steps, backwards relative to the accuracy risk.

## Brief Summary

The reviewed surface splits sharply. The mature paths are genuinely strong: LMM terminal-measure drift/deflator algebra, HW1F θ-fit and bank-account discipline, the credit bootstrap's no-silent-skip design, QE-Heston variance stepping, the rBergomi noise machinery (BLP hybrid, exact near-field), the COS engine, ISDA-style CDS accrual-on-default, and Philox determinism all verified cleanly. Defects cluster in (a) the newest research-grade pricers — rough-Heston Fourier is unusable as written, rough-vol discretizations carry refinement-divergent bias, Cheyette-rough has measure-level problems; (b) instruments whose tests cannot see the bug — HW tree's flat-curve 1bp tolerance, rBergomi's r=0 tests, the geometric Asian validating against its own wrong law, rough-Heston's circular parity test; and (c) the cash-economics layer (PE waterfall, commodity seasoning) where conventions, not stochastics, are wrong.

**Highest-leverage process fix:** external golden fixtures for every Fourier/rough pricer — four of the five worst findings survived only because all tests were self-referential.

## Quant Notes

References for the fixes: El Euch & Rosenbaum (2019) for the fractional Riccati and the I^{1−α} terminal term; Lewis (2000)/Lipton for the contour-integral prefactor; Bennedsen-Lunde-Pakkanen (2017) for the hybrid kernel near-field; Andersen (2008) §4.2 for the QE K0* correction; Hull-White (1994) forward-induction α placement (QuantLib's `ShortRateTree` is a good external cross-check on a steep curve); Black-Cox (1976) for the growing-barrier reflection term; Kemna-Vorst (1990) for the geometric-average drift; Castagna-Mercurio (2007) and Wystup (2006) for vanna-volga base-leg vol and KI-by-parity conventions; Diethelm-Ford-Freed (2004) Adams weights (verified correct in-repo); Longstaff-Schwartz (2001) / Glasserman (2003) Ch. 8 for the OOS-LSMC foresight-bias discussion.
