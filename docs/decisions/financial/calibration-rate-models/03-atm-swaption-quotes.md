# CALIBRATION-RATE-MODELS-003: ATM swaption schedules, market targets and exact quote reconstruction

**Decision status:** Accepted 2026-11-15 — complete ATM swaption preparation, market target and native quote reconstruction package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent lagged-forward, synthetic-grid and Black inverse boundary calculations verified. Production fitting, market references, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for exact ATM swaption targets and native quote reconstruction

1. **Prepare one exact dated physical payer contract.** Bind
   the expressly shared curve evaluator and exact exercise cut,
   nominal tenor, actual leg schedules, fixing and benchmark
   accruals, and payment dates. Validate nominal tenor against
   the contractual roll. Use an eligible known pre-exercise
   fixing as literal cash where the payoff model admits it.
   Reject unresolved pre-exercise resets. Admit qualified term
   coupons, exact overnight telescoping on its eligible support,
   or the explicitly elected continuous overnight approximation.
   [Accepted dated calibration](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication) ·
   [Accepted swaption payoff rules](../valuations-rates/08-hull-white-swaptions.md#accepted-decision-for-checked-hull-white-swaption-exercise-and-risk).

2. **Compute ATM from eligible dated cash.** Form the positive
   representable fixed annuity `A = Σᵢ αᵢ D(V,pᵢ)` per unit
   notional and the signed par rate `F = PV_receive_float/(N A)`.
   Use actual payment dates and distinct coupon and benchmark
   accruals. Price at the computed ATM strike and verify any
   authored strike with an explicit decimal-rate tolerance.
   Retire tiny-annuity thresholds and fallback rates. Keep
   parameter-dependent payment covariance in the model payoff.
   [Accepted dated forwards](../calibration-targets/13-swaption-forwards.md#accepted-decision-for-dated-swaption-coordinates-and-calibration-forwards).

3. **Use an explicitly selected exact synthetic grid.** Require the
   elected frequency `m` and checked whole period count `n`,
   with tenor exactly represented by `n/m`. Set exercise equal to
   swap start, use accruals `1/m` and pay at `e+i/m`. Use
   shared-curve telescoping on this unlagged numeric grid. Retire
   rounded counts and redistributed accruals. Dated fits use
   contractual Gaussian integration. Synthetic fits use Jamshidian
   within its qualified domain.
   [Accepted synthetic payoff](04-jamshidian-decomposition.md#accepted-decision-for-checked-synthetic-jamshidian-decomposition) ·
   [Accepted contractual payoff](05-contractual-gaussian-integration.md#accepted-decision-for-checked-contractual-gaussian-swaption-integration).

4. **Freeze each ATM market premium on its quote clock.** For
   positive quote expiry `Tq`, use `A σN sqrt(Tq/(2π))` in
   absolute annual Normal volatility, admitting signed forwards.
   For unshifted Black require `F > 0` and use
   `A F erf(σB sqrt(Tq)/(2 sqrt(2)))` in relative annual
   volatility. Model repricing uses the separate ACT/365F
   model clock. Evaluate same-law vega only where the elected
   objective needs it. Retire the fixed `1e-8` vega floor.
   [Henrard, Swaption Pricing](https://quant.opengamma.io/Swaption-Pricing-OpenGamma.pdf).

5. **Keep the checked analytic ATM inverse.** For Normal use the
   scaled equivalent of `P sqrt(2π)/(A sqrt(Tq))`. For Black
   enforce the exclusive upper premium `A F`, use the direct
   premium ratio near zero and preserve the complementary
   shortfall `A F − P` near the upper bound. Apply the inverse
   error function or its complementary form with checked scaling.
   Exact zero premium selects zero after full admission. A
   positive interior premium requires a finite positive result
   or a typed precision failure.
   [NIST inverse error functions](https://dlmf.nist.gov/7.17).

6. **Reprice the actual implied volatility on the frozen ATM basis.**
   Require premium agreement with explicit nonnegative absolute
   and relative tolerances, at least one positive. At exact ATM,
   gross premium and time value coincide. Independently gate
   the implied-minus-observed volatility in its original units.
   [Accepted premium acceptance principles](../models-analytic/implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance).

7. **Gate every quote before ranking fits.** Preserve exact quote
   IDs, authored order and positive native fit tolerances,
   including zero-weight rows. Admit mixed Normal and Black
   quotes under the qualified objective owned by record 006.
   Use tolerance ratios for mixed-law fit summaries. Preserve
   the actual method, work and typed failures. Failed fits may
   retain diagnostics.
   [Accepted multi-start selection](../calibration-framework/13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection) ·
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

## Independent calculation checks

- With start, end and floating payment factors `.99`, `.98`
  and `.97`, and a fixed accrual `.5` paid at the end,
  the dated par forward is `97/4802`. Endpoint telescoping
  gives `1/49`, about 2.082466 bp higher.
- A synthetic 1.10-year quarterly tenor is outside the
  regular grid. The observed rounded count gives four
  periods with accrual `.275`.
- At `A F = .125` and `Tq = 1`, Black volatility `1e-18`
  gives premium about `4.986779e-20`. The observed inverse-CDF
  argument rounds to `.5`. At the representable premium
  immediately below `.125`, the same argument rounds to one.
  Inverting the retained relative shortfall `2^-53`
  gives finite volatility about `16.58472215`.

## Observed choice and rationale at the audit revision

HW swaption calibration fits ATM payer premiums, with either Normal or unshifted Black quotation. The market target is derived from the initial curve and selected schedule; the final report converts model premiums back into each original quote convention. Source-documented rationale is to separate actual quote-fit error from the optimizer's approximate vega scaling.

## Observed domain and schedule validation

Expiry, tenor and quoted volatility must be finite/positive. At least two observations are required for two parameters; repeated expiry/tenor observations are retained with indexed residual keys and count toward this minimum. The minimum is not an identification/rank test.

For a supplied schedule, fixed payment times must be finite, strictly increasing and after the swap start; positive finite accruals align with them. Swap start is at/after expiry and maturity after start. Floating periods are contiguous from swap start to maturity, with positive accrual, `expiry ≤ fixing ≤ start < end`, and payment at/after start. Compounded coupons also require payment at/after end. Validation does not separately equate the quote's tenor with schedule maturity minus start or require fixed payments to coincide with accrual ends. The engine constructs those dates; see its [adapter](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

## Observed algorithm

1. For contractual schedules, fixed annuity `A=Σ α_i D(payment_i)`. The initial par target is `F=[Σ (D(start_j)/D(end_j)−1)D(payment_j)]/A` when `A>1e-15`, otherwise zero. This initial-curve convention is used for both simple and compounded schedules; the parameter-dependent delayed-coupon correction belongs to the model payoff, not this target forward.
2. Without a schedule, choose `n=max(round(tenor×payments_per_year),1)`, `δ=tenor/n`, and fixed payments at `expiry+iδ`. Then `A=ΣδD(t_i)` and `F=[D(expiry)−D(expiry+tenor)]/A`. If `A≤1e-15`, the helper substitutes a nonnegative logarithmic endpoint rate using `1e-12` factor and `1e-8` tenor floors. Later market-vega/model-price checks still apply.
3. Set strike to F. Market premium is `A×BachelierCall(F,F,σ_N,T)` or `A×BlackCall(F,F,σ_B,T)`. Compute matching ATM market vega once, and reject nonfinite or `<1e-8` vega. No Normal/Black conversion is inferred from rate signs.
4. Prepare either contractual payoff data or synthetic fixed-bond cashflows once for all optimizer starts. Their model prices use [Gaussian integration](05-contractual-gaussian-integration.md) or [Jamshidian decomposition](04-jamshidian-decomposition.md), respectively.
5. At fitted parameters, require every model premium finite/nonnegative. Normal implied volatility is `(P/A)√(2π/T)`. Black implied volatility is `2 Φ⁻¹((1+P/(AF))/2)/√T`. Reject nonfinite/negative implied results. Store signed error `σ_implied−σ_market` under `index:expiryYxtenorY`.

## Observed selection, conventions and numerical controls

The direct API supports mixed Normal/Black observations in one basket. Both residual weighting and the single final tolerance use each observation's own units: absolute rate volatility versus dimensionless relative volatility. They are not economically identical units merely because each is called a volatility error. Market prices and annuities are per unit notional on ACT/365F model time, with contractual coupon accruals separate.

The engine enforces supplied strikes within `1e-8` of its curve par rate before creating this strike-free quote. The direct public type cannot represent an off-ATM strike. Frequency is used only for synthetic schedules; contractual schedules replace it.

## Observed limitations, failure and alternatives

Exact ATM inversion removes the local Taylor approximation from final report residuals, but does not certify model accuracy, calibration identification or out-of-sample smile fit. A Black premium outside its admissible range fails inverse reconstruction. The low-annuity forward fallback is observed helper behavior, not approval of a degenerate swap. An off-ATM objective, per-convention normalized basket or common price/PV tolerance would be distinct choices; none is selected automatically.

## Implementation and evidence

[swaption.rs](../../../../finstack-quant/calibration/src/hull_white/swaption.rs), [quotes.rs](../../../../finstack-quant/calibration/src/hull_white/quotes.rs), and [targets.rs](../../../../finstack-quant/calibration/src/hull_white/targets.rs). Inline timing tests compare contractual annuities/forwards with explicit payment sums and reject malformed schedules. [tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) contains repeated-observation, frequency-sensitivity and parameter-recovery cases. These are internal formulas/round trips, not independent calibration-market validation. Formula dependencies are documented under [forward option kernels](../models-analytic/forward-normal-shifted.md).

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
