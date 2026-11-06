# MODELS-NUMERICAL-012: Nonuniform diffusion, monotone convection and mixed derivatives

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and formulation

[operator.rs](../../../../finstack-quant/models/src/pde/operator.rs) discretizes L u=a u_xx+b u_x+c u+f over interior unknowns. For neighboring widths h−,h+, diffusion coefficients are (2a/(h−H),−2a/(h−h+),2a/(h+H)), H=h−+h+. Central first-derivative weights are (−h+/(h−H),(h+−h−)/(h−h+),h−/(h+H)).

Add the central drift stencil only when b≥0 and b h+≤2a, or b<0 and −b h−≤2a. Otherwise use a forward difference for b≥0 or backward difference for b<0, preserving the off-diagonal sign of this backward pricing operator. The local order drops in convection-dominated cells; there is no global Peclet rejection. Add c to the diagonal and f to the affine source.

## Two-dimensional selection and conventions

[operator2d.rs](../../../../finstack-quant/models/src/pde/operator2d.rs) constructs one x-line operator per interior y and one y-line operator per interior x. Each receives half the reaction and half the source, preventing double counting. Mixed coefficient a_xy is stored divided by (h_x−+h_x+)(h_y−+h_y+). Application multiplies the four corner combination u++−u+−−u−++u−−, with boundary-inclusive values. ADI treats this term explicitly, independently of the directional upwind switch.

## Limitations and alternatives

On arbitrary asymmetric nonuniform grids, the secant cross stencil is exact for bilinear xy but generally contains first-order terms proportional to unequal left/right widths. Smooth sinh refinement may improve behavior; it does not establish second-order accuracy for all accepted `from_points` grids. Negative/nonfinite diffusion coefficients and general PDE ellipticity are not validated by operator construction. Local monotonicity of directional terms is not a proof of full mixed-operator positivity or ADI stability. A tensor product of full nonuniform derivative stencils is an unimplemented alternative.

## Evidence

`cross_derivative_of_product_on_non_uniform_grid` in [operator2d.rs](../../../../finstack-quant/models/src/pde/operator2d.rs) checks u=xy to 1e−10; it does not test nonlinear mixed derivatives. `mcs_step_handles_convection_dominated_grid_via_upwinding` in [adi.rs](../../../../finstack-quant/models/src/pde/adi.rs) and Heston high-κ tests are bounded examples. [MCS reference](../../../REFERENCES.md#in-t-hout-welfert-2009) supports the method family, not arbitrary-grid order or every coefficient regime.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
