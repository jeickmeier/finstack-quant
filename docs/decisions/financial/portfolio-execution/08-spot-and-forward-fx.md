# PORTFOLIO-EXECUTION-008 — Spot NAV conversion and covered-interest cashflow conversion

- **Decision status:** Accepted 2026-11-15 — complete valuation-date FX, prospective cashflow CIP, exact Money and raw endpoint package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent spot-date bridge, forward direction, PV equivalence and numeric range examples verified. Production pricing and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio NAV and cashflow FX

1. **Separate quote lookup from conversion basis.** Require
   an explicit ordered FX query at the effective portfolio
   valuation date `V`, with selected provider purpose, source
   authority and information cutoff. Rates are reporting
   currency `b` per native unit `c`. Qualify any inverse or
   pivot legs for consistent quote basis and value date.
   `CashflowDate` and `PeriodEnd` describe provider purpose;
   report the actual economic conversion basis separately.
   [Core FX resolution](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks).

2. **Translate already-discounted NAV using valuation-date cash FX.**
   Use a qualified cash quote `X_V` or derive it from a market
   outright `S_P` at the actual spot value date `P`:
   `X_V=S_P×D_b(V,P)/D_c(V,P)`. A directly qualified cash
   quote requires no bridge curves. Resolve `P` through
   the applicable FX date owner.
   [Accepted FX delivery and CIP](../valuations-fx-and-exotics/fx-delivery-cip.md#accepted-decision-for-checked-deliverable-fx-and-covered-interest-parity).

3. **Use the qualified forward rate for prospective payments.**
   For eligible `T>V`, use `F_T=X_V×D_c(V,T)/D_b(V,T)`,
   including qualified delivery before spot. At `T=P`, return
   an applicable selected outright exactly after admitting the
   basis and spot-date factors. An eligible `T=V` uses cash
   FX. Reject past-dated prospective rows, including same-currency
   rows. Historical cash and qualified overdue claims retain
   their owning treatment. Report nominal reporting-currency cash.
   [Cashflow ladders](09-cashflow-ladders.md).

4. **Require explicit matched discount sources where active.**
   Bind exact native and reporting curves on one qualified
   collateral or funding basis. Use positive finite dated
   relative factors on the respective source clocks and
   checked stable arithmetic for the complete rate. Remove the
   `1e−14` floor and ISO-code inference. Market forward quotes
   and basis calibration retain their owners.
   [Cross-currency discount and FX basis](../valuations-rates/16-cross-currency-swaps.md).

5. **Convert each Money amount once.** Admit the complete
   rate before one checked Decimal conversion from the original
   signed Money. An admitted same-currency amount remains
   exact without FX or curve lookups.
   [Money conversion](../core-conventions/012-money-decimal-precision.md).

6. **Keep qualified raw endpoints in binary64.** Use the
   selected model, settings, provider and endpoint state.
   Apply the same reporting basis directly to the finite
   unscaled raw value. Fail on unavailable raw pricing,
   overflow or nonzero underflow.
   [Qualified raw endpoints](04-execution-order-and-reduction.md#accepted-decision-for-checked-portfolio-execution-and-nav-reduction).

7. **Report the actual conversion basis and typed errors.** Retain
   the ordered pair, dates, purpose, quote authority, available
   route evidence and any selected curves and factors. Label an
   unconverted ladder as native. Preserve lookup, validation and
   calculation causes across existing Python and WASM paths.
   Exact Money is the canonical monetary output.

## Independent calculation checks

- An outright of `1.188 USD/EUR` and spot-date factors of
  `.99` for EUR and `.98` for USD give cash FX of `1.176`.
  Payment-date factors of `.95` and `.931` give a forward
  of `1.20`. EUR 100 converts to USD 120 nominal and both
  discounting paths give USD 111.72 at valuation.
- Positive equal factors of `1e−20` leave a qualified cash
  rate unchanged. A representable final conversion can also
  succeed when an unnecessary intermediate spot Money amount
  would overflow. EUR `1e28` at cash FX 16 and factor ratio
  `.125` gives USD `2e28`.

## Observed choice and rationale at the audit revision

Portfolio NAV collapses already-discounted native PV using the valuation-date FX matrix. A future cashflow ladder uses covered-interest-parity forward conversion. These solve different reporting questions: translating PV versus expressing a dated native cash amount in base currency before base discounting. The code does not apply a single future-date spot lookup to both.

## Algorithms and conventions

For source currency `c`, reporting currency `b`, and rate `S(c→b)` in base units per source unit:

- Same-currency conversion returns the original `Money` unchanged.
- Spot conversion returns `Money::new(amount_c * S(c→b, as_of), b)`. FX query uses the matrix's default resolution policy, so available direct, reverse and triangulated paths are governed by core. Valuation rows retain resolved rate and triangulation flag.
- A future payment at `T > as_of` returns `amount_c * S(c→b, as_of) * D_c(as_of,T) / D_b(as_of,T)`. For `T <= as_of`, it uses spot conversion; for equal currencies it returns immediately without curves.

Each discount curve is looked up from an optional currency→CurveId map, otherwise the currency ISO code. Interval discount factors use `df_between_dates`, so each curve owns its base date/day-count clock rather than the adapter assuming a tenor. The adapter requires finite factors with absolute magnitude at least `1e-14`; it does not add a positive-sign check beyond the curve's own invariants. Missing curves and factor problems become `MissingMarketData`. A present FX matrix whose lookup fails maps every lookup error into `FxConversionFailed`, discarding the finer underlying reason.

Raw factor endpoints call `value_raw_with_currency`, pass non-finite native values through for caller-specific rejection, otherwise construct Money and use the same spot helper before returning f64. They do not use the implied base/native PV ratio fallback of [metric aggregation](10-metric-aggregation-and-reporting.md). `Money::new` preserves a finite f64 through a shortest round-trippable Decimal representation without ingest-scale rounding; Decimal representability and f64 projections remain relevant boundaries.

## Selection and limitations

Ordinary valuation stamps `FxConversionPolicy::CashflowDate` despite applying the query at its effective valuation date; for an already-discounted PV the code documents this as the spot-equivalent endpoint. The stamp alone is not a complete path manifest. Cashflow reports instead stamp their sole `CipForward` policy explicitly.

The forward helper is the ratio of supplied discount curves with spot. It has no separate currency-basis, collateral-specific forward curve or observed forward-point input. Those can only be implicit in supplied curves, not assumed from the name. Missing data errors rather than returning the unconverted amount. Raw factor engines do not take a `FinstackConfig`/selected-pricer option argument through this helper.

Market forward quotes or basis-adjusted forward curves are possible alternative inputs but not implemented choices here. No current market convention certification is claimed.

## Evidence

[fx.rs](../../../../finstack-quant/portfolio/src/fx.rs), [valuation executor](../../../../finstack-quant/portfolio/src/evaluation/executor.rs), [cashflow collapse](../../../../finstack-quant/portfolio/src/cashflows.rs) and [Money construction](../../../../finstack-quant/core/src/money/types.rs). Inspected inline identity, direct/reverse and explicit/ISO-curve forward tests; [valuation FX tests](../../../../finstack-quant/portfolio/tests/valuation_fx.rs) locate portfolio scaling/conversion evidence. Formula tests are stronger for the simple CIP identity than comparisons between two production consumers, but do not validate every underlying curve convention. [Clark](../../../REFERENCES.md#clark-fx-options) supplies conceptual FX context only.

## Follow-up tracking

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
