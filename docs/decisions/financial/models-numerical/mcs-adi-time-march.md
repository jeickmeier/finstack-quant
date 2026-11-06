# MODELS-NUMERICAL-016: Modified Craig–Sneyd ADI and split startup damping

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and formulation

[adi.rs](../../../../finstack-quant/models/src/pde/adi.rs) splits F=F0+F1+F2 into explicit mixed derivative and implicit x/y line operators. For ordinary steps θ=1/3. Compute Y0=u+hF_old(u). For j=1,2 solve Yj=Yj−1+θh(Fj_new(Yj)−Fj_old(u)). Form Z0=Y0+θh(F0_new(Y2)−F0_old(u))+(1/2−θ)h(F_new(Y2)−F_old(u)). Repeat the two directional solves from Z0 using the same old directional terms; the final result is the new interior solution. Each line solve uses Thomas and affine boundary/source corrections.

## Startup, selection and controls

`McsStepper::new(N)` has no startup damping. `with_damping(k,N)` replaces each of the first k intervals with two halfsteps of factorized implicit Euler: predictor u+hF0_old(u), then implicit x and y solves, with no MCS correction. Thus the mixed derivative remains explicit even during damping. This differs from the full-interval 1D startup. The bare Douglas selector exists only under tests; no production caller can select it. [equity pde2d caller](../../../../finstack-quant/valuations/equity/src/equity_option/pde2d_pricer.rs) chooses two damped intervals.

[solver2d.rs](../../../../finstack-quant/models/src/pde/solver2d.rs) requires positive finite maturity and N>0, initializes terminal full/interior arrays, enforces boundaries at maturity, marches uniform intervals, and reconstructs t=0 edges. It has no obstacle/early-exercise support. Homogeneous operator caches are scoped to each solve, with scalar edge updates; mixed terms use refreshed full boundary arrays.

## Limits and alternatives

The implementation checks positive step width and Thomas pivots but not ellipticity, a cross-term CFL bound, finite solution norms or residuals. Local upwinding of directional drift is not by itself a proof of full MCS stability for arbitrary callbacks, grids, correlations and damping intervals. The cited general convection-diffusion theta bound and local upwinding do not establish stability for every exposed callback and grid. Other θ choices, other ADI schemes or a fully coupled implicit solve are unimplemented alternatives.

## Evidence

`heat_2d_adi_converges` and `implicit_start_damps_stiff_modes_as_split_euler_halfsteps` inspect analytic diffusion/damping behavior. Heston bridge tests compare MCS with the repository Fourier engine; some full-resolution tests are ignored as slow. [In ’t Hout–Welfert](../../../REFERENCES.md#in-t-hout-welfert-2009) is the catalogue reference. The source also names In ’t Hout–Mishra and Craig–Sneyd, but the catalogue lacks separate entries for those citations. No test run or universal stability certification is asserted.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
