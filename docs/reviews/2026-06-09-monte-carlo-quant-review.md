# Quant Finance Review — `finstack/monte_carlo` Crate and Bindings

> **Remediation status (2026-06-12): COMPLETE for all Blockers and Majors.**
> B1, M1, M2, M4 were fixed in earlier `fix/quant-review-findings` commits;
> B2, B3, B4, M3, M5 (Bates reintroduced with a proper `QeBates` scheme —
> QE variance + K0* spot leg with the compensator absorbed once + Poisson/
> lognormal jumps — plus an engine-level martingale test), M6, M7, M8,
> M9, M10 and the Moderate batch (fill_u01, Sobol scramble/aux streams, CIR
> full truncation, Taylor coefficients, registry strict serde, BGK constant
> unification, LSMC date hygiene, greeks module exports, bindings parity)
> were fixed in this session. New regression tests: Asian CV notional
> linearity, LRM closed-form unbiasedness, HybridFbm covariance
> reconstruction, QE Case B CDF inversion + monotonicity, psi_c validation,
> thread-pool-size bit-identity.
>
> **Moderate/Minor pass (same date):** antithetic `on_path_start` mirroring
> (`MirroredStream` + stderr-collapse test); `RunMetadata` stamping on
> `MonteCarloResult`; `requires_injected_noise` engine guard for rough
> processes; `dedicated_scheme`/`scheme_id` pairing contract (rejects
> Euler+`GbmWithDividends` and Euler+Bates); `ProportionalDiffusion` removed
> from `GbmWithDividends` and added as a bound on `ExactMultiGbm`; QeHeston
> σᵥ→0 fallback keeps full variance; Feller predicate unified to ≥; Philox
> Random123 known-answer test + adjacent-stream correlation test; seed-doc
> CRN example fixed; bridge sampling textbook u<p; loud errors replace
> silent antithetic downgrades; `map_exercise_dates_to_steps`
> sorted/deduped/tolerance-clamped; `TimeGrid::uniform` final knot pinned to
> t_max; lsq relative SVD cutoff; `lmm_numeraire` doc corrected. Items
> verified as already fixed upstream: HW1F θ-averaging, LMM Bermudan
> pair-mean stderr, at-hit rebate (analytical `RebateTiming` + loud MC
> approximation warning).
>
> **Model-improvement pass (same date) — nothing remains open.** The five
> previously deferred features are implemented: randomized-QMC stderr via 16
> independently Owen-scrambled replicates (valid CI, tested against
> Black-Scholes); exact at-hit knock-out rebates in the MC barrier payoff
> (hit-time tracking, forward compounding so DF(T) nets to DF(τ); wired in
> the equity, Heston, and FX barrier pricers, approximation warns removed);
> Schwartz-Smith futures reconstruction `futures_price(τ)` per SS(2000)
> eq. 9 with the valuations MC pinning `E^Q[S_T]` to the market forward;
> LMM predictor-corrector upgraded to Hunter–Jäckel–Joshi on log displaced
> forwards (end-of-step corrector drift, exponential step exact for frozen
> coefficients — terminal caplet now matches displaced Black within pure MC
> error); and the Cheyette LSMC regression basis augmented with the
> Volterra vol state (x, y, W̃_H).

**Date:** 2026-06-09
**Scope:** `finstack/monte_carlo` (~27k lines), `finstack-py/src/bindings/monte_carlo/`, `finstack-wasm/src/api/monte_carlo/`, plus the valuations pricers that directly consume the MC stack (rBergomi/rough-Heston/Cheyette, Asian/barrier exotics, Schwartz-Smith commodity).
**Method:** Six parallel subsystem reviews (RNG/fBM, classic discretization, rough-vol stack, pricers/payoffs/barriers, engine/determinism/stats, Greeks/variance reduction, bindings parity). All Blocker-level findings were independently re-verified against source before inclusion.

---

## Findings

### Blockers — wrong prices or risk in live code paths

#### B1. rBergomi MC pricer never discounts the payoff

