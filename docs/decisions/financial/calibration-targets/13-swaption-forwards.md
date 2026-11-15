# CALIBRATION-TARGETS-013 — Contractual swaption coordinates, schedules and calibration forwards

- **Decision status:** Accepted 2026-11-15 — complete dated swaption coordinate and calibration forward package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent tenor, fixing, payment-lag and synthetic-date calculations verified. Production fitting, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for dated swaption coordinates and calibration forwards

1. **Resolve one exact dated reference swap for each quote.** Reuse the
   checked European swaption and IRS preparation for a single-currency,
   constant-positive-notional fixed-for-floating swap without principal
   exchange. Require authored terms or an explicitly selected complete
   versioned convention. Resolve both legs' actual calendars, stubs,
   rolls, accruals, benchmark observations and payments. Preserve the
   quote ID, currency, payer or receiver side, exact strike and settlement
   profile. Exercise must be no later than either leg's first actual
   effective start. Quotes in one smile must share a qualified reference basis.
   [Accepted European swaption preparation](../valuations-rates/07-european-swaptions.md#accepted-decision-for-checked-european-swaption-pricing-and-risk).

2. **Keep expiry, nominal tenor and dated cashflows separate.** Compute
   positive expiry from the actual exercise date and declared quote-volatility
   clock. Preserve the exercise cut, model and curve clocks, and coupon day
   counts separately. Validate standard nominal tenor on the unadjusted
   contractual dates and roll rule. Use canonical months divided by 12 when
   the source specifies that mapping. Require an explicit qualified mapping
   for bespoke terms. Reject incompatible references that collide at a
   published coordinate. Retire the month-difference heuristic.
   [Accepted clocks and fixing eligibility](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility).

3. **Use contractual observations and explicitly bound curve roles.** Use
   the eligible published term fixing or project an unpublished fixing on
   its benchmark value dates and day count. Apply the separate contractual
   coupon accrual. For overnight coupons use the elected observation rule,
   exact weights, eligible fixings and compounded growth, with contractual
   spread applied outside compounding. A missing required published fixing
   errors. Preserve the publication cut and fixing authority. Resolve only
   active inputs and bind the typed discount and active projection roles
   explicitly. A shared source requires explicit qualification.
   [Accepted swap cashflows](../valuations-rates/03-interest-rate-swaps.md#accepted-decision-for-checked-interest-rate-swap-cashflows-and-risk) ·
   [Accepted swap preparation](../calibration-framework/06-swap-materialization.md#accepted-decision-for-term-and-overnight-swap-calibration).

4. **Compute par from eligible dated cashflows.** On one cut compute the
   per-unit curve annuity `A = Σᵢ αᵢ D(V,pᵢ)` and signed forward
   `F = PV_receive_float / (N A)` before applying payer or receiver
   direction. Discount actual payment dates using checked factors relative
   to valuation. Preserve contractual cash rounding and require a positive
   representable annuity without a magnitude floor. Allow telescoping only
   where the actual qualified shared source, observations, accruals, spread,
   payments and eligibility prove equivalence. On an explicitly qualified
   zero-spread quote mapping, subtract the same dated spread PV divided
   by `N A` from both forward and strike. Preserve the contractual values
   for settlement. The product owner qualifies the settlement annuity and
   any stochastic pricing or delayed-payment convexity treatment.
   [Accepted forward and settlement rules](../valuations-rates/07-european-swaptions.md#accepted-decision-for-checked-european-swaption-pricing-and-risk).

5. **Require a dated reference at each node needing a construction forward.**
   Reuse a fitted node's result only when the complete reference basis
   matches. An unquoted node requires an actual exercise date and either an
   authored swap or an explicitly selected dated convention and nominal
   term. Run the same checked evaluator and verify its coordinates against
   the node request. Retire index-default synthetic schedules and the
   reconstruction of dates by rounding numeric expiry or tenor.

6. **Preserve construction provenance and model eligibility.** Retain the
   resolved reference terms, schedules, observation and information cut,
   clocks, curve and fixing roles, annuity and forward in the fitted-state
   provenance. Keep the construction and live pricing forwards distinct.
   SABR fitting, bucketing, interpolation and cube acceptance belong to
   [accepted record 014](14-swaption-cube.md#accepted-decision-for-exact-sabr-swaption-smiles-and-strict-cube-publication).
   Hull–White retains its qualified physical payer ATM profile,
   expressly shared curve, separate ACT/365F model clock and
   narrower coupon eligibility.
   [Accepted SABR source rules](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning) ·
   [Accepted Hull–White adapter](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

## Independent calculation checks

- January 28 to February 28, 2026 has a nominal tenor of one month on
  roll day 28. Following adjustment on a weekdays calendar moves the
  maturity to March 2.
- A term benchmark accrual of `.25` and projection ratio `1.0025`
  yield a 1% benchmark. With coupon accrual `1/6`, spread `.001`,
  floating payment factor `.97` and fixed annuity `.5 × .96`,
  par is about 37.048611 bp. An eligible 1.2% fixing changes par to
  about 43.784722 bp.
- With fixed accrual `.5`, start and accrual-end factors `.99`
  and `.98`, and a floating payment factor of `.97`, the dated
  par forward is `97/4802`. Endpoint telescoping gives `1/49`,
  about 2.082466 bp higher.
- February 3 and 10, 2026 are 29 and 36 days after January 5. Both
  ACT/365F coordinates round to one month in the observed reconstruction,
  producing February 5. A two-business-day start lag on the actual
  exercise dates gives February 5 and February 12 on a weekdays calendar.

## Observed choice and conventions at the audit revision

Quoted swaption smiles are indexed by exact ACT/365F option expiry and contractual swap tenor derived from settlement-adjusted underlying start and explicit maturity. Calibration forwards are schedule-weighted par rates; fixed-leg day count is independent from the option-variance clock. These helpers also supply schedules to Hull–White adapters, whose payoff integration and rejection policies are separately owned.

## Observed algorithm

1. Resolve quote SwaptionConventionId, then floating rate-index conventions. Currency comes from floating index; fixed frequency/day count/BDC/calendar/settlement from swaption convention; floating frequency/day count/BDC/reset lag/compounding and both legs' payment lags come from index defaults. Parameter fixed-day-count/calendar overrides replace only those two fields. Quote currency must equal step currency.
2. Add settlement business days and fixed BDC to option expiry to obtain start; maturity must be after it. Use exact quote expiry in ACT/365F and `contractual_swap_tenor_years(start,maturity)` for tenor. These are model coordinates; rounded keys are only a bucket lookup mechanism [014](14-swaption-cube.md).
3. Build fixed and floating schedules with ShortBack stub, no EOM, raw accrual endpoints, no roll rule, leg BDC/day count/frequency/payment lag; floating reset lag is stored on periods. The annuity is `A=sum(alpha_fixed*D(payment))`, with D measured from discount-curve origin, and must be finite above `1e-16`.
4. Floating coupon growth is either `forward.rate_between_dates(start,end,alpha_float)*alpha_float`, or, without projection curve, `1/D(start,end)-1`. Discount each growth at its actual payment date. Return `Fswap=sum(growth_i*D(payment_i))/A`. Common discount-origin scale cancels. Single-curve telescoping to `D(start)-D(end)` applies only when coupon accrual-end and payment dates align; the implementation keeps individual payment discounting.
5. For **interpolated target** cube nodes without an exact calibrated bucket, require `params.swap_index`; use its default fixed/floating conventions plus overrides. Convert numeric expiry/tenor to rounded whole months, reject values outside integer month range, add expiry months to base, settle, then add tenor months to start to calculate a synthetic forward. Published node coordinates still retain requested numeric values.

## Observed limitations and alternatives

`float_compounding` is resolved but not consulted by this target's forward calculation. Constructed reset dates likewise do not select fixings; all coupon projection uses accrual endpoints/DF-implied growth. Consequently a registry lookback/observation-shift/cutoff convention is not fully represented by this par-forward helper. Hull–White adapter explicitly rejects nonzero lookback/shift/cutoff; this SABR cube target has no equivalent check. This is a static convention-application gap; numerical effect requires a configured convention/contract comparison.

Quoted nodes use quote-specific swap conventions, while interpolated nodes use index defaults and month rounding. Those can differ materially near stubs/settlement/holiday boundaries. Alternatives are reusing the full IRS cashflow projector, requiring explicit contracts at every node, or publishing both grid model coordinates and exact reference dates. Explicit projected forwards need not include a stochastic delayed-payment convexity correction; that is a separate pricing model decision.

## Evidence

- Owning [swaption.rs](../../../../finstack-quant/calibration/src/targets/swaption.rs), especially convention/date/schedule and par-rate helpers, inspected beyond the early test-only import.
- Inspected single-curve telescoping test and multicurve test compare DF-implied coupon growth against the rejected integral-average alternative. Located settlement-date, payment-lag and fixed-day-count-independent-expiry cases in [swaption tests](../../../../finstack-quant/calibration/tests/calibration/swaption_vol.rs). Same-library schedule generation limits independence.
- [European swaption methodology](../valuations-rates/07-european-swaptions.md) and [framework Hull–White adapter](../calibration-framework/18-hull-white-market-adapter.md) document downstream meanings.

No test run was performed.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
requires parameter-cube forwards and their financial reference association to
remain explicit, including actual contracts, dates/conventions and source clock.
Stored construction forwards are distinct from live pricing forwards; synthetic
interpolated-node construction and frozen/rebased smile policies require their
owners' qualification. A shared ID or tenor coordinate is insufficient to
qualify the reference basis. The accepted dated preparation above supplies
this producer's reference binding. Core source and model evaluation remain
with their owners.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
