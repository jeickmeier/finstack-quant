# MODELS-NUMERICAL-011: PDE grid mapping, interpolation and coordinate Greeks

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Independent scalar, manufactured-surface and formula checks executed. Production solver, product risk and host qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked PDE grids, interpolation and coordinate Greeks

1. **Use one immutable axis with explicit construction.** Select uniform,
   caller-supplied or sinh-concentrated spatial nodes. A PDE axis includes
   both boundary nodes and has at least three finite strictly increasing
   coordinates. Treat signed zeros as equal for ordering and lookup.
   Preserve supplied knots and exact generated endpoints. Use the actual
   finite positive adjacent widths. Admit representable small widths,
   including subnormals. Reject duplicates, unrepresentable required widths
   and collapsed generated knots without sorting, snapping or perturbing
   coordinates. A full-axis span may exceed binary64 when adjacent widths
   and the requested calculation are representable.
   [Scalar interpolation](../core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics)

2. **Define uniform knots by endpoint interpolation.** For endpoints
   \(L<U\) and count \(n\), use \(t_i=i/(n-1)\) and the convex
   combination \((1-t_i)L+t_iU\) for interior nodes, evaluated with
   checked scaling. Assign the original endpoints directly. Check exact
   conversion of any integer index or denominator used as binary64.
   Use the stored widths in downstream calculations. A nominal uniform
   spacing supplies no substitute for the realized grid.