- **Location:** `finstack/valuations/src/instruments/equity/equity_option/rough_bergomi_mc_pricer.rs:325-332`
- **Issue:** Comment claims "simulate_path_fractional already returns discounted PV — do not multiply by discount_factor again," but the shared path loop explicitly returns the **undiscounted** payoff (`finstack/monte_carlo/src/engine/simulation.rs:94`, doc: "Returns the undiscounted payoff amount"), and the local `simulate_rbergomi` helper applies no discounting ("discount" appears nowhere else in the file). The sibling rough-Heston pricer correctly applies `(-r*t).exp()`.
- **Impact:** Prices too high by exactly `e^{rT}` whenever r ≠ 0 (~5% on a 1y option at r=5%). Paths drift at r−q so the error does not cancel.
- **Fix:** Multiply `mean_pv` by `(-r * t).exp()`; fix the comment.
- **Why tests are green:** The only BS-parity test uses `rate = 0.0`; the `rate = 0.01` case only checks determinism. Add an r ≠ 0 parity test.

#### B2. Asian control-variate mixes notional-scaled MC means with a per-unit analytical control

- **Location:** `finstack/valuations/src/instruments/exotics/asian_option/pricer.rs:540-561` (call) and `:644-665` (put)
- **Issue:** MC payoffs are notional-scaled (`finstack/monte_carlo/src/payoff/asian.rs:179-180` returns `intrinsic * self.notional`; payoffs constructed with `inst.notional.amount()`), but `seasoned_geometric_asian_control(...)` returns a per-unit price never multiplied by notional. Adjusted estimate is `N·X̄ − β(N·Ȳ − P_unit)` — deterministic bias ≈ `−β·P_geo·(N−1)`.
- **Impact:** With the documented builder default notional 100,000, the price collapses by orders of magnitude. This is the registered production Asian pricer path (`price_dyn → price_internal`).
- **Fix:** `control_analytical *= inst.notional.amount()` in both branches (or run CV per-unit and rescale at the end).
- **Why tests are green:** Every pricing test uses notional = 1.0. Add a notional = 100,000 regression test vs the analytic geometric reference.

#### B3. LRM Greeks apply the terminal-marginal score to path-dependent payoffs

- **Location:** `finstack/monte_carlo/src/pricer/path_dependent.rs:663-766` (`price_with_lrm_greeks`), `finstack/monte_carlo/src/greeks/lrm.rs:35-92`; wired in `valuations/.../asian_option/pricer.rs:861,884,947` and `valuations/.../barrier_option/pricer.rs:258,322` (public `npv_with_lrm_greeks`).
- **Issue:** `lrm_delta` (score `Z_T/(S₀σ√T)`) and `lrm_vega` (score `(Z²−1)/σ − √T·Z`) are scores of the **terminal marginal density** — unbiased only for payoffs that are functions of `S_T` alone (Glasserman §7.3). They are fed Asian (path-average) and barrier (path-extremum) payoffs.
- **Impact:** Closed-form check: for payoff `S_{T/2}`, the terminal-Z estimator converges to exactly **half** the true delta. Uniform-fixing Asian delta understated ~50%. Barrier vega doubly wrong — LRM also misses the explicit σ-dependence of the Gobet–Miri bridge crossing probability inside the payoff. Silently wrong Greeks through a `pub` API.
- **Fix:** Use the first-transition score `z₁/(S₀σ√Δt₁)` for delta and the per-step sum `Σᵢ[(zᵢ²−1)/σ − √Δtᵢ·zᵢ]` for vega (requires capturing per-step shocks), or restrict the LRM contract to terminal payoffs and route Asians/barriers to the (correct) CRN finite-difference helpers. Add an LRM-vs-FD-CRN consistency test.

#### B4. `HybridFbm` far-field double-counts dependence and is dimensionally inconsistent

- **Location:** `finstack/monte_carlo/src/rng/fbm.rs:274-339` (construction), `:369-393` (generate)
- **Issue:** For step `i ≥ b` the generator computes a complete truncated conditional Gaussian (weights solved against full increment covariance; residual std from `var_i − explained`), then **adds** a far-field sum `K(t_mid,s_mid)·√dt_j · out[j]` on top. This:
  1. Double-counts long-memory dependence already channeled through the conditional mean.
  2. Multiplies a kernel level by an **fBM increment** (std ~ dt^H) where a Volterra representation requires the driving **Brownian** increment (std √dt). For H=0.1, dt=1/300 the spurious variance contribution is the same order as the increment variance.
  3. Leaves `cond_std` blind to the added variance → `Var(ΔB_i) ≠ dt^{2H}`, wrong autocovariance for all `i ≥ b`.
  4. Uses the wrong kernel anyway: `MolchanGolosovKernel` (`finstack/core/src/math/fractional.rs:94-131`) is actually the Riemann–Liouville kernel `√(2H)(t−s)^{H−1/2}`, which does not map dW to true fBM (rename to `RiemannLiouvilleKernel`; fix doc claims in fractional.rs, fbm.rs, volterra.rs, rng/mod.rs).
