# VALUATIONS-RATES-05-CAP-FLOOR-PRICING: Cap/floor projection, volatility conventions and Hull–White approximation

- ID: VALUATIONS-RATES-05-CAP-FLOOR-PRICING
- Decision status: **Accepted — complete checked cap and floor optionlet pricing package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, model, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked cap and floor optionlet pricing

1. **Resolve the contract and optionlets explicitly.** Admit a holder-long,
   European, cash-settled cap, floor, caplet or floorlet in one currency.
   Require positive exact Money notional, signed exact Decimal strike and
   signed fractional Decimal bp spread. A caplet or floorlet has one
   actual period. A cap or floor uses an explicitly selected schedule
   grid. Resolve authored terms or a qualified dated convention once,
   retaining nominal and actual accrual dates, day-count context, rolls,
   stubs, benchmark, and the separate fixing, observation and payment
   calendars and nonnegative lags. Use the actual adjusted payment dates.
   Remove frequency inference, implicit calendar and index selection,
   the 31-day heuristic cap, and a second overnight roll. Require positive
   contractual coupon accrual `A` on every admitted optionlet. Keep
   contractual accrual, benchmark value period and accrual, model and source
   clocks, and final payment separately. See
   [convention selection](../valuations-framework/convention-selection.md),
   [schedule periods](../cashflows/01-schedule-periods.md) and
   [Strata optionlet terms](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/capfloor/IborCapletFloorletPeriod.html).

2. **Select observations and active sources by exact identity and knowledge.**
   Bind the benchmark, dates, currency, publication convention and version,
   and an exact typed projection role. Term pricing uses the actual
   benchmark value dates and positive index accrual `αx`, independently
   of `A`. Consume an exact eligible fixing. An expressly unpublished
   observation can use the selected projection source when the pricing
   phase permits it. Unknown or conflicting status, missing required
   observations and projection failure error. Overnight projection uses
   the accepted full quote intervals, partial weights, selected arrears,
   lookback, observation shift or cutoff and actual observation calendar.
   Preserve every distinct used observation, including cutoff reuse, along
   with its fixing and determination time. The last used observation
   and the contractual payment have separate dates. A shared projection
   and discount evaluator requires explicit qualification. Equal ID text
   and market presence cannot pick or substitute a source. See
   [term projections](../cashflows/06-floating-rate-projection.md) and
   [overnight projections](../valuations-framework/overnight-projection.md).

3. **Keep one literal payoff and match the quoted rate basis.** Before
   discounting, pay `N A max(η(R−K),0)` with `η=+1` for a cap and
   `−1` for a floor. For term use `R=F+s`. For overnight with outside
   spread, use `R=H/A+s` with `H=Π(1+r_i w_i)−1`. The
   index annualization `H/Σw_i` is a separate value. For an explicitly
   elected inside spread, use `H_used=Π(1+(r_i+s)w_i)−1` with positive
   checked factors and `R=H_used/A`. For a live option with additive
   outside spread, use the pre-spread forward and `K−s` on that matched
   quote basis. Inside spread uses the resulting rate and original strike.
   Validate the quote coordinate before any model displacement. Use stable
   growth and payoff differences. See [overnight growth and spread](../valuations-framework/overnight-projection.md).

4. **Require the selected model and a compatible live quote.** Select
   Black76 with an explicit unshifted or shifted election, Normal
   (Bachelier), or HullWhite1F through the checked registry. Register Normal
   for this product and remove the implicit Black76 default and
   cap/floor Auto switch. For Black or Normal require one typed scalar
   override or exact compatible optionlet source. Match family, units,
   finite shift, actual expiry, benchmark and quote coordinate. Where
   the source uses tenor or smile, require its actual qualified tenor
   and forward/moneyness inputs. An unrelated swaption cube or zero
   tenor placeholder cannot supply a quote. A qualified SABR source
   evaluates the explicitly selected output convention. Apply declared
   supported source boundaries and reject undeclared off-grid extension.
   Require the matching valuation origin or a qualified aging operation,
   and preserve total variance when a clock bridge is admitted. Normal
   volatility is absolute annual rate volatility and accepts signed
   forward and strike. Black volatility is relative and the effective
   forward and strike must be finite and strictly positive even at zero
   volatility. Apply a finite signed displacement to both only for the
   shifted election. Use exact admitted zero-vol intrinsic. Remove small
   expiry and Normal vol floors and implicit convention conversion.
   For compounded coupons, admit an aggregate quote only on the
   matched optionlet's rate, observation seasoning and determination
   horizon. Treat this as a market quote pricing approximation and keep
   it distinct from cap-level quotes, daily-rate vol and calibration.
   See [model and quote selection](../valuations-framework/model-resolution.md),
   [Black, Normal and shifted kernels](../models-analytic/forward-normal-shifted.md),
   [SABR source boundary](../core-market/sabr-cube-data.md) and
   [Strata caplet volatility source](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/capfloor/BlackIborCapletFloorletExpiryStrikeVolatilities.html).

