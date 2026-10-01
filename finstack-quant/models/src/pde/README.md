# models::pde

Finite-difference PDE engines: a 1D convection-diffusion-reaction solver with
theta / Rannacher time stepping and a penalty method for early exercise, and a
2D tensor-product solver using the Modified Craig-Sneyd ADI scheme with an
explicit cross-derivative term.

The module is problem-agnostic — `PdeProblem1D` and `PdeProblem2D` supply raw
coefficients, boundary conditions, and a terminal payoff — with two ready-made
Feynman-Kac bridges (`BlackScholesPde`, `HestonPde`) so pricers do not implement
the traits from scratch.

## Position in the stack

Depends only on `std` and `thiserror`; it reads no market data, no curves, and
no `MarketContext`. Every input is a flat `f64`.

Three instrument pricers consume it, all in `crate::instruments`:

| Pricer | Model key | Uses |
|--------|-----------|------|
| `equity/equity_option/pde_pricer.rs` | `PdeCrankNicolson1D` | `BlackScholesPde`, `Grid1D`, `Solver1D` |
| `equity/equity_option/pde2d_pricer.rs` | `PdeAdi2D` | `HestonPde`, `Grid1D`, `Grid2D`, `Solver2D` |
| `exotics/barrier_option/pde_pricer.rs` | `PdeCrankNicolson1D` | `BoundaryCondition`, `Grid1D`, `PdeProblem1D`, `Solver1D` (its own `BarrierPde`) |

Nothing here is bound in Python or WASM; the solvers are reached only through
those pricers, selected via `ModelKey::PdeCrankNicolson1D` / `ModelKey::PdeAdi2D`
(see [`../../pricer/README.md`](../../pricer/README.md)).
[`models::closed_form`](../closed_form/README.md) is the convergence anchor for
both dimensions.

The module doc comment in [`mod.rs`](mod.rs) says the module "lives under
`instruments/common/models/`". That path no longer exists; the module is at
`models/src/pde/`.

## Layout

| File | Contents |
|------|----------|
| [`mod.rs`](mod.rs) | Pipeline overview, submodule declarations, root re-exports |
| [`problem.rs`](problem.rs) | `PdeProblem1D` — `diffusion`/`convection`/`reaction`/`source`, terminal condition, two boundaries, `is_time_homogeneous` |
| [`problem2d.rs`](problem2d.rs) | `PdeProblem2D` — `diffusion_xx`/`_yy`, `mixed_diffusion`, `convection_x`/`_y`, `reaction`, `source`, terminal condition, four edge boundaries |
| [`boundary.rs`](boundary.rs) | `BoundaryCondition::{Dirichlet, Neumann, Linear}` |
| [`grid.rs`](grid.rs) | `Grid1D` (uniform / sinh-concentrated / user points), `PdeGridError`, `pub(crate)` `find_interval` and `find_nearest` |
| [`grid2d.rs`](grid2d.rs) | `Grid2D` tensor product, row-major indexing, bilinear interpolation |
| [`operator.rs`](operator.rs) | `TridiagOperator` assembly, boundary elimination, Thomas solve, `ThomasError` |
| [`operator2d.rs`](operator2d.rs) | `Operators2D` (per-line directional tridiagonals + cross-derivative coefficients), the monotone upwind switch, `apply_cross_derivative[_into]` |
| [`stepper.rs`](stepper.rs) | `TimeStepper` trait, `ThetaStepper`, `RannacherStepper`, the CFL bound, `StepperError` |
| [`adi.rs`](adi.rs) | `CraigSneydStepper` (Modified Craig-Sneyd), `AdiWorkBuffers`, `fill_boundaries` |
| [`exercise.rs`](exercise.rs) | `PenaltyExercise`, `ExerciseType::{American, Bermudan}` |
| [`solver.rs`](solver.rs) | `Solver1D`, `Solver1DBuilder`, `PdeSolution`, `PdeSolverError` |
| [`solver2d.rs`](solver2d.rs) | `Solver2D`, `PdeSolution2D`, `PdeSolver2DError` |
| [`bridge.rs`](bridge.rs) | `BlackScholesPde` in log-spot coordinates |
| [`bridge2d.rs`](bridge2d.rs) | `HestonPde` in (log-spot, variance) coordinates; the Fourier convergence anchors |