- **Impact:** `create_fbm_generator` (`fbm.rs:421-436`) auto-selects `HybridFbm` for grids > 199 steps; the rough-Cheyette swaption pricer consumes it (`valuations/.../swaption/cheyette_rough_pricer.rs:393`). Long-grid fractional vol drivers have inflated variance and wrong long-range autocorrelation — biased swaption prices, silently.
- **Fix:** Delete the far-field term (the near-field conditional recursion alone is the standard, self-consistent approximation), or implement Davies–Harte circulant embedding for uniform grids (exact, O(n log n)) keeping Cholesky for non-uniform grids. Add a covariance-reconstruction test like `volterra.rs::variance_reconstructs_t_pow_2h` (the existing test tolerance `max_diff < 0.5` is non-diagnostic).

### Major

#### M1. Rough Heston Volterra discretization underweights the singular kernel — practically non-convergent at small H

- **Location:** `finstack/monte_carlo/src/discretization/rough_heston.rs:188-199`
- **Issue:** Midpoint point-evaluation `(t_next − t_mid)^{α−1}` per interval, including the most recent (singular) one. Exact L² mass of the dW term on the last interval is `Δt^{2H}/(2H)`; midpoint gives `Δt^{2H}·2^{1−2H}` — ratio ≈ 0.25 at the pricer's default H=0.07. The last interval carries ~half the kernel mass; error decays O(Δt^{2H}).
- **Impact:** Vol-of-vol and short-dated skew materially understated; MC will not agree with the rough-Heston Fourier pricer.
- **Fix:** Exact integrated kernel weights for drift `[(t_next−t_j)^α − (t_next−t_{j+1})^α]/(α·Δt_j)` and L²-matched weights for dW `√(((t_next−t_j)^{2α−1} − (t_next−t_{j+1})^{2α−1})/((2α−1)Δt_j))`, or reuse the correct BLP near-field from `rng/volterra.rs`. Add a parity test vs the Fourier pricer. (Also rename: this is a midpoint Riemann sum, not the BLP "hybrid" scheme.)

#### M2. Cheyette rough-vol step: end-of-step σ uses the current step's shock; correlation gives the rate driver fractional memory

- **Location:** `finstack/monte_carlo/src/discretization/cheyette_rough.rs:104-131`
- **Issue:** (a) `work[0] += db_h` runs **before** σ is computed and σ uses `t_next` — the diffusion coefficient is correlated with the step's own shock (the exact end-of-step-variance martingale bias the rBergomi module documents and fixed; see `discretization/rough_bergomi.rs:114-124`). (b) Rate–vol correlation uses the normalized fBM increment `db_h/dt^H`, whose cross-step autocorrelation gives the rate driver fractional memory — violating `dx = (y−κx)dt + σ dW` and the quasi-Gaussian bond reconstruction the swaption pricer relies on.
- **Fix:** Left-endpoint σ; accumulate `db_h` last; correlate against the unit-variance driving normal via the aux-injection mechanism rBergomi already uses (`engine/simulation.rs:29-37`). Add a bond-martingale test `E[DF_path(t)·P(t,T)] = P(0,T)`.

#### M3. Andersen QE Case B inverse CDF: `(u−p)` instead of `(1−u)`

- **Location:** `finstack/monte_carlo/src/discretization/qe_common.rs:115`
- **Issue:** Code: `((1.0 - p) / (u - p)).ln() / beta`. Andersen (2008) eq. (25): `Ψ⁻¹(u) = β⁻¹·ln((1−p)/(1−u))`. Since `u−p` and `1−u` are identically distributed on the branch, the one-step marginal is correct *by coincidence* — plain pseudo-random MC is unbiased — but monotonicity is inverted: antithetic coupling silently degrades, QMC use is invalid, and pathwise parity vs any reference QE implementation is impossible. The interior singularity at `u→p⁺` is what forced the `|u−p| < EPS` guard at `:112`.
- **Fix:** Use `(1−p)/(1−u)`, guard `u ≥ 1` instead, add a Case-B quantile test vs closed-form Ψ⁻¹.

