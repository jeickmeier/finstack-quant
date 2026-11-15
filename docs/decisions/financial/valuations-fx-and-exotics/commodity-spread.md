# VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPREAD: Commodity spread option Kirk approximation and leg risk

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPREAD`
- Decision status: **Accepted 2026-11-14** — complete commodity spread option terms, Kirk, fixing, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent exchange-limit, approximation, settlement and leg-risk calculations checked. Runtime, market data, approximation domain and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for commodity spread pricing and settlement

1. **Book an ordered European cash spread.** Require call or put, holder
   long or short, finite positive quantity, finite signed strike and exact
   identities for both ordered commodities, benchmarks and delivery
   references. Require payout currency quotes and explicit positive unit
   conversions and leg ratios. In the common price unit, call intrinsic is
   the positive part of leg one minus leg two minus strike. Put intrinsic
   is the positive part of the reverse difference. Book the common fixing
   event, determination and actual payment date with qualified phases.
   Keep any booked premium as separate trade cash.
   [CME crack spread units](https://www.cmegroup.com/articles/2024/trading-crack-spreads.html).

2. **Bind each price, volatility and correlation input.** Use each leg's
   qualified dated forward on its own delivery or benchmark reference under
   the common pricing law. Sample the elected unshifted Black strike smile
   at the leg's native ATM forward before price-unit conversion. Preserve
   total variance in mapping quote volatilities to the common fixing
   clock. Require checked surface support and explicit finite correlation
   in `[-1,1]` with provenance for the two relative price shocks on that
   clock. [Price curves](../core-market/price-curves.md),
   [financial clocks](../conventions/0045-financial-clocks-and-date-boundaries.md).

3. **Select and check Kirk explicitly.** Use the `kirk` model key. Require
   positive normalized forwards `F_1,F_2`, a qualified positive live horizon
   and positive adjusted strike `B=F_2+K`. Let `w=F_2/B`. For total
   loadings `a=σ_1√T`, `b=wσ_2√T`, evaluate the effective standard deviation
   using scaled covariance arithmetic equivalent to
   `hypot(a−ρb,b√((1−ρ)(1+ρ)))`. Retain original input geometry through
   cancellation. Remove the absolute `1e−10` cutoff. Exact cancellation
   gives intrinsic under the Kirk proxy. Unresolved arithmetic or lost
   positive volatility errors. Apply the checked Black law on `F_1,B`.
   Discount once to actual payment and apply quantity and holder sign once.
   At strike zero the matched joint lognormal law recovers Margrabe. For
   nonzero strikes, approximation accuracy requires separate qualification.
   [Checked forward law](../models-analytic/forward-normal-shifted.md),
   [Margrabe](https://onlinelibrary.wiley.com/doi/10.1111/j.1540-6261.1978.tb03397.x),
   [Bjerksund and Stensland](https://files.core.ac.uk/download/30836538.pdf).

4. **Resolve both observations before fixing cash.** Apply the booked
   exercise rule using the exact authorized pair, qualified versions and
   publication cut. An eligible missing leg errors. A partly fixed state
   requires a separately qualified live law. Actual observations may be
   signed where contractual. Retain established signed cash through
   discharge. Eligible same-day unpaid cash uses factor one. Partial,
   failed or overdue payment requires a qualified residual. A known claim
   uses only its payment basis after the signed amount is established.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [dated discounting](../core-conventions/016-discounting.md).

5. **Publish explicit leg and source risk.** Use `delta::leg1` and
   `delta::leg2` per native price unit of the selected leg and retire bare
   `delta` for this product. Shock one prepared forward at a time, resample
   its native ATM volatility on the held strike grid and divide by the
   actual represented native price span. Whole-curve scenarios move all
   bound roles. Joint `vega` moves each exact active native grid once and
   reports signed PV per `+0.01` annual volatility. An individual leg vega
   is available when its source can be shocked in isolation. Hold
   correlation fixed in these Greeks. Parallel and bucketed DV01 use the
   actual payment discount source. Qualified horizon theta uses the
   fixing and payment state at each crossed event.
   [Option sensitivities](../valuations-framework/option-sensitivities.md),
   [dated curve risk](../valuations-framework/finite-difference-rates.md).

6. **Export the actual resolved claim.** Emit one signed
   `DerivativeSettlement` for known cash with both fixing identities in
   provenance. Complete cash export is unavailable while payoff remains
   contingent. Confirmed zero payoff or discharge permits an empty
   residual. Use one Rust-owned pricing, risk and export state in the
   existing Python and WASM instrument envelopes.
   [Cashflow export](../valuations-framework/cashflow-export.md).

## Independent calculations

- For `F_1=95`, `F_2=80`, volatilities `0.33,0.40`, correlation `0.65`,
  fixing time `0.70`, payment factor `0.987` and quantity `2,000`, zero
  strike gives call PV USD `36,457.94037119` and put PV USD
  `6,847.94037119` under both Kirk and independent conditional Gaussian
  integration. At strike `9` the call is USD `24,067.59313328` under
  Kirk and `24,088.80683220` under integration. At strike `−70` the
  adjusted strike is `10`. The Kirk put is USD `9,661.45` and direct
  integration gives USD `47.16` on the same joint law.
- With `F_1=83`, `F_2=74`, `K=5`, time `0.6`, payment factor `0.985`,
  quantity `1,000` and correlation `0.45`, use native ATM volatility
  profiles `0.31+0.0025(F_1−83)` and `0.28−0.0015(F_2−74)`.
  Matched `±1%` one-leg forward shocks with ATM resampling give call
  deltas USD `656.9053315` and `−540.5627895` per native price unit.
  Holding sampled volatilities fixed instead gives `618.9958272` and
  `−526.9355684` on the same widths.
- At authorized settlement, USD `2.15/gal` times `42 gal/bbl` gives
  USD `90.30/bbl`. Against USD `82/bbl` and strike USD `6.25/bbl`,
  a long call on 1,000 barrels fixes USD `2,050`. The short has the
  opposite amount. At remaining payment factor `0.9925` the long claim
  has PV USD `2,034.625`.

## Observed behavior

### Choice, rationale and scope

The inspected spread option uses Kirk with expiry forwards and each
leg's ATM-forward volatility.

### Algorithm and conventions

Let expiry forwards `F1,F2`, spread strike `K`, correlation `ρ`, and each leg volatility sampled at `(t,F_leg)`. Define `K_adj=F2+K`, `w=F2/K_adj` and `σ_K²=σ1²-2ρσ1σ2w+(σ2w)²`. Price `DF×Black(F1,K_adj,σ_K,t)×quantity`. Nonpositive effective variance becomes deterministic spread intrinsic. At expiry use `max(±(F1-F2-K),0)`; after expiry zero.

Reject nonpositive/nonfinite forwards, wrong volatility quote/axis, and `F2+K<=1e-10`. Correlation is validated in [-1,1]. Strike may represent a positive or negative spread but must satisfy the denominator domain at valuation.

Default delta is leg-one forward delta. Explicit `delta::leg1` and `delta::leg2` separately bump each price curve by ±relative bump, dividing by actual forward bump. Vega bumps both vol surfaces once (deduplicating a shared surface ID); leg-specific vegas bump only the named surface. Normalize by decimal-vol width×100 for one-vol-point P&L. Generic discount DV01/key-rate risk reprices the discounted value.

### Selection and controls

The default/family key is `Black76`, while the local algorithm is Kirk, not a single-underlying Black option. There is no automatic normal/MC fallback outside Kirk's domain.

Both price curves and strike-axis Black surfaces are required, plus one scalar correlation. Curve lookup is at expiry and volatility lookup at each leg's forward, so forward bumps can also move the sampled smile strike. Quantity positive; no quantity ratio beyond the supplied curve price units is inferred.

### Limitations, failure and fallback

Kirk freezes the effective-weight approximation and cannot handle `F2+K` near zero or negative lognormal forwards. Correlation is supplied externally; forward bumps move the sampled smile strike. Default delta is not net spread delta; simultaneous leg movements are a separate scenario.

### Alternatives and tradeoffs

Bivariate normal or two-factor Monte Carlo models could support negative prices and fuller distributions; they are named by the error message but not available through this local pricer.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/types.rs)

The original audit inspected source and inline assertions. The independent calculations above cover the stated cases. Production qualification is pending.

Canonical reference index. [Kirk](../../../REFERENCES.md#kirk-1995), [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
