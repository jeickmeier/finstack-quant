# CALIBRATION-TARGETS-011 — Per-expiry equity SABR calibration

- **Decision status:** Accepted 2026-11-15 — complete per-expiry equity SABR package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent beta-zero, same-strike, ATM pin and native-cap calculations verified. Production fitting, dated market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for fixed beta equity SABR and native slice acceptance

1. **Fit each selected live expiry independently in unshifted Black.**
   Use the exact dated quotes and frozen forward from 009. Require
   finite positive forward, strike, expiry and observed annual
   volatility. A selected zero volatility fails fit admission while
   remaining valid quote data. Elect one fixed `β ∈ [0,1]` for the
   surface. Preserve literal endpoints including zero and one.
   Require absent displacement and remove default beta, the
   `1e−4` cutoff and Auto shift. The explicit Black convention
   governs output at beta zero on positive coordinates.
   [Accepted dated equity forwards](09-equity-forward-inputs.md#accepted-decision-for-explicit-equity-spot-carry-and-dated-smile-forwards) ·
   [Accepted SABR quote law](../models-analytic/sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes) ·
   [Hagan et al.](https://lesniewski.us/papers/published/ManagingSmileRisk.pdf).

2. **Require at least three distinct optimizer strikes per expiry.**
   For compatible calls and puts at one strike, explicitly elect
   one quote ID for the optimizer. Preserve each additional consumed
   quote as a mandatory check row at the same model prediction
   and its own native tolerance. Apply all required slice checks
   before ranking candidates. Preserve each row's identity and
   authored order.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

3. **Elect Free or observed-ATM Pinned mode explicitly.** Free mode
   fits `α`, `ν` and `ρ`. Pinned mode requires the elected optimizer
   quote at exact `K = F` and solves alpha on a supplied positive
   bracket over a strictly increasing branch. Require explicit
   Brent controls and a positive native ATM tolerance within
   the slice cap. Missing exact ATM, an invalid bracket or a
   failed pin produces a typed failure.
   [Accepted checked SABR calibration](../models-analytic/sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration).

4. **Use the checked SABR objective and nine-start solver.** Freeze
   normalized market Black vega weights with the accepted `1e−8`
   relative vega floor. Minimize `√wᵢ(σᵢ−qᵢ)/qᵢ`. Require
   explicit nu and rho intervals, solver tolerances and work
   budgets. Use checked augmented QR and the accepted derivatives
   in solver coordinates. Use quarter, midpoint and three-quarter
   starts on each interval, in nu-major order. Rank passing starts
   by weighted norm and break exact ties by earlier start.
   Preserve actual failures and work across all starts.
   [Accepted checked SABR calibration](../models-analytic/sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration).

5. **Apply a native slice cap and a separate delivered-grid gate.**
   Require an explicit positive absolute annual-volatility slice cap.
   Remove the default `.001` and conversion through the minimum
   quote. A failed selected slice blocks the surface. After all
   slices pass, use the elected materialization and tail rules and
   strictly reprice every consumed quote on the delivered grid.
   Apply the elected diagnostics separately.
   [Accepted surface materialization](10-total-variance-materialization.md#accepted-decision-for-explicit-total-variance-materialization-and-delivered-surface-fit) ·
   [Accepted surface validation](../calibration-framework/17-surface-validation.md#accepted-decision-for-surface-validation-and-publication).

6. **Publish the grid and fit evidence together.** Preserve slice
   dates, clock, forward, parameters, optimizer and check-only
   quote IDs, effective controls, winning start, actual solver
   stop and work, and ordered native residuals. Preserve distinct
   slice fit, delivered quote fit and sampled diagnostic coverage
   through the existing Rust-owned interfaces and restoration.
   Repair remains a separate explicit operation.
   [Accepted finite-grid repair](../models-analytic/sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair).

## Independent calculation checks

- At `F = K = 100`, `T = 1`, `β = 0`, `α = 20`,
  `ν = .40` and `ρ = 0` with absent shift, the Black ATM
  expansion returns annual sigma `.203`.
- Same-strike call and put quotes `.200` and `.205` with
  respective tolerances `.004` and `.003` admit common
  predictions in `[.202,.204]`. The primary-quote prediction
  `.200` fails the companion check.
- For a Black ATM example with `F = K = 1`, `T = 16`,
  `β = 1`, `ν = 1` and `ρ = −.5`, the ATM equation
  `α(11/6−2α) = 1/3` has positive roots `1/4` and
  `2/3`. The interval `[1/8,3/8]` selects the increasing
  branch with root `1/4`.
- Dividing native cap `.001` by minimum quote `.10` gives
  relative tolerance `.01`. A `.003` error on a `.40` quote
  passes that relative gate and fails the native cap.

## Observed choice and scope at the audit revision

Fit a fixed-beta (default beta .5) SABR smile independently at each live equity option expiry, then publish a Black volatility grid using [total-variance interpolation](10-total-variance-materialization.md). The SABR evaluator, multistart fitting residuals, bounds and convergence kernel are owned by [models SABR calibration](../models-analytic/sabr-calibration.md) and [expansions](../models-analytic/sabr-expansions.md); this target selects quotes, carry, convention and fit budget.

## Observed algorithm and controls

1. Require nonempty target expiries and at least three target strikes; filter matching `OptionVol` ticker and group future expiries by exact ACT/365F time. Expired/nonpositive-time quotes are omitted. Resolve positive equity forwards through the required discount curve and explicit yield/cash carry [009](09-equity-forward-inputs.md).
2. Require `beta>=1e-4` so this path does not publish a normal-vol SABR branch as Black; full admissible beta/strike/vol domains remain enforced by the shared calibrator and surface construction. Require at least three input quote entries per expiry; shared model validation governs distinctness/identifiability.
3. For each expiry, collect strike/vol arrays, construct the SABR calibrator with maximum iterations from global solver config and relative tolerance `absolute_vol_budget/min(quoted_vol)`, and use `SabrShift::Auto`. Positive equity forwards/strikes ordinarily need no displacement; retained fitted parameters contain the actual shift. Other generic global-solver choices are not used by this target.
4. Any slice fitting error aborts the whole surface; no nearest fitted expiry is substituted for an unfitted live bucket. Empty live slice set errors. Store all fitted parameters, summed total iterations, winning start/iterations, residual evaluation count and bound-hit metadata.
5. Materialize target grid, calculate published-grid quote errors and arbitrage diagnostics, then construct a per-slice-least-squares report in quoted volatility units. Default absolute surface fit tolerance is .001 (0.10 vol points). The relative slice tolerance is a necessary scale conversion; final raw quote errors decide artifact acceptance.

## Observed alternatives, limitations and evidence

A shared beta, independent expiries and automatic displacement are model choices. Alternatives are free-beta fit, global term-structure fit, price/vega-weighted loss, or a directly arbitrage-constrained surface. Option call/put identity does not alter a quoted-vol fit. The public quote schema allows zero vol, while the fit path computes a ratio using the minimum quote and the model requires positive vols; error semantics need to remain explicit.

- Owning [vol.rs](../../../../finstack-quant/calibration/src/targets/vol.rs), [schema/defaults](../../../../finstack-quant/calibration/src/api/schema.rs), [SABR model](../../../../finstack-quant/models/src/volatility/sabr/calibration.rs) and linked model record identify actual ownership.
- Inline `vol_surface_fails_when_any_expiry_cannot_calibrate` and interpolation tests are located. Published total-variance/acceptance-policy tests in [svi_surface.rs](../../../../finstack-quant/calibration/tests/calibration/svi_surface.rs) cover both equity surface workflows. They reuse model-generated quotes and/or production evaluators; none was run here.

A fitted local smile and positive grid are insufficient proof of global arbitrage freedom.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