#### M4. QE Heston: docs claim martingale-corrected spot leg; Andersen's K0\* is not implemented

- **Location:** `finstack/monte_carlo/src/discretization/qe_heston.rs:234-248` (doc claims `:70`, `process/heston.rs:47,92`)
- **Issue:** The implemented log update is algebraically the plain K0–K4 scheme with γ₁=γ₂=½, which has the known O(Δt) martingale bias; Andersen §4.2 derives `K0*` (branch-dependent, via the MGF of the QE variance draw) precisely to remove it. Nothing computes K0\*.
- **Fix:** Implement K0\* or relabel the docs; add an engine-level `E[S_T] = F` martingale test.

#### M5. `BatesProcess` cannot be used correctly with anything that compiles

- **Location:** `finstack/monte_carlo/src/process/bates.rs:73,99-129`
- **Issue:** Docs direct users to `BatesDiscretization`, which does not exist anywhere in the workspace. The only schemes that type-check (generic Euler/LogEuler) never apply jumps — yet `drift()` subtracts the jump compensator `λk` (`:113`), breaking the martingale by exactly the compensator. `factor_correlation()` is not overridden, so spot–variance ρ is silently dropped (independent shocks → no skew).
- **Impact:** Latent trap — no current production consumer (only a doc cross-reference in `process/gbm.rs`), but any user pairing it with Euler gets a mispriced jump-less Heston with zero correlation, silently.
- **Fix:** Implement a QE-based Bates scheme (QE variance leg + BK spot leg + Poisson jump factor, `factor_correlation` override) or remove/feature-gate the process.

#### M6. Results are not portable across machines: default chunking depends on `rayon::current_num_threads()`

- **Location:** `finstack/monte_carlo/src/engine/pricing.rs:42-48`; defaults `engine/config.rs:73,155`
- **Issue:** With the default `chunk_size: None`, the chunk partition (and hence the `OnlineStats::merge` float-reduction tree) is a function of CPU count / `RAYON_NUM_THREADS`. Even the *serial* path uses `adaptive_chunk_size` (`pricing.rs:648-655`). Identical (seed, paths, steps) gives ulp-different results across machines and across Python-vs-WASM (wasm sees 1 thread). The doc claim of thread-count independence (`pricing.rs:286-291`) is false for default configs. The registry already ships `rust.engine.chunk_size: 1000` — parsed, validated (`registry.rs:387-391`), never wired into `McEngineConfig`. Affected binding paths leave `chunk_size = None`: `finstack-py/.../engine.rs:42-43,291`, European pricer, WASM Heston/European (`finstack-wasm/src/api/monte_carlo/mod.rs:694,735-739`). The greeks binding and `PathDependentPricer` pin chunk sizes, so Asians/Greeks are reproducible; European/Heston/`McEngine` are not.
- **Fix:** Make the default chunk size a pure function of `num_paths` (or wire the registry default). Add a thread-count-invariance test (the existing bit-identity test pins `chunk_size(64)` and cannot catch this).

#### M7. Payoffs fail open on missing state: `unwrap_or(0.0)` spot defaults

- **Location:** `finstack/monte_carlo/src/payoff/vanilla.rs:69,124,194,266`; `payoff/barrier.rs:180`; `payoff/basket.rs:141,244,331`
- **Issue:** A missing/mis-keyed `SPOT` silently becomes 0.0: puts pay full strike, digitals always pay, down-barriers knock out instantly at step 0, worst-of baskets pin to zero.
- **Fix:** Default to `f64::NAN` — `validate_discounted_payoff` (`engine/pricing.rs:92-105`) already converts non-finite payoffs into a hard error, turning silent bias into loud failure at zero hot-path cost.

#### M8. Asian CV control mean inconsistent with the simulated drift on non-flat curves

