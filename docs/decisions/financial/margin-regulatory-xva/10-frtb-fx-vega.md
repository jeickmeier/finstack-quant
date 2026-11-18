# MARGIN-REGULATORY-XVA-010: FX vega pair and maturity correlations

- Decision status: **Accepted 2026-11-17 for checked FX vega.**
- Implementation status: Existing behavior is described below. Qualified pair and expiry admission, pair-bucket aggregation and checked input and result paths remain pending.
- Validation status: Primary references, expiry grid and vega weight checked. Independent arithmetic covered within-pair expiry decay, mixed-expiry cross-pair charges and all three scenario correlations. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked FX vega

1. **Use one bucket per qualified option pair.** A EUR/JPY option may
   contribute to a EUR/JPY vega bucket in a USD report, with the
   sensitivity expressed in USD. Make the pair treatment explicit in
   the elected profile and require qualified evidence for any
   jurisdiction-specific mapping.
   [Basel MAR21.14–15 and MAR21.91](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [EU CRR Article 325q(2)](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX%3A02013R0575-20260626).
2. **Require a declared quote orientation.** Use distinct currencies and
   one qualified ordered quote per regulatory pair. Map reciprocal
   quotes to that factor after a qualified transformation or reprice.
   Normalize applicable currency aliases and retain the source quote
   and pricing convention. Net matching pair and expiry contributions.
   [FX quote conventions](../core-conventions/011-fx-date-and-quote-conventions.md#accepted-decision-explicit-fx-quote-metadata-and-value-dates) ·
   [Basel MAR21.14](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Require prepared lognormal vega on the five-point expiry grid.**
   Admit signed `σ × ∂PV/∂σ` in the reporting currency, using matching
   volatility units and PV before CVA. Require documented mapping to
   `0.5Y`, `1Y`, `3Y`, `5Y` or `10Y`. Assign `10Y` and carry the RRAO
   requirement for an option without a specified maturity. Use the
   volatility-scaled definition across Rust, wire, Python and WASM.
   [Basel MAR21.14 and MAR21.25–30](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [Residual risk record](13-frtb-residual-risk.md).
4. **Apply the `1.00` weight and expiry decay within each pair.** The
   40-day liquidity horizon gives the published 100% vega weight.
   Apply it once to prepared vega. Use
   `exp(−0.01 × |T_i−T_j| / min(T_i,T_j))` for a pair's distinct expiry
   factors and apply the scenario once. Calculate the bucket charge
   `K_b` and retain the raw signed sum `S_b`.
   [Basel MAR21.92–94](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Aggregate across pairs with uniform cross-bucket correlation.**
   Use `0.45`, `0.60` and `0.75` for Low, Medium and High, independent
   of expiry. Use the checked quadratic and cap signed bucket sums
   only on a negative-quadratic retry. Distinct pairs with weighted
   amounts `+100` at `1Y` and `−100` at `5Y` yield `89.4427` in Medium.
   The observed formula gives `92.0355` for that case.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) ·
   [Basel MAR21.4, MAR21.6, MAR21.89 and MAR21.95](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
6. **Preserve coverage and calculation evidence.** Carry the pair mapping,
   expiry, source model, net weighted amounts, bucket charges and scenario
   results through Rust, wire, Python and WASM. Report evaluated zero only
   when coverage establishes it. Missing required mapping or sensitivity
   evidence makes the affected charge unavailable.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

FX vega combines ordered pair identity and option maturity in one implicit bucket. Inputs are monetary volatility-scaled vega; constant weight `1.00` does not mean the engine calculates an unweighted sum.

## Algorithm and conventions

1. Convert the exact maturity label to years and form `w=vega×1.00`.
2. Base pair correlation is one for identical ordered currency pairs and `0.60` otherwise. Multiply it by `exp(−0.01×|T_i−T_j|/max(min(T_i,T_j),1/365))`.
3. Scenario-transform that product and return the intra-bucket correlated norm with diagonal one. There is no separate second-level pair-bucket aggregation.

## Selection and controls

The FX vega branch returns zero for an empty map through the shared norm. Validation checks finite amounts and the ten maturity labels shared with GIRR delta. The current keys admit arbitrary ordered currency pairs, including self-pairs. Different expiries on one pair retain separate factor entries.

## Limitations and failure behavior

The current implementation leaves orientation and volatility scaling to the caller and applies expiry decay between different pairs. The Python binding source describes P&L per unit of implied volatility. The Python stub and WASM source document volatility-scaled vega. Both bindings forward the amount to Rust.
[Python binding](../../../../finstack-quant-py/src/bindings/margin/regulatory.rs) ·
[Python stub](../../../../finstack-quant-py/finstack_quant/margin/_regulatory.pyi) ·
[WASM binding](../../../../finstack-quant-wasm/src/api/margin/frtb.rs).

## Alternatives and tradeoffs

The accepted calculation applies maturity dependence within each pair and uses the profile's cross-pair coefficients on the bucket sums. The valuation owner supplies evidence for source quotes, reprices and any reciprocal or cross-currency volatility mapping.

## Evidence and role

The inspected runtime test uses two pairs at a shared maturity. Independent arithmetic checked the distinction between within-pair expiry decay and across-pair aggregation, including mixed-sign, mixed-expiry amounts under Low, Medium and High. Runtime verification remains pending.
[Runtime and vega tests](../../../../finstack-quant/margin/src/regulatory/frtb/vega.rs) ·
[Input validation](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs) ·
[FX parameters](../../../../finstack-quant/margin/src/regulatory/frtb/params/fx.rs) ·
[FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
