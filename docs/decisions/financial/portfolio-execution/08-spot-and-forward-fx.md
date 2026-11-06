# PORTFOLIO-EXECUTION-008 — Spot NAV conversion and covered-interest cashflow conversion

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and rationale

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
