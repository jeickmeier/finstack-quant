# CALIBRATION-RATE-MODELS-009: LMM loading scale from frozen basket weights and displaced ATM premium inversion

**Decision status:** Accepted 2026-11-15 — complete single-slice LMM loading-scale fit, ATM premium conversion, fit evidence and explicit consumption package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent clock, frozen loading and shifted ATM premium calculations verified. Production fitting, independent multi-forward comparisons, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for single-slice LMM loading-scale calibration

1. **Fit one positive scale to the earliest strictly future eligible slice.**
   Prepare the exact physical co-terminal European swaption from the
   admitted Bermudan and its actual remaining swap. Use the par strike
   of that slice. A valuation-date-only exercise yields no fit target.
   The fit qualifies one European ATM premium under the frozen-weight
   approximation. Later exercises, the smile, correlation and Bermudan
   Monte Carlo value lie outside the fit scope.
   [Accepted Bermudan contract and strip](../valuations-rates/09-lmm-bermudans.md#accepted-decision-for-checked-lmm-bermudan-pricing-and-risk).

2. **Reuse the product's prepared strip and explicit model inputs.**
   Share its dated discount and projection evaluator, actual contractual
   accruals, retained forwards and original period indices and count.
   Require the finite signed common displacement `d` and the elected
   `linear_two_factor_v1` profile. For original index `i` and count `m`,
   use the two lanes `g_i=(1−.4 i/m,.4 i/m)`. Check `F_i+d>0`,
   `1+τ_i F_i>0` and exact `τ_i d≤1` throughout the strip, including
   with literal zero displacement. Retire the curve-derived 1% shift
   and implicit shape. Prepare the strip without selecting a trial scale.
   [Accepted shifted LMM support](../models-monte-carlo/10-lmm.md#accepted-decision-for-checked-terminal-measure-lmm-paths).

3. **Bind a qualified unshifted Black strike grid and use strict lookup.**
   Select a named active BlackLognormal expiry-by-absolute-strike grid
   qualified for the actual co-terminal deliverable and quote convention.
   Require the source origin to match valuation. Look up the actual exercise
   on its declared quote clock at absolute par strike `S>0`, retaining the
   source's elected interpolation inside its support. Reject off-grid
   queries and inconsistent origins or conventions. Retain positive quote
   time `T_q` and ACT/365F model time `T_m` for the same dated interval.
   Preserve total volatility across the explicit clock bridge.
   [Accepted volatility-source evaluation](../models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization).

4. **Compute the frozen-weight factor from the actual annuity.** Use
   `A=Σ τ_i D(V,T_(i+1))` and `w_i=τ_i D(V,T_(i+1))/A` from the shared
   evaluator and contractual accruals. Use the checked par rate `S` and
   `B=Σ w_i(F_i+d)`, economically `S+d`. With checked two-lane accumulation,
   compute `R=||Σ (w_i(F_i+d)/B) g_i||_2`. Retire the reconstructed
   discount ladder, quadratic covariance sum and arbitrary denominator
   floors. Preserve the signed displacement through premium-bound checks.
   [Displaced frozen-weight approximation, equations 5.6–5.7](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=3591458).

5. **Match the finite-expiry ATM premium analytically.** Require a
   positive finite annual quote `σ_q`. Form the unit-annuity target
   `p=S erf(σ_q sqrt(T_q)/(2 sqrt(2)))` and reuse the checked analytic
   ATM conversion to match it at `B`. Convert the shifted total volatility
   to the ACT/365F model clock and divide by `R` to get the positive finite
   `lmm_base_vol` scale `a` per square root model year. At literal zero shift,
   `a=σ_q sqrt(T_q/T_m)/R` under the same checks. Require resolved
   `0<p<min(S,B)` using scaled premiums and complementary cap gaps. Preserve
   typed infeasibility, range and precision errors. A signed negative shift
   can make the target unattainable even on admitted shifted-forward support.
   [Accepted analytic ATM conversion](../models-analytic/atm-volatility-conversion.md#accepted-decision-for-checked-price-equivalent-atm-volatility-conversion).

6. **Gate the returned scale and make consumption explicit.** Rebuild
   the exact represented loading rows `λ_i=a g_i` and replay the frozen
   ATM premium. Require both absolute and relative premium agreement
   under the accepted ATM conversion rules. Reconstruct the unshifted
   annual Black quote on its original clock and apply the independent
   positive native tolerance to the signed implied-minus-observed error.
   Return the admitted scale with its exact exercise, strip and market
   bindings, profile, displacement, clocks, premium and native residuals,
   tolerances and actual work. Keep the fit read-only. Pricing may consume
   a qualified fit on its exact basis or take an explicitly authored scale.
   Preserve the accepted frozen-scale direct curve risk. Use Rust admission
   through the existing typed, JSON, Python and WASM paths and apply the
   same recorded-basis qualification on restored fits before consumption.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

7. **Use the correct Rebonato attribution.** Rebonato's 2002 book,
   sections 10.3 and 10.5, covers the swap-rate volatility approximation
   and co-terminal European calibration. Chapter 8 covers instantaneous
   volatility functions. Chapter 11 treats displaced diffusion.
   [References](../../../REFERENCES.md#rebonato-2002-lmm-swaption-calibration).

## Independent calculation checks

- For one forward with `g=(1,0)`, accrual `1`, par rate `.02`, 365-day
  expiry and annual ACT/360 quote `.60`, the target unit-annuity premium
  is about `.004748119725719348`. With shift `.01`, the ACT/365F scale
  is about `.3993638713876322`. At zero shift it is `.6041522986797285`.
  With shift `−.016`, the shifted level `.004` passes shifted-forward
  support and the target exceeds its premium cap. No finite scale exists.
- For two retained periods with accruals `.5` and `1`, original indices
  `0,1` of `2`, and relative discount factors `1,.99,.96`, use shift
  `.01`. The annuity is `291/200`, par rate is `8/291` and the shifted
  factor vector is `(4663,792)/5455`. Its norm is about `.867054353072438`.

## Observed choice and rationale at the audit revision

The explicit LMM helper fits one common loading scale to the earliest future co-terminal European swaption embedded in a Bermudan contract. It uses frozen initial basket weights (Rebonato approximation), then preserves the finite-expiry Black ATM premium when converting to displaced dynamics. Source rationale is that quoted swap-rate volatility is not identical to instantaneous forward-factor loading, and a simple instantaneous displacement ratio does not preserve finite-expiry option value.

## Input domain and caller selection

The helper calls the valuations-owned `build_lmm_params` with scale 1. It inherits physical co-terminal settlement, a single discount/projection curve, matching fixed/floating schedules/day counts, simple floating compounding, zero spread/reset/payment lag, payment on accrual end and exercise dates on tenor starts. It trims all periods before the earliest exercise strictly after `as_of`; exercises on that date are already elapsed.

The same builder supplies contractual accruals, initial forwards, a common displacement `max(0,0.01−min F_i)`, and a fixed two-factor shape `[1−0.4f_i,0.4f_i,0]`. Here `f_i` retains the index in the original full contract divided by its original period count after trimming. Accruals need not equal ACT/365F tenor differences. These structure methods are owned by the [LMM product record](../valuations-rates/09-lmm-bermudans.md), not fitted here.

The input surface must have secondary axis Strike and quote type BlackLognormal. Expiry is the first live tenor time in ACT/365F years. Look up the ATM surface at that expiry and the unshifted basket swap rate using clamped edges. Normal, shifted-Black and expiry-by-tenor grids are not accepted by this helper. Despite displaced model forwards, the calibration still requires a strictly positive unshifted ATM swap rate.

## Ordered algorithm and equations

1. Reconstruct relative discount factors from live forwards: `D₀=1`, `D_{i+1}=D_i/(1+α_iF_i)`. The common normalization cancels in annuity weights. Reject a denominator with absolute value below `1e-15`; upstream structure validation supplies valid shapes/accruals/forwards.
2. Compute `A=Σα_iD_{i+1}` and `w_i=α_iD_{i+1}/A`; reject `|A|<1e-15`. Set unshifted swap rate `S=Σw_iF_i` and shifted basket `B=Σw_i(F_i+d_i)`; require finite B>`1e-12`.
3. Compute `R²=Σ_iΣ_j w_iw_j(F_i+d_i)(F_j+d_j)(g_i·g_j)/B²`. Require finite positive R²; the final map also requires R>`1e-12` and S>`1e-12`.
4. Require positive finite market Black vol and expiry. If S equals B exactly, retain market vol. Otherwise price the unshifted ATM Black call `c=Black(S,S,σ_market,T)`, then use the shared Black implied-vol solver to find `σ_shifted` satisfying `Black(B,B,σ_shifted,T)=c`.
5. Return `base_vol=σ_shifted/R` if finite/positive. Multiplying the fixed loading shape by this scale matches the single target premium within the frozen-weight approximation; it does not imply exact multi-forward Monte Carlo repricing.

The JSON entry parses a canonical Bermudan instrument envelope then delegates to the typed helper. Calibration is read-only and returns a scalar, with no `CalibrationReport`, persistence, independent fit gate or multi-expiry optimization. The caller explicitly supplies the result as `instrument_pricing_overrides.model_config.lmm_base_vol`; pricing requires that override and does not read/calibrate the surface implicitly.

## Controls, failures and limitations

The correlation/loading shape, common displacement floor and target exercise are fixed by the product builder; none is estimated by this fit. Surface wing/expiry clamps and the shared [Black inverse controls](../models-analytic/implied-volatility-solvers.md) remain material. A degenerate basket, missing surface/curve, unavailable inverse or invalid scale returns an error, not the raw market vol as fallback. Internal slices assume the public builder's aligned validated arrays; this is not an independently safe general-purpose basket API.

All later exercise maturities, smile strikes and correlation structure remain uncalibrated. Frozen weights ignore state-dependent swap weights. Negative/zero unshifted swap rates remain outside the Black-quote target even when shifted forwards are positive. A one-forward case is an exact displaced-Black special case; that does not extend exactness to a multi-forward Bermudan price.

## Alternatives and tradeoffs

Using raw surface vol as base scale omits R; multiplying by S/B alone matches instantaneous absolute volatility but misses finite-expiry convexity. Both are explicitly avoided. Multi-expiry fitting, correlation fitting or Monte Carlo premium fitting could broaden the target but introduce additional parameters, simulation error and computational cost; they are not implemented here.

## Implementation and evidence

Owned source: [lmm.rs](../../../../finstack-quant/calibration/src/lmm.rs). Caller structure/pricing: [lmm_pricer.rs](../../../../finstack-quant/valuations/rates/src/swaption/lmm_pricer.rs); process dynamics: [LMM model](../models-monte-carlo/10-lmm.md). Inline tests cover rolled exercise selection, wrong contracts/JSON, surface-free pricing after override, frozen-formula round trips and finite-expiry premium preservation. `public_helper_matches_exact_one_forward_displaced_black_premium` includes an analytical numerical ATM premium and distinguishes it from the shared-kernel round trip; provenance beyond that in-code assertion was not requalified. The small-path pricing test checks consumption, not statistical calibration accuracy. No tests or simulations were run.

At the audit revision the module cited chapter 8 of the 2002 book and linked
to [Rebonato's 2004 book](../../../REFERENCES.md#rebonato-2004-volatility-correlation).
The accepted attribution above supplies the corrected chapter and book.
See also [Andersen–Piterbarg](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling).

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
