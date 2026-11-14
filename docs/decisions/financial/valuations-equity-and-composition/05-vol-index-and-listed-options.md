# VALUATIONS-EQUITY-AND-COMPOSITION-05: Volatility-index futures and listed equity-option adapters

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-05`.
- **Decision status:** Accepted 2026-11-14 — complete volatility-index futures and listed futures-option package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Source and primary venue rules inspected. Independent futures-point risk, unpaid variation, option quote and exercise-versus-clearing arithmetic checked. Venue, runtime and host qualification pending.

## Accepted decision for checked volatility-index futures and listed futures options

1. **Select one qualified live futures mark.** Bind the exact contract,
   volatility index and series, point units, settlement currency and
   information cut. Elect an observed futures-point quote or an expressly
   selected, matched volatility-index futures strip. Query the contract's
   designated fixing reference using the strip's admitted dated query,
   original clock, coverage and snapshot. Require positive finite product
   levels. A market roll needs an explicitly qualified retained or refreshed
   source. Exclude cash-index spot, option-implied volatility and the square
   root of expected variance from this mark calculation.
   [Accepted level-source rules](../core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks).

2. **Separate cutoff, official fixing and actual cash.** Use the fill for
   the new interval or last applied official clearing mark for a carried
   position. Live variation is the signed point difference times contracts
   and cash per point on the admitted undiscounted basis. After cutoff
   require the exact official final observation when needed or authoritative
   clearing cash. Apply the effective venue's publication and rounding
   rules. For Cboe VX, the final observation is the SOQ and the cash is
   payable the following business day. Value confirmed unpaid variation
   at the actual cash date and phase, with factor one for included same-day
   cash. Treat overdue obligations explicitly and require evidence of
   closure before an empty residual.
   [Accepted listed lifecycle](../valuations-framework/listed-products.md),
   [Cboe VX contract specifications](https://www.cboe.com/tradable_products/vix/vix_futures/specifications).

3. **Define `delta_vol` as a live futures-point sensitivity.** Report
   signed settlement cash per positive one-point move in the selected
   live futures mark, with the exact futures-quote factor identity. Require
   an admitted movable mark. A held quote has no dependence on unrelated
   model inputs. Curve-pillar risk requires a separately qualified reprice
   with the actual interpolation. Finalized cash has no live point risk
   and can retain discount risk while unpaid. Cash-index and option-volatility
   moves are different experiments. Reconcile the metric's public meaning
   and exact attribution.
   [Accepted metric attribution](../valuations-framework/0010-metric-identifiers.md),
   [observed metric](../../../../finstack-quant/valuations/equity/src/vol_index_future/metrics.rs).

4. **Use one shared live option model for both adapters.** Bind the exact
   underlying futures series and mark, option clock, exercise rights,
   premium style, settlement form and authorized delivery ratio. Elect
   Black-76 or Bachelier with an explicit native volatility. Apply the
   shared discount basis for premium-paid pricing and separate the gross
   futures-style quote from the position mark measured against its option
   reference. The selected American branch must satisfy the shared
   exercise, cash and margining conditions. Example volatilities of 20%
   and 50% are construction inputs. Live qualification requires the actual
   terms and model basis. [Accepted shared option scenarios and pricing](../valuations-framework/listed-products.md#accepted-decision-for-checked-listed-futures-and-options),
   [CME premium and margining conventions](https://www.cmegroup.com/education/articles-and-reports/a-primer-on-margining-styles-for-options).

5. **Resolve exercise and delivery under the exact contract authority.**
   Preserve the separate option exercise reference, action and effective
   outcome, underlying mark and later futures settlement. CFE options on
   VX futures use the matched monthly future's daily settlement on option
   expiry and deliver the matched future. Cash-settled VIX index options
   use VRO and are outside these futures-option adapters. CME E-mini
   weekly and month-end options use a distinct exercise fixing. Eurex OVS2
   has its own American exercise, futures-style and delivery terms. These
   profiles illustrate the required authority. Obtain authorized exercise
   or assignment state. On futures delivery, create the correct signed
   lot at strike with the contract ratio and transfer it once, retaining
   clear ownership. Reconcile final option premium, option variation and
   initial delivered-futures variation as distinct amounts. The option
   reference never becomes the delivered future's clearing reference.
   [CFE Rulebook, Chapter 26](https://cdn.cboe.com/resources/regulation/rule_book/cfe-rule-book.pdf),
   [Cboe cash-settled index options](https://www.cboe.com/tradable_products/vix/vix_options/specifications),
   [CME exercise fixing](https://www.cmegroup.com/trading/equity-index/weekly-eom-options-faq.html),
   [Eurex OVS2 terms](https://www.eurex.com/ex-en/markets/vol/vstoxx/vstoxx-futures-and-options/Options-on-VSTOXX-Futures-158298).

6. **Keep pricing, risk, horizon and export on the same resolved state.**
   On the selected live option model, report delta and gamma per futures
   point and point squared, and vega per `+0.01` of its native volatility.
   Apply the shared matched repricing and work/accuracy rules. Horizon
   calculations crossing exercise or payment require authorized actual or
   hypothetical state. Export confirmed unpaid cash and retain delivered
   ownership separately. An unresolved contingent decomposition is
   unavailable. Reconcile native/registered pricing, portfolio, Python,
   WASM and schema consumers through the same Rust owner.
   [Accepted risk inputs](../valuations-framework/option-sensitivities.md),
   [horizon valuation](../valuations-framework/carry-theta-breakeven.md),
   [cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Relationship to accepted level-source boundaries

[CORE-MARKET-08](../core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks)
was accepted on 2026-11-08: explicit nonnegative index-point sources without
automatic spot anchors. Use its checked dated source query and complete clock,
preserving quoted/model/live/SOQ distinctions and product settlement eligibility.
Core source admission retains its own zero boundary. The accepted product
package above selects the stricter positive product domain.

### Choice and rationale

A volatility-index future reads a prebuilt volatility-index futures curve; it does not derive a futures quote by taking a square root of expected variance. Equity and volatility-index future options are instances of the shared future-option contract/kernel.

### Scope and input domain

Positive volatility-index entry/quote/settlement prices with listed contract terms. Listed options carry `FutureOptionTerms`, including explicit model, premium style, exercise and settlement terms; the equity wrappers add no separate formula.

### Algorithm

For the vol-index future, convert settlement date to the curve's base-date/day-count coordinate, floor time at zero and query `curve.price(t)`. Before settlement, shared mark resolution selects the quote/model. On settlement require `settlement_price`; after settlement PV is zero. PV is signed point delta times mark minus entry; `DeltaVol` is point delta per index point. The cashflow schedule has no residual and an entry-notional hint.

The two option wrappers invoke `define_future_option!`, delegating validation, value, Greeks and lifecycle to valuations-core. Equity's example supplies 20% implied vol; the vol-index example supplies 50%, American exercise, Black76, futures-style premium and delivery into a future. These are examples, not defaults for all contracts.

### Selection, controls and conventions

The volatility-index curve uses its own day count, not a new option clock. `fair_price` ignores the passed as-of and relies on curve base-date state; callers must roll the market if they want a rolled curve. Shared future-option dispatch remains authoritative for Black/Bachelier/early exercise and premium treatment.

### Limitations, failures and alternatives

Constant `DeltaVol` expresses one index-point exposure even when PV is zero after settlement or the mark is fixed. It is not volatility-of-volatility vega. A modeled VIX expectation with convexity is a different calculation. The option example's product name does not establish real exchange eligibility or calendar compliance.

### Evidence

[VIX index reference](../../../REFERENCES.md#cboe-vix-white-paper) and [Black futures-option reference](../../../REFERENCES.md#black-1976); inline tests in [types.rs](../../../../finstack-quant/valuations/equity/src/vol_index_future/types.rs) locate settlement/curve cases. Shared owner [future-option adapter](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/mod.rs) is the implementation boundary for the macro; its owning audit covers that kernel.

## Independent arithmetic and qualification

- Two long VX contracts at 1,000 per point give `3760` of unpaid final
  variation from an applied mark of `20.25` and official `22.13` final.
  At a `.998` payment factor the pending value is `3752.48`. A direct
  one-point mark move instead gives `2000`. A parallel one-point move
  in log-linear pillars 16 and 25 changes their midpoint from 20 to
  `sqrt(17×26)`, giving `2047.592083` on those two contracts.
- A 5,000 E-mini call exercises on an eligible 5,000.01 fixing. The
  delivered future's initial variation is 10 per contract on a later
  5,000.20 futures settlement at 50 per point.
- A generic Black-76 check with at-the-money futures and strike 20,
  `.25` years and 50% volatility gives `1.9895289932` gross unit points.
  At 100 per point, an option reference of 1.8 gives a futures-style
  position mark `18.95289932`. On the otherwise matched premium-paid
  basis with discount `.97`, the value is `192.98431234`.

Qualify actual contract mapping, mark/settlement phases, model basis,
exercise, delivered lots, venue cash and native/host consumers. Repository
runtime and host suites remain unrun.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/vol_index_future/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/vol_index_future/metrics.rs)
- [equity_future_option.rs](../../../../finstack-quant/valuations/equity/src/equity_future_option.rs)
- [vol_index_future_option.rs](../../../../finstack-quant/valuations/equity/src/vol_index_future_option.rs)
