# CALIBRATION-TARGETS-003 — Discount-factor bootstrap, global zero-rate fit and polishing

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and rationale

A discount curve solves quote-built instruments to zero PV per notional. Bootstrap optimizes discount factors one pillar at a time; GlobalSolve optimizes continuously compounded zero rates simultaneously. Nonlocal interpolation invokes a global polishing solve even when the requested method is Bootstrap, because later knots can alter earlier repricing. This rationale is explicit in source; acceptance remains a separate fit policy.

`DiscountCurveParams` deserialization defaults to Bootstrap and LogLinear/FlatForward; explicit GlobalSolve and supported interpolation overrides remain available. `InterpStyle::default()` is Linear, so supplying that explicit Rust value is different from omitting the serialized field. The [schema rationale](../../../../finstack-quant/calibration/src/api/schema.rs) identifies positivity and piecewise-constant continuously compounded forwards for calibrated LogLinear. Direct construction has separate [defaults and validation](../core-market/discount-evaluation.md).

## Ordered algorithm

1. Prepare rates at `N=1,000,000`; default curve clock ACT/365F, interpolation LogLinear and flat-forward extrapolation. Pricing discount/projection IDs independently default to output curve ID. Persist original rate quotes and roles in a replay recipe.
2. Define maturity-dependent DF bounds `lo=exp(-r_max*t)`, `hi=exp(-r_min*t)` from the effective currency/explicit rate bounds. Bootstrap seed is `clamp(1/(1+r_implied*t),lo,hi)` and starts with `(0,1)`.
3. The discount-specific scan includes valid bounds, a coarse grid (`max(scan_grid_points-10,8)` subdivisions), and logarithmic offsets around the clamped seed at ±`{1e-4,2e-4,5e-4,1e-3,2e-3,5e-3,1e-2,2e-2}` in log DF. Coarse spacing is logarithmic only if `lo>1e-12` and `hi/lo>10`; otherwise linear. Sort descending and deduplicate within `0.001*(hi-lo)`; fill toward minimum grid size with linear points. Defaults: 48 desired/20 minimum. The framework then normalizes scans and uses nearest-first bracketing for this target.
4. Trial and final curves share interpolation and nonmonotonicity policy. Explicit bounds with a negative minimum, or AutoCurrency EUR/JPY/CHF, allow nonmonotone DFs unless overridden. Trial curves omit the forward floor. Final nonmonotone curves use Raw without a floor; otherwise MarketStandard validation applies. Positive finite DF and maturity-specific bound checks constrain accepted bootstrap knots.
5. Global control parameters `z_i` are clamped to rate bounds and converted to `DF_i=exp(-z_i*t_i)`, then hard-clamped to configured `[df_hard_min,df_hard_max]` (defaults `1e-12,1e6`). Positive quote times survive; initial zeros come from a bootstrap seed or 3% clamped to bounds. Residuals are raw prepared PV/N with annuity-proxy weights from [002](02-rate-preparation-and-residuals.md).
6. `use_analytical_jacobian` selects the target's **central finite-difference** Jacobian, not an analytic derivative. For column j, `h=max(abs(z_j)*eps,eps)` with default eps `1e-6`; differentiate the full normalized repricing at ±h. For Linear/LogLinear only, quotes with pillar before previous control time minus `1e-4` are assigned zero derivative. Nonlocal styles evaluate every quote. Parameter clipping remains inside both evaluations.
7. Optional bootstrap seeding is enabled for GlobalSolve. Seed failures are saved as diagnostics and global fitting continues. A successful seed within fit tolerance and no worse than the global max residual may replace the global result. Bootstrap with Linear/LogLinear returns directly; other styles use the seed for global polishing. Fit tolerance defaults to `1e-8` PV/N; recipe and solver/model metadata are attached afterward.

## Limits, alternatives and discrepancies

Clipping creates flat/non-smooth residual regions and can distort finite differences at bounds. Nearest-first bracketing, scan resolution, the causal Jacobian shortcut and currency presets require qualification over the intended quote mix. Alternatives are log-DF parameters with explicit inequalities, forward-rate controls, true analytic adjoints, or quote-error minimization. General solvers and acceptance semantics belong to the [framework](../calibration-framework/12-weighted-global-solve.md).

**Observed ID discrepancy:** trial curves are built under `pricing_discount_id`, but final curves under `curve_id`; the residual context merely inserts by curve ID while prepared instruments continue to reference the pricing ID. If those IDs differ, final repricing can still read the last scratch trial curve at the pricing alias, and a separate pre-existing discount dependency is overwritten during trials. The default identical-ID path avoids this. No runtime reproduction was run.

## Evidence

- Owning [discount.rs](../../../../finstack-quant/calibration/src/targets/discount.rs); [schema](../../../../finstack-quant/calibration/src/api/schema.rs) and [preflight](../../../../finstack-quant/calibration/src/validation/preflight.rs) were inspected for defaults/ID checks.
- Inline payment-delay pillar, notional-invariance, global sanity, convexity-seed and replay-shape tests are located. [Repricing tests](../../../../finstack-quant/calibration/src/tests/repricing.rs) are same-library contract checks. [Bloomberg accuracy tests](../../../../finstack-quant/calibration/tests/calibration/bloomberg_accuracy.rs) contain a source-attributed 2025-12-10 USD OIS fixture; the fixture's provenance and current run result were not independently verified.
- [Failure-mode tests](../../../../finstack-quant/calibration/tests/calibration/failure_modes.rs) and [reference-envelope tests](../../../../finstack-quant/calibration/tests/calibration/reference_envelopes.rs) locate execution and failed-fit handling evidence. [Discount-curve tests](../../../../finstack-quant/core/tests/market_data/curves/discount.rs) cover clocks, interpolation, negative rates and forward identities; [interpolation tests](../../../../finstack-quant/core/tests/math/interp.rs) include Hagan–West reference-region formulas. These tests were not rerun.

A version string is not model certification.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
