# CORE-CONVENTIONS-020: Shared barrier direction, activation and payout-timing defaults

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `types/barrier.rs` only; valuation kernels are outside this record.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

A four-state classification identifies UpAndOut/UpAndIn/DownAndOut/DownAndIn. Default BarrierType is UpAndOut; default PayoutTiming is AtHit. These default enum values can affect downstream dispatch, so they are financially material despite containing no pricing formula. Adoption for a particular product remains pending.

## Algorithm and conventions

`is_knock_out` matches the two Out variants; `is_knock_in` is its complement; `is_up` matches the two Up variants; `direction()` maps Up to BarrierDirection::Up and the rest to Down. The docs define touching as a hit (`spot >= barrier` for up and `spot <= barrier` for down), but this file does **not** execute spot comparisons or monitor paths.

PayoutTiming has AtHit and AtExpiry. Documentation associates at-hit payments with first-passage-time discounting and at-expiry with expiry discounting; knock-in rebates are documented as expiry-only. This type does not enforce that exception, calculate first passage, apply a continuity correction, or validate a rebate against barrier type. Those are downstream algorithms with separate audit scopes.

## Selection, limitations and alternatives

Caller or enclosing instrument defaults choose the enum; serde accepts canonical snake_case names. Continuous/discrete monitoring, observation calendars, already-hit state, level validation and rebate amounts are not encoded. Both payout timings are supported labels, but availability in an actual pricer must be established at that pricer. No claim is made here that all combinations are implemented or financially approved.

## Evidence

[Owning enum/predicates and serde test](../../../../finstack-quant/core/src/types/barrier.rs). The inspected test checks canonical wire labels, not pricing or inclusive barrier-touch behavior. [Reiner–Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991) is background for barrier pricing, not evidence that a particular downstream implementation honors these semantics.

Type documentation is not numerical validation of the consuming pricing algorithms.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
