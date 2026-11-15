# CALIBRATION-TARGETS-003 — Discount-factor bootstrap and simultaneous zero-rate fit

- **Decision status:** Accepted 2026-11-15 — complete discount calibration package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent prefix, mapped-bound, derivative and role-binding calculations verified. Production calibration, market reference, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for explicit discount calibration and delivered-curve fit

1. **Require explicit curve construction and fit settings.** Elect the
   curve base date, clock and required calendar context, interpolation,
   tail policy and fit method directly or through a named versioned
   preset. Use positive finite discount factors and the exact `(0, 1)`
   anchor. Admit negative rates and factors above one. Allow Linear,
   LogLinear, CubicHermite/PCHIP, MonotoneConvex and
   PiecewiseQuadraticForward for simultaneous fits when the selected
   method supports the grid and actual pricing queries. Flat holds the
   boundary factor, Extend uses the method's own continuation, and
   Error rejects outside queries. Retire currency-inferred bounds
   and generic shape presets.
   [Accepted rate-curve construction](0032-curve-interpolation-and-calibration.md#accepted-decision-for-explicit-rate-curve-interpolation-and-calibration) ·
   [Accepted discount evaluation](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction).

2. **Qualify standalone bootstrap on actual pricing dependencies.**
   Admit Linear or LogLinear when each prepared quote identifies its
   new knot, all fitted-source queries are supported on that prefix,
   and later knots cannot change their results. Check actual payments,
   resets and both endpoints of relative-factor queries under the
   elected tails. A coupled fit requires an explicit simultaneous
   solve. Retire automatic polishing.
   [Accepted bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Hagan and West on curve interpolation and bootstrap](https://bank.uni-hohenheim.de/uploads/media/Hagan_and_West__2006__-_Interpolation_Methods_for_Curve_Construction.pdf).

3. **Solve each bootstrap knot inside explicit bounds.** Require
   annual continuously compounded zero-rate bounds on the curve
   clock. At positive knot time `t`, convert them to
   `[exp(−z_max t), exp(−z_min t)]` and require distinct positive
   finite represented bounds for a free knot. Elect the in-domain
   factor-space seed, scan coordinates, search order, tolerances and
   work limits. Accept a valid computed zero or a Brent result from
   a qualified continuous bracket, followed by the native quote
   gate. A sign change across a contractual cash-rounding jump
   cannot certify a root. Require root evidence for this bootstrap
   and remove hidden scan grids and fallback guesses.
   [Accepted scalar search](../calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence) ·
   [Accepted rate preparation and fit residuals](02-rate-preparation-and-residuals.md#accepted-decision-for-explicit-rate-preparation-and-native-fit-residuals).

4. **Fit bounded zero rates simultaneously without clipping.** Use
   the accepted open logistic map for free rates and construct each
   factor as `exp(−zᵢtᵢ)`. Keep explicitly fixed rates and elected
   endpoints outside the solver vector. An all-fixed curve undergoes
   full repricing with zero optimizer work. Reject collapsed free
   ranges, zero underflow, infinite factors and unusable probes.
   Positive subnormal factors are eligible when the evaluator supports
   them. Retire hard factor clamps.
   [Accepted global solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

5. **Elect seeds and evaluate the actual weighted objective.** Require
   an aligned native seed or an explicitly elected, qualified private
   bootstrap seed on the target grid. Convert admitted seed factors
   with `−log(Dᵢ)/tᵢ`. A selected seed failure propagates. Retire the
   automatic 3% seed and seed promotion. Use the accepted augmented-QR
   solver with explicit tolerances and work limits, and checked
   central differences of weighted residuals in free solver
   coordinates. Remove the misleading analytical-Jacobian flag and
   the pillar-time derivative shortcut. Any restart follows the
   elected deterministic multi-start policy. Require enough positive-
   weight rows and actual identification of the free parameters.
   [Accepted global solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates) ·
   [Accepted deterministic multi-start](../calibration-framework/13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection).

6. **Bind the candidate to the actual output role throughout.** Use the
   exact fitted discount output identity in every private trial, probe
   and final reprice. Remove the separate pricing discount ID. Preserve
   independent projection sources and bind shared overnight projection
   only through the qualified explicit link. Term FRA eligibility
   requires the candidate to supply actual live term projection.
   A listed future can constrain this target only when the candidate
   supplies actual live projection through a qualified link. A fixed
   nonzero FRA settlement cannot reprice to par through discount alone.
   [Accepted FRA preparation](../calibration-framework/04-deposit-fra-materialization.md#accepted-decision-for-deposit-and-fra-materialization-and-prepared-clocks) ·
   [Accepted futures preparation](../calibration-framework/05-futures-materialization.md#accepted-decision-for-listed-rate-futures-calibration-and-reference-periods) ·
   [Accepted linked OIS roles](../calibration-framework/20-linked-ois-replay.md#accepted-decision-for-linked-single-curve-ois-replay-and-publication).

7. **Gate and publish the exact delivered curve.** Reprice every
   consumed quote, including zero-weight rows, in its native units
   against its own tolerance. Require structural validity and actual
   pricing support, then apply any elected economic bands over their
   declared coverage and validate affected dependencies. Sampled checks
   establish only the measured coverage. Keep solver termination,
   quote fit and curve validation distinct. Publish the evaluator,
   bindings, construction record and report atomically under the
   same contract for direct fits, plans and replay. Independent
   dated market and host qualification remain open.
   [Accepted fit reports](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted curve validation](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication) ·
   [Accepted rate replay](../calibration-framework/19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication).

## Independent calculation checks

- Linear with a Flat tail on `(0, 1), (1, .98)` gives `D(1.25) = .98`.
  Adding `(2, .94)` changes that query to `.97`.
- At two years, annual continuous zero-rate bounds `[−.02, .05]`
  give factor bounds of about `[.9048374180, 1.0408107742]`. At one
  year, bounds `[0, 1e−17]` both map to binary64 factor `1`.
- For a two-year deposit with gross repayment `1.04`, weight four
  and logistic zero-rate bounds `[−.10, .10]`, the weighted
  residual's derivative in the free coordinate is `−.208` at its
  midpoint.
- A unit one-year deposit with a 4% coupon fits at `25/26` and
  gives residual `−.0016` at `.96`. A trial pricing alias can
  therefore mask the delivered curve's residual.

## Observed choice and rationale at the audit revision

A discount curve solves quote-built instruments to zero PV per notional. Bootstrap optimizes discount factors one pillar at a time; GlobalSolve optimizes continuously compounded zero rates simultaneously. Nonlocal interpolation invokes a global polishing solve even when the requested method is Bootstrap, because later knots can alter earlier repricing. This rationale is explicit in source; acceptance remains a separate fit policy.

`DiscountCurveParams` deserialization defaults to Bootstrap and LogLinear/FlatForward; explicit GlobalSolve and supported interpolation overrides remain available. `InterpStyle::default()` is Linear, so supplying that explicit Rust value is different from omitting the serialized field. The [schema rationale](../../../../finstack-quant/calibration/src/api/schema.rs) identifies positivity and piecewise-constant continuously compounded forwards for calibrated LogLinear. Direct construction has separate [defaults and validation](../core-market/discount-evaluation.md).

## Observed ordered algorithm

1. Prepare rates at `N=1,000,000`; default curve clock ACT/365F, interpolation LogLinear and flat-forward extrapolation. Pricing discount/projection IDs independently default to output curve ID. Persist original rate quotes and roles in a replay recipe.
2. Define maturity-dependent DF bounds `lo=exp(-r_max*t)`, `hi=exp(-r_min*t)` from the effective currency/explicit rate bounds. Bootstrap seed is `clamp(1/(1+r_implied*t),lo,hi)` and starts with `(0,1)`.
3. The discount-specific scan includes valid bounds, a coarse grid (`max(scan_grid_points-10,8)` subdivisions), and logarithmic offsets around the clamped seed at ±`{1e-4,2e-4,5e-4,1e-3,2e-3,5e-3,1e-2,2e-2}` in log DF. Coarse spacing is logarithmic only if `lo>1e-12` and `hi/lo>10`; otherwise linear. Sort descending and deduplicate within `0.001*(hi-lo)`; fill toward minimum grid size with linear points. Defaults: 48 desired/20 minimum. The framework then normalizes scans and uses nearest-first bracketing for this target.
4. Trial and final curves share interpolation and nonmonotonicity policy. Explicit bounds with a negative minimum, or AutoCurrency EUR/JPY/CHF, allow nonmonotone DFs unless overridden. Trial curves omit the forward floor. Final nonmonotone curves use Raw without a floor; otherwise MarketStandard validation applies. Positive finite DF and maturity-specific bound checks constrain accepted bootstrap knots.
5. Global control parameters `z_i` are clamped to rate bounds and converted to `DF_i=exp(-z_i*t_i)`, then hard-clamped to configured `[df_hard_min,df_hard_max]` (defaults `1e-12,1e6`). Positive quote times survive; initial zeros come from a bootstrap seed or 3% clamped to bounds. Residuals are raw prepared PV/N with annuity-proxy weights from [002](02-rate-preparation-and-residuals.md).
6. `use_analytical_jacobian` selects the target's **central finite-difference** Jacobian, not an analytic derivative. For column j, `h=max(abs(z_j)*eps,eps)` with default eps `1e-6`; differentiate the full normalized repricing at ±h. For Linear/LogLinear only, quotes with pillar before previous control time minus `1e-4` are assigned zero derivative. Nonlocal styles evaluate every quote. Parameter clipping remains inside both evaluations.
7. Optional bootstrap seeding is enabled for GlobalSolve. Seed failures are saved as diagnostics and global fitting continues. A successful seed within fit tolerance and no worse than the global max residual may replace the global result. Bootstrap with Linear/LogLinear returns directly; other styles use the seed for global polishing. Fit tolerance defaults to `1e-8` PV/N; recipe and solver/model metadata are attached afterward.

## Observed limits, alternatives and discrepancies

Clipping creates flat/non-smooth residual regions and can distort finite differences at bounds. Nearest-first bracketing, scan resolution, the causal Jacobian shortcut and currency presets require qualification over the intended quote mix. Alternatives are log-DF parameters with explicit inequalities, forward-rate controls, true analytic adjoints, or quote-error minimization. The [accepted global solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates) requires one checked coordinate map and consistent weighted derivatives.

**Observed ID discrepancy:** trial curves are built under `pricing_discount_id`, but final curves under `curve_id`; the residual context merely inserts by curve ID while prepared instruments continue to reference the pricing ID. If those IDs differ, final repricing can still read the last scratch trial curve at the pricing alias, and a separate pre-existing discount dependency is overwritten during trials. The default identical-ID path avoids this. No runtime reproduction was run.

## Evidence

- Owning [discount.rs](../../../../finstack-quant/calibration/src/targets/discount.rs); [schema](../../../../finstack-quant/calibration/src/api/schema.rs) and [preflight](../../../../finstack-quant/calibration/src/validation/preflight.rs) were inspected for defaults/ID checks.
- Inline payment-delay pillar, notional-invariance, global sanity, convexity-seed and replay-shape tests are located. [Repricing tests](../../../../finstack-quant/calibration/src/tests/repricing.rs) are same-library contract checks. [Bloomberg accuracy tests](../../../../finstack-quant/calibration/tests/calibration/bloomberg_accuracy.rs) contain a source-attributed 2025-12-10 USD OIS fixture; the fixture's provenance and current run result were not independently verified.
- [Failure-mode tests](../../../../finstack-quant/calibration/tests/calibration/failure_modes.rs) and [reference-envelope tests](../../../../finstack-quant/calibration/tests/calibration/reference_envelopes.rs) locate execution and failed-fit handling evidence. [Discount-curve tests](../../../../finstack-quant/core/tests/market_data/curves/discount.rs) cover clocks, interpolation, negative rates and forward identities; [interpolation tests](../../../../finstack-quant/core/tests/math/interp.rs) include Hagan–West reference-region formulas. These tests were not rerun.

A version string is not model certification.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