Pipelines, as in the module doc:

```text
PdeProblem1D → TridiagOperator → TimeStepper → PenaltyExercise → PdeSolution
PdeProblem2D → Operators2D     → CraigSneydStepper             → PdeSolution2D
```

## Public API vs internal plumbing

Implementation submodules are private. The supported surface is explicitly
re-exported at the `pde` root:

`CraigSneydStepper`, `BoundaryCondition`, `BlackScholesPde`, `HestonPde`,
`ExerciseType`, `PenaltyExercise`, `Grid1D`, `PdeGridError`, `Grid2D`,
`TridiagOperator`, `apply_cross_derivative`, `Operators2D`, `PdeProblem1D`,
`PdeProblem2D`, `PdeSolution`, `PdeSolverError`, `Solver1D`, `Solver1DBuilder`,
`PdeSolution2D`, `PdeSolver2DError`, `Solver2D`, `RannacherStepper`,
`StepperError`, `ThetaStepper`, and `TimeStepper`.

Work buffers, in-place operator helpers, interval searches, and test-only
steppers remain internal. The crate root re-exports the smaller set used by
most valuation engines; advanced solver components remain under `pde::*`.

Outside this directory only `Grid1D`, `Grid2D`, `BlackScholesPde`, `HestonPde`,
`BoundaryCondition`, `PdeProblem1D`, `Solver1D`, and `Solver2D` are actually
named. `TridiagOperator`, `Operators2D`, `apply_cross_derivative`,
`ThetaStepper`, `RannacherStepper`, `PenaltyExercise`, and `ExerciseType` are
public but reached only through the builders. `apply_cross_derivative` (the
allocating variant) has no caller at all outside `operator2d`'s own tests — the
ADI hot path uses `apply_cross_derivative_into`.

## Grids

`Grid1D` guarantees strictly increasing points and at least 3 of them.

| Constructor | Notes |
|-------------|-------|
| `uniform(x_min, x_max, n)` | Equal spacing |
| `sinh_concentrated(x_min, x_max, n, center, intensity)` | Concentrates near `center`, typically `ln K` |
| `from_points(Vec<f64>)` | Caller-supplied; rejects non-monotonic input |

Accessors: `n`, `n_interior` (= `n - 2`), `points`, `h_left(i)`, `h_right(i)`,
`x_min`, `x_max`, `interpolate(values, x)`. `interpolate` clamps to the boundary
value outside the domain rather than extrapolating.

The sinh map is
`x(ξ) = center + d·sinh(a_min + ξ(a_max − a_min))` with `d = intensity·(x_max − x_min)`
and `a_min`/`a_max` the `asinh` of the scaled endpoints, so it is monotone by
construction and hits the endpoints exactly (the two end nodes are overwritten
with `x_min`/`x_max` to remove floating-point drift). Smaller `intensity` means
tighter concentration; 0.05–0.5 is the usable band. Every production spot/log-spot
grid uses 0.1; the one exception is the Heston variance axis, which uses 0.15
concentrated on `theta_v` (0.2 in the put-call-parity test).

**A collapsed sinh range is an error, not a silent fallback.** When
`|a_max − a_min| < 1e-15` — because `intensity` is so large that `d` dwarfs the
domain, or `center` sits far outside it — `sinh_concentrated` returns
`PdeGridError::DegenerateConcentration` instead of quietly handing back a grid
with none of the requested strike resolution.

`Grid2D::new(x, y)` is a tensor product of two `Grid1D`s. Layout is **row-major
with `y` fastest**: `flat_index(i, j) == i * ny + j`, and `index_2d` inverts it.
`interpolate(values, x, y)` is bilinear and clamps outside the domain.

## Boundary conditions

`BoundaryCondition` is applied at assembly time by eliminating the boundary node
from the tridiagonal system and folding its contribution into an RHS correction.

