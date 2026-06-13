# Quant Finance Review — `finstack/valuations` + Python/WASM Bindings (follow-up pass)

- **Date:** 2026-06-13
- **Scope:** `finstack/valuations` (all instrument families, models, calibration, metrics, market conventions, pricer registry/JSON) plus `finstack-py/src/bindings/valuations/` and `finstack-wasm/src/api/valuations/` with stubs, facades, and `parity_contract.toml`.
- **Method:** Six parallel read-only review agents (rates; credit/structured credit; cash fixed income; FX/equity/commodity/exotics; models/calibration/correlation; metrics/pricer/results/bindings), each instructed to re-derive the math and find issues the 2026-06-09 review **missed** or that **remediation introduced**. The Blocker and the two Major findings were independently re-verified at the source by the orchestrator (✅).
- **Relationship to prior review:** The 2026-06-09 review (`docs/reviews/2026-06-09-valuations-quant-review.md`) found 5 blockers + ~24 majors. **All** of them — including the FX/equity/exotics majors 18–22 whose `[FIXED]` markers were never updated in that file — have since been remediated (verified). This pass looks only for **new** defects.
- **Known intentional convention (not flagged):** DV01/CS01 are dPV/dy native sign — negative for long bonds/receivers is intentional.

---

## TL;DR

The crate is in very good shape; the prior remediation holds up under re-derivation, and the models/calibration/numerical-methods layer turned up **nothing new** (strong signal). The new defects cluster in two recurring patterns the prior review also called out: **(1) a fix applied to the PV path but not its cashflow-provider twin**, and **(2) a refactor that swapped an argument**. One **Blocker**, two **Majors**, three **Moderates**, plus minors.

---

## Findings

### Blocker

#### B1 ✅ — FX barrier Monte Carlo discounts by the year-fraction `t` instead of the domestic discount factor

- **Location:** `finstack/valuations/src/instruments/fx/fx_barrier_option/pricer.rs:67-68` (destructure) and `:129-137` (call site); helper `collect_fx_barrier_inputs` at `:318-356`.
- **Issue:** `collect_fx_barrier_inputs` returns `(spot, r_domestic, r_foreign, sigma, inputs.t)`. The 5th element is `FxOptionInputs.t` — documented as *"Time to expiry on the vol basis"* (`fx/shared.rs:72-73`), i.e. a **year fraction**. At the call site it is destructured into a variable named `discount_factor` and passed verbatim as the `discount_factor` argument to `PathDependentPricer::price(...)`. The MC engine applies it as the PV multiplier directly: `discounted_value = payoff_value * discount_factor` (`finstack/monte_carlo/src/engine/pricing.rs:100`). The engine's only guard is finite-and-non-negative (`pricing.rs:242`), which a year fraction passes.
- **Impact:** The simulated payoff is multiplied by `t` instead of `e^{-r_d·t}`. PV error scales with maturity:
  - 0.5y → ×0.5 instead of ~0.985 → PV ≈ **halved**
  - 1.0y → ×1.0 instead of ~0.97 → ~3% overstated (this is why the 1y regression test misses it)
  - 2.0y → ×2.0 instead of ~0.94 → PV **more than doubled**
  The at-hit rebate adjustment (`with_rebate_at_hit(r_dom)`, `:112`) is also corrupted since it compounds forward expecting to be re-discounted by `DF(T)`.
- **Reachability:** Production-reachable. `FxBarrierOption::base_value` routes to `npv_mc` whenever `use_gobet_miri = true` (discrete-monitoring barriers), and `default_model()` returns `MonteCarloGBM` in that case (`fx_barrier_option/types.rs:418-456`). Discrete monitoring is the realistic convention for actually-traded barriers. Default `use_gobet_miri = false` (analytical continuous path) is unaffected.
- **Likely origin:** the rebate-timing refactor (`7c53b0d6a fix(valuations): update FX and barrier option schemas for rebate timing…`). The sibling MC pricers all pass a real DF here: Asian `exotics/asian_option/pricer.rs:357`, lookback `exotics/lookback_option/pricer.rs:73`, autocallable `equity/autocallable/pricer.rs:298` — this one is the outlier.
- **Fix:** pass the true domestic DF. Either extend `collect_fx_barrier_inputs` to return `domestic_disc.df_between_dates(as_of, inst.expiry)` (preferred — matches siblings and is correct when curve base ≠ as_of), or inline `let discount_factor = (-r_dom * t).exp();`. Add a regression test at **2y** maturity comparing MC to the analytical continuous-monitoring price so the error can no longer hide in the 1y≈1.0 coincidence.

