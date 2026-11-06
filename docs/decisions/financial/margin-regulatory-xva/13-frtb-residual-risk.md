# MARGIN-REGULATORY-XVA-013: Gross-notional residual-risk add-on

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

RRAO adds a fixed gross-notional charge from caller-classified positions. It does not infer residual-risk eligibility or exemption from instrument features, and it is independent of correlation scenarios.

## Algorithm and conventions

For each supplied position choose `0.01` when `is_exotic=true`, otherwise `0.001`. Sum `abs(notional)×weight`. The fold starts from positive zero so an empty vector returns `+0.0`. There is no netting across long/short positions, repeated instrument IDs or different risk flags.

## Selection and controls

The full engine validates nonblank instrument IDs and finite signed notionals, then always computes RRAO even when the SBA risk-class list is restricted. DRC/RRAO vectors append records instead of accumulating keyed risk. Negative notional is accepted and charged by magnitude.

## Limitations and failure behavior

The single Boolean is the entire local taxonomy; no transaction-level exemption, notional normalization, currency conversion or deduplication is implemented. Large finite notionals can overflow the total; there is no final finite-result check.

## Alternatives and tradeoffs

An instrument classifier could supply auditable categories and exceptions but would be new methodology. Netting signed notionals would violate the present gross-input convention and materially change output.

## Evidence and role

Runtime: [src/regulatory/frtb/rrao.rs](../../../../finstack-quant/margin/src/regulatory/frtb/rrao.rs); input collection/validation: [src/regulatory/frtb/types.rs](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs). [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates explicit one-percent/point-one-percent examples. These pin arithmetic and selection, not legal eligibility. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