| Variant | Elimination |
|---------|-------------|
| `Dirichlet(g)` | `u_bnd = g`; correction `lower[0]·g` (resp. `upper[last]·g`), coupling zeroed |
| `Neumann(g)` | **One-sided at the boundary node**: `u[0] = u[1] − h·g` with `h = h_left(1)`; `main[0] += lower[0]`, correction `−lower[0]·h·g` |
| `Linear` | `d²u/dx² = 0` ⇒ `u[0] = (1 + q)u[1] − q·u[2]`, with `q = (x[1] − x[0])/(x[2] − x[1])`; the upper edge is analogous |
| `LinearInExp` | Zero gamma in `S = exp(x)` ⇒ `u[0] = (1 + q)u[1] − q·u[2]`, with `q = (exp(x[1]) − exp(x[0]))/(exp(x[2]) − exp(x[1]))`; the upper edge is analogous |

The Neumann form is deliberately one-sided rather than a centered ghost node: a
centered ghost imposes the derivative at a different location and is wrong on a
non-uniform grid. It must stay identical to the reconstruction in
`solver::boundary_value` and `adi::fill_boundaries`, or the interior solve and
the reported boundary value disagree. `solver::tests::neumann_preserves_linear_profile_on_nonuniform_grid`
is the guard.

Both option bridges use `LinearInExp` on their deep-ITM log-spot edges to make
the option value affine in spot, with zero spot gamma. `Linear` instead makes
the value affine in the grid coordinate. The deep-OTM edges use `Dirichlet(0.0)`.

## 1D solver

`TridiagOperator::assemble(problem, grid, t)` discretizes
`a·u'' + b·u' + c·u + f` with the three-point non-uniform stencils

```text
u''  →  [ 2a/(h_m·h_s) ,  −2a/(h_m·h_p) ,  2a/(h_p·h_s) ]
u'   →  [ −b·h_p/(h_m·h_s) ,  b(h_p − h_m)/(h_m·h_p) ,  b·h_m/(h_p·h_s) ]
h_m = h_left(i),  h_p = h_right(i),  h_s = h_m + h_p
```

Both 1D and 2D assembly use the centered stencil when its off-diagonal
coefficients are nonnegative. In convection-dominated cells where either
coefficient would be negative, a first-order one-sided convection stencil
preserves their nonnegativity (see below).

The one-sided branch adds about `|b|·h/2` numerical diffusion, so positivity
does not establish pricing accuracy on a fixed mesh. Near-deterministic
payoffs need spatial refinement. For example, the call with `S=109.7`, `K=100`,
`r=0`, `q=0.10`, `sigma=0.001`, and `T=1` has an analytical value below
`1e-10`, but the production 200-node/100-step grid returns about `0.183114`.
Refining to 400/200, 800/400, and 1600/800 reduces the values to `0.079615`,
`0.026986`, and `0.005956`. Increasing only the time steps to 400 leaves
`0.179620`; spatial resolution is the limiting factor. The regression checks
nonnegative prices and decreasing absolute errors under joint refinement;
the default mesh carries no uniform accuracy guarantee in this regime.

`ThetaStepper` advances

```text
(I − θ·dt·A_to)·u_to = (I + (1−θ)·dt·A_from)·u_from
                       + dt·[ θ·(src_to + bc_to) + (1−θ)·(src_from + bc_from) ]
```

with `dt = t_from − t_to > 0`. `is_time_homogeneous() == true` collapses the two
assemblies into one. Constructors: `crank_nicolson(n)` (θ = 0.5),
`implicit(n)` (θ = 1.0), `explicit(n)` (θ = 0, debugging only), `custom(θ, n)`.
`RannacherStepper::new(implicit_steps, n_steps)` runs `implicit_steps` fully
implicit steps at the terminal condition and Crank-Nicolson thereafter, which
damps the high-frequency modes a payoff kink injects and which plain CN
propagates undamped.

`Solver1DBuilder` exposes `grid`, `crank_nicolson`, `implicit`, `rannacher`,
`american`, `bermudan`, `build`. There is no `explicit` on the builder —
construct `ThetaStepper::explicit` directly if you need it.

### Stability and failure modes

Every failure that would otherwise surface as a silent `NaN` is a typed error.