---

### Majors

#### M1 ✅ — CMS swap cashflow-provider path re-projects seasoned coupons from the live curve (phantom P&L; twin of an already-"fixed" PV bug)

- **Location:** `finstack/valuations/src/instruments/rates/cms_swap/types.rs:413-496` (`cms_leg_flows`), feeding `cashflow_schedule` (`:639`) → the public `dated_cashflows` / cashflow-provider surface.
- **Issue:** The 2026-06-09 remediation *"seasoned CMS coupons re-projected instead of using FIXING: lookups (FIXED)"* was applied only to `CmsSwapPricer::pv_cms_leg` (`pricer.rs:93,102-122` — it skips `payment_date <= as_of` and uses `historical_cms_fixing` for `fixing_date < as_of`). The cashflow-provider twin `cms_leg_flows` was **not** fixed: it has neither the `payment_date <= as_of` skip nor any seasoned branch, and unconditionally re-projects every coupon via `calculate_forward_swap_rate` from the live curve (`types.rs:431-445`).
- **Impact:** For any seasoned (mid-life) CMS swap the **reported cashflows diverge from the PV**. A fixed-but-unpaid coupon is re-projected from today's curve instead of its recorded fixing (the exact phantom-P&L the review claimed remediated). Worse, for `fixing_date < as_of` the helper sets `swap_start = fixing_date` (in the past), so the annuity discounts over past payment dates (DF > 1) → a numerically degenerate forward swap rate, and on a negative forward it then hard-errors (`:446-451`), failing the whole cashflow schedule. The reconciliation test (`types.rs` `~:795`) only exercises forward-starting coupons, so it does not catch this.
- **Fix:** mirror `pv_cms_leg`: skip `payment_date <= as_of`; for `fixing_date < as_of` use `historical_cms_fixing(...)` with `time_to_fixing = 0.0` and no convexity adjustment. Factor the per-coupon logic into one shared helper used by both paths so they cannot drift again.

#### M2 ✅ — CMS swap leg cannot price negative forward swap rates (regime gap; the "negative-rate fallback (FIXED)" covered only the CMS option)

- **Location:** `finstack/valuations/src/instruments/rates/cms_swap/pricer.rs:144-149` (PV path) and `cms_swap/types.rs:446-451` (cashflow path).
- **Issue:** Both CMS-swap paths hard-error when `forward_swap_rate <= 0.0`, *before* any cap/floor handling. The 2026-06-09 *"CMS negative-rate Bachelier fallback (FIXED)"* lives only in the CMS **option** pricer (`cms_option/pricer.rs:131-169`) and the embedded-option helper (`cms_swap/pricer.rs:388-399`, which itself handles `adjusted_forward <= 0`). The swap leg rejects the negative forward before it can reach that helper, and `convexity_adjustment` also returns 0 for `F <= 0`.
- **Impact:** A CMS swap in any negative-rate regime (EUR/JPY/CHF — precisely where CMS trades) cannot be priced at all; its sibling CMS option prices fine in the same market. Fail-loud (errors rather than silently mispricing), which limits the danger, but it is a real coverage gap and contradicts the "fixed" marking.
- **Fix:** on the swap leg, when `forward_swap_rate <= 0.0`, drop the lognormal Hagan convexity term (→ 0 anyway) and let the coupon use the Bachelier-capable `cms_embedded_option_value`; the uncapped/unfloored linear coupon is simply `forward_swap_rate + spread` (no error).

---

### Moderates

#### Mo1 — Inflation-linked bond `real_duration` divides a dirty-price derivative by the clean price

- **Location:** `finstack/valuations/src/instruments/fixed_income/inflation_linked_bond/types.rs:1017-1053`.
- **Issue:** `dp_dy = (p_up − p_dn)/(2·bp)` uses `price_from_ytm_compounded_params(...)`, which discounts all future flows from `as_of` and returns the **dirty** PV. The duration is then `−(dp_dy / p0)` with `p0 = base_clean` (the **clean** quoted price). Modified duration must use the dirty price in the denominator: `D = −(1/P_dirty)·dP_dirty/dy`.
- **Impact:** Real duration is biased by `accrued / P_dirty`. Small for low-coupon TIPS, but ~0.5–1% mid-period for a 2% real coupon, and inconsistent with the bond/term-loan paths, which all use the dirty price as the base.
- **Fix:** `let p0 = price_from_yield(y0)?;` (dirty model price at the solved real yield) instead of `base_clean`.

