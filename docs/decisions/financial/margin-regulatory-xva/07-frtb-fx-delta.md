# MARGIN-REGULATORY-XVA-007: FX delta equicorrelation norm

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

FX delta treats each ordered currency-pair key as a risk factor and uses one uniform pair correlation. Inputs are P&L per 1% FX-rate move already in reporting currency.

## Algorithm and conventions

1. For each pair form `w=15×delta` using the fixed unrelieved FX risk weight.
2. Transform base `rho=0.60` under the chosen low/medium/high scenario.
3. Calculate `K=sqrt(max((1−rho)Σw² + rho(Σw)²,0))`. This O(n) equicorrelation identity replaces explicit pairwise summation and is the entire FX delta charge.
4. Repeated identical ordered pairs net through map accumulation; reversing a pair creates another factor rather than negating or transforming the original.

## Selection and controls

Selected by the FX delta branch; an empty map returns zero. Engine validation verifies finite amounts, but neither distinct currencies nor pair orientation relative to `base_currency`. The fixed weight has no runtime pair-relief selector.

## Limitations and failure behavior

The source documents absent specified-pair relief. Current regulation/jurisdiction eligibility was not independently assessed. A caller must normalize pair conventions and convert sensitivities; otherwise equal economic exposures may occupy different factors or create false diversification. No valuation-date or currency-conversion provenance is stored by the charge.

## Alternatives and tradeoffs

Canonical reporting-currency risk mapping would make offset logic more controlled but requires an explicit market-risk conversion method. A dense pairwise implementation is mathematically equivalent for the uniform rho and adds quadratic cost.

## Evidence and role

Runtime norm: [src/regulatory/frtb/delta.rs](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs); fixed constants/provenance notes: [src/regulatory/frtb/params/fx.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/fx.rs). [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates a single FX factor weight check and multi-factor scenario differences. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
