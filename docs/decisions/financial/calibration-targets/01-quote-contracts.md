# CALIBRATION-TARGETS-001 — Quote identity, domains and risk units

- **Decision status:** Accepted 2026-11-15 — complete quote admission, identity, units and product-domain package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent futures, tranche, shifted-strike, inflation and FX examples verified. Production admission, pricing, replay and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for exact typed calibration quotes and native units

1. **Use one checked quote admission path.** Require an exact nonblank
   ID at Rust construction, wire load and restoration. Preserve
   authored text and quote-set order. Require unique IDs across
   quote kinds and unique membership within each set. Allow explicit
   reuse across sets. Validate every supplied quote, including unused
   rows. A target consumes a qualified mixed set or an explicitly
   selected subset and reports its exact consumed IDs. Retire silent
   class filtering. Keep optional bid, mid or ask metadata and
   freshness assertions separate from contractual side.
   [Accepted authoring and admission](../calibration-framework/01-wire-and-authoring.md#accepted-decision-for-strict-calibration-envelopes-and-explicit-authoring) ·
   [Accepted freshness controls](../calibration-framework/02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market).

2. **Tag tenors and date roles explicitly.** Distinguish a nominal
   date subject to roll from an exact effective date where supported.
   Preserve literal CDS maturity and actual listed contract identity
   and reference interval for futures. The instrument owner resolves
   contractual dates and the applicable solver pillar under the
   selected conventions.
   [Deposit and FRA dates](../calibration-framework/04-deposit-fra-materialization.md#accepted-decision-for-deposit-and-fra-materialization-and-prepared-clocks) ·
   [Futures reference periods](../calibration-framework/05-futures-materialization.md#accepted-decision-for-listed-rate-futures-calibration-and-reference-periods).

3. **Keep rate and futures quote units literal.** Deposit, FRA and
   swap rates are signed annual decimals. Use exact Decimal swap
   quotes and floating spreads, including literal zero. Futures
   carry native IMM points `P` and explicit signed annual decimal
   convexity adjustment `CA`. The projected-rate seed is
   `(100−P)/100−CA`; fit in the elected points or cash scale.
   A signed move of `b` bp adds `b/10000` to a rate or subtracts
   `b/100` from futures points. Keep terms and convexity fixed.
   Validate source and result and reject nonzero moves that round away.
   [Accepted swap contract](../calibration-framework/06-swap-materialization.md#accepted-decision-for-term-and-overnight-swap-calibration) ·
   [Accepted rate replay](../calibration-framework/19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication) ·
   [CME IMM convention](https://www.cmegroup.com/rulebook/CME/IV/400/460.pdf).

4. **Distinguish single-name par and upfront.** Require a positive
   observed par spread in bp on an explicit clean or dirty basis,
   with exact recovery agreement in `[0,1)`. Upfront quotes carry
   the contractual fixed coupon in bp and a signed clean or
   actual-settlement fraction of notional with settlement basis and
   date. Apply coupon and amount restrictions from elected terms.
   Retire the generic coupon whitelist. Spread shocks use the
   validated par-risk view. Recovery and dependency replay preserve
   original quote forms.
   [Accepted CDS calibration](../calibration-framework/07-cds-materialization.md#accepted-decision-for-single-name-cds-calibration-and-quote-settlement) ·
   [Accepted hazard replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication) ·
   [ISDA contract-specific coupon settings](https://cdsmodel.com/fee-computations.html).

5. **Use original tranche notional for upfront.** Require exact index,
   series and version, currency and maturity. Attachment and detachment
   are original-pool fractions with `0 ≤ a < d ≤ 1`. Running coupon
   is contractual bp. Upfront is a signed clean or actual-settlement
   fraction of original tranche notional. Apply qualified contract
   limits and convert venue points or surviving-notional inputs
   explicitly. Retire the universal absolute-one cap.
   [Accepted tranche calibration](../calibration-framework/08-tranche-materialization.md#accepted-decision-for-index-tranche-calibration-and-quote-settlement) ·
   [ICE quotation terms](https://www.ice.com/products/34933772/Markit-CDX-North-AmericanInvestment-Grade-Index-Tranche).

6. **Bind the volatility law, clock and contractual side.** Require
   explicit Normal, Black or shifted Black convention, annualization
   clock, literal strike, side and exact underlying or index. Normal
   sigma is in absolute underlying units per square root year and
   admits signed strike and forward. Black sigma is relative and
   requires positive strike and forward. Shifted Black applies its
   explicit signed displacement to both before checking positivity.
   Admit finite zero sigma as quote data. Apply the elected fit's
   eligibility rules. Distinguish swaption expiry from underlying
   maturity and cap/floor strip maturity from optionlet expiry.
   The dated Hull–White cap/floor adapter uses its accepted flat
   Normal term-strip profile.
   [Accepted volatility conventions](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) ·
   [Accepted dated Hull–White adapter](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

7. **Distinguish zero-coupon and YoY inflation quotes.** Require the
   exact CPI index, convention, maturity and signed annual decimal
   fixed rate. Compounded zero-coupon quotes require `1+k>0`.
   Periodic YoY quotes use signed linear coupons and an explicit
   qualified frequency. Resolve reference lags and CPI coverage in
   contract preparation.
   [Accepted inflation swap terms](../valuations-rates/13-inflation-swaps.md).

8. **Bind cross-currency quote leg and FX authority.** Require the
   ordered currency pair, quoted leg, signed fractional Decimal bp
   and companion spread. A supplied spot is finite positive and
   expressed in quote currency per base unit, with its value date
   and authorized purpose. Preserve the separate authority of
   principal-sizing spot and valuation FX.
   [Accepted cross-currency calibration](../calibration-framework/09-xccy-materialization.md#accepted-decision-for-cross-currency-swap-calibration-and-fx-authority).

## Independent calculation checks

- Futures price `100.025` and convexity `.00025` give projected
  seed `−.0005`. A +2 bp move gives price `100.005` and seed
  `−.0003`. A `1e−13` bp move rounds away at the original
  binary64 price.
- A 5%–15% tranche on a 50 million pool has original notional
  5 million. A 2% clean upfront starts at 100,000 and settles at
  92,500 with a 250 bp coupon and 300,000 elapsed unit-coupon amount.
- A shift of `.015` maps forward `−.005` and strike `−.01`
  to positive `.01` and `.005`. A two-year zero-coupon fixed rate
  of −2% gives growth `−.0396`. USD 130 at 1.25 USD/EUR sizes
  EUR 104 of initial principal.

## Observed choice and rationale at the audit revision

Typed quote variants preserve contractual identity, market units and convention keys until instrument construction. The observed rationale is explicit dispatch and dimensional separation; no historical adoption or independent market-convention certification is inferred. Extraction by asset class silently excludes other quote classes, so a heterogeneous quote set is not a simultaneous multi-asset objective.

## Observed inputs, stages and equations

1. `QuoteId` wire decoding rejects empty/whitespace-only text; `QuoteId::new` and `From` do not. `MarketQuote::validate` supplies the same check for constructed Rust values. `Pillar` parses a trimmed tenor first, then ISO date; date/tenor financial resolution belongs to the instrument builder.
2. Deposit/FRA/swap rates and optional swap spread are finite **decimal rates**, with signed values permitted. Futures carry a price on the 100 scale and a finite decimal convexity adjustment: `r_implied=(100-price)/100-adjustment`. A `bump_rate_bp(b)` adds `b/10000` to rate quotes and subtracts `b/100` from futures price; contractual convexity/spread fields remain fixed. The method does not itself validate the bump or result.
3. CDS par spread is finite positive bp; recovery is in `[0,1)`. Upfront quotes retain a positive running coupon selected from `25,100,500,1000 bp` within absolute `1e-9`, and a finite signed upfront **fraction of notional** (`0.01=1%`). Single-name upfront has no magnitude bound here. `bump_spread_bp` adds raw bp only to par quotes and rejects upfront contracts rather than altering their coupon. Recovery overrides and conversion to par quotes preserve identity/conventions; downstream validation is required.
4. Tranche attachment/detachment are decimal fractions in `[0,1]` with attachment strictly below detachment. Upfront is a finite signed fraction of **tranche notional**, bounded by absolute 1; running coupon is positive bp. Index, series, date and convention remain explicit.
5. Inflation quotes carry finite decimal ZC or YoY swap rates, named index/convention, maturity and optional YoY frequency. Positivity of a CPI growth factor, date eligibility and frequency suitability are not enforced by this numeric quote validator.
6. Equity option quotes require positive strike and nonnegative vol. Swaption/cap-floor strikes need only be finite; vol is nonnegative. Swaption maturity must follow expiry. Normal vol is decimal rate per square-root year; Black/shifted Black vol is dimensionless annual volatility. Swaption quote types are checked against plan convention by the target; equity quotes have no quote-type field. `option_type` and cap/floor side are data, not part of this validation's arithmetic.
7. XCCY basis is a finite signed bp spread; optional spot must be finite positive. Pair orientation and settlement are resolved through the convention, not encoded in a generic scalar formula here.

## Observed selection, failures and alternatives

Public envelope validation calls `MarketQuote::validate`; constructing a Rust leaf enum does not prove validity. Unknown wire fields are rejected. Numerical domains remain layered: for example quote vol zero passes the general schema but SABR/SVI fitting requires positive quotes. Missing identity, unsupported dates/conventions and mixed curves are rejected later by preflight/builders. Raw bump helpers can create nonfinite or inadmissible quotes and rely on subsequent validation.

Alternatives include validated smart constructors, price-space risk bumps, permitting arbitrary CDS running coupons, or bid/ask-aware typed observations. Those change current contracts. Quote identification and unit choice are repository decisions; neither finite-value checking nor an ISDA label proves an executable contract or standard-model reconciliation.

## Source and validation evidence

- Owning implementations: [quote IDs/pillars](../../../../finstack-quant/calibration/src/quotes/ids.rs), [MarketQuote/extraction](../../../../finstack-quant/calibration/src/quotes/market_quote.rs), [rates](../../../../finstack-quant/calibration/src/quotes/rates.rs), [CDS](../../../../finstack-quant/calibration/src/quotes/cds.rs), [tranches](../../../../finstack-quant/calibration/src/quotes/cds_tranche.rs), [inflation](../../../../finstack-quant/calibration/src/quotes/inflation.rs), [volatility](../../../../finstack-quant/calibration/src/quotes/vol.rs), [XCCY](../../../../finstack-quant/calibration/src/quotes/xccy.rs).
- [Quote-schema tests](../../../../finstack-quant/calibration/tests/calibration/quote_schemas.rs), [market quote serialization](../../../../finstack-quant/calibration/tests/calibration/market_quote.rs), and zero/negative spread/nonstandard-coupon cases in [hazard tests](../../../../finstack-quant/calibration/src/tests/hazard_curve.rs) are located domain/serialization evidence, not price validation.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
