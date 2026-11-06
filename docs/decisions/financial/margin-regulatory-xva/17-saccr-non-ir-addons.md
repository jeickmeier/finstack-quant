# MARGIN-REGULATORY-XVA-017: SA-CCR credit, equity, commodity and FX effective-notional aggregation

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

Non-IR classes share an entity-factor aggregation but preserve different supervisory factors, correlations and grouping. Credit and equity are pooled across all hedging-set labels; commodity and FX retain the supplied hedging-set label. This dispatch is financially material.

## Algorithm and conventions

1. Form the per-trade effective amount `d=delta×abs(notional)×duration×MF×SF`. Duration is SD for credit and one for equity/commodity/FX. MF is record 15's trade/config factor.
2. Group credit/equity into one class-wide outer group; group commodity/FX by `hedging_set`. Within each group net d by `underlier`, retaining that entity's correlation rho. Engine validation requires category consistency across all instances of an entity.
3. Credit factors/rhos are AA `0.0038/0.5`, A `0.0042/0.5`, BBB `0.0054/0.5`, BB `0.0106/0.5`, B `0.016/0.5`, CCC `0.06/0.5`; investment-grade index `0.0038/0.8`, high-yield index `0.0106/0.8`. Equity single name is `0.32/0.5`, index `0.20/0.8`. Commodity electricity is `0.40/0.4`, other `0.18/0.4`. FX without category uses `0.04/1.0`.
4. For each outer group calculate `A=sqrt(max((Σ_entity rho_i d_i)²+Σ_entity (1−rho_i²)d_i²,0))`. Sum A across groups. Thus FX rho one gives `abs(Σ d)` inside a hedging set, even when underlier strings differ.
5. PFE aggregates each strictly positive asset-class add-on by simple addition, with no cross-class diversification. Zero classes are omitted from the result map.

## Selection and controls

The enum category chooses SF/rho; the engine rejects absent categories outside IR/FX and inconsistent categories for an entity. The low-level absence-of-category fallback is therefore FX in validated non-IR calculations. All inputs must already have the correct adjusted-notional units; no currency pair conversion or commodity quantity/price multiplication occurs here.

## Limitations and failure behavior

FX labels determine which pairs fully offset; arbitrary same labels can net unrelated underliers. Credit/equity hedging-set labels are deliberately ignored in this implementation. Matured equity/commodity/FX trades are not removed and can retain positive add-ons through the MF floor. Fixed categories cover only the listed taxonomy, with no runtime rule-version selection.

## Alternatives and tradeoffs

Separate per-class engines could make exceptions explicit but duplicate the present shared factor norm. Typed FX pair and commodity-set keys improve classification controls. Extending the category taxonomy or changing pooling is an adoption decision that changes exposure, not a format-only change.

## Evidence and role

Runtime grouping and norm: [src/regulatory/sa_ccr/add_on.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/add_on.rs); parameter table and validation: [src/regulatory/sa_ccr/types.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/types.rs); class summation: [src/regulatory/sa_ccr/pfe.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/pfe.rs). Inspected [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) pins electricity factor 0.40 and BBB credit factor with SD; inspected add-on tests show different FX sets do not offset. They do not establish all category combinations or external classification validity. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