- **Location:** `finstack/valuations/src/instruments/exotics/asian_option/pricer.rs:398-406` (DriftSchedule), `:213-273` (control)
- **Issue:** MC paths use a curve-implied `DriftSchedule`, but `seasoned_geometric_asian_control` assumes constant drift `(r−q−σ²/2)tᵢ` at every fixing. On sloped curves `E_MC[Y] ≠ control_analytical`, so the CV carries a systematic bias `β·(E_MC[Y] − E_an[Y])` that dominates stderr at high path counts.
- **Fix:** Evaluate the control's μ with the same cumulative `M(tᵢ)` at fixing times; the variance term stays valid (constant σ, exact scheme). Add a non-flat-curve CV test.

#### M9. Schwartz-Smith risk-neutralization is wrong; no futures reconstruction

- **Location:** `finstack/valuations/src/instruments/commodity/commodity_option/types.rs:398-408`; `finstack/monte_carlo/src/process/schwartz_smith.rs`
- **Issue:** `rn_kappa = kappa + lambda_x` — but Schwartz-Smith (2000) Q-dynamics shift the short-term factor by a **constant** drift `−λ_χ` at unchanged κ. Inflating κ distorts the futures vol term structure `e^{−κτ}` and the variance of χ. No futures formula `F(t,T) = exp(e^{−κτ}χ + ξ + A(τ))` exists anywhere; nothing pins `E^Q[S_T]` to the market forward the Black-76 branch uses.
- **Fix:** Constant λ-shift Q-dynamics; implement A(τ); calibrate drift so simulated `E^Q[S_T]` matches `F(0,T)` per maturity. Add an `E^Q[S_T] = F` test.

#### M10. Bindings: Python `MonteCarloResult` is actually Rust `MoneyEstimate`; Greek functions renamed

- **Location:** `finstack-py/src/bindings/monte_carlo/results.rs:8-11`; `greeks.rs:195-366`
- **Issue:** The Python class named `MonteCarloResult` wraps Rust `MoneyEstimate`, while Rust has a distinct public `results::MonteCarloResult` (estimate + captured paths) — same name, different type across the canonical boundary. Python `fd_delta`/`fd_delta_crn`/`fd_gamma`/`fd_gamma_crn` rename the canonical `finite_diff_*` functions. Both violate the names-match-Rust-exactly rule.
- **Fix:** Rename the Python class to `MoneyEstimate` (stub/contract updates) or wrap the real `MonteCarloResult`; rename Greek functions to the Rust names.

### Moderate

