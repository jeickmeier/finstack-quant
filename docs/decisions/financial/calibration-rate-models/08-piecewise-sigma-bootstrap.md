# CALIBRATION-RATE-MODELS-008: Sequential cap/floor bootstrap on live-fixing volatility intervals

**Decision status:** Accepted 2026-11-15 — complete live-fixing sigma bootstrap, branch selection and fitted-schedule acceptance package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent live-fixing variance and delayed-payment two-branch calculations verified. Production bootstrap, dated market references, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for live-fixing Hull–White sigma bootstrapping

1. **Prepare one live Normal cap or floor strip per segment.** Supply
   finite fixed κ ≥ 0, including the Ho–Lee limit. Elect the dated or
   synthetic profile and prepare and freeze every eligible strip and
   market premium before solving. Keep exact quote identities and
   require unique contractual maturities and strictly increasing last
   live fixings in maturity order. Solve in that order and report in
   the supplied order. Strikes and cap/floor sides may vary across
   maturities. Apply the same prepared inputs and positive native
   Normal tolerances throughout.
   [Accepted entry profiles](01-curve-entry-and-quote-domains.md#accepted-decision-for-explicit-hull-white-calibration-entry-and-quote-domains) ·
   [Accepted cap and floor strips](02-cap-schedules-and-quote-inversion.md#accepted-decision-for-checked-cap-and-floor-strips-and-implied-normal-inversion).

2. **Start the new segment at the preceding last live fixing.** Start
   the first segment at the fit origin. On each trial retain every prior
   coefficient and reprice the full current strip, including coupons
   that use the frozen prefix. Use the checked piecewise variance through
   each fixing and the actual contractual payment measure. A later
   segment has zero exposure through the fixing at its start, even when
   the earlier strip pays after that fixing.
   [Accepted term-optionlet law](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures) ·
   [Henrard's Hull–White model and payment formula](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

3. **Use one qualified branch and bounded Brent solve per segment.**
   Require an explicit finite interval `0 ≤ σ_lo < σ_hi`, qualified for
   continuous pricing and a single crossing in its declared direction.
   Require a positive sigma-coordinate tolerance, explicit nonzero
   update and evaluation limits and an aggregate work cap. Solve the
   finite signed full-strip residual `P_HW − P_market` per unit notional.
   Check exact endpoints lower first and reuse residuals by exact
   represented coordinate. A computed zero at either endpoint is
   eligible. Bind exact-zero or valid Brent bracket evidence to the
   actual candidate and propagate typed pricing and solve failures.
   Retire automatic expansion, approximate fallback and upper-bound
   proximity rejection.
   [Accepted Brent contract](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search).

4. **Require financial agreement and local identification before freezing.**
   Apply a positive unit-notional premium tolerance independently of
   the sigma tolerance. Require distinct represented sigma probes on
   both sides of an interior root or inward at an endpoint, including
   zero. Each premium move must follow the qualified direction and
   exceed the combined operational pricing allowances. These probes
   test local response. Reconstruct the entire strip's annual Normal
   volatility, requiring the accepted gross-premium, time-value and
   signed implied-minus-observed volatility gates. Freeze the
   coefficient privately after all checks pass.
   [Accepted strip inversion](02-cap-schedules-and-quote-inversion.md#accepted-decision-for-checked-cap-and-floor-strips-and-implied-normal-inversion).

5. **Reprice and re-invert on the exact delivered schedule.** In the
   original supplied order repeat the premium and native gates for
   every consumed quote. Retain complete finite signed final residuals
   by exact quote ID on both bases, and keep local evidence distinct.
   Report the bracket, installed coefficient, endpoint status and actual
   checked solve, pricing, probe and inversion work. Retire the pillar
   count as an iteration count. Failed runs may retain bounded evidence.
   [Accepted sequential bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

6. **Publish the complete admitted schedule atomically.** Preserve
   fixed κ, absolute decimal sigma per square root model year, exact
   segment starts and the end of calibrated support at the last live
   fixing of the final strip. Retain original dates on the dated
   ACT/365F model clock and exact model times on synthetic input.
   Use a right-continuous schedule with a flat final coefficient and
   retain the sequential fit without smoothing or joint reoptimization.
   Apply complete output admission before publication and validate
   restored state under the same contract.
   [Accepted fitted-state publication](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

## Independent calculation checks

- Quarterly synthetic strips maturing at 1 and 1.25 years have last
  live fixings at 0.75 and 1 year. With zero κ and sigmas `.01` then
  `.02`, the state variances through those fixings are `3/40000` and
  `7/40000`. Starting the second segment at the first maturity would
  leave the second strip with zero exposure to its coefficient.
- With zero κ, unit curve factors and unit coupon and index accruals,
  take a zero-strike caplet that fixes and starts at year 1 and ends
  and pays at year 2. Its first-segment sigma `.02` gives a unit premium
  of about `.007978712629`. A second zero-strike caplet fixes and starts
  at year 2, ends at year 3 and pays at year 4. With the first sigma
  frozen and `q = hypot(.02, σ)`, its premium is
  `exp(−q²) Φ(−q/2) − Φ(−3q/2)`. The target `.05` has roots
  near `.15410606745212` and `.96625770714664`. The brackets `[0,.25]`
  and `[.5,2]` select opposite branches. The residuals at both
  endpoints of `[0,2]` are negative.

## Observed choice and rationale at the audit revision

With κ fixed, the piecewise path solves one constant σ segment per cap/floor quote. Earlier segments remain frozen. The next segment starts at the preceding quote's last live fixing, because a cap price is insensitive to volatility after its final rate fixing; payment maturity would create unidentifiable adjacent segments. Source explicitly rejects repeated maturities instead of inventing a smile aggregation objective.

## Domain and preparation

Require at least one valid Normal cap/floor quote, finite positive fit tolerance, fixed κ in inclusive `[0.001,1]`, and finite bounds `0<σ_min<σ_max`. Public direct config does not impose the scalar solver's upper limit 2; the engine adapter supplies `[1e-5,2]`. Prepare [live schedules](02-cap-schedules-and-quote-inversion.md), keeping each schedule paired with its quote, then sort by maturity. Duplicate maturity is rejected. Each last fixing must be finite/positive and these horizons must increase strictly after sorting, independently of maturity ordering.

## Ordered algorithm

1. Initialize segment times `[0]` and an empty σ vector.
2. For the next sorted quote, compute its Bachelier strip premium. Append a candidate σ to the already-fitted segments and construct the piecewise HW model on the current times.
3. Reprice the full quote, including coupons affected by previous segments. Solve `P_HW(candidate)−P_market=0`. The audit revision mapped pricing failures to NaN. The current callback propagates typed pricing errors.
4. Evaluate both configured σ endpoints. Reject nonfinite residuals or equal nonzero signs. Use Brent within the supplied bracket, tolerance `1e-12`, maximum 200 iterations. Reject a solution at/within relative `1e-6` of σ_max; a lower-bound solution is allowed.
5. Reconstruct that quote's Normal implied volatility and store signed error keyed by maturity. Freeze the solved σ. If another quote remains, append the current quote's last fixing horizon as the next segment start.
6. Construct the final piecewise model, whose last σ extrapolates flat. Return `SequentialBootstrap` diagnostics with quoted-vol residual units, fixed κ, schedule source and piecewise mode. Report iterations equals pillar count, not total Brent iterations.

## Selection and conventions

This is an explicit public bootstrap or the engine's `volatility_mode=piecewise`; scalar remains the engine default. Multiple strikes or cap/floor sides can be used at distinct maturities, but there is one equation per newly fitted segment. Sorting accepts out-of-order inputs; “strictly increasing maturities” describes the normalized order and uniqueness rather than requiring callers to pre-sort.

σ is absolute rate volatility, κ inverse years, and interval times are ACT/365F years. Coupon accruals remain contractual. Unlike scalar fitting, this path applies no aggregate-vega admission gate. Model lookup/integration is delegated to the existing piecewise HW kernel, not approximated by an average σ. Dated sigma support and persistence belong to the [engine adapter](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

## Limitations, failure and alternatives

Earlier segment choices are not revisited, and no smoothing/regularization or joint best fit is applied. Inconsistent quotes can be unbracketed; no automatic bracket expansion, clipping to a fitted endpoint or fallback to scalar volatility occurs. Final tolerance checks rely on implied-vol residuals recorded when each pillar was solved; later intervals begin at prior last-fixing horizons, so they cannot affect earlier option variance by construction.

A global piecewise fit could combine repeated strikes and regularize term structure, but changes identification, residual weights and solve behavior. The scalar model is simpler and may intentionally leave quote error; the bootstrap prioritizes exact attainable pillar premiums without claiming a smooth or stable term structure.

## Implementation and evidence

[cap_floor.rs](../../../../finstack-quant/calibration/src/hull_white/cap_floor.rs), [cap_schedule.rs](../../../../finstack-quant/calibration/src/hull_white/cap_schedule.rs) and [pricing.rs](../../../../finstack-quant/calibration/src/hull_white/pricing.rs). [tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) creates a piecewise model with a 0.75-year breakpoint and recovers it from adjacent synthetic caps. [Contractual schedule tests](../../../../finstack-quant/calibration/src/hull_white/cap_schedule/tests.rs) reverse quote order and verify paired schedule sorting and repricing. These are same-kernel recovery/invariant tests, not external term-vol calibration evidence. No tests were run.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
