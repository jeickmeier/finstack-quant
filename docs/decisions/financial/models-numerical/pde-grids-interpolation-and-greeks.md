# MODELS-NUMERICAL-011: PDE grid mapping, interpolation and coordinate Greeks

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and algorithm

[grid.rs](../../../../finstack-quant/models/src/pde/grid.rs) requires at least three finite strictly increasing points. Uniform grids fix endpoints exactly. Concentrated grids set d=intensity·(xmax−xmin), a_min=asinh((xmin−center)/d), a_max likewise, then x_i=center+d sinh(a_min+i(a_max−a_min)/(N−1)). Smaller positive intensity concentrates more strongly. Center must be finite but need not lie in the domain. Degenerate sinh range (<1e−15 or nonfinite) and nonmonotone floating-point output fail; no uniform fallback is made.

1D values interpolate linearly inside the enclosing cell and clamp to endpoint values outside. [grid2d.rs](../../../../finstack-quant/models/src/pde/grid2d.rs) uses the Cartesian product, x-major/y-contiguous storage, bilinear interpolation and individually clamped cell fractions. There is no extrapolation beyond the spatial domain.

## Derivatives and controls

[solver.rs](../../../../finstack-quant/models/src/pde/solver.rs) chooses the nearest node (ties go lower). Interior delta uses the nonuniform three-point first-derivative stencil with weights −h_plus/(h_minus·sum), (h_plus−h_minus)/(h_minus·h_plus), h_minus/(h_plus·sum). Boundary delta is one-sided; gamma chooses the nearest interior node and uses 2[(u_plus−u)/h_plus−(u−u_minus)/h_minus]/(h_plus+h_minus). Tiny spacing branches return zero. [solver2d.rs](../../../../finstack-quant/models/src/pde/solver2d.rs) snaps y to its nearest node and applies the x derivative stencils; it does not interpolate Greeks in y.

## Scope, alternatives and limitations

These are derivatives in grid coordinates. In x=ln S, market delta is u_x/S and gamma is (u_xx−u_x)/S²; solution accessors do not perform that conversion. Query-point Greeks jump when the selected node changes. Grid extents, counts and concentration are caller controls, with no automatic error estimator. Public interpolation helpers use debug-only shape assertions, and nonfinite queries/values are not broadly validated; callers own those invariants. Smooth derivative interpolation or adaptive grids would be alternative implementations.

## Evidence

Inline `sinh_grid_concentrates_near_center`, `interpolation_exact_at_nodes`, and `grid2d_interpolation_outside_domain` check construction/interpolation. `gamma_stencil_is_centred_on_nearest_node_not_left_node`, `delta_stencil_is_centred_on_nearest_node`, and `gamma_x_and_delta_x_centre_stencil_on_nearest_node` test analytic/polynomial stencils. They do not prove derivative accuracy for kinked option payoffs. [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes) is general numerical background; it does not establish query-Greek or domain-truncation accuracy for a specific product.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