#### Mo2 — Callable bond on the default (Workout) basis: modified duration mixes a workout-path Macaulay with a maturity-flow YTM; YieldDv01 reprices on maturity flows

- **Location:** `bond/metrics/duration_modified.rs:44-75`, `duration_macaulay.rs:70-119`, `yield_dv01.rs:20-40`.
- **Issue:** For a callable/putable bond with a `quoted_clean_price` on the default `BondRiskBasis::Workout`, `MacaulayDurationCalculator` uses the **workout** yield and truncated workout cashflows, but `ModifiedDurationCalculator` divides that `D_mac` by `(1 + YTM/m)` where `YTM` is solved on the **full maturity** flows. Likewise `yield_basis_dv01` reprices on full maturity flows at the maturity YTM, not the workout price.
- **Impact:** Near par (YTW≈YTM) negligible; for a premium callable likely to be called (YTW ≪ YTM) the scaling factor and repricing base are materially inconsistent with the workout numerator. (Confidence: medium — verify intended workout semantics.)
- **Fix:** thread the workout yield/flows through the modified-duration denominator and the YieldDv01 repricing base so numerator and denominator use the same yield/cashflow set.

#### Mo3 — `HeteroMethod::Spa` (the **default** tranche method) doc materially misrepresents the implemented method

- **Location:** `cds_tranche/pricer/config.rs:352-358` (enum doc) and `:240` (default); implementation `heterogeneous.rs:275,340` → `saddlepoint.rs:45` `conditional_min_loss_normal`.
- **Issue:** The variant doc claims a *"genuine saddle-point approximation (SPA)… via a Lugannani-Rice / Antonov-Mechkov-Misirpashaev expansion… never places mass on L<0."* In reality every `Spa` path routes to a **moment-matched Gaussian** approximation, whose own module doc (`saddlepoint.rs:14-22`) states it *does* place `O(Φ(−μ/σ))` mass on `L<0` and that a genuine SPA "is not implemented here (deferred)."
- **Impact:** Not a numerical bug — the normal-approx error is bounded `< 1e-3` of tranchelet EL and well tested. But a user selecting the default for a skewed/low-PD senior tranche believes they get a no-negative-mass saddle-point estimator. Correctness-of-contract defect on a public API.
- **Fix:** rename the variant (e.g. `NormalApproximation`) or rewrite the doc to state it is a moment-matched Gaussian that leaks bounded mass below zero.

---

### Minors

- **Cap/Floor `Discounting` model key silently returns the full Black-76 price** — `rates/cap_floor/pricing/pricer.rs:206-257`, registered `pricer/rates.rs:90-96`. `SimpleCapFloorBlackPricer` stores `model: ModelKey` but `price_dyn` ignores it; the `Discounting` registration returns the optionality price, not an intrinsic/forward bound. Make the field drive behavior or drop the `Discounting` registration.
- **Convertible tree includes a coupon dated exactly on `as_of`** — `fixed_income/convertible/pricer.rs:175-187` skips only `cf.date < base_date`; a coupon on `base_date` maps to step 0 and is added in backward induction. The bond family uses strict `> as_of` filters (unified same-day rule). Change to `<= base_date { continue; }`.
- **Credit: mid-period loss discount fraction uses base-date origin for the first coupon** — `cds_tranche/pricer/engine.rs:252,267-271,385-399`. For `i==0`, `prior_time = 0.0` (curve base date) but the fraction is applied over `[contractual_effective_date, payment]`; when base ≠ effective date the interpolated default date for the first within-period loss increment is slightly mis-placed (second-order DF effect). Use `t(period_start)` as `prior_time` for `i==0`.
- **Credit: stochastic-recovery factor uses `Z`, not the systematic `M`, for the Student-t copula** — `cds_tranche/pricer/expected_loss.rs:541-554` and `heterogeneous.rs:179-207`. Recovery is evaluated at `z = factors.first()`; for Student-t the systematic driver is `M = Z/√W`. Exact for Gaussian; mildly mis-specifies recovery/default co-movement for stochastic-recovery + Student-t tranches (renormalization preserves total index EL, so the bias is in tranche allocation only). Pass `M` to `exposure_at` for mixing-variable copulas.
- **Dollar-roll pricer header doc contradicts the (correct) code** — `fixed_income/dollar_roll/pricer.rs:13-39` says "Net value = Front − Back" while the code correctly computes `back_value − front_value`. Doc-only.
- **Bindings: FX/exotics `priceWithMetrics` argument-order drift Python vs WASM** — Python `(market, as_of, model="default", metrics, …)` vs WASM `(marketJson, asOf, metrics, model?, …)` (`finstack-py/.../valuations/fx.rs:65-79` vs `finstack-wasm/.../valuations/fx.rs:138-156`, `index.d.ts:1282-1289`). Forced by wasm-bindgen trailing-`Option` rules; internally consistent and matches stubs, but a positional port silently swaps `model`/`metrics`. Also WASM is internally inconsistent: the free `priceInstrumentWithMetrics` uses model-before-metrics. Document the seam.
- **Bindings: wrong doc comment on WASM FX `greeks`** — `finstack-wasm/src/api/valuations/fx.rs:248` reads "Benchmark regression alpha/beta statistics per asset" (copy-pasted from analytics). Doc-only.

