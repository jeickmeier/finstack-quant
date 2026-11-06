# MARGIN-REGULATORY-XVA-010: FX vega pair and maturity correlations

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

FX vega combines ordered pair identity and option maturity in one implicit bucket. Inputs are monetary volatility-scaled vega; constant weight `1.00` does not mean the engine calculates an unweighted sum.

## Algorithm and conventions

1. Convert the exact maturity label to years and form `w=vega×1.00`.
2. Base pair correlation is one for identical ordered currency pairs and `0.60` otherwise. Multiply it by `exp(−0.01×|T_i−T_j|/max(min(T_i,T_j),1/365))`.
3. Scenario-transform that product and return the intra-bucket correlated norm with diagonal one. There is no separate second-level pair-bucket aggregation.

## Selection and controls

Selected by the FX vega branch; empty inputs yield zero through the shared empty norm. Exact standard labels and finite sensitivities are validated; currency orientation/distinctness are not. Same pair/different expiry entries retain their separate term dimension.

## Limitations and failure behavior

Canonical orientation, reporting currency and raw-to-scaled vega conversion are external. The implementation aggregates pairs within a single implicit bucket; that structure does not establish equivalence to every regulatory factor aggregation. There is no smile dimension or cross-currency volatility mapping.

## Alternatives and tradeoffs

A dense factor-specific correlation structure could express richer relationships but needs a new standard/model specification. The current factorized pair/expiry correlation is transparent and inexpensive for small feeds.

## Evidence and role

Runtime: [src/regulatory/frtb/vega.rs](../../../../finstack-quant/margin/src/regulatory/frtb/vega.rs); pair/weight constants: [src/regulatory/frtb/params/fx.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/fx.rs). The located `fx_vega_applies_weight_and_mar21_89_pair_correlation` unit test checks two pairs at one maturity; additional same-pair/multi-expiry evidence was not established as an independent benchmark. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