5. **Keep fixing eligibility and payment entitlement separate.** A fully
   fixed, unpaid optionlet uses the literal intrinsic payoff and needs
   no projection, live volatility or short-rate parameters. An unpublished
   same-day fixing remains unresolved. Live stochastic pricing requires
   an admitted positive remaining horizon. Use a qualified intraday
   clock only if the selected model and source support it, or return
   unavailable on that live request. Never turn the projected rate into
   known intrinsic by flooring maturity or inferring publication from
   the date. The ordinary holder includes future payments and excludes
   same-day settlement. An explicit entitled same-day payable amount
   requires the necessary observations and uses factor one. Discount
   future actual or priced optionlet cash on the qualified payment date,
   applying `N A D(V,P)` once to the unit quote price. Validate terms and
   skip unused inputs on a settled or otherwise deterministic route. See
   [publication and clocks](../conventions/0045-financial-clocks-and-date-boundaries.md)
   and [cashflow valuation](../valuations-framework/cashflow-export.md).

6. **Use the accepted Hull–White term law and coherent parameters.** For
   live Hull–White choose one complete authored or exact fitted cap/floor
   block, with finite nonnegative `κ` and scalar or right-continuous
   scheduled absolute short-rate `σ`, qualified ACT/365F origin and
   schedule, and exact curve roles. Preserve the fitted parameter origins.
   Reject partial or conflicting fields and mixing blocks. Remove the
   persisted-series reanchor heuristic, monetary model coefficients, and
   positive-only sigma and mean-reversion checks. Keep caplet quote
   volatility separate from HW sigma. For a term optionlet pass the
   actual fixing `f`, benchmark
   `Sx<Ex`, positive index accrual `αx`, qualified projection growth
   `G0` and actual payment `P` to the accepted deterministic
   multiplicative-basis law. Require `V≤f≤Sx<Ex` and `P≥Sx` on the
   admitted model clock. The kernel uses the actual payment measure and
   discounts once with `D(V,P)`. Its discounted per-unit gross-option
   value at strike `1+αx(K−s)` is multiplied by `N A/αx`. See
   [Hull–White analytic kernels](../models-analytic/hull-white-analytic-kernels.md),
   [dated model clock and projection](../models-analytic/rate-clock-and-projection.md)
   and [Henrard, Hull–White, §5](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

7. **Identify the overnight Hull–White approximation.** Select the
   first-order Normal moment match explicitly for a qualified discrete
   compounded coupon. Approximate the mean by the prepared projected
   growth and hold observed contributions fixed. For each unique
   stochastic quote use its actual fixing and benchmark interval.
   With `L_j=exp(−κ(S_j−f_j)) B(S_j,E_j)`, the growth loading is
   `λ_j=(∂H_used/∂F_j) (1+F_j α_j) L_j/α_j`. Include every use
   of a cutoff quote in its derivative. Compute variance
   `Σ_iΣ_j λ_i λ_j Cov[x(f_i),x(f_j)]` on the selected clock, using
   checked ordered conditional variance accumulation. Combine loadings
   at equal times and use nonnegative variance contributions from
   successive independent state innovations. Do not clamp negative
   variance or move a past unresolved stochastic fixing to the origin.
   Use total standard
   deviation on the growth payoff, or divide consistently by `A` for
   the rate payoff. Report annualized equivalent Normal volatility only
   with a positive supported horizon. Use exact intrinsic at genuine
   zero variance. The approximation omits nonlinear compound terms and
   payment-measure mean correction. Qualify its range by benchmark,
   schedule, seasoning, volatility and payment lag. Reject requests
   outside the qualified range. See
   [Hull–White covariances](../models-analytic/hull-white-analytic-kernels.md)
   and [Fontana on backward-looking caplets](https://arxiv.org/abs/2202.09116).

8. **Publish gross value, premium and cash on their actual bases.** Sum
   checked optionlet values on one cut. Subtract the separately dated
   eligible nonnegative same-currency holder premium once. A same-day
   premium requires explicit entitlement and uses factor one. A future
   premium may remain even when no optionlets need pricing. Keep pure
   option premium distinct from the net trade mark and from any generic
   scenario adjustment. Export known contractual payable cash and its
   provenance where supported, and explicitly identify projected or
   model-valued items. A placeholder cannot represent realized cash.
   Reuse the same validated terms, model, dated sources and publication
   and payment cuts through direct, registered, calibration, risk,
   Python and WASM routes and generated schemas. Preserve the
   risk and implied-volatility choices for [record 06](06-cap-floor-risk.md)
   and leave fit controls with the calibration owner. A term fit does
   not itself qualify the overnight approximation. See
   [cap/floor calibration schedules](../calibration-rate-models/02-cap-schedules-and-quote-inversion.md).

## Observed behavior

### Choice, scope and rationale

European cash-settled caps/floors are sums of optionlets on either term rates or explicitly configured compounded overnight coupons. The default registry key is Black76, but Auto volatility convention follows source metadata and can dispatch Bachelier or shifted Black. HullWhite1F is a separate explicit model. [ADR 0029](0029-rate-option-exercise-and-volatility.md) records the broader selection question; this record states the actual numerical paths.

### Algorithm and controls

1. Build one actual period for Caplet/Floorlet or a frequency schedule for Cap/Floor. Defaults are short-front/ModifiedFollowing, unadjusted accruals and no EOM. Calendar priority is explicit, forward-index registry, supported currency default, else error. Term reset/payment lags come from registry or no reset/zero payment lag. If constructor frequency is absent, a day-span heuristic selects monthly/quarterly/semiannual/annual; the product enum determines single versus multiple periods.
2. Term rates require exact past fixing (strictly before valuation) or period forward otherwise. Add bp spread to coupon; for simple additive spread, remove it from both forward and strike before option pricing. Overnight settings require a registered overnight index and explicitly select compounding, spread inclusion/exclusion, fixing/payment calendars and lag (0–31 business days). Project daily observations, retain first/second parallel-forward sensitivities and observation exposures, and use the final fixing as option expiry. Same-ID overnight projection prefers an existing forward object, otherwise the discount curve.
3. For unpaid periods use ACT/365F expiry, floored to `1e-6` only for a still-future volatility lookup. Fixed optionlets use zero-vol Normal intrinsic without a vol lookup. Resolve Auto/Normal/Lognormal/ShiftedLognormal quote with clamping; shifted quotes transform F and K together. Price `N*alpha*D*Black_or_Bachelier(F,K,sigma,T)`. Normal vol below `1e-8` becomes intrinsic. Black requires its model domain after quote transformation; there is no automatic negative-rate model conversion.
4. Hull–White term optionlets call the model's term-caplet formula with forward-curve pseudo-DFs rebased at valuation, actual reset/start/end/payment times, discount-payment DF and spread-adjusted strike. The model kernel is owned by models.
5. Hull–White overnight optionlets instead linearize the compounded coupon against each unfixed daily rate. Loading is `coupon_forward_derivative*(1+f_i*alpha_i)*B(0,delta_t_i)/alpha_i`; coupon variance sums every pair of loadings times OU state covariance, floored at zero. Match `sigma_normal=sqrt(variance/T_final_fixing)` and apply Bachelier to the projected coupon mean. This is a first-order moment match, not exact compounded-option pricing.
6. Hull–White params accept explicit sigma schedule plus positive mean reversion, or a persisted cap/floor-family sigma series anchored at `max(as_of,discount_base)` with its active value at time zero. Explicit scalar sigma suppresses persisted schedule selection; simultaneous explicit scalar/schedule errors. Otherwise use shared family parameter resolution. All models subtract the discounted nonnegative same-currency holder premium only while its date is strictly future.

### Limitations and alternatives

Non-European exercise and physical cap/floor requests error. Rates/strikes are decimals; normal volatility is absolute annualized rate volatility (`0.005` is 50 bp), while Black volatility is relative (`0.20` is 20%). Displacement is a decimal rate applied to both forward and strike. Model expiry uses ACT/365F independently of contractual coupon accrual day count. The normal moment match omits higher-order distribution effects; direct simulation of compounded Gaussian bond ratios is an unimplemented alternative. Surface quotes on compounded coupons are used as aggregate coupon-rate quotes, with no local stripping/calibration. Same-day fixing uses the live projected value at zero option time. Cashflow export is Placeholder, not a realized exercise ledger.

## Evidence

[Types/schedules](../../../../finstack-quant/valuations/rates/src/cap_floor/types.rs), [projection](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/projection.rs), [convention dispatch](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/pricer.rs), [Black kernel](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/black.rs), [Normal kernel](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/normal.rs), [HW adapter/moment match](../../../../finstack-quant/valuations/rates/src/cap_floor/hw_pricer.rs) establish these branches. [QuantLib parity](../../../../finstack-quant/valuations/rates/tests/cap_floor/integration/quantlib_parity.rs), [convention tests](../../../../finstack-quant/valuations/rates/tests/cap_floor/validation/auto_vol_type.rs) and inline HW tests provide scoped formula/invariant evidence, not executed. The cap/floor file labelled QuantLib parity includes broad range checks; its name alone does not establish independent vendor agreement. Canonical entries are [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900), [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives).

The Auto variant prose and test introduction describe negative-rate conversion, while executable convention resolution preserves quote units and rejects invalid Black coordinates. This is a documentation discrepancy, not an adopted conversion fallback.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
requires financially compatible source selection before evaluation. A caplet's
tenor 0 must not clamp to a same-ID swaption cube's first tenor; supply an actual
qualified underlying coordinate and supported mapping or refuse that source.
Cube requests select output units explicitly, without beta inference or implicit
shift retagging. Scalar overrides with explicit model inputs need no unused
source. Record 16 was accepted on **2026-11-08**, with reconciliation and
qualification pending. The accepted decision above governs this product's
pricing, projection, lifecycle and off-grid policy.

## Independent review evidence

- For a known term caplet with notional 1m, accrual 0.25, fixing
  4.5%, outside spread 10 bp and strike 4%, payable cash is 1,500.
  At payment discount 0.99 and with an outstanding 500 premium at
  factor 0.98, net PV is 995.
- Two overnight blocks at 4% for `3/360` and 5% for `1/360`, with
  coupon accrual `4/365`, spread 10 bp, strike 4.4% and notional 1m,
  give approximately 1.035642 caplet cash with outside spread and
  1.189955 with inside spread. Annualizing on the index weights
  before applying coupon accrual would give zero on the outside case.
- For forward −1%, strike −0.5%, time 1, notional 1m, accrual 0.25
  and factor 0.98, Normal volatility 0.005 gives caplet and floorlet
  PV about 102.061451 and 1,327.061451. A shifted Black quote with
  shift 0.02 and volatility 0.20 gives about 4.715645 and 1,229.715645.
  Unshifted Black rejects the negative coordinates. At the money,
  a Normal volatility of `1e−9` gives a positive PV of about
  0.0000977409 at the same scale and horizon.
- In the deterministic Hull–White term limit, benchmark accrual
  0.25, coupon accrual 0.5, projection gross 1.01, strike 3%,
  no spread, discount factor 0.98 and notional 1m give PV 4,900.
  Applying the gross option without the coupon-to-index ratio gives
  2,450.
- For two unresolved overnight observations with fixing times
  0.125 and 0.25, model intervals `1/365` and `3/365`, weights
  and index fractions `1/360` and `3/360`, and forwards 4% and
  5%, use `κ=0.15` and `σ=0.01`. Growth variance is about
  `2.261349e−9`. At notional 1m and payment factor 0.99, an
  at-the-money optionlet is worth 18.78145. Keeping only the
  individual variances gives 16.37485.

The examples test specific cash, domain and scaling choices. Existing
inline and Monte Carlo fixtures are qualification entry points. The
overnight approximation needs matched independent coverage across its
intended product and market regimes.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
