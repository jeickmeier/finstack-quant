# MARGIN-REGULATORY-XVA-003: GIRR tenor, inflation and cross-currency-basis delta aggregation

- Decision status: **Accepted 2026-11-17 for checked GIRR yield, inflation and cross-currency basis delta.**
- Implementation status: Existing behavior is described below. Exact curve keys, special factors, elective weight relief and checked caller reconciliation remain pending.
- Validation status: Source and primary references inspected. Independent examples checked same- and different-curve correlations, the tenor floor, relief arithmetic and offsetting weighted deltas. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked GIRR delta

1. **Preserve the yield curve and tenor in each factor key.** Use one bucket
   per risk currency and net ordinary yield delta only by exact currency,
   curve and tenor. Keep OIS, distinct term-index curves, and onshore and
   offshore curves separate where applicable. Accept prepared sensitivities
   on the ten prescribed vertices. The valuation owner supplies and documents
   any mapping onto that grid. Reject unsupported tenors.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR21.8 and MAR21.41](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
2. **Keep inflation and cross-currency basis flat and identifiable.** Retain
   each qualified inflation curve as a separate factor without a tenor axis.
   For cross-currency basis, preserve the risk currency, USD or EUR anchor
   and signed quote convention. Assign each contribution once to its qualified
   flat basis. A cross involving neither anchor requires an evidenced mapping
   to one of them. Keep the instrument's ordinary yield delta where applicable.
   [Basel MAR21.8](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Use the prescribed weights with an explicit relief election.** Apply the
   percentage-point weights to the reporting-currency delta accepted in record
   001. Use `1.7` for `0.25Y` and `0.5Y`, `1.6` for `1Y`, `1.3` for `2Y`,
   `1.2` for `3Y`, and `1.1` for `5Y`, `10Y`, `15Y`, `20Y` and `30Y`.
   Inflation and cross-currency basis each use `1.6`. Require standard weights
   or a qualified relief election. Where elected, divide that eligible
   currency's weights by `√2` once. The Basel eligibility list includes EUR,
   USD, GBP, AUD, JPY, SEK, CAD and the bank's domestic reporting currency.
   [Basel MAR21.42–44](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
4. **Apply curve distinctions before scenario scaling.** For yield tenors `T_i`
   and `T_j`, use `max(exp(−0.03 × |T_i − T_j| / min(T_i, T_j)), 0.40)`,
   multiplied by `0.999` for different curves. Use `0.999` between distinct
   inflation curves and `0.40` between inflation and yield within a currency.
   A cross-currency basis factor has zero correlation with other distinct
   factors in its bucket. Keep the unit diagonal and apply the record 002
   scenario transformation. Weighted deltas of `+100` and `−100` on different
   `5Y` curves give a Medium bucket charge of `√20 ≈ 4.47`.
   [Basel MAR21.45–49 and the inflation-curve FAQ](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Aggregate currencies through the accepted checked calculation.** Use base
   correlation `0.50` between currency buckets and apply the selected scenario.
   Retain each bucket's `K` and raw signed `S`, with the capped retry only when
   record 002 calls for it. Report zero only with qualified coverage. Preserve
   factor identity, base and effective weights, the relief election and scenario
   evidence through Rust, wire restoration, Python and WASM.
   [Basel MAR21.50](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [FRTB aggregation contract](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation).

## Choice and domain

The observations below describe the audited design. The accepted method preserves each yield and special-factor identity before scenario aggregation.

GIRR groups interest-rate risk by currency. Only one yield-curve sensitivity per tenor/currency is represented; different underlying yield curves sharing that key are netted before calculation. Inflation and cross-currency basis each have one separate factor per currency.

## Algorithm and conventions

1. Multiply yield sensitivities by tenor weights: `[0.25Y:1.7, 0.5Y:1.7, 1Y:1.6, 2Y:1.3, 3Y:1.2, 5Y:1.1, 10Y:1.1, 15Y:1.1, 20Y:1.1, 30Y:1.1]`. Weights are numerical percent units against P&L per 1pp yield change. Inflation and basis weights are both `1.6`.
2. Yield-tenor base correlation is `max(exp(−0.03×|T_i−T_j|/min(T_i,T_j)),0.40)`. Tenors within `1e−12` return one; the public helper returns the floor if the minimum tenor is nonpositive. Validated engine inputs use the positive standard grid.
3. Inflation-to-yield correlation is `0.40`; basis-to-yield and basis-to-inflation are zero. Same special-factor types use one, though the keyed map normally has only one entry of each type. Apply the selected scenario transform to the composite base correlation.
4. Calculate `(K_currency,S_currency)` by the ordinary correlated quadratic. Across currencies use scenario-transformed `0.50` with the standard/conditional capped-S procedure in record 02.

## Selection and controls

The GIRR delta branch is selected by `FrtbRiskClass::Girr`; empty yield/inflation/basis maps return zero. Exact standard labels are required by validation; private unknown-tenor risk-weight/year lookups return NaN if called outside that boundary. No interpolation distributes an arbitrary tenor into the standard grid.

## Limitations and failure behavior

The source declares that specified-currency risk-weight relief and the distinct-curve `0.999` correlation dimension are not implemented. This is a representational limitation, not a conclusion about current regulatory permission or a universally conservative effect for hedged books. Cross-currency basis labels distinguish only currency, not separate basis curves.

## Alternatives and tradeoffs

Curve identity lets the calculation apply the prescribed correlation across distinct curves at the same tenor. The valuation owner prepares the assigned standard vertices and supplies their mapping evidence.

## Evidence and role

Runtime: [src/regulatory/frtb/delta.rs](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs); exact weights, lookup and correlation formula: [src/regulatory/frtb/params/girr.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/girr.rs). [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs) contains inspected literal-table pins and the independent `1Y/5Y` exponential worked example. [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates single-factor, tenor-floor and factor-selection tests. Source claims of historical primary-source review are not rerun. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
