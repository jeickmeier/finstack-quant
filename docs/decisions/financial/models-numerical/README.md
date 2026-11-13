# Numerical models methodology audit

Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

The exact assigned scope is **36 Rust files** under `finstack-quant/models/src`: 28 algorithm-bearing files, 6 support files, 2 test-only files and 0 generated files. All assigned runtime implementations were inspected, including defaults, validation, numerical helpers and dispatch. The [coverage ledger](coverage.json) assigns each source exactly once. Source test bodies were inspected selectively for the evidence below; lengthy test-only fixtures were classified separately. Root/module/schema registration files add no independent pricing method. The schema catalogue is hand-written support, not generated code.

There are **17 methodology records**. [MODELS-NUMERICAL-001](equity-binomial-construction.md#accepted-decision-for-checked-crr-and-leisenreimer-lattices)
was accepted on 2026-11-11 for checked CRR and Leisen–Reimer
construction, explicit lattice work controls and exact vanilla boundaries.
[MODELS-NUMERICAL-002](dividends-and-lattice-exercise.md#accepted-decision-for-checked-escrowed-dividends-and-lattice-exercise)
was accepted on 2026-11-11 for dated ex-claim values, a common escrow
reserve horizon, exact placement and product-qualified exercise phases.
[MODELS-NUMERICAL-003](tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)
was accepted on 2026-11-11 for checked interval discounting, explicit
nodewise OAS, validated rollback and genuine terminal state.
[MODELS-NUMERICAL-004](ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting)
was accepted on 2026-11-11 for origin-relative Ho–Lee targets,
checked additive row fits and one repricing gate.
[MODELS-NUMERICAL-005](bdt-lognormal-curve-fit.md#accepted-decision-for-checked-bdt-curve-fitting)
was accepted on 2026-11-11 for strictly decreasing BDT targets,
unclamped log-rate fits and one inclusive replay gate.
[MODELS-NUMERICAL-006](black-karasinski-trinomial.md#accepted-decision-for-checked-bk-trinomial-fitting)
was accepted on 2026-11-11 for capped Black–Karasinski with exact
conditional OU moments, positive rate fitting and one inclusive
replay gate.
[MODELS-NUMERICAL-007](hull-white-event-grid-and-affine-bonds.md#accepted-decision-for-checked-hullwhite-event-lattices-and-node-bonds)
was accepted on 2026-11-11 for exact Hull–White event grids,
conditional OU moments, Continuous base fitting and explicit
affine exercise valuation.
[MODELS-NUMERICAL-008](rates-credit-marginal-calibration.md#accepted-decision-for-checked-rates-credit-marginal-calibration)
was accepted on 2026-11-11 for centered binary marginals, strict
variance retention, local integrated-hazard fitting and checked replay.
[MODELS-NUMERICAL-009](rates-credit-coupling-and-sampling.md#accepted-decision-for-checked-rates-credit-coupling-and-path-sampling)
was accepted on 2026-11-11 for strict Bernoulli coupling, indexed
Philox midpoints and bound segment replay.
[MODELS-NUMERICAL-010](rates-credit-coupon-folding.md#accepted-decision-for-checked-joint-credit-rollback-and-coupon-folding)
was accepted on 2026-11-11 for checked joint survival and recovery,
projection on exact index dates and a full coupon fold with explicit
exercise eligibility.
[MODELS-NUMERICAL-011](pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks)
was accepted on 2026-11-11 for validated spatial grids, closed-domain
value extraction and interpolated quadratic nodal derivative estimates.
[MODELS-NUMERICAL-012](pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils)
was accepted on 2026-11-11 for checked diffusion admission, an inclusive
central-to-one-sided drift switch and an explicit tensor mixed stencil.
[MODELS-NUMERICAL-013](pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction)
was accepted on 2026-11-11 for shared checked edge relations, explicit
x-edge corner ownership and stage-consistent reconstruction and cache reuse.
[MODELS-NUMERICAL-014](theta-time-march-and-thomas.md#accepted-decision-for-checked-theta-marching-and-tridiagonal-solves)
was accepted on 2026-11-11 for exact event-grid theta steps, full-interval
damping and one checked Thomas kernel with a componentwise residual gate.
[MODELS-NUMERICAL-015](pde-exercise-projection.md#accepted-decision-for-checked-grid-based-pde-exercise)
was accepted on 2026-11-11 for exact post-step payoff comparison,
exact exercise knots and actual-spot immediate exercise at eligible
equity valuations.
[MODELS-NUMERICAL-016](mcs-adi-time-march.md#accepted-decision-for-checked-modified-craigsneyd-adi)
was accepted on 2026-11-11 for fixed full MCS, checked factorized startup
and atomic stages with application-specific stability and price qualification.
[MODELS-NUMERICAL-017](black-scholes-and-heston-pde-bridges.md#accepted-decision-for-checked-pde-coefficient-bridges)
was accepted on 2026-11-11 for checked bridge parameters and diffusion,
explicit edge laws, exact deterministic limits and qualified price and
risk boundaries. All 17 trees and finite differences records are accepted.
Independent scalar and finite-lattice checks
support the packages. Runtime reconciliation, end-to-end production
pricing, independent reference qualification and actual risk paths
remain open. Calendar conversion, market resolution and contractual
exercise and recovery policy belong to callers.

## Methodology inventory

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation | Scope and evidence limits |
|---|---|---|---|---|---|
| European, American and Bermudan vanilla options | CRR moments; odd-step LR Peizer–Pratt inversion; deterministic zero-volatility branch | Explicit TreeType; equity caller selects LR, 201 steps unless overridden | [MODELS-NUMERICAL-001](equity-binomial-construction.md) | Observed behavior and independent scalar checks | Accepted strict risk-neutral support, odd LR counts and checked finite-grid premiums |
| Vanilla options with fixed cash dividends and continuous yield | Carry-discounted reserve; rounded ex-date nodes; max of continuation and cum/ex intrinsic | Nonempty schedule activates escrow; American every pre-expiry step; Bermudan mapped steps | [MODELS-NUMERICAL-002](dividends-and-lattice-exercise.md) | Observed source; independent claim, date and finite-tree checks | Accepted explicit escrow horizon and residual carry, exact dates and qualified exercise phases |
| Spot and calibrated short-rate claims through TreeValuator | Node expectations followed by discount and callback; compounding bases floored; OAS added on chosen quote convention | ShortRateTree::price explicit OAS compounding; shared binary engine except BK | [MODELS-NUMERICAL-003](tree-discounting-and-induction.md) | Observed source; independent discount, OAS and finite-tree checks | Accepted checked discounting, nodewise OAS, validated rollback and no synthetic terminal interval |
| Normal short-rate claims, including negative rates | Equal-probability additive moves; analytical continuous fit or Brent for other compounding | ShortRateModel::HoLee; generic default 100 steps, normal vol 0.01 | [MODELS-NUMERICAL-004](ho-lee-curve-fit.md) | Observed source; independent anchored-target, row-fit and option arithmetic | Accepted origin-relative targets, checked outgoing rate rows and one relative-DF fit gate |
| Positive short-rate claims without mean reversion | Equal-probability lognormal rates; capped Brent row-scale fit under selected compounding | Explicit BlackDermanToy config; callable bond uses Simple base | [MODELS-NUMERICAL-005](bdt-lognormal-curve-fit.md) | Observed source; independent feasibility, range and option arithmetic | Accepted strict base targets, unclamped log-center fit and one replay gate |
| Positive mean-reverting short-rate claims | Euler log-rate moments; shifted boundary branches; Brent additive log shift | Explicit BlackKarasinski config; stochastic kernel requires positive kappa and sigma | [MODELS-NUMERICAL-006](black-karasinski-trinomial.md) | Observed source; independent capped moment, fit and option arithmetic | Accepted exact conditional moments, same-grid zero volatility and one replay gate |
| Normal mean-reverting rates, bonds and swaptions | Mandatory-time segmentation; nearest-mean branches; Arrow–Debreu fit; separate affine conditional bond formula | Explicit HW config; default kappa 0.03, sigma 0.01, 100 steps; schedule overrides sigma per interval | [MODELS-NUMERICAL-007](hull-white-event-grid-and-affine-bonds.md) | Observed source; independent grid, OU moment, fit and affine-bond arithmetic | Accepted exact event nodes, Continuous base discount, checked replay and qualified affine exercise basis |
| Joint lattice inputs for credit-sensitive bonds and loans | Two additive-normal marginal fits; floored survival objective; clipped mean-reversion probabilities | Explicit conditional targets; default 100 steps and both volatilities zero | [MODELS-NUMERICAL-008](rates-credit-marginal-calibration.md) | Observed source; independent support, floor, survival and joint-valuation checks | Accepted centered strict binary support, local hazard fitting and checked replay and floor limits |
| Rates/credit joint transitions and sampled conditional path segments | Bernoulli coupling with local clamp; Philox stream/offset; uniform antithetics; band cache | Correlation checked at calibration; seed/path/checkpoint supplied by caller | [MODELS-NUMERICAL-009](rates-credit-coupling-and-sampling.md) | Observed source; independent coupling, cutoff and Philox checks | Accepted strict nullable correlation, common joint law, open midpoint draws and provenance-checked resumption |
| Joint bond/loan rollback and floating coupons | Risk-free conditional forward increments; risky unit-claim fold to reset; callback exercise and survival | Explicit NodeCoupon descriptors; recalculated at each OAS | [MODELS-NUMERICAL-010](rates-credit-coupon-folding.md) | Observed source; independent projection, recovery and fold arithmetic | Accepted one survival application, exact index dates, full coupon replacement and exercise eligibility |
| Generic 1D/2D PDE grids and solution extraction | Uniform/custom/sinh grids; clamped linear/bilinear interpolation; nearest-node finite differences | Grid and domain explicit; equity caller concentrates at log strike | [MODELS-NUMERICAL-011](pde-grids-interpolation-and-greeks.md) | Observed source; independent mesh, extraction and coordinate conversion checks | Accepted checked closed-domain grids and values, interpolated nodal derivative estimates and product-owned market risk |
| 1D/2D convection-diffusion-reaction equations | Three-point diffusion; central-to-upwind switch; explicit four-corner mixed stencil; split source/reaction | Internal node-by-node selection from local diffusion/drift/spacing | [MODELS-NUMERICAL-012](pde-spatial-stencils.md) | Observed source; independent nonlinear mixed, directional, affine and range checks | Accepted strict diffusion admission, exact switch and explicit tensor mixed derivatives with local accuracy bounds |
| Finite-difference edges and log-spot option far fields | Dirichlet, one-sided Neumann, linear-coordinate and linear-exp extrapolation | Problem callbacks choose each edge; 2D fills y edges before x corners | [MODELS-NUMERICAL-013](pde-boundary-elimination.md) | Observed source; independent affine, corner, stage and range checks | Accepted shared checked edge relations, explicit corner ownership and stage-consistent reconstruction and cache reuse |
| One-dimensional backward terminal-value PDEs and shared tridiagonal solves | Endpoint theta scheme; full implicit startup intervals; unpivoted Thomas with relative pivot guard | CN, implicit, full-interval damping and clamped custom theta | [MODELS-NUMERICAL-014](theta-time-march-and-thomas.md) | Observed source; independent time, event, pivot and residual checks | Accepted exact time grids, theta in [1/2,1], full-interval damping and checked Thomas solves |
| 1D American and grid-aligned Bermudan exercise | Convex pull toward intrinsic after PDE solve; transitions recorded before pull | Builder American/Bermudan; fixed factor 1e8 and one iteration | [MODELS-NUMERICAL-015](pde-exercise-projection.md) | Observed source; independent projection, schedule, interpolation and benchmark checks | Accepted exact payoff comparison, exact knots, exercise brackets and eligible actual-spot payoff |
| 2D terminal-value PDEs and European Heston pricing | MCS theta 1/3 with explicit mixed term; four sweeps; two implicit split substeps per startup interval | McsStepper::new or with_damping; Heston selects two startup intervals | [MODELS-NUMERICAL-016](mcs-adi-time-march.md) | Observed source; independent stage, stability and Heston reference checks | Accepted fixed full MCS, checked startup geometry and atomic publication with qualified stability |
| Black–Scholes and Heston vanilla PDE bridges | Log-coordinate generators; linear-exp ITM and zero OTM edges; linear variance edges | Explicit BlackScholesPde or HestonPde; both declare time homogeneity | [MODELS-NUMERICAL-017](black-scholes-and-heston-pde-bridges.md) | Observed source; independent coefficient, limit, clock, edge and Fourier checks | Accepted checked parameters and PSD coefficients, explicit closures, exact limits and financial price bounds |

## Interpretation and follow-up tracking

The records state formulas, execution ordering, input conventions, thresholds, fallback behavior and actual selection controls. They distinguish supported variants from unimplemented alternatives. Shared financial conventions and broad method references are linked to the existing [reference catalogue](../../../REFERENCES.md); absent method-specific sources and unproven acceptance limits are identified explicitly. Financially material defaults and validations are classified as algorithms even when implemented in config or trait files.

Adoption questions, implementation reconciliation and validation work are owned by the [tree and finite-difference follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences). No existing financial choice is approved or changed by this audit.
