# STATEMENTS-CONTRACTS-013: Missing cells, non-finite values and sign diagnostics

**Decision status:** Accepted for the full Statements data-quality contract (2026-11-15).

**Implementation status:** Reconciliation pending. The audited checks can pass on empty coverage or warn on non-finite input.

**Validation status:** Existing source and regression evidence mapped. Independent qualification of coverage, invalid inputs and sign diagnostics remains pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for data-quality coverage and validity

1. Configure the required node and period scope explicitly, selecting actuals, forecasts or both. Missing or unavailable required observations make the covered scope incomplete. Optional gaps are allowed. Report evaluated, missing and unavailable observations, and mark empty scope unevaluated.
2. Treat an explicit typed zero as a valid observation. A result that is undefined for a documented reason is unavailable. Raw NaN, infinity or a value unrepresentable as its declared `Money` is an error, including on optional cells. [Missing and invalid data](../conventions/0009-missing-and-invalid-data.md).
3. Select validity checks by named nodes or by explicitly selecting all modeled nodes. Reject unknown or duplicate configured IDs, off-plan cells and unresolved or contradictory units. [Model timeline and explicit values](01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness) · [Formula units](05-currency-dimensions.md#accepted-decision-for-formula-units-and-currency).
4. Apply advisory sign diagnostics to finite values. Zero is valid in either sign group, and overlapping groups are invalid. Each operation owns its mandatory sign rules. [Retained-earnings dividend signs](12-retained-earnings-reconciliation.md#accepted-decision-for-retained-earnings-reconciliation).

## Audited input domain

Three separate checks distinguish absent observations, present non-finite values and unexpected signs. Their coverage and severities differ deliberately; no one check is a substitute for the others.

## Audited algorithm and controls

| Check | Iteration and predicate | Finding and result |
|---|---|---|
| MissingValueCheck | Required node IDs × model periods selected by AllPeriods, ActualsOnly or ForecastOnly; result lookup returns None | Missing actual = Error; missing forecast = Warning; only Error fails |
| NonFiniteCheck | All result nodes if configured node list is empty, otherwise selected result nodes; every stored period cell | NaN or ±infinity = Error |
| SignConventionCheck | Every model period; positive-node group first, then negative-node group | Positive node <0 or negative node >0 = Warning; non-finite present value also Warning |

MissingValueCheck does not treat a present NaN/infinity as missing. NonFiniteCheck does not create findings for absent nodes or absent cells; it can inspect stored periods outside the model timeline. SignConventionCheck skips absent cells, permits zero in either sign group, and is advisory-only. An ID repeated in configured lists can create duplicate findings.

The separate SignConventionPolicy helper emits a Warning for a negative finite MagnitudePositive input, emits nothing for InflowPositive, and leaves non-finite handling to its caller. The retained-earnings check combines that helper with prior finite-data guards.

## Limitations and alternatives

An empty required-node list makes the missing-value check vacuous. An explicit nonexistent NonFiniteCheck node is ignored. A SignConventionCheck can pass despite missing observations, unexpected signs and non-finite values because it reports only warnings. These are observable contracts, not proof that a complete or clean dataset passed.

The audited check suite leaves composition to callers. The accepted data-quality contract specifies coverage and validity for the configured checks.

## Evidence

[Missing-value source](../../../../finstack-quant/statements/src/checks/builtins/missing_values.rs), [non-finite source](../../../../finstack-quant/statements/src/checks/builtins/non_finite.rs), [sign source](../../../../finstack-quant/statements/src/checks/builtins/sign_convention.rs), [data-quality tests](../../../../finstack-quant/statements/tests/checks/data_quality_tests.rs) and [property tests](../../../../finstack-quant/statements/tests/checks/proptest_checks.rs) provide direct predicate/invariant evidence. Tests cover actual versus forecast severity, period scope and all three non-finite classes.

The [invalid/missing/undefined policy](../conventions/0009-missing-and-invalid-data.md) requires operation-specific treatment; its performance-analytics decisions do not automatically adopt these statements choices.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
