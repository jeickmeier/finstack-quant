# MODELS-NUMERICAL-015: Post-step penalty projection for early exercise

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and algorithm

[exercise.rs](../../../../finstack-quant/models/src/pde/exercise.rs) implements an operator-split obstacle update, not a simultaneous linear complementarity solve. After each selected PDE interval, for any u<g apply u←u/(1+P)+P g/(1+P), repeated `iterations` times. Default P=1e8 and one iteration. The update is independent of dt after validating dt>0; the factor equals λdt in the motivating formulation. It approaches intrinsic from below and may retain a finite shortfall.

Before the update, record every adjacent interior pair whose strict continuation status `(u>g)` changes. The solver returns each transition as (t_to, coordinate of its right-hand node), including multiple regions. This is a grid diagnostic, not a solved continuous free boundary. All-continuation or all-exercise states produce no transition points.

## Selection and domain

[solver.rs](../../../../finstack-quant/models/src/pde/solver.rs) enables American at every post-step time level, or Bermudan only within 1e−10 of supplied times. Bermudan times must be finite, inside [0,T] and already aligned to the uniform grid; the solver rejects off-grid times instead of snapping/refining. Exercise payoff length must equal the number of interior nodes and all payoff/continuation values must be finite. P must be finite positive, with iterations≥1. The public builder exposes exercise payoffs/schedules but not custom penalty tuning; the penalty type's module is private.

## Limitations and alternatives

The maturity condition is supplied by the problem; an exercise time at T does not run a separate projection before the first interval. American exercise is a time-grid approximation, with no automatic refinement or repeated post-exercise damping. A policy iteration, projected SOR or coupled penalty/LCP method would be a different implementation. Do not label the convex pull as a converged global complementarity solution.

## Evidence

Inline `american_penalty_enforces_floor`, `exercise_boundaries_cover_calls_and_multiple_regions`, and `exercise_rejects_malformed_inputs_before_mutation` test local behavior. `w08_rannacher_american_put_price_matches_implicit_within_discretisation_error` compares two versions of the same PDE/obstacle kernel with a 0.5% relative tolerance; it is not an independent American reference. [Broadie–Detemple](../../../REFERENCES.md#broadie-detemple-1996) is a potential benchmark-family reference, not evidence approving this projection.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
