# MODELS-ANALYTIC-19: Finite-difference Dupire extraction, smoothing and local-vol interpolation

**Decision status:** Accepted (complete package adopted 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent scalar arithmetic checks support the proposed distinctions. Production extraction, simulation, pricing and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked Dupire extraction and local-volatility grids

All 25 decisions were accepted on **2026-11-10**. Implementation reconciliation
and independent complete-path qualification remain open.

1. **Use the deterministic proportional-carry Dupire law.** The
   underlying is a positive multiplicative diffusion with deterministic
   discounting and proportional carry, on one qualified model clock.
   Use normalized state `S_T/F(T)` and divide call premium per
   underlying unit by the discount factor times forward.
   For `k = log(K/F(T))` and `w = T sigma_imp²`,
   local variance is `w_T|k / g`. The forward curve supplies the
   coordinate change. Cash-dividend jumps, stochastic rates or a
   different normalized state require their own qualified model.

2. **Require a checked Black source with actual forwards.** Consume
   one validated unshifted Black annual-volatility surface on an
   absolute-strike axis, with at least two positive expiry rows and
   three positive strike columns. Require one finite positive
   forward tied to each exact source expiry, underlying and strike
   units. Preserve the source interpolation mode, clock and
   construction basis. The dated owner qualifies origin and carry
   under ADR-0045. Numeric use may carry an absent clock.

3. **Choose the output nodes explicitly.** Supply a nonempty contiguous
   rectangular range of original expiry and strike indices, which
   may be the whole grid or a singleton on either axis. Compute
   stencils from the full effective source, including any required
   rows or columns outside the requested output. No automatic
   trimming, skipped nodes or interpolation of output coordinates.

4. **Choose implied-wing support explicitly.** Select strict matched
   support or flat implied sigma wings for remapped strikes. Strict
   extraction errors when a required sample is outside the source.
   Flat wings extend the implied smile at that row's nearest
   boundary sigma and retain the requested fixed k. Flat samples
   are extension assumptions. With changing forwards a full
   rectangular grid will need a wing extension at some edges.

5. **Map matched strikes using original coordinates.** Compute k
   from the original K and row F with a stable log ratio. The
   center uses its exact original strike. In another expiry row
   use the complete ratio product `K_j = K × F_j / F_i` and reuse
   exact K for equal forwards. Classify source support against
   the complete ratio product before final strike rounding.
   For an out-of-range flat sample choose the effective endpoint
   without first forming an overflowing requested strike. Pass
   only the finite effective endpoint to the source lookup.
   For in-range sampling require resolvable positive strike geometry.

6. **Use the declared source interpolation at each row.** Sample
   the effective source at the row's exact expiry and its matched
   strike through [record 16](volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization).
   Preserve exact knots and the stored
   Vol or TotalVariance strike interpolation. Build each w from
   that row's actual time and sampled quote, in complete scale.
   The output's local grid boundary is a separate query decision.

7. **Select one explicit smoothing operation.** The single checked
   extraction accepts unsmoothed input or Gaussian smoothing
   with a finite strictly positive bandwidth in absolute strike
   units. Apply smoothing to annual implied sigma within each
   original expiry row across the full original strike grid,
   before sampling or differentiating. Preserve the axes, source
   quote convention, interpolation and clock. Use exact omission
   of smoothing for the unsmoothed operation.

8. **Define normalized Gaussian row smoothing.** At K_i take the
   weighted mean of original row sigmas with weights
   `exp(-0.5 ((K_j-K_i)/bandwidth)^2)`. Each original observation
   contributes once and the center weight is exactly one.
   Keep scaled weight-times-sigma contributions and stable
   normalization, even when a weight alone would round to zero.
   Preserve constant rows. Check work and allocation. Smoothing
   changes the input surface whose derivative is estimated.
   A change of price units scales the bandwidth with the strike axis.

9. **Take the time derivative at fixed log-forward moneyness.** At
   an interior expiry use the derivative of the local quadratic
   through the previous, current and next samples. For gaps
   `h_- = T_i - T_(i-1)`, `h_+ = T_(i+1) - T_i`, and adjacent
   slopes d_- and d_+, use `(h_+ d_- + h_- d_+) / (h_- + h_+)`.
   Use the actual unequal gaps.

10. **Use the adjacent time secant at source endpoints.** At the
    first row use the first two rows and at the last use the last
    two. A two-row source uses that secant at both ends. The
    endpoint choice is first order under smooth refinement and
    preserves the sign of the adjacent interval's variance move.
    The derivative still holds requested k fixed.

11. **Differentiate the local smile quadratic in log-moneyness.**
    In the same expiry row take the nearest three source strike
    nodes in order. Use the prior, center and next node inside,
    the first three at the left edge and the last three at the
    right edge. Define the actual k spacings with log ratios
    of original strikes. These are spatial derivatives of the
    sampled quadratic through native node variances.

12. **Use the same three-node derivative formulas at edges and inside.**
    For ordered k_0<k_1<k_2 let u and v be the successive secants
    of w, and `A = (v-u)/(k_2-k_0)`. At the selected center k,
    `w_k = u + A ((k-k_0) + (k-k_1))` and `w_kk = 2A`.
    Form local offsets from original strike ratios. This avoids
    a separate cash-strike derivative and chain-rule evaluation.

13. **Check scaled stencil arithmetic.** Use positive ordered
    represented source axes, stable increments and complete
    differences of scaled variance. Exact constants have zero
    derivatives. Remove absolute spacing deadbands and switches
    based on apparent near-uniformity. Reject unresolved geometry
    and failed arithmetic. Preserve representable final local
    sigma when an intermediate variance, derivative or square
    is outside binary64 range.

14. **Share the checked density-factor algebra.** For positive
    center w use
    `g = (1 - k w_k/(2w))² - (w_k²/4)(1/w + 1/4) + w_kk/2`,
    the same factor as in [accepted raw SVI](svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance).
    Use stable
    complete signed terms. A zero center makes this formula
    undefined. Neighbor variance samples may be zero.

15. **Require the actual extraction domain.** Require checked
    center w>0, g>0 and w_T≥0. Exact zero slope gives local
    sigma zero with positive g. Otherwise compute the positive
    square root of w_T/g in scale. Reject negative computed
    slope, nonpositive density or lost positive/nonfinite
    output. Remove the variance floor, calendar tolerance and
    zero clipping. A rejected stencil is a sampled result.

16. **Separate node computation from diagnostics policy.** Share
    checked signed derivatives and g with the arbitrage scanner.
    Under strict support, distinguish an unavailable matched
    neighbor from a failed computation and from a computed
    negative slope or density. The extraction requires every
    requested result. Record 20 owns scan coverage, thresholds,
    severity, status and the treatment of unavailable points.

17. **Return the complete derived grid atomically.** Preflight
    requested axes, stencil support where applicable, shape and
    resources. Evaluate all requested nodes before publishing.
    Preserve row and spot coordinates with errors. Retain the
    source snapshot or equivalent reconstruction provenance,
    expiry-to-forward association, clock, chosen output region,
    interpolation, smoothing and implied-wing policy, with
    any flat support identifiable.

18. **Define one immutable local-volatility grid.** Support
    explicit grids and derived grids with nonempty finite
    increasing elapsed times ≥0, finite increasing positive
    absolute spot levels, and one finite nonnegative
    instantaneous relative sigma per node in time-major order.
    A singleton axis and zero local sigma are valid. Local
    vol is a diffusion coefficient per square root model year.

19. **Use truthful local grid state and restore it strictly.**
    Use times and spot levels for local grid coordinates and
    host labels. Retain the actual optional model clock, with
    source clock and construction provenance for extracted
    grids. Validate checked shape and complete state through the
    same constructor on every restore. Reject unknown, duplicate
    and malformed state. A supplied numeric grid is a directly
    specified diffusion input.

20. **Evaluate local sigma with an explicit boundary.** Validate
    finite elapsed t≥0 and finite S>0 first. Select one strict
    error or flat-coordinate clamp policy for the local axes.
    On singleton axes strict requires the stored coordinate.
    Return exact node sigma and use checked bilinear native
    sigma interpolation over active knots elsewhere. Use the
    scaled convex geometry in record 16. Total implied variance
    interpolation is a different operation.

21. **Keep time-zero and spot support explicit.** The positive
    source expiries yield positive local grid times. A strict
    time-zero query needs a supplied zero row. Flat local time
    use at zero selects the first local time value. Keep this
    selection and the spot boundary with the actual grid
    consumer. Infinite queries are invalid under either policy.

22. **Reconcile the actual local-volatility simulation route.**
    The local-vol process and path simulation must propagate
    checked lookup and arithmetic errors and require finite
    positive states. Direct use requires the same qualified clock
    and compatible carry. For an extracted grid the existing
    constant-carry process must agree with its forward basis.
    A different clock requires an owner-qualified instantaneous
    time change of diffusion and drift. The implied-volatility
    horizon bridge cannot supply that conversion. Choose the
    local grid boundary explicitly.
    [MODELS-MONTE-CARLO-04](../models-monte-carlo/04-lognormal-diffusions.md)
    and [MODELS-MONTE-CARLO-28](../models-monte-carlo/28-capture-and-simulation-contracts.md)
    own step scheme, time grid, initial state and path acceptance.

23. **Reconcile existing Python, WASM and state paths.** Use
    one checked Rust extraction and local sigma query. Replace
    the separate smoothed/unsmoothed paths and error-to-number
    behavior. Update class and plain state, array/DataFrame
    views, schemas, declarations, facade, parity, simulation
    request and examples together. The existing UI surface
    diagnostic report remains a record 20 consumer.

24. **Disclose the model and numerical limits.** The extracted
    values are a finite-stencil estimate on the selected
    effective implied surface, including any smoothing and
    wing extension. Spatial second-derivative and endpoint
    errors depend on the actual grid. A complete grid does
    not establish continuous arbitrage freedom or exact
    vanilla repricing by a discrete path or PDE engine.

25. **Require independent and complete-path evidence.** Cover
    analytic flat and nonflat examples, irregular stencils
    and edges, carry and exact knot remapping, unsupported
    strict versus deliberate flat samples, tiny variance and
    Gaussian range, signed slope and density refusals, local
    interpolation, state and actual simulation. Qualify mesh
    convergence and source-vanilla replay independently.
    See the [Dupire and finite-difference references](../../../REFERENCES.md#checked-dupire-extraction-and-local-volatility-grids).

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| T=(0.25,0.5,1.25), w=0.04T+0.02T² | Interior quadratic derivative at 0.5 is 0.06. The across-neighbors secant is 0.07. The first-row secant is 0.055 against the analytic 0.05. |
| T=(1,2,3), w=(0.01,0.011,0.1) | The first-row adjacent secant is 0.001. A one-sided quadratic derivative there would be −0.043. |
| k=(−0.2,−0.05,0.25), w=0.04+0.03k+0.02k² | The three-node formulas recover w_k=0.03+0.04k and w_kk=0.04 at every selected node. |
| K range [80,120], F=(100,110), unit time step and true fixed-k w=0.2+0.1k | K=120 maps to 132 at the second expiry. The true fixed-k slope is zero. Flat implied sigma at the second row's K=120 gives a slope of about −0.00953102. |
| Flat sigma 1e−170 | Squaring sigma in binary64 produces zero. The complete scaled Dupire ratio and root recover sigma 1e−170. |
| Gaussian bandwidth 1, strikes (1,41,81), sigmas (0,1e300,0) | Smoothed sigma at the first strike is about 3.667874584e−48, although the second weight exp(−800) alone rounds to zero. |

These arithmetic checks compare stencil definitions, support choices and
complete-result scaling. Production code, simulation and host qualification
remain pending.

## Observed implementation and evidence

### Scope and input domain

Black-lognormal strike-axis implied surface, at least2 expiries/3 positive strikes and one positive forward per expiry. Output grids accept nonnegative local vols and nonnegative expiries.

### Choice and rationale

Extract local variance at fixed log-moneyness from total variance derivatives. Optional Gaussian smoothing acts on implied vol in cash-strike units before differentiation; it changes the input smile.

### Algorithm

At each node k=ln(K/F(T)),w=σimp²T; reject w<1e−14. Compute time derivative with neighboring expiries at matched k (Kneighbor=Fneighbor exp k): central secant inside, one-sided at boundaries. Strike derivatives use unequal-grid finite differences in K, then wk=K wK and wkk=K²wKK+wk. Density=1−k wk/w+.25(−.25−1/w+k²/w²)wk²+.5wkk. Local σ=√(max(wT,0)/density). Require finite positive density and wT≥−1e−10; finite result. Smoothing averages input σ using weights exp[−.5((Kj−K)/bandwidth)²], reconstructs same-grid implied surface then extracts. Query bilinearly interpolates local σ with clamped axes.

### Selection and controls

Extraction uses clamped matched-moneyness lookup; shared density diagnostics instead request strict lookup and may omit unavailable time derivatives. Bandwidth finite≥0, <1e−14 means unsmoothed. Negative calendar slope within tolerance becomes zero; no implied-vol substitution on invalid density.

### Conventions

Forwards determine fixed-moneyness time motion; rates are not guessed from strikes. Bandwidth is absolute strike units, not log-moneyness or vol points. Output is instantaneous diffusion volatility per√year.

### Limitations, failure and fallback

Interior time secant on an irregular expiry grid is not the full unequal-grid second-order formula; edges have lower accuracy. Matched-moneyness clamping can distort wing time derivatives. Local smoothing is not arbitrage repair; extraction still errors. `value` rejects NaN but clamps infinities to endpoint values.

### Alternatives and tradeoffs

Analytic parametric smile derivatives or price-space Dupire would be alternatives; neither is selected here. Smoothing reduces numerical noise at the cost of repricing the smoothed rather than original surface.

### Implementation and validation evidence

[dupire.rs](../../../../finstack-quant/models/src/volatility/dupire.rs) and [local_vol.rs](../../../../finstack-quant/models/src/volatility/local_vol.rs). `local_vol_matches_the_exact_ssvi_local_vol` compares to analytic SSVI derivatives on a fine grid and excludes boundary rows/columns; tolerance5e−4 within three standard deviations. Flat-vol/forward-dependence/arbitrage rejection tests supplement it. Reference: [Dupire](../../../REFERENCES.md#dupire-1994).

The actual local-volatility process uses infallible diffusion lookup.
Simulation starts at time zero, before the first extracted time, and
path capture currently checks finiteness alone. Python and WASM expose
both extraction variants and grid queries. The surface report currently
shares the Dupire kernel with a separate strict sampling behavior.
See the [local-volatility process](../../../../finstack-quant/models/src/monte_carlo/process/local_vol.rs),
[simulation dispatch](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[arbitrage checks](../../../../finstack-quant/models/src/volatility/arbitrage/checks.rs),
[Python binding](../../../../finstack-quant-py/src/bindings/models/volatility.rs)
and [WASM binding](../../../../finstack-quant-wasm/src/api/models/volatility.rs).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