| Condition | Error |
|-----------|-------|
| θ < 0.5 and `dt` past the CFL bound | `StepperError::CflViolation` |
| `dt ≤ 0` or non-finite | `StepperError::NonPositiveStep` |
| Degenerate Thomas pivot | `ThomasError::DegeneratePivot`, wrapped as `StepperError::ThomasFailure` |
| `maturity ≤ 0` or non-finite | `PdeSolverError::NonPositiveMaturity` |
| `n_steps == 0` | `PdeSolverError::ZeroTimeSteps` |
| Missing grid / stepper on the builder | `PdeSolverError::MissingGrid` / `MissingStepper` |

The CFL check runs **only** for θ < 0.5; θ ≥ 0.5 is unconditionally stable and
is never gated. The von-Neumann bound is

```text
dt ≤ dx_min² / (2·max|a|)
```

where `dx_min` is the *smallest* adjacent spacing on the grid — a
strike-concentrated sinh grid has a minimum spacing far below its average, which
is exactly the case that breaks an explicit scheme sized off the average — and
`max|a|` is the largest interior diffusion coefficient sampled at `t_from`,
`t_to`, **and the step midpoint**. Sampling the endpoints alone misses a
local-vol surface whose variance peaks inside the step; `stepper::tests::cfl_bound_uses_max_diffusion_over_step_interval_not_just_t_from`
pins that with a tent-shaped diffusion. A vanishing diffusion returns
`f64::INFINITY` (a convection/reaction-only problem has no diffusive CFL limit).

The Thomas guard rejects a pivot that is non-finite or whose magnitude has
fallen below `THOMAS_PIVOT_REL_EPS = 1e-12` times the row scale
`max(|term_a|, |term_b|, 1.0)`. The threshold sits far above machine epsilon, so
a small but well-conditioned pivot is not flagged; a flagged pivot means
`(I − α·A)` has lost diagonal dominance, usually through a Neumann or Linear
boundary modification on a near-degenerate row.

### Early exercise

`PenaltyExercise` enforces `u ≥ payoff` after each step rather than via PSOR, so
it composes with any theta scheme without inner-iteration tuning. At a violated
node

```text
u ← (u + λ·dt·payoff) / (1 + λ·dt),    λ = penalty_factor / dt
```

With the default `penalty_factor = 1e8`, `λ·dt = 1e8 ≫ 1` and the update is
effectively a hard clamp to the payoff. American exercise is approximated by
exercise at every time level; projection does not replace time-grid convergence
or eliminate a newly introduced exercise kink.

`apply` returns `Result<Vec<usize>, ExerciseError>` containing every transition
between a binding obstacle and strictly better continuation. Each index is the
higher-coordinate node of the adjacent pair crossing a boundary. This handles
calls, puts, and multiple exercise regions; an entirely continuing or exercised
grid has no interior transition. The classification uses the unconstrained
continuation values so rounding during repeated penalty updates cannot change
it. Payoff dimensions, finite values, penalty settings and the step width are
validated before mutation. The solver builder also rejects malformed payoff
vectors before pricing starts.

`iterations` defaults to 1 and both builder methods (`american`, `bermudan`)
hard-code it; the field is `pub`, so raising it requires constructing
`PenaltyExercise` by hand. The doc comment on the struct describing the solver
as "optionally doing 2–3" overstates what `Solver1D` actually does — it calls
`apply` exactly once per exercise-eligible step.

Bermudan exercise times are matched against the step's `t_to` with a `1e-10`
absolute tolerance, so they must land on the time grid. `Solver1D` generates a
uniform grid (`TimeStepper::time_levels`), so schedule them accordingly.

### Solution and Greeks

`PdeSolution` carries `grid`, `values` (all nodes, boundaries included),
`exercise_boundary: Option<Vec<(time, spot_level)>>`, and `n_time_steps`.
`interpolate(x)` is linear. `delta(x)` and `gamma(x)` use the non-uniform
stencils **centered on the node nearest `x`**, not on the left node of the
containing interval: a left-anchored difference evaluates the slope at the cell
midpoint, up to a full cell away on a concentrated grid, which misstates a
fast-varying delta or a peaked gamma. At the first/last node `delta` falls back
to a one-sided difference.

