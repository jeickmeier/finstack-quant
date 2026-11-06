# ADR 0032: Rate-curve interpolation during calibration

**Decision status:** Draft — observed implementation; adoption and rationale
pending agreement.

**Implementation status:** Observed for direct discount/forward construction
and the corresponding calibration targets; other curve families have separate
owners in the [target catalogue](README.md).

**Validation status:** Evidence located and inspected, not rerun; independent
reference provenance and coverage are not fully assessed.

**Audit date:** 2026-11-05

## Observed choice and rationale

Rate calibration fits instruments using the requested final interpolation.
Discount factors and index forward rates are distinct representations; numerical
solver termination, quote-fit acceptance and financial validation are separate
contracts. These are observed choices, not approval of current defaults.

The source rationale for calibrated discount LogLinear is positive discount
factors and piecewise-constant continuously compounded forwards. For calibrated
forward CubicHermite it is shape preservation with zero/negative rates; global
fitting addresses contractual reset intervals that couple adjacent controls.
Nonlocal discount interpolation requires simultaneous polishing after bootstrap
because later knots can change earlier residuals. Source rationale and exact
parameterization belong to the [discount](03-discount-calibration.md) and
[forward](04-forward-calibration.md) target records.

**Adoption boundary:** this draft does not establish the suitability of differing
direct-construction/calibration defaults, smoothing, extrapolation, bounds or
fit tolerances for a particular use.

## Scope and selection

The scope is direct `DiscountCurve`/`ForwardCurve` construction and calibration
from supported deposits, FRAs, futures and swaps. Discount and projection roles
are distinct. Forward calibration requires an existing discount dependency and
uses actual contractual reset intervals. Hazard, CPI, volatility, correlation
and parametric methodologies remain separate Draft choices; Hull–White and LMM
have [rate-model calibration records](../calibration-rate-models/README.md).

| Selection boundary | Authoritative methodology owner |
| --- | --- |
| Direct discount construction selects MonotoneConvex; calibration selects LogLinear unless overridden | [Discount evaluation](../core-market/discount-evaluation.md) and [discount target](03-discount-calibration.md) |
| Direct forward construction selects Linear; calibration selects CubicHermite and GlobalSolve | [Forward projection](../core-market/forward-projection.md) and [forward target](04-forward-calibration.md) |
| Local discount bootstrap returns directly; nonlocal styles trigger global polishing; explicit GlobalSolve may adopt its bootstrap seed | [Discount dispatch and acceptance](03-discount-calibration.md#ordered-algorithm) |
| Forward Bootstrap and MonotoneConvex are unsupported; the target uses numerical derivatives | [Forward selection and input contract](04-forward-calibration.md#choice-and-inputs) |
| Explicit Rust parameter values versus serialized omissions | Entry-point default notes in the [discount](03-discount-calibration.md#choice-and-rationale) and [forward](04-forward-calibration.md#choice-and-inputs) targets |

This table records the selection question; defaults, units, clock inference,
tails, bounds and algorithm controls are defined only by the linked owners.
Equal extrapolation names do not imply equal economics across curve types.

## Numerical and validation boundaries

[Value](../core-numerics/value-interpolation.md) and
[forward interpolation](../core-numerics/forward-interpolation.md) own kernel
formulas and working-space tail semantics. The [LM kernel](../core-numerics/levenberg-marquardt.md)
owns generic optimization; [configuration](../calibration-framework/02-controls-and-market-context.md),
[bootstrap](../calibration-framework/10-sequential-bootstrap.md), and
[global solve](../calibration-framework/12-weighted-global-solve.md) own caller
controls and dispatch. A requested target-supplied Jacobian need not be analytic:
the discount target supplies central finite differences through that interface.

[Fit reports](../calibration-framework/14-fit-reports.md) gate native unweighted
residuals separately from the weighted objective. Rate residuals are generally
raw PV/notional, but [forward deposits](04-forward-calibration.md#algorithm)
use undiscounted contractual accrual-growth error. Numerical solver tolerance
is distinct from target fit acceptance and the additional
[sampled financial validation](../calibration-framework/16-curve-validation.md).

Invalid grids, missing dependencies, unsupported combinations and unusable
solver settings error through their owners. [Execution policy](../calibration-framework/03-dependencies-and-execution.md)
gates failed fits before installing outputs by default; explicit permissive
policy allows diagnostic failed reports that callers must inspect.
[Provenance](../core-market/calibration-provenance.md) records replay boundaries.

## Alternatives and tradeoffs

Log-linear DFs are local and preserve positivity; smoother interpolation changes
off-pillar values and can require a coupled solve. CubicHermite rates admit
zero/negative values and preserve local shape; Linear is an explicit simpler
alternative. LogLinear requires positive ordinates and cannot represent
arbitrary negative-rate inputs.

Sequential fitting is simpler for local discount interpolation. Global least
squares accommodates coupled reset periods and nonlocal styles with greater
solver/initialization sensitivity, not a guarantee of a global optimum.
Polishing, derivative substitution and seed adoption are observed numerical
paths; this draft does not approve them as fallback policy. Requested
interpolation is retained, and sampled checks do not prove arbitrage freedom
at every maturity.

## Evidence and adoption boundary

The linked owners retain source/test evidence, including Hagan–West formula
cases, failed-fit handling and the attributed Bloomberg OIS comparison.
Located evidence does not certify market accuracy. Technical context remains
[Hagan–West](../../../REFERENCES.md#hagan-west-monotone-convex) and
[multi-curve construction](../../../REFERENCES.md#ametrano-bianchetti-2013).
This draft does not adopt the other calibration families.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
