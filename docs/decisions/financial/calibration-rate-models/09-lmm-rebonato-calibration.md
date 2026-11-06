# CALIBRATION-RATE-MODELS-009: LMM loading scale from frozen basket weights and displaced ATM premium inversion

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

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

Existing background references: [Rebonato catalogue entry](../../../REFERENCES.md#rebonato-2004-volatility-correlation) and [Andersen–Piterbarg](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling). The module cites a 2002 book/chapter while linking the catalogue's different 2004 title; the inspected citation does not establish exact bibliographic attribution.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