Coordinates are the problem's, not the instrument's. Under the log-spot bridges
`delta` returns `∂V/∂x = S·∂V/∂S`; **divide by `S`** for spot delta.

No production pricer reads these accessors. All three PDE pricers return PV only
and let the metric layer produce Greeks by bump-and-revalue, so `delta`, `gamma`,
`delta_x`, `gamma_x`, and `exercise_boundary` are currently exercised by tests
alone.

## 2D solver (Modified Craig-Sneyd ADI)

`Operators2D::assemble` builds one `TridiagOperator` per interior y-level along
x (`op_x[j]`, size `nx_interior`) and one per interior x-level along y
(`op_y[i]`, size `ny_interior`), plus a flat vector of cross-derivative
coefficients. **The reaction term `c·u` is split 50/50** between the two
directional operators so the ADI splitting does not double-count it.

The mixed term is always explicit, precomputed as `a_xy / (4·hx·hy)` with
`hx = ½(h_left + h_right)` and applied against the boundary-inclusive `u_full`
via the four-point stencil

```text
∂²u/∂x∂y ≈ [ u(i+1,j+1) − u(i+1,j−1) − u(i−1,j+1) + u(i−1,j−1) ] / (4·hx·hy)
```

`node_stencil` carries the **monotone upwind switch**: the second-order central
convection stencil is used while the off-diagonals stay non-negative
(`b·h_p ≤ 2a` for `b ≥ 0`, `−b·h_m ≤ 2a` otherwise — cell Péclet at most 1), and
falls back to the first-order one-sided upwind stencil otherwise. This replaced
a global `MCS_PECLET_MAX = 4` rejection: a strongly mean-reverting Heston
configuration (large κ) now solves with locally reduced order near the variance
floor instead of erroring out, which `bridge2d::tests::heston_pde_high_kappa_solves_via_upwinding`
pins against the Fourier reference at κ = 10.

`CraigSneydStepper` implements the MCS scheme of In 't Hout & Welfert (2009), in
the form given by In 't Hout & Mishra (2010, eq. 1.4). With
`F = F₀ + F₁ + F₂` (mixed, x, y):

```text
Y₀     = uⁿ + dt·F(tₙ, uⁿ)                                        [predictor]
Yⱼ     = Yⱼ₋₁ + θ·dt·(Fⱼ(tₙ₊₁, Yⱼ) − Fⱼ(tₙ, uⁿ))        j = 1,2   [implicit sweeps]
Ŷ₀     = Y₀ + θ·dt·(F₀(tₙ₊₁, Y₂) − F₀(tₙ, uⁿ))                    [mixed corrector]
Ỹ₀     = Ŷ₀ + (½ − θ)·dt·(F(tₙ₊₁, Y₂) − F(tₙ, uⁿ))                [MCS corrector]
Ỹⱼ     = Ỹⱼ₋₁ + θ·dt·(Fⱼ(tₙ₊₁, Ỹⱼ) − Fⱼ(tₙ, uⁿ))       j = 1,2
uⁿ⁺¹   = Ỹ₂
```

The `Yⱼ` lines alone are the first-order Douglas scheme; the `Ŷ₀`/`Ỹⱼ` stages
upgrade it to second order. At θ = ½ the `Ỹ₀` line vanishes and MCS degenerates
to plain Craig-Sneyd — the type name is historical.

**What θ buys.** Corrector-less Douglas is unconditionally stable only for
θ ≥ ½; at θ = ⅓ it is an *inadmissible* scheme, unstable even for pure diffusion
and with no mixed term present. The MCS corrector lowers the admissible bound to
θ ≥ ⅓ (pure 2D diffusion) and θ ≥ ⅖ (general convection-diffusion). The stepper
runs `MCS_THETA = 1/3`, the standard literature choice for the Heston PDE; the
⅖ bound is a worst case, and ⅓ holds here because the implicit start damps the
non-smooth payoff, the Heston convection terms (`r − q − v/2`, `κ(θ − v)`) are
mild, and the mixed term is diffusion-like. The corrector does not "stabilize
the mixed term" — it lowers the admissible θ. `bridge2d::tests::heston_pde_mcs_vs_douglas_high_correlation`
checks pricing, stability, and second-order time convergence on a fixed spatial
grid. It uses differences between successive time refinements to cancel spatial
bias; the absolute Fourier errors of different schemes have no guaranteed
ordering because spatial and time errors can cancel.

