# ADR 0032: Rate-curve interpolation during calibration

**Decision status:** Accepted on 2026-11-15. Explicit factor construction,
interpolation, tails, fit selection and delivered-evaluator admission.

**Implementation status:** Observed for direct discount and projection
construction and the corresponding targets. Reconciliation is pending.

**Validation status:** Independent factor, tail and PCHIP prefix calculations
verified. Production calibration, reference-market, restoration and host
qualification remain pending.

**Audit date:** 2026-11-05

**Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`

## Accepted decision for explicit rate-curve interpolation and calibration

1. **Elect construction and fit choices explicitly.** Select the
   source base date, clock and calendar context, interpolation and
   tail policy for direct construction and calibration. Elect the
   fit method and effective settings explicitly or through a named
   versioned preset. Apply the same choices in Rust, Python, WASM and
   restored state. Retire defaults tied to the entry point, omitted
   fields, currency or curve ID.
   [Accepted calibration controls](../calibration-framework/02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market).

2. **Use positive normalized factors with distinct financial roles.**
   Discount curves use discount factors `D`. Projection curves use
   auxiliary factors `P` and evaluate the actual benchmark value
   dates `S,E` with contractual accrual `α`, giving
   `(P(S)/P(E) − 1)/α` through checked factor arithmetic. Admit
   signed rates. Retire raw forward-rate ordinates, synthetic reset
   grids and generic sign floors. Bind the exact typed roles during
   preparation and pricing. A shared OIS evaluator requires an explicit
   qualified link.
   [Discount factors](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction) ·
   [Projection factors](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates) ·
   [Linked OIS replay](../calibration-framework/20-linked-ois-replay.md#accepted-decision-for-linked-single-curve-ois-replay-and-publication).

3. **Preserve the selected interpolation and literal tails.** Retain
   Linear, LogLinear, CubicHermite/PCHIP, MonotoneConvex and
   PiecewiseQuadraticForward for factor evaluation. Target records
   qualify the methods they admit in calibration. Flat holds the
   boundary factor, Extend follows the method's actual working-space
   continuation, and Error rejects outside queries. A required
   inadmissible factor fails through its owner.
   [Accepted discount factor and tail semantics](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction).

4. **Match the fit method to prepared dependencies.** Admit standalone
   discount bootstrap only when each quote can be solved on its
   current prefix and later knots cannot move its pricing inputs.
   Elect simultaneous fitting for coupled discount fits and the
   contractual projection target. Elect any private sequential seed
   explicitly. Retire automatic polishing and seed promotion. Elect
   the actual derivative method and skip a derivative only when the
   admitted pricing dependencies prove it is zero.
   [Accepted bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Accepted simultaneous fit](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates) ·
   [Hagan and West on curve interpolation and calibration](https://bank.uni-hohenheim.de/uploads/media/Hagan_and_West__2006__-_Interpolation_Methods_for_Curve_Construction.pdf).

5. **Reprice the exact delivered evaluator before publication.** Keep
   trial bindings private and bind the completed evaluator to every
   required pricing role for final repricing. Require one signed
   finite unweighted native residual per consumed quote, including
   contractual growth for projection deposits and the elected points
   or cash scale for futures. Keep numerical termination, quote fit
   and required curve checks distinct. Publish the admitted evaluator
   with exact construction choices for restoration and replay.
   Sampled checks cover their declared scope. Detailed quote domains,
   grids, bounds and weights belong to the target records.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted curve checks](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication) ·
   [Accepted rate replay](../calibration-framework/19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication).

## Independent calculation checks

- Factors `1,.98` at years 0 and 1 yield year-2 factor `.98`
  under Flat, `.96` under Linear Extend and `.9604` under
  LogLinear Extend. Error rejects the query.
- Projection factors `P(S)=1,P(E)=1.01` and accrual `1/2` give
  an annual decimal rate of `−2/101`.
- PCHIP on `(0,1),(1,.98)` gives midpoint factor `.99`.
  Adding `(2,.94)` changes the same midpoint to `2381/2400`,
  a unit-cashflow PV difference of `1/480` while the original
  pillars still agree.

## Observed choice and rationale at the audit revision

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

## Observed scope and selection

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
| Local discount bootstrap returns directly; nonlocal styles trigger global polishing; explicit GlobalSolve may adopt its bootstrap seed | [Observed discount dispatch and acceptance](03-discount-calibration.md#observed-ordered-algorithm) |
| Forward Bootstrap and MonotoneConvex are unsupported; the target uses numerical derivatives | [Observed forward selection and input contract](04-forward-calibration.md#observed-choice-and-inputs-at-the-audit-revision) |
| Explicit Rust parameter values versus serialized omissions | Entry-point default notes in the [discount](03-discount-calibration.md#observed-choice-and-rationale-at-the-audit-revision) and [forward](04-forward-calibration.md#observed-choice-and-inputs-at-the-audit-revision) targets |

The table records the audit revision's selection. The linked owners
describe its defaults, units, clock inference and algorithm controls.

## Observed numerical and validation boundaries

[Value](../core-numerics/value-interpolation.md) and
[forward interpolation](../core-numerics/forward-interpolation.md) own kernel
formulas and working-space tail semantics. The [LM kernel](../core-numerics/levenberg-marquardt.md)
owns generic optimization; [accepted configuration](../calibration-framework/02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market),
[accepted bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing), and
[accepted weighted global solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates) own caller
controls and dispatch. A requested target-supplied Jacobian need not be analytic:
the discount target supplies central finite differences through that interface.

[Accepted fit reports](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) gate native unweighted
residuals separately from the weighted objective. Rate residuals are generally
raw PV/notional, but [forward deposits](04-forward-calibration.md#observed-algorithm)
use undiscounted contractual accrual-growth error. Numerical solver tolerance
is distinct from target fit acceptance and the additional
[sampled financial validation](../calibration-framework/16-curve-validation.md).

Invalid grids, missing dependencies, unsupported combinations and unusable
solver settings error through their owners. [Accepted execution policy](../calibration-framework/03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution)
gates failed fits before installing outputs by default; explicit permissive
policy allows diagnostic failed reports that callers must inspect.
[Provenance](../core-market/calibration-provenance.md) records replay boundaries.

## Observed alternatives and tradeoffs

Log-linear DFs are local and preserve positivity; smoother interpolation changes
off-pillar values and can require a coupled solve. CubicHermite rates admit
zero/negative values and preserve local shape; Linear is an explicit simpler
alternative. LogLinear requires positive ordinates and cannot represent
arbitrary negative-rate inputs.

Sequential fitting is simpler for local discount interpolation. Global least
squares accommodates coupled reset periods and nonlocal styles with greater
solver/initialization sensitivity, not a guarantee of a global optimum.
Polishing, derivative substitution and seed adoption were numerical
paths at the audit revision. Off-pillar validation depends on the
actual evaluated scope.

## Evidence and qualification

The linked owners retain source/test evidence, including Hagan–West formula
cases, failed-fit handling and the attributed Bloomberg OIS comparison.
Production market qualification is pending. Technical context remains
[Hagan–West](../../../REFERENCES.md#hagan-west-monotone-convex) and
[multi-curve construction](../../../REFERENCES.md#ametrano-bianchetti-2013).
Other calibration families have their own target records.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
