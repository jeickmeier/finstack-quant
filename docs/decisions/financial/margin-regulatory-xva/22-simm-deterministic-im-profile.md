# MARGIN-REGULATORY-XVA-022: One-time SIMM calculation and deterministic IM decay

- Decision status: **Accepted 2026-11-17 for opening-anchored deterministic IM and qualified external expected IM profiles.**
- Implementation status: The SIMM decay bridge is implemented. Opening-node admission, authoritative SIMM and profile provenance, and qualified external profile admission require reconciliation.
- Validation status: Exact arithmetic checked linear runoff. Rust fixtures were inspected. Profile admission and proxy accuracy against simulated future IM require qualification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision

1. Use one qualified current SIMM result and anchor the projected IM profile to that value at time zero. Record valuation cut, currency, portfolio scope, SIMM edition, and selected decay rule. [SIMM assembly](../margin-collateral-im/11-simm-total-and-mpor.md).
2. Explicitly select constant, linear, or square-root decay. Constant holds the opening value through the declared horizon. Linear and square-root runoff require a declared maturity and reach zero there. Include a maturity node when runoff occurs within the projection horizon.
3. Identify deterministic decay as an MVA approximation. Admit a qualified external expected posted-IM profile with its own opening value and provenance. Use the profile for MVA. Credit adjustments retain the collateral-aware exposure basis under [XVA 020](20-xva-credit-adjustments.md). [Simulated future IM research](https://arxiv.org/abs/1405.0508).

With opening IM 100 and linear runoff to one year, the profile values at years 0, 0.5, and 1 are 100, 50, and 0.

## Observed choice and domain

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

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
