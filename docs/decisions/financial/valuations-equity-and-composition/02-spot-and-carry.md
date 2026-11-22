# VALUATIONS-EQUITY-AND-COMPOSITION-02: Cash equity valuation and dividend-adjusted carry

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-02`.
- **Decision status:** Accepted 2026-11-14 — full cash-equity and dividend-adjusted carry package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Source and primary references inspected. Independent share, FX, continuous carry and dated cash-dividend arithmetic checked. Market, entitlement, runtime and host qualification pending.

## Accepted decision for checked cash-equity valuation and dividend carry

1. **Admit explicit shares and one spot authority.** Require a finite signed
   share count, including fractional shares and literal zero. A per-share
   position supplies one share explicitly. Bind exact security identity,
   share and quote basis, native currency and information cut. Select one
   qualified dated authored quote or one exact market quote binding. Require
   finite nonnegative value and qualified currency-per-share units. Use an
   explicit dated FX policy and exact conversion under CORE-CONVENTIONS-012
   and 013 for a different-currency quote of the same security and share
   basis. A distinct share class, listing unit or depositary receipt needs
   its own authority. [Checked observations](../core-market/scalar-series-and-fixings.md),
   [monetary precision](../core-conventions/012-money-decimal-precision.md),
   [FX resolution and shocks](../core-conventions/013-fx-resolution-and-shocks.md).

2. **Value signed shares and distinguish economic exposure from quote risk.**
   Native raw stock value is `N×S`, with admitted signed shares `N` and
   native spot per share `S`. Use checked scaling and apply any authorized
   position scale once. Spot valuation uses only its active quote and FX
   inputs. Publish shares and native price per share independently. On an
   admitted movable spot, quote delta is `N` per unit of native share price,
   or `N×FX` per unit of a qualified foreign share quote with FX held fixed.
   Attribute the qualified result to the exact active quote ID. An authored
   frozen quote has signed share exposure and no applicable market quote
   delta. Spot rate DV01 has structural zero on the admitted held-quote and
   FX basis. Publish no synthetic curve buckets. Forward carry and its
   sensitivities require their own basis. [Metric attribution and key agreement](../valuations-framework/0010-metric-identifiers.md),
   [checked rate risk](../valuations-framework/finite-difference-rates.md).

3. **Require a dated forward request and one explicit carry choice.** Bind
   valuation `V`, delivery `T`, the corresponding phases and exact
   underlying/share and information basis. Elect no distributions,
   continuous yield, or an identified qualified discrete cash schedule.
   Remove the ticker-derived horizon and missing or invalid horizon fallback.
   For the identical cut return qualified spot without a curve or carry
   lookup. On an admitted same-date change of phase, use a unit delivery
   factor and apply the actual change in dividend rights. Reject a delivery
   cut before valuation or unsupported phase ordering.

4. **Use dated funding and an independent yield clock.** For the admitted
   same-currency forward, use cash-equivalent spot `S` and checked positive
   relative funding factor `D(V,T)`. With no distributions use `S/D(V,T)`.
   With explicit finite signed continuous annual decimal yield `q`, use
   `S exp(−q τ)/D(V,T)` where `τ` is ACT/365F from valuation to delivery.
   Bind the yield as an authored value or an exact qualified annual quote.
   An inapplicable, missing or wrong-type selected input errors. Report
   `equity_dividend_yield` as `q` on continuous carry and literal zero on an
   explicit no-distributions basis. Discrete cash leaves that metric
   unavailable. Preserve the date, phases, sources, currency and per-share
   units on `equity_forward_price`. [Checked valuation model inputs and carry](../valuations-framework/model-resolution.md).

5. **Select discrete dividend rights by entitlement and value the actual
   payment dates.** Use the CORE-MARKET-13 cash-only event source on the same
   security, share and currency basis, with complete selected coverage and
   source vintage. Include each right acquired with the qualifying spot and
   retained on delivery. The ex-event and its phases determine inclusion.
   The actual pay date `P_i` determines `D(V,P_i)`, including when cash pays
   after delivery. Compute prepaid spot `S−Σ d_i D(V,P_i)` and divide it by
   `D(V,T)`. Preserve separate events, including those with the same ex-date.
   Accept exact zero prepaid spot and reject negative prepaid spot or
   unrepresentable arithmetic. An empty schedule requires explicit coverage.
   Keep original dates. Due bills, nonstandard distributions and stock-loan
   economics require separate qualified treatment. [Accepted dividend event source](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates),
   [prepaid-forward replication](https://www.kellogg.northwestern.edu/faculty/hagerty/ftp/D65/lecture/Fall2006/financial_futures_Fall2006.pdf#page=13),
   [FINRA dividend timing](https://www.finra.org/rules-guidance/rulebooks/finra-rules/11140).

6. **Keep actual holder cash and receivables separate from the stock mark.**
   A detached unpaid dividend requires confirmed entitlement, entitled
   shares, actual payment date and paid/unpaid phase. The selected carry
   schedule supplies source events, while the holding terms establish
   ownership. A new ex-dividend buyer receives no already detached dividend.
   In portfolio and attribution retain the distinct stock mark, receivable
   and paid cash, counting each economic component once. Admit `NoResidual`
   only for a proved empty requested cash scope. Complete cashflow PV
   decomposition of the security mark is unavailable. Reconcile Rust,
   existing Python and WASM entry points, schema and dependent consumers
   with the same source and quantity rules. [Valuation trace and cash export](../valuations-framework/cashflow-export.md),
   [SEC dividend entitlement guidance](https://www.investor.gov/introduction-investing/investing-basics/glossary/ex-dividend-dates-when-are-you-entitled-stock-and).

## Observed behavior

### Choice and rationale

Cash equity is quantity times an explicitly resolved share price. Forward reporting uses rebased discount factors and either a discrete-dividend escrow or continuous dividend yield. This is the observed carry convention; its suitability for a particular security remains a caller/adoption decision.

### Scope and input domain

Finite signed quantities (omission means one share), nonnegative quoted/market share prices, explicit currency and optional dividend scalar/cash-dividend schedule. The forward horizon is a finite nonnegative number of years in the discount curve's time coordinate.

### Algorithm

1. `quoted_spot` takes precedence over `spot_id`. A `Price` scalar is converted into instrument currency using market FX; a unitless scalar is interpreted in that currency. Missing both sources fails.
2. PV is `effective_quantity × share_price`; no discounting enters current cash-equity PV.
3. For a forward horizon `t`, compute `D = DF(curve_time(as_of)+t)/DF(curve_time(as_of))`.
4. If the discrete-dividend vector is nonempty, subtract PVs of dividends with `as_of < ex_date ≤ horizon` and divide prepaid spot by `D`; otherwise use `S exp(-q t)/D`. Omitted dividend-yield ID means zero, while a specified missing/wrong-type scalar errors.
5. Metrics expose price, effective shares, yield and forward price. Delta is the signed share quantity, also stored under the ticker component. The forward metric reads `<ticker>-FWD_T`; any lookup failure defaults to zero, and a monetary scalar is read by its amount.

### Selection, controls and conventions

The generic equity registration selects its native spot calculation. Rate risk uses shared parallel/key-rate bump calculators and ordinarily vanishes for a fixed spot. Cashflow export is `NoResidual`, with a notional hint only when a quoted spot exists; configured dividends affect forward reporting but are not emitted as this instrument's dated cashflows.

### Limitations, failures and alternatives

The forward method does not reject a negative prepaid spot after cash dividends. A nonempty dividend list suppresses continuous yield even when no dividend remains inside the horizon. Quoted-spot delta denotes economic share exposure although bumping a separate market spot cannot move that quote. Modeling dividend events as dated cashflows, or requiring a horizon scalar instead of zero fallback, are alternatives requiring agreement.

### Evidence

[Hull carry reference](../../../REFERENCES.md#hull-options-futures); [types_tests.rs](../../../../finstack-quant/valuations/equity/tests/equity/types_tests.rs) and [test_bucketed_dv01.rs](../../../../finstack-quant/valuations/equity/tests/equity/test_bucketed_dv01.rs) contain local construction/carry and rate-risk checks; [pricer_tests.rs](../../../../finstack-quant/valuations/equity/tests/equity/pricer_tests.rs) supplies pricing cases. These are formula/behavior comparisons, not a historical dividend or corporate-action benchmark.

### Relationship to the accepted cash-dividend source boundary

[CORE-MARKET-13's complete accepted package](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates) selects explicit cash
events with ex/payment dates, gross per-share Money and authoritative underlying/
share/currency association. Reconcile this instrument's ex-only numeric vector
and continuous-yield priority with that checked source; no silent pay=ex,
currency stripping or nonempty-vector suppression. Source preservation does not
grant an ex-past receivable to a new buyer. Carry and position entitlement
require an explicit qualified adapter or refusal. CORE-MARKET-13 was accepted
on **2026-11-08**, with reconciliation and qualification pending.

## Independent arithmetic and qualification

- A short position of `−2.5` shares at 120 per share has native value `−300`.
  A qualified quote of 100 in another currency with frozen FX `1.2` gives
  the same mark and source-quote delta `−3` per foreign price unit.
- For spot 100 and delivery factor `.98`, dividends of 2 and 3 detach by
  delivery and pay afterward at factors `.975` and `.965`. Their PV is
  `4.845` and the forward is `97.0969387755` per share. Using `.99` for both
  ex-date factors yields `96.9897959184`. At the same spot and delivery
  factor, continuous carry of 3% over 180 ACT/365F days gives `100.5422837411`.

Qualify exact share/quote identity and FX directions, zero and frozen marks,
delivery boundaries, distinct dividend dates and actual holder cash with
the dependent runtime, schema, portfolio and host routes. Independent
market and holder-entitlement reconciliation and runtime suites remain open.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/spot/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/spot/metrics.rs)