1. **LSMC implicit terminal exercise** — `pricer/lsmc.rs:441-444` (mirrored at `:751-753`, `:884-887`): cashflows seeded with terminal exercise value regardless of whether `num_steps ∈ exercise_dates` → phantom European exercise overprices Bermudans whose last exercise date precedes maturity. Fix: seed 0 unless configured, or validate/document.
2. **Nearest-neighbor exercise-date mapping with silent drops/collapses** — `finstack/core/src/math/time_grid.rs:278-321`: `.round()` snapping (up to dt/2 off), dates past maturity silently skipped, two dates can collapse to one step (exercise right disappears), `unwrap_or(0.0)` on day-count errors. Exact-knot machinery (`uniform_with_required_times`) already exists — use it and error on miss. Zero tests on these functions.
3. **Antithetic does not mirror `on_path_start` randomness** — `engine/pricing.rs:680-685,858-863`: pair members draw sequential independent path-start values (e.g. payoff thresholds); unbiased but variance reduction silently lost. Also silent antithetic downgrades: `path_dependent.rs:637,689` and the CRN FD helpers hardcode/ignore `antithetic` without error, unlike `validate_runtime` which rejects loudly.
4. **No run-metadata stamping** — `estimate.rs`, `results.rs`: no seed, parallel flag, antithetic, chunk size, or RNG family in result envelopes (project invariant requires it); only the tracing span logs them.
5. **QMC stderr meaningless** — `pricer/path_dependent.rs:464-475`: single scrambled Sobol sequence treated as i.i.d.; CIs unreliable. Fix: randomized-QMC replicates (16–32 scrambles, SE over replicate means) or null the SE. Also `seed = 0` disables scrambling (`core/.../sobol.rs:129-135`) and path 0 consumes Sobol index 0 — every normal ≈ −6.24σ, a deterministic extreme first path; skip index 0 and reject/derive nonzero scramble seeds.
6. **Asian average divides by fixings *seen*, not contracted** — `payoff/asian.rs:130-142,276-285`: a fixing step beyond the grid silently shrinks the average. Validate `fixing_steps ⊆ 0..=num_steps`; assert seen == contracted in `value()`.
7. **Barrier rebate at-maturity only** — `payoff/barrier.rs:96-98,247-251`: dominant knock-out convention pays at hit; rebate PV understated by `exp(-r(T−τ))`. Add `RebateTiming::{AtHit, AtMaturity}`.
8. **`GbmWithDividends` + generic schemes silently drop all dividends** — `process/gbm_dividends.rs:153-182` implements `ProportionalDiffusion`, so Euler/LogEuler/Milstein accept it but only `ExactGbmWithDividends` applies jumps. Remove the impl or gate it.
9. **CIR partial vs Heston full truncation under Euler** — `process/cir.rs:156-165` uses raw (possibly negative) v in drift; `process/heston.rs:285-305` uses v⁺ (full truncation, Lord et al. 2010). Make CIR match; name and cite the scheme. Also `psi_c` unvalidated: `with_psi_c(3.0)` accepted → ψ ∈ (2,3] enters Case A → NaN (`qe_common.rs:28-32,86-95`); validate `1 ≤ psi_c ≤ 2`.
10. **HW1F "exact" step uses θ(t) at left endpoint** — `discretization/exact_hw1f.rs:70-76`: silently inexact if a θ breakpoint falls inside a step. Subdivide at breakpoints (OU transition composes exactly) or assert grid alignment.
11. **BGK corrections `#[cfg(test)]`-only; pathwise Greeks test-only; lrm crate-private** — `barriers/mod.rs:10-11` (valuations carries duplicated copies of the constant claiming identity with it: `convertible/pricer.rs:508`, `barrier_option/pricer.rs:399`); `greeks/mod.rs:11-14` advertises `pathwise`/`lrm` that users can't reach — the only production Greeks are spot FDs. Export properly or fix docs.
12. **FD Greeks bias near zero spot** — `greeks/finite_diff.rs:43,64-70`: down-bump clamp makes the grid asymmetric while divisors stay `2h`/`h²` — delta biased up to ~2× for |S| < h (the rates/FX case the doc cites). Divide by actual spread; asymmetric second-difference weights when the clamp binds. Also no FD vega/rho/theta helpers exist at all.
13. **Asian CV operational hazards** — `asian_option/pricer.rs:444-487`: two full MC passes with `PathCaptureConfig::all().with_payoffs()` (O(paths × steps) memory, hundreds of MB at defaults) to read one f64/path; and `mc_paths > 100,000` hard-errors on the capture budget instead of pricing (`MAX_MC_PATHS` = 5M vs `MAX_CAPTURED_PATHS` = 100k). Single-pass combined payoff; drop full capture.
14. **`fill_u01` can return exactly 0.0; antithetic mirror produces exactly 1.0** — `rng/philox.rs:197-203`: `bits·2⁻⁵³` ∈ [0,1); `1−u` then yields 1.0, outside the documented contract. Use centered `(bits+0.5)·2⁻⁵³` (matching core Sobol). **No Philox known-answer test** vs Salmon et al./Random123 reference vectors — moment-sanity tests cannot catch a transposed multiplier; add KATs and an adjacent-stream cross-correlation test. Also `path_dependent.rs:418` derives per-path aux RNGs by XOR in **key** space (`seed ^ (path_id << 1)`) instead of the counter-space `with_stream` mechanism — stream-reuse foot-gun; use `with_stream(seed ^ SALT, path_id)`.
15. **Registry defaults accept unknown fields in nested structs** — `registry.rs:267-300`: leaf structs lack `deny_unknown_fields`; a typo in a determinism-critical config extension is silently ignored (violates strict-serde invariant).
16. **Generic engine accepts `SobolRng` in serial mode with no dimension contract** — `pricing.rs:206-213` validates Sobol only against parallel; per-step `fill_std_normals` misuses QMC points as i.i.d. across time steps (debug_assert-only guard). Reject non-splittable RNGs in the generic engine; point users at `PathDependentPricer`.
17. **Bindings drift** — WASM Asian silently inherits `use_parallel = true` from the Rust registry (`finstack-wasm/.../mod.rs:322`; everything else forces serial; Python defaults false). WASM hardcodes `num_steps = 252` in three places instead of reading the registry (`mod.rs:158,319,692`). Python stub `__all__` omits the four Greek functions (`finstack/monte_carlo/__init__.pyi`). `index.d.ts` `MonteCarloEstimateJson` missing 7 serialized fields; `std_dev: null` should be `undefined`/optional. `parity_contract.toml` has no monte_carlo symbols section and omits the `greeks` module — surface effectively contract-unenforced. No cross-host golden test (same seed → same value Rust/Python/WASM).
18. **LMM/rough-process edges** — LMM Bermudan stderr ignores antithetic pairing (`valuations/.../lmm_bermudan.rs:372-381`; update stats on pair averages; odd path counts silently drop a path). `lmm_numeraire` PathState doc claims `P(t,T_N)/P(0,T_N)` but stores `P(t,T_N)/P(t,T_first_alive)` (`process/lmm.rs:196-199,313-324`) — internally consistent for the Bermudan pricer (stub cancels) but wrong for any other consumer. LMM predictor-corrector evaluates corrector drift at t with stale loadings across vol breakpoints; arithmetic Euler with displacement floor rather than log-Euler (`lmm_predictor_corrector.rs:87-111`). Cheyette LSMC regression basis omits the volatility state → low-biased Bermudans at high η (`cheyette_rough_pricer.rs:535-546`). Rough processes are silently usable through the standard engine — `McEngine::price` fills the Volterra slots with raw i.i.d. N(0,1) → garbage variance paths with no error; add a `requires_fractional_noise()` validation flag.

