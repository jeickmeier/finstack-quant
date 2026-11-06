# SCENARIOS-001: Specification validation and ordered composition

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: a scenario is an ordered list of typed operations, not a vector of additive factor moves. `ScenarioSpec::compose` concatenates validated lists after a stable ascending-priority sort. This preserves percentage compounding, assignments, replay dependencies and user order among equal priorities. This rationale follows the implementation; no historical adoption is asserted.

## Domain and ordered algorithm

1. Validate each constituent before composing, retaining its ID in any validation error. Empty input creates ID `composed`, name `Composed Scenario`, no operations and default policies.
2. Reject differing `HazardBumpMode` values even when a constituent has no credit operation. Sort ascending priority stably; join source IDs with `+`, names with ` + `, and available descriptions with ` + `. The output priority is the first sorted priority.
3. Retain an agreed hierarchy resolution mode; if constituents disagree, select `Cumulative`. Concatenate all operation vectors without cancellation, netting or deduplication. More than one time-roll operation errors rather than adding roll periods.
4. Each execution validates nonblank scenario ID, at most one roll, and each operation. `from_json` also performs semantic validation; direct serde construction does not call `validate` automatically.

Finite changes are mandatory. FX base and quote must differ and FX percentage must be greater than -100%; price percentages permit exactly -100% but reject lower values. Correlation changes are decimal points bounded to [-2,2], where 0.01 means one percentage point. Rate/commodity node lists must be nonempty and contain nonblank tenor text with finite values; actual tenor parsing occurs in adapters. Curve IDs, statement IDs and binding IDs must be nonblank. Instrument attribute maps must be nonempty to prevent accidental universal matching. Empty instrument-type lists are permitted and match nothing. Hierarchy paths must contain at least one element. Rate-binding and roll tenors are parsed at validation; the forward-only roll constraint is checked after dates are resolved.

`Curve*Bp` uses `bp/10,000` for rate/spread kinds but **percent** for `Commodity`. Volatility and forecast percentages have no lower-bound validation here. Surface bucket filter entries, base-correlation detachment integers, individual equity ID strings, and individual hierarchy path components are not all semantically checked at this boundary; downstream adapters determine their outcomes.

## Selection, failure and alternatives

Defaults are `SolveToPar`, `TenorMatchMode::Interpolate`, `TimeRollMode::BusinessDays`, `apply_shocks=true`, and the core hierarchy default `MostSpecificWins`. Priority controls composition order only; the engine has its own execution phases. Instrument requirements distinguish mutating shocks from time roll: time roll can execute without an inventory, while instrument mutations cannot.

A factor-netting composer would erase noncommutative semantics. Rejecting mixed hierarchy modes would make policy conflicts explicit; the observed cumulative fallback may increase stress on overlapping targets. Strict validation of all targeting fields at load time would move some failures earlier but does not replace market-dependent validation.

## Evidence

Owning source: [spec.rs](../../../../finstack-quant/scenarios/src/spec.rs). Inspected [engine tests](../../../../finstack-quant/scenarios/src/engine/tests.rs) assert rejection of two rolls, stable composed identity, retention of an agreed first-order hazard mode and rejection of mixed hazard modes. The test named `compose_falls_back_to_solve_to_par_when_modes_disagree` actually asserts an error; its name is stale, not evidence of a runtime fallback. [Composition edge tests](../../../../finstack-quant/scenarios/tests/engine/engine_edge_cases_test.rs) locate same-priority and differing-priority ordering checks. These are contract tests, not economic calibration evidence.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