`CraigSneydStepper::with_damping(implicit_start, n_steps)` replaces the first
`implicit_start` intervals with pairs of factorized implicit-Euler halfsteps.
Each halfstep solves the x and y directional operators implicitly and keeps
the mixed derivative explicit. This first-order directional split damps stiff
payoff modes; it is not a fully coupled backward-Euler solve, nor MCS with θ=1.
Note the stepper does **not** implement `TimeStepper`
(that trait is 1D-only); it carries its own inherent `step`,
`step_with_buffers`, `n_steps`, and `time_levels`. `Solver2D` holds it
concretely, so there is no 2D stepper polymorphism.

`AdiWorkBuffers` holds the fourteen scratch vectors one step needs.
`Solver2D::solve` allocates them once per solve and reuses them across the whole
time march; the allocating `step` is a convenience wrapper that builds fresh
buffers per call.

`fill_boundaries` writes the interior into `u_full`, then fills the two y-edges
across interior `i`, followed by the x-edges across all `j`. The corners thus
use current y-edge values. It runs after each MCS step — the mixed corrector needs a
boundary-inclusive `Y₂` for its four-point stencil — and again at `t = 0`.

**`Solver2D` has no early exercise.** `Solver2D::new` takes only a grid and a
`CraigSneydStepper` (plain or `with_damping`), and the `Solver2D` struct has
no exercise field. `PdeSolution2D` likewise exposes only `interpolate`,
`delta_x`, and `gamma_x` — there is no variance-direction sensitivity.

`PdeSolver2DError` mirrors the 1D error set minus the grid variant:
`MissingGrid`, `MissingStepper`, `NonPositiveMaturity`, `ZeroTimeSteps`, and
`Stepper` (wrapping the shared `StepperError`).

## Bridges and convergence anchors

`BlackScholesPde` solves, in `x = ln S`,

```text
∂u/∂t = ½σ²·∂²u/∂x² + (r − q − ½σ²)·∂u/∂x − r·u
```

with `Dirichlet(0)` on the deep-OTM edge and `LinearInExp` on the deep-ITM edge, and
reports `is_time_homogeneous() == true`. `bridge::tests` grades it against
`finstack_quant_core::math::norm_cdf`-based Black-Scholes at 301 sinh points and
300 CN steps: relative error below 1e-3 for both a call and a put.

`HestonPde` solves, in `x = ln S` and `y = v`,

```text
∂u/∂t = ½v·∂²u/∂x² + ½σ_v²v·∂²u/∂y² + ρσ_v v·∂²u/∂x∂y
      + (r − q − ½v)·∂u/∂x + κ(θ − v)·∂u/∂y − r·u
```

Variance is floored with `y.max(0.0)` in every diffusion and in `convection_x`,
but not in `convection_y`, so the mean-reversion drift stays signed. Both `v`
edges use `Linear`; log-spot far fields use `LinearInExp` to impose zero spot
gamma (`u_xx = u_x`). The PDE degenerates at `v → 0`, and the option value is
insensitive at `v → ∞`.

The documented anchor is against
[`closed_form::heston::heston_call_price_fourier`](../closed_form/README.md):

| Test | Runs by default | Configuration | Tolerance |
|------|-----------------|---------------|-----------|
| `heston_pde_vs_fourier_coarse_anchor` | yes | K ∈ {100, 120}, 141 × 61 grid, MCS + implicit damping(4), 150 steps, ρ = −0.7 | 2.5% |
| `heston_pde_high_kappa_solves_via_upwinding` | yes | κ = 10, same grid | 2.5% |
| `heston_pde_put_call_parity` | yes | 121 × 51 grid, 200 MCS steps | 2% |
| `heston_pde_vs_fourier_atm` | `#[ignore]` | 201 × 81 grid, 400 steps | 2% |
| `heston_pde_mcs_vs_douglas_high_correlation` | `#[ignore]` | ρ = −0.9, MCS vs Douglas θ=⅓ and θ=½ | MCS < 1e-3 |
| `heston_pde_mcs_positive_correlation` | `#[ignore]` | ρ = +0.9 | < 1e-3 |

