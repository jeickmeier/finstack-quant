# MARGIN-REGULATORY-XVA-017: SA-CCR credit, equity, commodity and FX effective-notional aggregation

- Decision status: **Accepted 2026-11-17 for checked SA-CCR credit, equity, commodity, and FX add-ons.**
- Implementation status: Observed behavior is retained below. Typed hedging sets, checked aggregation, and replayable supervisory inputs require reconciliation.
- Validation status: Primary Basel provisions and the Rust add-on path were inspected. Independent arithmetic checked within-pair and cross-pair FX add-ons. Pair orientation, special sets, and cross-host replay still require qualification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision

1. Use signed effective notional equal to adjusted notional times supervisory delta and maturity factor. Apply supervisory duration to the reporting-currency credit notional. Use qualified FX leg conversion and equity or commodity price-times-units sizing, including the prescribed volatility or variance sizing. [Trade inputs](14-saccr-inputs-and-supervisory-delta.md) · [Maturity factors and supervisory duration](15-saccr-maturity-and-duration.md) · [Basel CRE52.30–37](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-06-05).
2. Derive hedging-set identities from validated trade terms. For FX, orient each currency pair consistently and net signed effective notionals within the eligible pair set. Apply the FX factor to the absolute pair total. For credit and equity, net by reference entity or distinct index and apply the prescribed supervisory factors and partial offsets across entities within each set. For commodities, net by commodity type within energy, metals, agriculture, or other and use the prescribed within-set partial offsets. Sum distinct set add-ons. Keep eligible basis and volatility trades in their prescribed separate sets with supervisory-factor multipliers of 0.5 and 5, respectively. [Basel CRE52.45–47 and 52.58–73](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-06-05).
3. Retain the derived set and entity, signed effective notional, applied supervisory parameters, and set and class add-ons for replay. Sum class add-ons without cross-class diversification. Reject unresolved assignments and nonfinite calculations. [Basel CRE52.25–30](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-06-05) · [PFE and EAD assembly](18-saccr-rc-pfe-ead.md).

With ordinary FX effective notionals of +100 and −100, the 4% factor gives an add-on of zero in the same eligible pair and 8 across two different pairs.

## Observed choice and domain

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

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
