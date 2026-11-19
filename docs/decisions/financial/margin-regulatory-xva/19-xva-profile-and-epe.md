# MARGIN-REGULATORY-XVA-019: XVA exposure-domain checks, opening interval and effective-EPE diagnostics

- Decision status: **Accepted 2026-11-17 for XVA exposure admission, curve alignment, and effective EPE diagnostics.**
- Implementation status: Observed behavior is retained below. Opening exposure, curve-clock and currency alignment, and completeness checks require reconciliation.
- Validation status: Exact arithmetic checked the effective EPE diagnostic. Rust fixtures were inspected. Opening-node, clock, and host validation remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision

1. Require a complete exposure grid starting at valuation time. Use strictly increasing nodes and aligned finite MTM and finite nonnegative EPE and ENE. Require one reporting currency and an identified exposure basis. Reject reported roll or valuation failures, reconcile diagnostic counts with the grid, and require completeness provenance for external profiles.
2. Map exposure dates onto each curve's declared clock from the valuation cut. Require positive discount factors and nonincreasing survival probabilities in [0, 1]. Reject missing coverage and incompatible inputs.
3. Report effective EPE as an XVA diagnostic. Time-weight the right-endpoint running maximum through the earlier of one year and the final exposure node. For EPE [20, 40, 80] at years [0, 0.5, 1.5], the diagnostic is 60. The local SA-CCR engine does not consume it. Basel specifies regulatory effective EPE under the [Internal Models Method](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/53/inforce/2023-01-01/published/2020-06-05).

## Observed choice and domain

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

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