The **OTM K = 120 leg of the coarse anchor is the load-bearing one**. Put-call
parity is insensitive to the mixed-derivative term and to the variance dynamics
(`C − P` satisfies a driftless linear PDE regardless), and the tight-tolerance
Fourier tests are `#[ignore]`d as slow — so without an always-running OTM anchor
a wrong-signed `ρσ_v·v` cross term would pass the default suite. At ATM the
ρ-sensitivity is smallest; at K = 120 with ρ = −0.7 the skew moves the price by
far more than the tolerance. The ρ = +0.9 companion covers the opposite cross-
stencil sign.

## Production configurations

For reference when reading a PDE price, the defaults the three pricers use:

| Pricer | Grid | Steps | Notes |
|--------|------|-------|-------|
| Equity option 1D | 200 sinh points, centered on `ln K`, intensity 0.1, domain `[min(ln S, ln K) − 5σ√t, max(ln S, ln K) + 5σ√t]` | 100, Rannacher(4) | Rannacher on both European and American; American adds `PenaltyExercise`; Bermudan is rejected |
| Equity option 2D Heston | 200 x-points (sinh on `ln S`, intensity 0.1) × 80 v-points (sinh on `theta_v`, intensity 0.15) | 100, MCS + implicit damping(2) | European only; future discrete dividends rejected |
| Barrier option 1D | 200 sinh points, centered on `ln K`, intensity 0.1, domain truncated so the barrier lands on the edge node | 100, Rannacher(2) | Knock-out via `Dirichlet(0)` at the barrier; knock-in as `Vanilla − KO` on a vanilla grid that *extends* the KO grid, so shared nodes cancel discretization error rather than summing two independent grid errors |

## Verification

```bash
# Colocated unit tests (never `cargo test` — it would also run doc tests).
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/models::pde/)'

# One file at a time.
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/pde::stepper/)'
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/pde::bridge2d/)'

# The #[ignore]d Fourier convergence anchors.
mise run rust-test-slow

# Instrument-level tests for both model keys.
cargo nextest run -p finstack-quant-valuations --test valuations instruments \
  -E 'test(/equity_option::test_alt_models/)'

mise run rust-test
mise run rust-lint
```

Tests are colocated in each file. Coverage is heaviest on the failure paths
added to replace silent `NaN`: CFL violation (including the interval-max
diffusion case), non-positive `dt` and maturity, zero time steps, degenerate
Thomas pivot, degenerate sinh concentration, and iteration-count invariance of
the exercise boundary. Correctness is anchored on the analytic heat equation
(1D and 2D), Black-Scholes closed form, and the Heston Fourier pricer.

No Criterion bench targets these engines, and no existing bench selects
`ModelKey::PdeCrankNicolson1D` or `PdeAdi2D` — PDE pricing is currently
unmeasured. See [`../../../benches/README.md`](../../../benches/README.md).

## Adding a scheme or a problem

1. **A new PDE**: implement `PdeProblem1D` or `PdeProblem2D` on a plain struct
   of `f64` parameters. Work in log-spot so diffusion is constant for flat vol,
   convection is `r − q − ½σ²`, and the payoff kink lands on `ln K`. Return
   `true` from `is_time_homogeneous` whenever the coefficients are static — it
   halves the per-step operator assembly.
2. Pick boundaries deliberately: `Dirichlet(0)` where the value genuinely
   vanishes, `Linear` for zero curvature in the grid coordinate, `LinearInExp`
   for zero spot gamma on log-spot grids, and `Neumann` only if you
   know the derivative — and remember the one-sided discretization above.
3. **A new 1D time scheme**: implement `TimeStepper`. Return
   `StepperError::NonPositiveStep` on a non-positive or non-finite `dt`, and gate
   any conditionally stable scheme on `cfl_max_dt` rather than letting it produce
   `NaN`. Propagate `ThomasError` with `?` — `StepperError` has the `#[from]`.
4. **A new 2D time scheme** needs more than a trait impl: `Solver2D` holds
   `CraigSneydStepper` concretely, so a second scheme means introducing a 2D
   stepper trait first.
