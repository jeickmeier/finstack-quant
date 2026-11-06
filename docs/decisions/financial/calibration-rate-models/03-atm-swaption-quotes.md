# CALIBRATION-RATE-MODELS-003: ATM swaption schedules, market targets and exact quote reconstruction

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

HW swaption calibration fits ATM payer premiums, with either Normal or unshifted Black quotation. The market target is derived from the initial curve and selected schedule; the final report converts model premiums back into each original quote convention. Source-documented rationale is to separate actual quote-fit error from the optimizer's approximate vega scaling.

## Domain and schedule validation

Expiry, tenor and quoted volatility must be finite/positive. At least two observations are required for two parameters; repeated expiry/tenor observations are retained with indexed residual keys and count toward this minimum. The minimum is not an identification/rank test.

For a supplied schedule, fixed payment times must be finite, strictly increasing and after the swap start; positive finite accruals align with them. Swap start is at/after expiry and maturity after start. Floating periods are contiguous from swap start to maturity, with positive accrual, `expiry ≤ fixing ≤ start < end`, and payment at/after start. Compounded coupons also require payment at/after end. Validation does not separately equate the quote's tenor with schedule maturity minus start or require fixed payments to coincide with accrual ends. The engine constructs those dates; see its [adapter](../calibration-framework/18-hull-white-market-adapter.md).

## Algorithm

1. For contractual schedules, fixed annuity `A=Σ α_i D(payment_i)`. The initial par target is `F=[Σ (D(start_j)/D(end_j)−1)D(payment_j)]/A` when `A>1e-15`, otherwise zero. This initial-curve convention is used for both simple and compounded schedules; the parameter-dependent delayed-coupon correction belongs to the model payoff, not this target forward.
2. Without a schedule, choose `n=max(round(tenor×payments_per_year),1)`, `δ=tenor/n`, and fixed payments at `expiry+iδ`. Then `A=ΣδD(t_i)` and `F=[D(expiry)−D(expiry+tenor)]/A`. If `A≤1e-15`, the helper substitutes a nonnegative logarithmic endpoint rate using `1e-12` factor and `1e-8` tenor floors. Later market-vega/model-price checks still apply.
3. Set strike to F. Market premium is `A×BachelierCall(F,F,σ_N,T)` or `A×BlackCall(F,F,σ_B,T)`. Compute matching ATM market vega once, and reject nonfinite or `<1e-8` vega. No Normal/Black conversion is inferred from rate signs.
4. Prepare either contractual payoff data or synthetic fixed-bond cashflows once for all optimizer starts. Their model prices use [Gaussian integration](05-contractual-gaussian-integration.md) or [Jamshidian decomposition](04-jamshidian-decomposition.md), respectively.
5. At fitted parameters, require every model premium finite/nonnegative. Normal implied volatility is `(P/A)√(2π/T)`. Black implied volatility is `2 Φ⁻¹((1+P/(AF))/2)/√T`. Reject nonfinite/negative implied results. Store signed error `σ_implied−σ_market` under `index:expiryYxtenorY`.

## Selection, conventions and numerical controls

The direct API supports mixed Normal/Black observations in one basket. Both residual weighting and the single final tolerance use each observation's own units: absolute rate volatility versus dimensionless relative volatility. They are not economically identical units merely because each is called a volatility error. Market prices and annuities are per unit notional on ACT/365F model time, with contractual coupon accruals separate.

The engine enforces supplied strikes within `1e-8` of its curve par rate before creating this strike-free quote. The direct public type cannot represent an off-ATM strike. Frequency is used only for synthetic schedules; contractual schedules replace it.

## Limitations, failure and alternatives

Exact ATM inversion removes the local Taylor approximation from final report residuals, but does not certify model accuracy, calibration identification or out-of-sample smile fit. A Black premium outside its admissible range fails inverse reconstruction. The low-annuity forward fallback is observed helper behavior, not approval of a degenerate swap. An off-ATM objective, per-convention normalized basket or common price/PV tolerance would be distinct choices; none is selected automatically.

## Implementation and evidence

[swaption.rs](../../../../finstack-quant/calibration/src/hull_white/swaption.rs), [quotes.rs](../../../../finstack-quant/calibration/src/hull_white/quotes.rs), and [targets.rs](../../../../finstack-quant/calibration/src/hull_white/targets.rs). Inline timing tests compare contractual annuities/forwards with explicit payment sums and reject malformed schedules. [tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) contains repeated-observation, frequency-sensitivity and parameter-recovery cases. These are internal formulas/round trips, not independent calibration-market validation. Formula dependencies are documented under [forward option kernels](../models-analytic/forward-normal-shifted.md).

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
