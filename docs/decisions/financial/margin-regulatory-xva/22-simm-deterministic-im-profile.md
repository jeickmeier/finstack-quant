# MARGIN-REGULATORY-XVA-022: One-time SIMM calculation and deterministic IM decay

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

The SIMM-to-IM bridge computes today's SIMM once, then applies a chosen deterministic decay over supplied future times. It is a projection proxy; it does not recalculate future sensitivities, simulate collateral calls, apply MPOR exposure dynamics, or estimate an expected pathwise future SIMM distribution.

## Algorithm and conventions

1. Require a nonempty time grid of finite, strictly positive, strictly increasing times. Zero is invalid, unlike the exposure-profile grid. Validate decay maturity finite/positive for Linear and Sqrt modes.
2. Call `SimmCalculator::calculate_from_sensitivities_parts` once with the supplied sensitivities and reporting currency; take the scalar total IM. The detailed SIMM aggregation belongs to the separate margin-collateral-im scope.
3. Constant decay uses factor one; Linear uses `max(1−t/T,0)`; Sqrt uses `sqrt(max(1−t/T,0))`. Multiply the same base IM by each sample's factor.
4. Return an ImProfile with those times and values, requiring equal lengths and finite nonnegative amounts. No date/currency metadata or decay provenance is attached to that profile.

## Selection and controls

The caller selects the decay enum, its maturity and grid. The currency argument delegates classification/scaling to the current SIMM calculator; this bridge performs no FX conversion. For supplied profiles, only shape/value checks apply: callers may instead provide an independently generated expected IM profile. The public decay-factor helper does not validate its scalar t, while this bridge validates the entire grid before using it.

## Limitations and failure behavior

MVA and DVA hold the **first already-decayed IM value** flat back to time zero because ImProfile prohibits an explicit zero-time node. A single node at T with Linear/Sqrt decay therefore generates zero IM and zero MVA, despite positive today's SIMM. This is a confirmed consequence of the selected grid/decay rules, not a measured numerical error. Negative or nonfinite t passed directly to the low-level helper lacks the bridge's validation.

## Alternatives and tradeoffs

Recomputing SIMM along future risk paths captures changing portfolio risk at significant cost; projecting selected sensitivities/Greeks is an intermediate model requiring its own controls. Allowing an explicit t=0 IM anchor would align with exposure profiles but changes the present profile domain and integration contract.

## Evidence and role

Runtime bridge, validation and decay: [src/xva/mva.rs](../../../../finstack-quant/margin/src/xva/mva.rs); SIMM implementation is linked from [collateral/IM methodology scope](../margin-collateral-im/README.md). Located unit tests cover decay values and SIMM-profile construction; inspected MVA linear-grid example demonstrates how the first sampled IM is held back to zero. They are functional examples, not calibration to future margin. [SIMM reference index](../../../REFERENCES.md#isda-simm), [Green XVA](../../../REFERENCES.md#green-xva).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
