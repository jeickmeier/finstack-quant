# MODELS-NUMERICAL-013: PDE boundary elimination and reconstruction

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and algorithm

[boundary.rs](../../../../finstack-quant/models/src/pde/boundary.rs) defines four edge policies. Dirichlet fixes u=g and moves the missing boundary coupling into the RHS. Neumann is the derivative in the increasing coordinate direction: lower u0=u1−h g, upper uN=uN−1+h g. [operator.rs](../../../../finstack-quant/models/src/pde/operator.rs) eliminates this same one-sided value into the adjacent interior row, matching reconstruction in [solver.rs](../../../../finstack-quant/models/src/pde/solver.rs).

`Linear` extends the slope of the first/last two interior points, u_edge=u1+ratio·(u1−u2), ratio=h_boundary/h_interior. `LinearInExp` uses the exponential-coordinate spacing ratio: lower (1−exp(−hb))/(exp(hi)−1); upper (exp(hb)−1)/(1−exp(−hi)), evaluated with `expm1`. This extends an affine function of exp(x), appropriate for log-spot far fields. With only one interior node both variants extend a constant.

## Selection, ordering and controls

Problem callbacks provide edge types and scalar values at each evaluation time. Homogeneous operator caches refresh scalar Dirichlet/Neumann RHS data; this assumes fixed boundary type and fixed operator coefficients/source. [adi.rs](../../../../finstack-quant/models/src/pde/adi.rs) fills interior y edges first, then all x edges including corners, so the cross stencil consumes fresh edge values. The x-edge rule therefore has corner precedence if supplied edge conditions conflict.

## Limitations, alternatives and evidence

Neumann uses first-order one-sided boundary differences; linear/exponential extension is a closure assumption, not a remote exact boundary. The model bridge chooses these automatically, but domain size remains the caller's responsibility. Time-varying boundary types or source under a declared homogeneous problem are incompatible with cache reuse. Higher-order ghost-node, exact asymptotic or degenerate-PDE boundary closures would be different choices.

Inline `linear_boundaries_preserve_affine_profile_on_nonuniform_grid`, `neumann_preserves_linear_profile_on_nonuniform_grid`, `time_homogeneous_cache_refreshes_time_varying_boundaries_1d`, and the matching 2D tests exercise algebra and cache invariants. [bridge.rs](../../../../finstack-quant/models/src/pde/bridge.rs) includes `log_spot_far_field_converges_to_deep_itm_call_value`, a BS-formula comparison. General [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes) does not supply an adoption decision for these financial boundaries.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
