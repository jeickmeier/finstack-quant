# CALIBRATION-TARGETS-001 — Quote identity, domains and risk units

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and rationale

Typed quote variants preserve contractual identity, market units and convention keys until instrument construction. The observed rationale is explicit dispatch and dimensional separation; no historical adoption or independent market-convention certification is inferred. Extraction by asset class silently excludes other quote classes, so a heterogeneous quote set is not a simultaneous multi-asset objective.

## Inputs, stages and equations

1. `QuoteId` wire decoding rejects empty/whitespace-only text; `QuoteId::new` and `From` do not. `MarketQuote::validate` supplies the same check for constructed Rust values. `Pillar` parses a trimmed tenor first, then ISO date; date/tenor financial resolution belongs to the instrument builder.
2. Deposit/FRA/swap rates and optional swap spread are finite **decimal rates**, with signed values permitted. Futures carry a price on the 100 scale and a finite decimal convexity adjustment: `r_implied=(100-price)/100-adjustment`. A `bump_rate_bp(b)` adds `b/10000` to rate quotes and subtracts `b/100` from futures price; contractual convexity/spread fields remain fixed. The method does not itself validate the bump or result.
3. CDS par spread is finite positive bp; recovery is in `[0,1)`. Upfront quotes retain a positive running coupon selected from `25,100,500,1000 bp` within absolute `1e-9`, and a finite signed upfront **fraction of notional** (`0.01=1%`). Single-name upfront has no magnitude bound here. `bump_spread_bp` adds raw bp only to par quotes and rejects upfront contracts rather than altering their coupon. Recovery overrides and conversion to par quotes preserve identity/conventions; downstream validation is required.
4. Tranche attachment/detachment are decimal fractions in `[0,1]` with attachment strictly below detachment. Upfront is a finite signed fraction of **tranche notional**, bounded by absolute 1; running coupon is positive bp. Index, series, date and convention remain explicit.
5. Inflation quotes carry finite decimal ZC or YoY swap rates, named index/convention, maturity and optional YoY frequency. Positivity of a CPI growth factor, date eligibility and frequency suitability are not enforced by this numeric quote validator.
6. Equity option quotes require positive strike and nonnegative vol. Swaption/cap-floor strikes need only be finite; vol is nonnegative. Swaption maturity must follow expiry. Normal vol is decimal rate per square-root year; Black/shifted Black vol is dimensionless annual volatility. Swaption quote types are checked against plan convention by the target; equity quotes have no quote-type field. `option_type` and cap/floor side are data, not part of this validation's arithmetic.
7. XCCY basis is a finite signed bp spread; optional spot must be finite positive. Pair orientation and settlement are resolved through the convention, not encoded in a generic scalar formula here.

## Selection, failures and alternatives

Public envelope validation calls `MarketQuote::validate`; constructing a Rust leaf enum does not prove validity. Unknown wire fields are rejected. Numerical domains remain layered: for example quote vol zero passes the general schema but SABR/SVI fitting requires positive quotes. Missing identity, unsupported dates/conventions and mixed curves are rejected later by preflight/builders. Raw bump helpers can create nonfinite or inadmissible quotes and rely on subsequent validation.

Alternatives include validated smart constructors, price-space risk bumps, permitting arbitrary CDS running coupons, or bid/ask-aware typed observations. Those change current contracts. Quote identification and unit choice are repository decisions; neither finite-value checking nor an ISDA label proves an executable contract or standard-model reconciliation.

## Evidence

- Owning implementations: [quote IDs/pillars](../../../../finstack-quant/calibration/src/quotes/ids.rs), [MarketQuote/extraction](../../../../finstack-quant/calibration/src/quotes/market_quote.rs), [rates](../../../../finstack-quant/calibration/src/quotes/rates.rs), [CDS](../../../../finstack-quant/calibration/src/quotes/cds.rs), [tranches](../../../../finstack-quant/calibration/src/quotes/cds_tranche.rs), [inflation](../../../../finstack-quant/calibration/src/quotes/inflation.rs), [volatility](../../../../finstack-quant/calibration/src/quotes/vol.rs), [XCCY](../../../../finstack-quant/calibration/src/quotes/xccy.rs).
- [Quote-schema tests](../../../../finstack-quant/calibration/tests/calibration/quote_schemas.rs), [market quote serialization](../../../../finstack-quant/calibration/tests/calibration/market_quote.rs), and zero/negative spread/nonstandard-coupon cases in [hazard tests](../../../../finstack-quant/calibration/src/tests/hazard_curve.rs) are located domain/serialization evidence, not price validation.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
