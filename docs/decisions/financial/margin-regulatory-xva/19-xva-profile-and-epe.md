# MARGIN-REGULATORY-XVA-019: XVA exposure-domain checks, opening interval and effective-EPE diagnostics

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

XVA integrates a caller-supplied expected exposure profile; it does not produce paths, revalue instruments, apply CSA collateral mechanics, or estimate simulation error. Monetary values are implicitly in one common caller-selected currency. Numeric year fractions are sent directly to discount/hazard curves without date-origin/day-count reconciliation.

## Algorithm and conventions

1. Require equal time/MTM/EPE/ENE vector lengths; finite nonnegative, strictly increasing times; finite signed MTM; finite nonnegative EPE and ENE. Reject any positive recorded market-roll or valuation failure count. An empty profile passes the type validator but CVA/DVA/FVA entry points reject it.
2. The opening interval is from time zero to the first profile node. Hold that first node's EPE/ENE constant back to zero; initialize discount and survival factors at one. An explicit time-zero profile node is allowed, contributes a zero-length interval and updates factors for later nodes.
3. During CVA traversal update `running_EPE=max(previous_running_EPE,current_EPE)` before integrating. The effective-EPE time average is the sum of this **right-endpoint running maximum** times each interval length clipped to `H=min(last_time,1)`, divided by H. If H is zero return the running maximum.
4. Store every `(time,running_EPE)` point, the original EPE/ENE/MTM profiles, sampled maximum EPE, and original diagnostics. The last exposure node defines maturity; there is no extrapolation beyond it.

## Selection and controls

No condition requires `MTM=EPE−ENE`; the inputs may be separately estimated moments. Diagnostic total-point counts are not reconciled with the vector size. Curve evaluations in the adjustment kernels reject nonfinite values but do not independently recheck positivity, survival bounds or monotonicity; normal core curve constructors provide their own domain controls.

## Limitations and failure behavior

The inspected SA-CCR engine consumes trades/MTM/collateral and does not use effective EPE; it is an XVA diagnostic here. If the first positive node is beyond one year, its EPE supplies the whole first-year diagnostic; an increase at an endpoint affects the preceding interval. This diagnostic does not establish a regulatory measure. No percentile PFE or confidence interval is calculated. Curve origin/currency/time units and completeness of failure diagnostics remain caller responsibilities.

## Alternatives and tradeoffs

Piecewise-linear exposure integration or left-continuous running maxima change coarse-grid effective EPE. Requiring an explicit valuation-time node removes opening ambiguity but changes the accepted domain. Full simulation-backed exposure generation is a separate upstream method, not an implicit feature of these integrators.

## Evidence and role

Profile domain/results: [src/xva/types.rs](../../../../finstack-quant/margin/src/xva/types.rs); integration and effective EPE: [src/xva/cva.rs](../../../../finstack-quant/margin/src/xva/cva.rs). Inspected [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) tests opening FVA for both `[1]` and `[0,1]` grids; located CVA tests cover effective-EPE monotonicity and first-year horizon. These pin conventions; monotonicity is not independent regulatory certification. Existing references: [Gregory XVA](../../../REFERENCES.md#gregory-xva-challenge), [Green XVA](../../../REFERENCES.md#green-xva).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