5. Declare the file in [`mod.rs`](mod.rs) and add the root re-export.
6. Anchor it. Every scheme here is graded against an independent closed form —
   the heat equation for the operators, `closed_form` Black-Scholes for 1D,
   `closed_form::heston` for 2D. Add at least one always-running case that is
   sensitive to the term you introduced; a parity or symmetry identity alone is
   not enough (see the K = 120 note above).
7. Cite the scheme in the module doc with author, year, and journal per
   [`.agents/rules/rust/documentation.md`](../../../../../.agents/rules/rust/documentation.md),
   and add a `docs/REFERENCES.md#anchor` where one exists.

## References

- Craig, I. J. D. & Sneyd, A. D. (1988). "An alternating-direction implicit
  scheme for parabolic equations with mixed derivatives." *Computers &
  Mathematics with Applications*, 16(4), 341-350.
- In 't Hout, K. J. & Welfert, B. D. (2009). "Unconditional stability of
  second-order ADI schemes applied to multi-dimensional diffusion equations with
  mixed derivative terms." *Applied Numerical Mathematics*, 59(3-4), 677-692.
  [`docs/REFERENCES.md#in-t-hout-welfert-2009`](../../../../../docs/REFERENCES.md#in-t-hout-welfert-2009)
- In 't Hout, K. J. & Mishra, C. (2010). "Stability of the Modified Craig-Sneyd
  scheme for two-dimensional convection-diffusion equations with mixed
  derivative term." arXiv:1011.6528. (Source of the MCS form in eq. 1.4 above
  and of the θ ≥ ⅓ / θ ≥ ⅖ bounds.)
- In 't Hout, K. J. & Foulon, S. (2010). "ADI finite difference schemes for
  option pricing in the Heston model with correlation." *International Journal of
  Numerical Analysis and Modeling*, 7(2), 303-320.
- Rannacher, R. (1984). "Finite element solution of diffusion problems with
  irregular data." *Numerische Mathematik*, 43(2), 309-327. (Implicit start-up
  smoothing.)
- Forsyth, P. A. & Vetzal, K. R. (2002). "Quadratic Convergence for Valuing
  American Options Using a Penalty Method." *SIAM Journal on Scientific
  Computing*, 23(6), 2095-2122.
- Ayache, E., Forsyth, P. A. & Vetzal, K. R. (2003). "Valuation of Convertible
  Bonds with Credit Risk." *Journal of Derivatives*, 11(1), 9-29.
  [`docs/REFERENCES.md#ayache-forsyth-vetzal-2003`](../../../../../docs/REFERENCES.md#ayache-forsyth-vetzal-2003)
- Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic
  Volatility with Applications to Bond and Currency Options." *Review of
  Financial Studies*, 6(2), 327-343.
  [`docs/REFERENCES.md#heston-1993`](../../../../../docs/REFERENCES.md#heston-1993)
- Duffy, D. J. (2006). *Finite Difference Methods in Financial Engineering: A
  Partial Differential Equation Approach*. Wiley, ch. 8. (Upwinding
  convection-dominated cells.)
- Giles, M. B. & Carter, R. (2006). "Convergence analysis of Crank-Nicolson and
  Rannacher time-marching." *Journal of Computational Finance*, 9(4), 89-112.
- Thomas, L. H. (1949). *Elliptic Problems in Linear Difference Equations over a
  Network*. Watson Scientific Computing Laboratory, Columbia University.
  (Tridiagonal algorithm.)

Full bibliography with stable anchors:
[docs/REFERENCES.md](../../../../../docs/REFERENCES.md).

## Related

- [`../closed_form/README.md`](../closed_form/README.md) — the analytic reference the PDE engines are graded against
- [`../trees/README.md`](../trees/README.md) — the lattice alternative for early exercise
- [`../credit/README.md`](../credit/README.md), [`../volatility/README.md`](../volatility/README.md) — sibling model families
- [`../../pricer/README.md`](../../pricer/README.md) — `ModelKey::PdeCrankNicolson1D` / `PdeAdi2D` dispatch
- [`../../instruments/README.md`](../../instruments/README.md) — the pricers that call in here