---

## Verified-clean this pass (recorded so it isn't re-reviewed)

- **Models / numerical methods / calibration:** SABR (Hagan/Obloj, z/χ(z) coefficients, ρ→±1 limits, `strike_from_delta` sign, normal-vs-Black tagging), trees (CRR/JR/Tian/LR probability validity, HW trinomial sum-to-1 in release, affine bond reconstruction), Hull-White (Jamshidian payer = ZCB puts, r* Newton+Brent, cap/floor ZCB equivalence with `(1+τK)` gearing, convexity), PDE (Thomas pivot guard, non-uniform stencils, boundary corrections), LM/Newton/bootstrap solvers (no zero Jacobian columns, consistent-unit success gate, hard no-bracket failure), Heston Little-Heston-Trap, bump units (futures parallel bp shift correct sign/magnitude) — **no new defects**.
- **FX/commodity:** Garman-Kohlhagen deltas (incl. premium-adjusted), digital/touch Rubinstein-Reiner, FX variance swap (date-based DF, Carr-Madan, 52/26 annualization), FX swap/NDF CIP, quanto drift, commodity forward/option/Asian/spread — clean. (FX barrier **MC** path is the exception above; analytical barrier path remains correct.)
- **Equity/exotics:** seasoned autocallable/cliquet/TRS fixing handling (prior fixes hold), Asian MC control variate + seasoning, lookback, basket NAV, index/vol-index futures undiscounted MTM — clean.
- **Credit:** CDS single-name ISDA legs/AoD, index decomposition, CDSO Bloomberg quadrature, copulas (Gaussian/Student-t/multi-factor/correlated recovery), tranche EL/recovery writedown/base-correlation, heterogeneous exact-convolution mass conservation, structured-credit waterfall conservation, CMO waterfall, Merton/Black-Cox/KMV/CreditGrades — clean.
- **Metrics/pricer/bindings:** FD framework (central-diff guards, clamp-aware vega, mixed-partial denominators), vega per-vol-point scaling (no 100×), DV01/CS01 sign/units + Neumaier summation, registry scenario-override/order-preserving batch/duplicate guard, strict JSON metric parsing + ISO as_of, results schema/flatten, GIL release, WASM `json_compatible` object emission, parity-contract resolution — clean.

---

## Open Questions / Assumptions

1. **B1 reachability:** confirm whether any shipped instrument/example sets `use_gobet_miri = true`. The fix is unconditional regardless, but it scopes the production blast radius.
2. **Mo2 (workout duration):** is the maturity-flow YTM in the modified-duration denominator intentional, or should the Workout basis be fully self-consistent (workout yield + workout flows everywhere)?
3. **M1/M2 recurrence:** both Majors are PV-path fixes not propagated to the cashflow-provider / sibling path. A shared per-coupon CMS helper (used by both `pv_cms_leg` and `cms_leg_flows`) would prevent the next instance.
4. Read-only review; no tests executed. Numerical claims verified by derivation/recomputation. B1 + M1 + M2 re-verified at source by the orchestrator.

## Recommended Regression Additions (highest leverage)

1. FX barrier MC at **2y** maturity vs analytical continuous-monitoring price within a tight band (catches B1).
2. Seasoned CMS swap: assert `dated_cashflows` for a past-fixing coupon equals the recorded fixing × accrual × notional, and equals the PV-path contribution (catches M1).
3. CMS swap on a negative-forward EUR market prices without error (catches M2).
4. ILB real duration vs a hand-computed dirty-price modified duration mid-coupon-period (catches Mo1).