### Minor (selected)

- Small-κΔt Taylor coefficient wrong (1/3 should be 1/2) in `exact_hw1f.rs:80-82` and `discretization/schwartz_smith.rs:85-88` — numerically immaterial (gates < 1e-8), mathematically wrong.
- Margrabe combined variance can go −1e-18 → NaN bypasses the degenerate-vol guard (`payoff/basket.rs:390-394`); clamp `sigma_sq.max(0.0)`.
- `bridge_hit_probability` degenerate-input guard runs before the straddle check (`barriers/bridge.rs:58-60`); payoff-grid mismatch maps dt → 0, silently disabling the bridge (`payoff/barrier.rs:200`).
- Barrier monitoring not gated at `maturity_step` (`payoff/barrier.rs:179-237`); bridge local vol taken at interval end under stochastic vol (`:204-207`).
- QE Heston σᵥ→0 fallback drops the correlated diffusion fraction — total spot variance `(1−ρ²)∫v` instead of `∫v` (`qe_heston.rs:240-246`; reachable only via struct-literal construction).
- `ExactMultiGbm` generic over any `StochasticProcess` — silently a frozen-coefficient scheme for non-GBM; bound on `ProportionalDiffusion` (`discretization/exact.rs:115-118`).
- Feller predicate inconsistency: Heston strict `>`, CIR `≥`.
- LSMC: duplicate exercise dates not deduped; no `discount_rate` vs process-drift consistency check (`lsmc.rs:150-184`). SVD truncation threshold absolute (1e-10), not relative (`lsq.rs:55`).
- `seed.rs:16-21` doc example derives different seeds per bump leg — would destroy CRN if followed; production code does it right.
- Code comments cite "INVARIANTS.md §2.1" — no such file exists in the repo.
- Inverted warning text "is not rough (H < 0.5)" should be "(H ≥ 0.5)" in three process files.
- `TimeGrid::uniform` last knot can differ from `t_max` by 1 ulp; `SimulatedPath::num_steps()` returns point count (steps+1) — rename `num_points()`.
- Python: panic-based `embedded_defaults_or_panic()` in user-callable entry points (inconsistent with `pricers.rs`); dead `Estimate` class (registered, never returned); result-vocabulary drift vs WASM (`num_skipped`, `relative_stderr`).
- Control-variate β guard is an absolute threshold (scale-dependent); `covariance()` panics on length mismatch in a `pub fn`; `lrm_rho` test-only while delta/vega are production-wired.

---

## Open Questions or Assumptions

- `BatesProcess` and Schwartz-Smith MC were treated as latent rather than active defects because no production consumer wires them today — if anything prices commodities via the SS MC branch, M9 escalates to Blocker.
- QE Case B (M3): plain pseudo-random prices are unbiased by distributional equivalence; severity rests on antithetic/QMC usage and external parity.
- HybridFbm impact magnitude (B4) asserted from structure and dimensional analysis, not a numerical experiment; a covariance-reconstruction test would quantify it.

