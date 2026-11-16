# SCENARIOS-001: Specification validation and ordered composition

- Decision status: **Accepted 2026-11-15 — complete specification validation, ordered composition and roll-admission package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent ordering, percentage compounding and ID collision checks completed.** Production execution, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked scenario specifications and ordered composition

1. **Compose ordered blocks.** Stably sort the immediate inputs by
   ascending priority, retaining supplied order on ties. Concatenate
   their operation lists intact. A previously composed input remains
   one block. Reuse the lowest input priority on the result and use
   `0` for an empty input. Preserve repeated actions and intermediate
   states without netting, cancellation or deduplication. Priority
   governs composition. [Execution record 002](02-execution-and-partial-success.md)
   owns the application phases.

2. **Require an explicit result ID and one hierarchy policy.** The
   caller supplies the exact nonblank result ID. Admit a named empty
   scenario as a reference case. Keep `MostSpecificWins` as the
   default, persist the effective policy, and reject different
   effective policies even if one input has no hierarchy action.
   The default also applies to empty composition.
   [Accepted hierarchy rules](../core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap) ·
   [Hierarchy record 003](03-hierarchy-resolution.md).

3. **Validate through one checked Rust contract.** Use the same
   validation at checked construction, strict JSON load, restoration,
   composition and execution. Python and WASM delegate to Rust.
   Reject unknown or duplicate fields, nonfinite values, blank required
   IDs, malformed tenors or selectors, and repeated members within a
   single target list. Validate even before precedence or an intentional
   no-op. Preserve scenario ID, operation position and offending field
   in errors where applicable. Require explicit applicable node-matching
   and binding-compounding controls.
   [JSON object interoperability](https://www.rfc-editor.org/rfc/rfc8259#section-4) ·
   [Accepted compounding](../core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads).

4. **Make units and operation types explicit.** Use basis points for
   rate and spread shocks, separate relative-percent operations for
   commodity prices, absolute points for volatility indices and
   additive decimal correlation increments (`0.01` is one percentage
   point). Require FX percentages above `−100` and equity,
   instrument-price and quoted-volatility percentages at or above
   `−100`. Signed commodity and forecast shocks follow their owners'
   domains. Retire the global `HazardBumpMode` and `FirstOrderShift`.
   Author native hazard-intensity shocks and exact CDS quote replay
   as distinct operations.
   [Accepted commodity price shocks](../core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks) ·
   [Accepted hazard shocks](../core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery).

5. **Validate intentional no-ops and exact active targets.** Admit
   valid zero shocks and preserve `None` for all buckets and an explicit
   empty list for none where the grid owner supports it. Reject empty
   required target lists and attribute selectors. Bind active targets by
   exact namespace, kind and ID against the applicable execution state.
   Missing, wrong-kind or unsupported active targets fail, including
   on a zero shock.
   [Accepted typed market lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

6. **Allow one forward roll with explicit intent.** Select the roll
   method and whether shocks continue after it. A roll with
   `apply_shocks=false` must be the sole operation. Enforce this with
   the single-roll limit before execution and on the composed result.
   Active instrument mutations require an inventory. Roll carry admits
   an optional inventory. [Roll record 013](13-market-time-roll.md)
   owns resolved date rules and roll mechanics.

## Independent calculation checks

- Starting at `100`, the ordered `+10%` and `−10%` operations give
  `100 × 1.10 × 0.90 = 99`.
- Composing `[A@0, B@10]` as one block and composing it with `C@5`
  delivers `A, B, C`. The flat inputs deliver `A, C, B`.
- IDs `[a+b, c]` and `[a, b+c]` both yield `a+b+c` under the
  audit-era concatenation rule.

## Observed choice and rationale at the audit revision

Observed choice: a scenario is an ordered list of typed operations, not a vector of additive factor moves. `ScenarioSpec::compose` concatenates validated lists after a stable ascending-priority sort. This preserves percentage compounding, assignments, replay dependencies and user order among equal priorities. This rationale follows the implementation; no historical adoption is asserted.

## Domain and ordered algorithm

1. Validate each constituent before composing, retaining its ID in any validation error. Empty input creates ID `composed`, name `Composed Scenario`, no operations and default policies.
2. Reject differing `HazardBumpMode` values even when a constituent has no credit operation. Sort ascending priority stably; join source IDs with `+`, names with ` + `, and available descriptions with ` + `. The output priority is the first sorted priority.
3. Retain an agreed hierarchy resolution mode; if constituents disagree, select `Cumulative`. Concatenate all operation vectors without cancellation, netting or deduplication. More than one time-roll operation errors rather than adding roll periods.
4. Each execution validates nonblank scenario ID, at most one roll, and each operation. `from_json` also performs semantic validation; direct serde construction does not call `validate` automatically.

Live composition rejects differing hierarchy modes, and live volatility
percentage validation rejects requests below `−100%`.

Finite changes are mandatory. FX base and quote must differ and FX percentage must be greater than -100%; price percentages permit exactly -100% but reject lower values. Correlation changes are decimal points bounded to [-2,2], where 0.01 means one percentage point. Rate/commodity node lists must be nonempty and contain nonblank tenor text with finite values; actual tenor parsing occurs in adapters. Curve IDs, statement IDs and binding IDs must be nonblank. Instrument attribute maps must be nonempty to prevent accidental universal matching. Empty instrument-type lists are permitted and match nothing. Hierarchy paths must contain at least one element. Rate-binding and roll tenors are parsed at validation; the forward-only roll constraint is checked after dates are resolved.

`Curve*Bp` uses `bp/10,000` for rate/spread kinds but **percent** for `Commodity`. Volatility and forecast percentages have no lower-bound validation here. Surface bucket filter entries, base-correlation detachment integers, individual equity ID strings, and individual hierarchy path components are not all semantically checked at this boundary; downstream adapters determine their outcomes.

## Selection, failure and alternatives

Defaults are `SolveToPar`, `TenorMatchMode::Interpolate`, `TimeRollMode::BusinessDays`, `apply_shocks=true`, and the core hierarchy default `MostSpecificWins`. Priority controls composition order only; the engine has its own execution phases. Instrument requirements distinguish mutating shocks from time roll: time roll can execute without an inventory, while instrument mutations cannot.

A factor-netting composer would erase noncommutative semantics. Rejecting mixed hierarchy modes would make policy conflicts explicit; the observed cumulative fallback may increase stress on overlapping targets. Strict validation of all targeting fields at load time would move some failures earlier but does not replace market-dependent validation.

## Evidence

Owning source: [spec.rs](../../../../finstack-quant/scenarios/src/spec.rs). Inspected [engine tests](../../../../finstack-quant/scenarios/src/engine/tests.rs) assert rejection of two rolls, stable composed identity, retention of an agreed first-order hazard mode and rejection of mixed hazard modes. The test named `compose_falls_back_to_solve_to_par_when_modes_disagree` actually asserts an error; its name is stale, not evidence of a runtime fallback. [Composition edge tests](../../../../finstack-quant/scenarios/tests/engine/engine_edge_cases_test.rs) locate same-priority and differing-priority ordering checks. These are contract tests, not economic calibration evidence.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
