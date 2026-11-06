# MARGIN-REGULATORY-XVA-003: GIRR tenor, inflation and cross-currency-basis delta aggregation

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

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

Retaining a curve identifier would permit same-tenor cross-curve correlation; the current map is simpler but removes that distinction irreversibly. Caller-distributed tenor sensitivities can supply the required grid; an internal mapping algorithm would require its own interpolation/adoption decision.

## Evidence and role

Runtime: [src/regulatory/frtb/delta.rs](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs); exact weights, lookup and correlation formula: [src/regulatory/frtb/params/girr.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/girr.rs). [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs) contains inspected literal-table pins and the independent `1Y/5Y` exponential worked example. [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates single-factor, tenor-floor and factor-selection tests. Source claims of historical primary-source review are not rerun. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
