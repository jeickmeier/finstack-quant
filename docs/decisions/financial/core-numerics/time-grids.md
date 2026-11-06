# CORE-NUMERICS-20: Simulation grids and event-to-step mapping

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/time_grid.rs`: zero-origin continuous time grids, optional required times, exercise-time snapping, and date-based step mapping. It does not choose a product timestep, calendar, or numerical scheme.

## Choice and rationale

Uniform grids store the exact terminal time; required times are merged into the grid so product events can be represented. Separate nearest-step helpers snap events onto an existing uniform grid. These approaches have materially different event timing and must not be treated as equivalent.

## Algorithm

1. `uniform(T,n)` requires finite T>0, n>0 and allocatable capacity. Emit 0, iT/n for interior i, and exact T; store n−1 copies of T/n and final dt=T−last interior time. It does not pass through the custom-grid minimum-dt validator.
2. `uniform_with_required_times(T,density,min_steps,events)` uses `n=max(round(T*density),min_steps)`, creates uniform knots, accepts only finite event times>1e-10 up to T, and snaps events less than 1e-10 beyond T onto T. Others are ignored.
3. Sort knots, deduplicate differences<1e-10, force last knot=T, then call `from_times`. That constructor requires nonempty grid beginning numerically at zero, finite strictly increasing times, finite differences and every dt≥1e-10. A singleton [0] is valid.
4. `map_exercise_dates_to_steps` accepts ratios in `[0,1+0.5/steps]`, rounds `ratio*steps`, clamps to final step, then sorts/deduplicates. Invalid ratios are ignored. Nonpositive total time or zero steps returns empty.
5. `map_date_to_step` computes maturity/event year fractions with caller day count/context, clamps event time into [0,T], rounds to nearest uniform step; T≤0 or zero steps returns 0. Day-count errors propagate.

## Selection, controls and conventions

T, density, minimum count, event times, and day count are explicit. Times must share units, normally years; tolerances are absolute time units. Indexing accessors assume a valid index. `is_uniform` compares all stored dt values with the first within 1e-10.

## Limitations, failure and fallback

Required-time construction silently drops invalid/out-of-range inputs and merges close events. Nearest-step mapping changes timing by up to half a step and can include slightly beyond-maturity events. Uniform and custom construction differ on tiny dt acceptance. No grid-convergence evidence or event-preservation guarantee follows from allocation success.

## Alternatives and tradeoffs

Merge required events for exact represented timing, or snap to a fixed uniform grid for schemes requiring uniform spacing. Both are implemented explicitly. Adaptive refinement and rejection of every invalid event are not automatic options.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/time_grid.rs). `test_uniform_final_knot_is_exactly_t_max`, `test_uniform_with_required_times_merges_and_dedups_events`, and `test_map_date_to_step_clamps_and_handles_degenerate_grid` establish construction/mapping behavior; they do not establish any pricing convergence tolerance.
- Generic grid construction has no separate canonical financial reference requirement; product PDE/tree/MC records must cite their own discretization choices.

Dropped exercise dates do not establish a validated product contract.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
