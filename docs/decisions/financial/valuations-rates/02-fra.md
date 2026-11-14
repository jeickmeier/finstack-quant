# VALUATIONS-RATES-02-FRA: FRA historical fixings, early settlement and quote risk

- ID: VALUATIONS-RATES-02-FRA
- Decision status: **Accepted — complete checked FRA fixing, settlement and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, calibration, reporting and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked FRA fixing, settlement and risk

1. **Select the supported contract and dates explicitly.** Use one
   single-index term FRA that pays the net amount at the effective accrual
   start. Require exact positive native-currency Money notional `N`, an
   exact signed annual Decimal fixed rate `K`, and explicit direction.
   Pay means pay fixed and receive floating, with `s=+1`. Receive means
   receive fixed and pay floating, with `s=−1`. Use authored terms or a
   deliberately selected qualified convention. Resolve ordered
   contractual accrual dates `S<E`, positive finite contract accrual
   `α` with its full day-count context, and payment date `S`. Keep
   payment and end of settlement exposure distinct from accrual maturity.
   The supported upfront election uses the selected floating rate and
   the contract accrual for simple settlement discounting. A different
   payment date or settlement convention, including end settlement,
   AFMA yield discounting or an independent settlement lag, requires
   separate product support. See [dated convention selection](../valuations-framework/convention-selection.md)
   and [ISDA rates disclosure](https://www.isda.org/a/ORiDE/isda-rates.pdf).

2. **Bind the exact benchmark observation.** Require the benchmark
   identity and currency, fixing date `Q≤S`, benchmark value dates
   `Sx<Ex` and positive finite index accrual `αx`. Use an explicit fixing
   date or derive it once using an explicit nonnegative business-day lag
   from the resolved start and the qualified fixing calendar. Positive
   lag steps backward excluding the start. Zero preserves the start.
   Apply a further fixing roll only when the selected rule calls for it,
   and validate the final fixing date. Remove the implicit Modified
   Following, forward-ID-to-index inference and magnitude-based lag
   guard. Preserve original terms, resolved dates and source provenance.
   Contract accrual, benchmark accrual and curve clocks each retain their
   meaning. Share dates or accruals only when the convention establishes
   their equality. See [term observations](../cashflows/06-floating-rate-projection.md)
   and [Strata Ibor observation](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/index/IborIndexObservation.html).

3. **Choose the rate by publication authority.** Use an explicit information
   phase and selected observation source and version. Once eligible, require
   the exact fixing for the benchmark, date, currency and annual-decimal
   quote. An explicitly supplied observation must qualify on the same
   basis. Unknown status, missing required observations, conflicting
   values or invalid inputs fail. A known unpublished observation may
   project before payment on the admitted forecast basis. Use the exact
   index-to-projection-source binding and
   `F=(Px(Sx)/Px(Ex)−1)/αx` from checked dated factors. Remove fallback
   between a naked supplied rate and a curve, and preserve the source
   error. This forecast substitutes the deterministic forward in the
   settlement formula. Futures or stochastic settlement convexity
   requires a separately qualified model. Report active inputs for the
   requested cut and operation.
   An eligible fixing does not activate the unused projection curve.
   See [information cutoffs](../conventions/0045-financial-clocks-and-date-boundaries.md),
   [fixing series](../core-market/scalar-series-and-fixings.md) and
   [projection factors](../core-market/forward-projection.md).

4. **Check one signed upfront settlement formula.** Use
   `C=s N α(F−K)/(1+Fα)` with the selected signed finite annual rate
   `F`. Require finite strictly positive `1+Fα`, independently of
   benchmark projection growth. Admit negative rates within that domain.
   The denominator constraint applies to `F`; the fixed quote must be
   representable and comply with the selected contract. Replace the
   near-zero floor, accrual clamp and generic rate warning range with
   checked, range-safe arithmetic. Preserve signed representable cash
   and exact par zero; a lost required nonzero result fails. The raw
   valuation discounts the checked raw settlement before any final
   Money restoration. Publish one signed net index-dependent interest
   row at `S` using the existing Floating kind, with no principal
   exchange or delta. Keep the actual fixing or projected provenance,
   accrual context and both rates in the owning result where a single
   row rate cannot describe them. See
   [Strata upfront FRA formula](https://raw.githubusercontent.com/OpenGamma/Strata/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/fra/DiscountingFraProductPricer.java)
   and [cashflow semantics](../core-conventions/015-cashflow-semantics.md).

5. **Choose settlement cut and par applicability together.** Before
   `S`, multiply eligible nonzero raw cash by the exact native discount
   source's positive dated factor `D(V,S)`. On `S`, the ordinary
   post-settlement holder view is zero. An explicit pre-payment same-day
   forecast can use a qualified unpublished projection at factor one.
   Label it as forecast. Actual same-day payable cash requires the eligible
   fixing and uses factor one without a discount lookup. Missing payable
   cash inputs produce an error. After `S` there is no residual under
   this ordinary settlement profile. Validate contractual terms before shortcuts and
   skip unused market inputs for a settled view. Historical cash or
   overdue claims require their own evidence and lifecycle. Report
   `ParRate=F` only for the selected outstanding settlement basis,
   including a qualified same-day forecast or payable view. Once settled
   there is no unique par rate. Historical observations remain separately
   addressable. Reconcile the published settlement cash with the admitted
   raw basis under the Money precision rules. See
   [dated NPV](../core-conventions/016-discounting.md) and
   [cashflow valuation and export](../valuations-framework/cashflow-export.md).

6. **Give each rate sensitivity one basis.** Use signed native currency
   per positive bp and the checked represented central reprices from
   the accepted framework. Keep `dv01` for direct combined parallel
   rate risk, `forward_pv01` for direct projection-only parallel risk,
   and `bucketed_dv01` for direct triangular key-rate risk. Remove the
   redundant FRA `pv01` registration. Require explicit market-quote replay
   with exact quote scope, an admitted center and the authorized provider.
   Missing or unsupported replay fails on that basis. Reprice on the same
   cut, side, currency, fixed rate and publication state. A common text
   ID does not merge distinct discount and projection bindings. At par, an
   unfixed FRA can still have projection risk. A fixed unpaid amount has
   zero projection risk and can retain discount risk until payment.
   Return settled structural zero after admission without fetching unused
   market inputs, and require a qualified grid for requested bucket rows.
   See [shared rate risk](../valuations-framework/finite-difference-rates.md).

7. **Share resolved terms across pricing, calibration and hosts.** Make
   the same validated terms and provenance available through direct,
   registry, tagged instrument, restore, cashflow and risk routes.
   Quote materialization freezes the confirmed benchmark and contract
   periods and the convention edition. Retain accrual maturity as the
   reference end for a calibration profile that qualifies that pillar.
   Calibration uses the intended
   live projected basis with matched materialized type and checked raw
   PV on the calibration owner's residual. Reject a settled or fixed
   request as a live projection target. Keep solver, residual acceptance
   and weighting decisions with their owners. Use the existing Python
   and WASM instrument envelopes and calibration quote surfaces, with
   matched generated schemas and fixture updates. See
   [FRA quote materialization](../calibration-framework/04-deposit-fra-materialization.md)
   and [forward calibration](../calibration-targets/04-forward-calibration.md).

## Observed behavior

### Choice, scope and rationale

FRAs settle the discounted interest difference at accrual start, with no residual value on or after that start. Pay denotes pay fixed/receive floating. The forward-rate and discount curve are independent dependencies, subject to the observed-fixing selection caveat below.

### Algorithm and controls

1. Require ordered start/maturity, positive notional, finite observed fixing and reset lag magnitude no greater than 31. Explicit fixing date must be on/before start. Otherwise resolve calendar from the explicit fixing calendar or forward-ID convention registry, subtract reset-lag business days and apply the selected fixing BDC (default ModifiedFollowing); no calendar means error.
2. Compute accrual alpha from raw start/maturity and instrument day count; clamp at zero and reject below the local minimum period threshold.
3. If fixing is strictly before valuation, use explicit observed fixing first, otherwise exact fixing-series observation. If fixing is today/future, prefer a forward-curve period rate; only if that curve is unavailable use the supplied observed fixing. Thus an observed fixing is not an unconditional override before its date.
4. Before start, compute settlement `N*(F-K)*alpha/(1+F*alpha)`, positive for Pay, negative for Receive, and discount at the start payment date. Reject a denominator close to zero by absolute-value threshold; a substantially negative denominator is not separately rejected. Extreme forwards warn. Start-or-later returns an empty NoResidual schedule.
5. ParRate returns the selected forward/fixing. PV01 and ForwardPv01 both use forward-only curve bumps; BucketedDv01 uses generic triangular rate risk.
6. DV01 centrally bumps/replays calibration quotes only when both discount and forward carry recipes and the forward recipe has no Basis quote. Otherwise it uses generic combined curve bumps. Replay errors after eligibility selection propagate; this is not a catch-all numerical fallback.

### Conventions, limitations and alternatives

Fixed rate and observed rates are decimal; reset lag is business days; quote-risk bumps are bp. Futures convexity is not applied to an FRA. End-settled FRAs or different fixing publication cutoffs would need distinct behavior and are not branches here. Quote-replay risk and direct curve-shift risk can differ materially despite sharing the DV01 name.

## Evidence

- [FRA implementation](../../../../finstack-quant/valuations/rates/src/fra/types.rs), [metrics and replay eligibility](../../../../finstack-quant/valuations/rates/src/fra/metrics.rs).
- [Pricing](../../../../finstack-quant/valuations/rates/tests/fra/pricing.rs), [cashflows](../../../../finstack-quant/valuations/rates/tests/fra/cashflows.rs), [QuantLib parity](../../../../finstack-quant/valuations/rates/tests/fra/quantlib_parity.rs), [DV01](../../../../finstack-quant/valuations/rates/tests/fra/metrics/dv01.rs): formula, lifecycle, external-reference and risk evidence with the scope encoded by each fixture; not executed in this audit.
- [Hull](../../../REFERENCES.md#hull-options-futures), [ISDA conventions](../../../REFERENCES.md#isda-2006-definitions).

## Evidence limits

`market_dependencies` omits the forward curve and fixing series whenever `observed_fixing` exists, although today's/future-fixing pricing still prefers an available forward curve. This can hide a real curve dependency from generic sensitivity/risk tooling.

## Independent review evidence

- From Friday 2025-01-10 a two-business-day lag on an explicitly selected
  weekends-only calendar gives Wednesday 2025-01-08. With `α=0.25`,
  `αx=91/360` and benchmark growth of `1.0125`, projection gives
  about 4.945055%. Using the contract accrual instead would give 5%.
  For a pay-fixed trade of 1m at 4.8%, the projected cash is about
  +358.21. An eligible same-day 4.6% observation gives about −494.32.
- For pay-fixed `N=1m`, `α=0.25`, `F=5%` and `K=4%`, upfront cash
  is about 2,469.14. The opposite side owes that amount. Discounting
  at 0.98 gives PV about 2,419.75. On payment day the admitted payable
  cut takes the full cash and the ordinary holder cut takes zero.
- With `α=0.25`, `F=−4` and `F=−5` fail the zero and negative
  denominator checks. With exactly represented `F=−4+2⁻⁴⁰`,
  `K=−4+2⁻³⁹`, the growth is a positive `2⁻⁴²` and cash is `−N`
  on the pay-fixed side, even though the legacy floor rejects the denominator.
- For the 5% versus 4% case, with projection source times 0.25 and
  0.50 and settlement discount horizon 0.25, centered ±1 bp
  continuous zero shocks give about +24.4395 projection-only,
  −0.06049 discount-only and +24.3790 combined in native currency per
  bp. Locking the fixing removes the projection component before payment.

These independently calculated cases cover selected dates, formula,
cash cuts and direct risk. Existing FRA pricing and risk tests,
the [QuantLib fixture generator](../../../../scripts/golden/quantlib/rates.py)
and [Python golden route](../../../../finstack-quant-py/tests/golden/test_pricing_fra.py)
provide implementation qualification entry points.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