---

## Brief Summary

Foundations are genuinely strong: Philox4x32-10 matches the Random123 reference structurally; per-path counter-based streams give serial ≡ parallel bit-identity on a fixed machine; the BLP hybrid scheme in `rng/volterra.rs` is correct and well-tested; the LSMC core has no foresight bias and ships a real two-pass unbiased mode; antithetic standard errors are computed on pair means; Welford/Chan statistics are sound; the BGK constant, bridge law, exact GBM/OU transitions, QE Case A, and the Merton compensator are all right. Bindings are thin, GIL-correct, and don't truncate seeds (u64 → BigInt in JS).

Defects cluster in two places:

1. **The rough-vol stack beyond `volterra.rs`** (HybridFbm, rough Heston discretization, rough Cheyette) was built without the covariance/martingale test discipline the rBergomi path received — all three are quantitatively wrong in ways their current tests cannot see.
2. **The estimator-adjustment layer** (control variates, LRM Greeks) combines correct components with wrong wiring — per-unit controls against notional-scaled payoffs, terminal scores against path-dependent payoffs.

**Highest-leverage remediation:** add the missing invariant tests. Nearly every Blocker would have been caught by one of:

- Engine-level martingale tests `E[S_T] = F` per scheme (catches B1, M4, M5, the QE σᵥ fallback).
- Covariance-reconstruction tests for every fBM generator (catches B4).
- MC vs closed-form golden tests: barrier with BGK vs Merton/Reiner–Rubinstein; Heston QE vs Heston (1993) semi-analytic; LSMC vs Longstaff-Schwartz (2001) Table 1; geometric Asian vs closed form; rough Heston MC vs the in-repo Fourier pricer (catches M1).
- Notional ≠ 1 and r ≠ 0 pricing tests (catches B1, B2).
- LRM/pathwise vs FD-CRN Greek consistency tests (catches B3).
- Philox known-answer tests vs Random123 vectors; thread-count-invariance test on default configs (catches M6).
- Cross-host golden fixture: fixed seed/paths/steps, pinned chunk size, asserted identical from Rust, Python, and WASM.
- KI + KO = vanilla parity; antithetic SE-reduction sanity; Bond-martingale test for Cheyette.

---

## Quant Notes

- Andersen, L. (2008), "Simple and efficient simulation of the Heston stochastic volatility model," *J. Computational Finance* — eqs. (25), (27)–(30) for QE branches; §4.2 for K0\* (M3, M4).
- Bennedsen, Lunde & Pakkanen (2017), "Hybrid scheme for Brownian semistationary processes," *Finance & Stochastics* — correctly implemented in `rng/volterra.rs`; `fbm.rs` should converge on it or Davies–Harte (B4).
- Bayer, Friz & Gatheral (2016), "Pricing under rough volatility" — rBergomi variance/martingale structure (verified correct in the discretization; B1 is the pricer's discounting).
- El Euch & Rosenbaum (2019) — rough Heston kernel `(t−s)^{α−1}/Γ(α)`, α = H+½ (kernel correct; weights are M1).
- Glasserman (2003), *Monte Carlo Methods in Financial Engineering* — §7.2–7.3 pathwise/LRM applicability (B3); §5.4 randomized-QMC error estimation; §4.1 control-variate same-sample bias; §6.4 bridge crossing law.
- Lord, Koekkoek & van Dijk (2010), "A comparison of biased simulation schemes for stochastic volatility models" — full vs partial truncation naming/bias (Moderate 9).
- Broadie, Glasserman & Kou (1997), "A continuity correction for discrete barrier options" — β₁ = −ζ(½)/√(2π) ≈ 0.5826 implemented correctly; needs one canonical production-compiled constant.
- Schwartz & Smith (2000), "Short-term variations and long-term dynamics in commodity prices," *Management Science* — Q-measure is a constant λ-shift, not a κ-shift (M9).
- Longstaff & Schwartz (2001), "Valuing American options by simulation" — Table 1 values are the standard LSMC golden test, currently absent.
- Salmon et al. (2011), "Parallel Random Numbers: As Easy as 1, 2, 3" — Philox KAT vectors for the missing known-answer tests.