3. **Retain the endpoint-normalized sinh family.** Require a finite
   center \(c\) and finite positive dimensionless intensity \(I\).
   In real arithmetic let \(d=I(U-L)\),
   \(a=\operatorname{asinh}((L-c)/d)\),
   \(b=\operatorname{asinh}((U-c)/d)\), and
   \(x_i=c+d\sinh(a+t_i(b-a))\). Use checked scaling or
   endpoint-relative differences to preserve the formula when a
   detached intermediate overflows, underflows or cancels. Keep exact
   endpoints, including any prescribed boundary coordinate. Smaller
   \(I\) concentrates spacing toward the center. An off-domain center
   is allowed and grades toward the nearer edge. Center insertion
   requires an explicit custom axis. Remove the transformed-span
   \(10^{-15}\) floor. Admit valid near-linear results of the requested
   map and return a range error if required knots cannot be resolved.
   A failed map cannot switch the construction to uniform.
   [Sinh mesh reference](../../../REFERENCES.md#checked-pde-grids-interpolation-and-coordinate-greeks)

4. **Validate tensor geometry and resource sizes.** Form the Cartesian
   product of two admitted axes with \(n_x n_y\) nodes and
   x-major, y-contiguous indexing \(i n_y+j\). Check counts, products,
   offsets, storage bytes and reserves before allocation or indexing.
   Public spacing and indexed access are fallible outside their valid
   index ranges. Keep actual-axis work and storage within the existing
   solver and product limits. Use the accepted checked sizing principles
   without imposing the simulation time grid on spatial axes.
   [Checked time-grid sizing](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots)

5. **Bind finite values to their grid before extraction.** A successful
   solution owns its validated axis or axes and exactly the corresponding
   number of finite signed values, including the declared boundary
   values. Expose read-only access. A caller-supplied field used for
   extraction must pass the same shape and finiteness checks. Reuse
   those checks for the existing interior-only barrier extraction.
   Remove the debug-only shape assumption and public mutable
   grid/value pairings that bypass validation. Validate once per
   admitted field and avoid rescanning the complete solution on queries.

6. **Use the closed solved domain for all queries.** Reject NaN,
   infinities and a query strictly outside any axis for both value
   and coordinate derivatives. Admit exact endpoints and match stored
   knots by numerical equality. Remove implicit endpoint clamping,
   including independent x and y clamping. A boundary value or
   derivative inside the solved domain carries only the selected PDE
   closure. Pricing beyond that domain requires a product-owned
   qualified route.

7. **Keep linear and bilinear value extraction.** Use the enclosing
   cell's actual widths and fractions, with a linear blend in 1D and
   its tensor product in 2D. Use stable signed convex combinations
   and the same interpolation arithmetic as core where applicable.
   Exact node queries return the stored ordinate, including signed
   zero. An exact grid line in 2D blends along the other axis only.
   Evaluate the full required weighted result and propagate
   unresolved arithmetic.

8. **Use the local quadratic for both nodal derivatives.** At each
   interior x node select its two neighbors. At the lower and upper
   endpoint use the first and last triples, evaluated at the actual
   endpoint. For ordered triple \(a<b<c\) with ordinates
   \(u_a,u_b,u_c\), let
   \[
   l=\frac{u_b-u_a}{b-a},\qquad
   r=\frac{u_c-u_b}{c-b},\qquad
   C=\frac{r-l}{c-a}.
   \]
   The derivative estimates at the selected node \(z\) are
   \(l+C[(z-a)+(z-b)]\) and \(2C\). Use stable equivalent
   divided differences and checked complete expressions. The same
   first/last triple defines the inward endpoint estimates and
   the adjacent interior estimate.
   [Nonuniform finite differences](../../../REFERENCES.md#checked-pde-grids-interpolation-and-coordinate-greeks)

9. **Interpolate the nodal derivative estimates at the query.** In 1D
   blend the two enclosing nodal estimates. In 2D compute the x
   estimates at the required x and y nodes and use the same tensor
   interpolation as for value. These approximate derivatives at
   fixed y. Exact axes and knots use only the required stencils.
   Remove nearest-node snapping and its lower-node ties. The
   interpolation defines continuous estimates in the closed domain
   where they are numerically resolvable. These estimates and
   derivatives of the piecewise linear or bilinear price interpolant
   are distinct calculations.

10. **Check the requested output at its full scale.** A finite signed
    solution or derivative and a genuine zero are valid. Use stable
    differences, widths and weighted accumulation to avoid discarding
    representable outputs because of an unneeded intermediate or
    stencil. Remove tiny-width zero branches. Return an explicit
    input, shape, resource or numerical-range error for a failed
    request. A value query does not require a successful derivative
    calculation. An unresolved derivative cannot become zero or an
    adjacent-node estimate.

11. **Keep coordinate and market risk separate.** The Rust PDE
    extractors return derivatives with respect to x in the axis
    units. For an owner electing spot Greeks from \(x=\log S\),
    use \(\Delta=U_x/S\) and
    \(\Gamma=(U_{xx}-U_x)/S^2\) at the actual positive finite
    query spot, with the other model coordinates held fixed. Check
    the complete conversion, including signed cancellation and
    scale. The product owns quantity, currency, risk units, bump
    conventions and the choice of extraction or repricing. Current
    live product PDE Greeks use product-selected repricing.

12. **Reconcile live callers and keep the generic surface Rust-only.**
    Equity 1D and Heston 2D solve and query on their admitted log-spot
    domains. Heston admits the zero-variance edge and uses actual
    initial variance for the query. Continuous barriers keep the
    actual barrier as a boundary and the product's shared
    knock-out/vanilla knots. Discrete barrier interior-only queries
    must be covered by the actual admitted interior axis. Product
    owners choose extents, counts and concentration and own already
    touched barriers, observation projections, maturity and payoff
    limits. Existing Rust pricing and Python/WASM product routes
    propagate failures with their owning error mappings.

13. **Qualify extraction and product accuracy separately.** Verify
    exact knots and endpoints, uniform and sinh limits, off-domain
    centers, malformed shape/queries, signed and extreme scales,
    manufactured 1D/2D value and derivative fields, and the log-spot
    chain rule. Smoothness of the estimated fields does not establish
    option Greek accuracy. The three-point curvature can have
    first-order error on general asymmetric grids and at the one-sided
    endpoints even on uniform grids. Qualify mesh,
    time and truncation effects separately using independently
    expected Black–Scholes prices and Greeks, Heston prices and
    eligible barrier cases, including payoff kinks and near-edge
    queries. Spatial operators, boundary closure, time stepping,
    exercise and bridge economics retain their separate decisions
    in records 012–017.
    [Black–Scholes price and Greeks](../models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results),
    [Heston Fourier pricing](../models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing)

## Observed choice and algorithm

[grid.rs](../../../../finstack-quant/models/src/pde/grid.rs) requires at least three finite strictly increasing points. Uniform grids fix endpoints exactly. Concentrated grids set d=intensity·(xmax−xmin), a_min=asinh((xmin−center)/d), a_max likewise, then x_i=center+d sinh(a_min+i(a_max−a_min)/(N−1)). Smaller positive intensity concentrates more strongly. Center must be finite but need not lie in the domain. Degenerate sinh range (<1e−15 or nonfinite) and nonmonotone floating-point output fail; no uniform fallback is made.

1D values interpolate linearly inside the enclosing cell and clamp to endpoint values outside. [grid2d.rs](../../../../finstack-quant/models/src/pde/grid2d.rs) uses the Cartesian product, x-major/y-contiguous storage, bilinear interpolation and individually clamped cell fractions. There is no extrapolation beyond the spatial domain.

## Derivatives and controls

[solver.rs](../../../../finstack-quant/models/src/pde/solver.rs)
chooses the nearest node with ties to the lower node. Interior delta
uses the nonuniform three-point first derivative. Boundary delta
uses an inward two-point secant. Gamma uses the nearest interior node
with the equivalent three-point curvature stencil. Tiny spacing
branches return zero. [solver2d.rs](../../../../finstack-quant/models/src/pde/solver2d.rs)
uses the nearest y node for both x derivative estimates.

## Scope, alternatives and limitations

These are derivatives in grid coordinates. In x=ln S, market delta is u_x/S and gamma is (u_xx−u_x)/S²; solution accessors do not perform that conversion. Query-point Greeks jump when the selected node changes. Grid extents, counts and concentration are caller controls, with no automatic error estimator. Public interpolation helpers use debug-only shape assertions, and nonfinite queries/values are not broadly validated; callers own those invariants. Smooth derivative interpolation or adaptive grids would be alternative implementations.

## Evidence

- On x knots \((0,1,3)\) and y knots \((0,2,5)\), take
  \(u=x^2(1+y)\) and query \((2,1)\). The exact bilinear price
  interpolant gives 10. Interpolating nodal x derivatives gives
  \((8,4)\). The observed nearest-node selection gives \((2,2)\).
  At \((3,1)\) the proposed endpoint estimates give \((12,4)\).
- For \(u=x^3\), knots \((0,1,3)\) give curvature 8 at 1,
  while the exact curvature is 6. Halving both widths about 1
  halves the error. For \(u=x^2\), knots \((0,h,2h)\) with
  \(h=2^{-105}\) yield representable derivatives \(2h\) and
  2 at \(h\). Both observed spacing floors return zero.
- A five-point sinh grid on \([-1,1]\) with center 0 and
  intensity 0.5 has positive interior node
  \(\sqrt{(\sqrt 2-1)/2}\). Center 2 on \([0,1]\) grades
  toward the upper edge. A five-point grid on \([0,1]\) with
  center 0.5 and intensity \(10^{16}\) has valid knots that
  round to quarters despite the observed transformed-span gate.
  With \(M\) the largest finite binary64, uniform
  \([-M,0,M]\) has finite actual widths. Three points on
  \([1,\operatorname{nextUp}(1)]\) cannot be distinct.
- For a Black–Scholes call with spot 103, strike 100, rate 0.03,
  yield 0.02, volatility 0.2 and time 0.75, analytic price,
  delta and gamma are approximately 8.877144760390,
  0.608922913495 and 0.021056263262. Independent log-spot price
  differences reproduce \(S\Delta\) and \(S\Delta+S^2\Gamma\)
  at that smooth interior point.

Inline `sinh_grid_concentrates_near_center`, `interpolation_exact_at_nodes`, and `grid2d_interpolation_outside_domain` check construction/interpolation. `gamma_stencil_is_centred_on_nearest_node_not_left_node`, `delta_stencil_is_centred_on_nearest_node`, and `gamma_x_and_delta_x_centre_stencil_on_nearest_node` test analytic/polynomial stencils. They do not prove derivative accuracy for kinked option payoffs. [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes) is general numerical background; it does not establish query-Greek or domain-truncation accuracy for a specific product.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
