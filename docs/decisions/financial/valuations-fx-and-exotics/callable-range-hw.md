# VALUATIONS-FX-AND-EXOTICS-CALLABLE-RANGE-HW: Callable rate range-accrual HW1F projection and issuer LSMC

- Record: `VALUATIONS-FX-AND-EXOTICS-CALLABLE-RANGE-HW`
- Decision status: **Accepted 2026-11-15** — complete callable rate range-accrual contract, Hull–White projection, issuer policy, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent cash, issuer stopping and delayed-start Gaussian reference calculations checked. Production pricing, risk and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for callable rate range accrual

1. **Book one conditional coupon and one principal redemption.** Require
   explicit long or short, positive face amount `N` in note currency,
   finite nonnegative annual coupon rate `c` and a nonempty exact
   equally weighted fixing schedule of `n` slots. After `k` in-range
   slots, earned cash is `N*c*alpha*k/n`, with the qualified contractual
   accrual fraction from start through the last scheduled observation.
   Keep the full denominator on call. Use finite strictly ordered bounds,
   with both endpoints inclusive. Signed absolute bounds are allowed.
   Relative bounds resolve once from booked multiples of a positive
   qualified inception rate. Keep the effective band fixed in every
   reprice. Require each eligible fixing or a certified tally bound to
   the exact completed prefix, benchmark, band, authorized version and
   publication cut.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [ICE range-accrual conventions](https://idd.ice.com/IRHelp/Content/FM/Range_Accrual_Note.htm).

2. **Project the dated contractual index period on one curve.** Bind the
   index, currency, fixing `f`, value start `S`, end `E` and positive
   index fraction `delta` to the same qualified curve for both roles.
   Use `expm1(log P(f,S)-log P(f,E))/delta` with checked affine bonds.
   Keep model ACT/365F from valuation, native curve clock, index day
   count and coupon day count separate. Rebase on the source curve's
   admitted native coordinates. Require coverage through the needed
   index ends and payment dates, even when an index end follows the
   final cash or simulation event.
   [Accepted term projection](../models-analytic/rate-clock-and-projection.md).

3. **Pay actual cash at the selected branch's date.** Admit strictly
   ordered supplied exercise dates and finite positive percent-of-par
   call price. Admit dates strictly after the lockout and within the
   qualified live term. When events share a date, apply the contract's
   authorized fixing, issuer decision and payment in that order.
   A live exercise pays immediate `N*call_price_pct/100` plus earned
   coupon and extinguishes later obligations. Continuation retains actual
   later policy cash or maturity par plus final earned coupon. Resolve
   a maturity-date call directly only when the decision is explicitly
   before payment. Pay common cash once. There is no synthetic retained
   terminal accrual or conditional bond adjustment. Require authoritative
   lifecycle state for elapsed decisions. This live profile covers
   same-node exercise settlement. Notice and delayed live exercise
   settlement require a separate qualified model.
   [Shared contract controls](../valuations-framework/shared-contract-controls.md),
   [Accepted issuer-call rules](../models-analytic/hw1f-issuer-lsmc.md).

4. **Select scalar Hull–White parameters and cash discounting explicitly.**
   Use a complete finite `kappa,sigma >= 0` pair or an expressly
   selected, provenance-matched cap/floor fit supplying the pair. Fail
   incomplete, inconsistent or unsupported parameter selections. Use
   exact joint centered-rate and log-bank steps at the required event
   nodes. Discount cash at actual payment, with no deterministic fallback
   for missing simulated path state. Use deterministic rollback for zero
   volatility, plain simulation for stochastic cash with no unresolved
   call, and direct dated discounting for fixed unpaid cash. A resolved
   cash-only or discharged state avoids inactive model inputs and reports
   zero simulated work.
   [Hull and White](https://doi.org/10.1093/rfs/3.4.573),
   [Accepted exact path and discount rules](../models-analytic/hw1f-monte-carlo-discounting.md).

5. **Fit continuation from the rate and earned count.** Fit later
   policy's realized continue-exclusive cash discounted by the path to
   each decision in decision-date units. Use the basis
   `[1,x,x^2,h,x*h]`, with centered factor `x` and `h=k/n`. Omit
   count features only when count is structurally fixed or irrelevant.
   Use the accepted common-state intercept when the decision state is
   structurally common. Apply the accepted scaled full-rank fit and
   sparse-fit errors on live regressions. Call only when the actual
   immediate holder cash is strictly below fitted continuation. Continue
   at equality. Apply position sign after the issuer's holder-view
   minimization.
   [Longstaff and Schwartz](https://people.math.ethz.ch/~hjfurrer/teaching/LongstaffSchwartzAmericanOptionsLeastSquareMonteCarlo.pdf),
   [Accepted issuer policy](../models-analytic/hw1f-issuer-lsmc.md).

6. **Report independent frozen-policy pricing.** Require explicit seed,
   pairing and group counts. Train backward, freeze the policy and replay
   forward on disjoint pricing groups, keeping antithetic partners
   together. Use at least two pricing groups and report actual work,
   pricing-group standard error and the 95% Student-t sampling interval.
   The population holder PV of the fitted admissible policy is at least
   the issuer-optimal holder PV in the same model. A finite estimate can
   lie on either side. The interval measures sampling uncertainty
   conditional on the fitted policy.
   [Accepted sampling and policy rules](../models-analytic/hw1f-issuer-lsmc.md).

7. **Compute signed rate risk with a refitted policy.** For parallel and
   triangular bucketed DV01 shock the one actual curve once for both
   roles. Hold scalar parameters, contractual band and authorized
   history fixed. Rebuild the curve fit and refit each shocked policy
   using matched training and pricing draws across shocks and disjoint
   draws within each run. Report signed native currency per positive bp
   from actual represented widths and paired sampling uncertainty.
   Keep the registered rate metrics. This scalar model has no surface vega.
   [Accepted rate risk](../valuations-framework/finite-difference-rates.md).

8. **Export resolved signed cash and reuse Rust validation.** Export
   known coupon as `Floating`, early redemption as `PrePayment` and
   maturity principal as `Notional`. Carry contractual retired par and
   its date separately from the premium- or discount-bearing cash. Retain
   unpaid residuals with actual eligible payment dates through discharge.
   Keep recognized pending, paid and overdue claims explicit. Use the
   validated Rust terms and state in the existing Python and WASM JSON
   routes and generated schemas.
   [Cashflow semantics](../core-conventions/015-cashflow-semantics.md),
   [Dated discounting](../core-conventions/016-discounting.md).

## Independent calculations

- USD 1 million face, an 8% annual coupon, a 0.5 coupon accrual fraction
  and four slots give USD 10,000 per hit. At a 99.98% call price, with
  one future continuation hit and a 0.99 factor to maturity, the
  immediate/continuation pairs are USD 999,800/999,900 at zero earned
  hits, 1,009,800/1,009,800 at one and 1,019,800/1,019,700 at two.
  The issuer calls only in the first state, even at the same short rate.
- For a flat 3% continuous curve and face 100, use valuation 2026-01-02,
  fixing 2026-06-30, ACT/360 index period 2026-07-02 to 2026-10-02,
  payment 2026-08-31, `kappa=.05`, `sigma=.02`, a closed 2.5%–3.5%
  band and cash bonus 3 on the sole hit. The dated inception forward
  is 2.9701194501%. Gaussian discount tilting gives an in-band
  probability of 0.2855728932 under the payment measure and a no-call
  PV of 98.8785830414. Direct integration agrees. The index end is
  beyond the final payment date.

## Observed implementation and scope

Callable rate range-accrual notes use a curve-fitted one-factor Gaussian short-rate process, term-forward observations and issuer exercise. Principal is part of this note, unlike the standalone asset coupon-only range accrual.

The following sections describe the audited implementation and qualification
limits. The accepted contract above governs reconciliation.

## Algorithm and conventions

Require explicit index/forward curve/tenor, and forward ID equal to discount ID: projection is single-curve. Build a union of future observation, eligible call and final payment dates. At each observation compute a simple tenor forward from HW affine bond coefficients `L=(1/P(t,t+τ)-1)/τ`; inclusive lower≤L≤upper increments count. Observation occurs before same-day exercise snapshot.

Coupon is `N×coupon_rate×accrual_year_fraction×(past+simulated in-range)/(past+future observations)`. At final payment path value includes coupon plus principal, discounted by reciprocal simulated bank account; deterministic DF is the fallback when the bank marker is absent. Exercise stores in-range count and the HW bond price to final payment. The local intrinsic is `call_price_fraction×N+accrued_coupon×(1-P(call,final))`; value-after subtracts accrued coupon discounted at final payment, preserving its settlement treatment during issuer exercise comparisons.

Shared LSMC regresses continuation using `ShortRateBasis`; degree-two row is `[1,r,r²,t r]`. The local basis omits accrued-count state even though the note payoff tracks it. Default shared config is 10,000 independent estimator streams, antithetic true (twice the raw simulated paths), four minimum substeps between events, degree two and in-sample LSMC (`oos_lsmc=false`). A custom pricer config can enable split-sample estimation. Instrument path/scenario overrides replace count/derive seed. The short-rate process is prepared from the discount curve over the event horizon; initial state comes from its short rate, while the scalar `spot_id` supplies the reference rate for relative bounds.

DV01 and bucketed DV01 reprice combined curve exposures. Shared diagnostics attach estimate/error information. No dedicated local volatility-risk metric is registered.

## Selection and controls

Default `MonteCarloHullWhite1F` invokes shared `Hw1fNotePricer`. No-call schedules use plain HW MC; eligible future calls use HW LSMC. Parameters resolve in order: complete instrument kappa/sigma pair, explicit pricer pair, then complete pre-fitted CapFloor-family market scalars keyed by discount curve. No volatility-surface fitting occurs during pricing.

Scalar kappa/sigma are strictly positive; partial overrides/calibrated pairs and supplied sigma schedules error. The optional surface field is not an on-demand fit source. Calls after final payment error; historical observation totals must match elapsed dates. Call dates on/before valuation are excluded from future exercise.

## Limitations, failure and fallback

The regression basis omits accumulated in-range count; residual coupon subtraction alone does not establish Markov sufficiency for issuer decisions. Single-curve projection and in-sample policy estimation are observed defaults. Past issuer-call/redemption state is not represented by merely excluding past call dates.

## Alternatives and tradeoffs

A basis including accrued count, split-sample policy evaluation and a tree with augmented coupon state could improve exercise assessment at additional cost. Multi-curve basis-adjusted projection is not implemented here (unlike snowball/TARN additive deterministic basis).

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/types.rs)
- [finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs](../../../../finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs)

Delegated implementation dependencies:

- [finstack-quant/valuations/core/src/instruments/pricing/hw1f_params.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/hw1f_params.rs)
- [finstack-quant/models/src/rates/hw1f/config.rs](../../../../finstack-quant/models/src/rates/hw1f/config.rs)
- [finstack-quant/models/src/rates/hw1f/exercise.rs](../../../../finstack-quant/models/src/rates/hw1f/exercise.rs)
- [finstack-quant/models/data/defaults/pricer_defaults.v1.json](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json)

- [finstack-quant/valuations/exotics/tests/exotic_harness/hw1f_curve_repricing.rs](../../../../finstack-quant/valuations/exotics/tests/exotic_harness/hw1f_curve_repricing.rs): Curve-repricing and simulated bank-account discount tests; tests process consistency, not independent callable-note exercise accuracy.

Canonical reference index: [Hull White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives) and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). Production qualification remains pending.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
