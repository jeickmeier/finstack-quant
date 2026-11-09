# CORE-MARKET-18: Explicit base-correlation sources and bounded parameter shocks

- Decision status: **Accepted — complete base-correlation source/shock package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted admission, query, diagnostic, shock and consumer reconciliation is unimplemented.
- Validation status: **Primary/source evidence and independent source/shock, credit-model and consumer/host reviews inspected; standalone rational/high-precision/binary64 examples executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Examples do not certify loss-model pricing, calibration, risk or arbitrage freedom.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns `BaseCorrelationCurve`'s detachment/correlation representation,
checked source queries, literal shape diagnostics and direct parameter shocks.
It preserves accepted [native-unit boundaries](scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
[record 14's exact-target/checked replacement principle](volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks)
and [lossless construction history](calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).
A detachment curve is different from a correlation matrix or an automatically
qualified tranche loss model.

[Base-correlation calibration](../calibration-targets/15-base-correlation.md),
[scenario financial operations](../scenarios/10-correlation-shocks.md),
[loss-family selection](../models-and-valuations-credit/0031-tranche-loss-model-selection.md),
[loss distributions](../models-and-valuations-credit/17-tranche-loss-distribution.md),
[tranche risk](../models-and-valuations-credit/19-tranche-risk.md),
[credit-index composition](credit-index-composition.md) and
[context lifecycle](context-lifecycle.md) retain their remaining Draft choices.
This source contract does not adopt their complete copula, loss/recovery,
calibration, interpolation-in-time, realized-state mapping or risk methodology.

## Foundational findings

Core correctly keeps nodes and interpolation state private and rebuilds on
restore, but currently sorts input/restore, requires two knots and inherits an
arbitrary minimum knot gap. Queries return unchecked f64 and extrapolate even
outside the financial detachment domain. Shape diagnostics hide small literal
decreases and use arbitrary near-boundary warning bands.

Both generic and bucket shocks clip results to [0,1]; generic resolution also
admits negative factors and hidden RateBp units. A tolerance selector can select
two close knots for one unmatched request. Actual scenario detachment input is
basis points, but its adapter divides by 10,000 even though core coordinates are
percent: 700bp becomes 0.07 rather than 7. Correlation-change units are a separate
quantity, with 0.01 meaning one absolute correlation percentage point.

Current tranche pricing smooths source correlations near 0/1 into interior
values. Correlation01 separately clips shifted nodes yet retains a nominal
central denominator. Neither numerical source bounds nor an increasing
correlation curve establishes consistent tranche expected losses.

## Accepted decision: exact base-correlation sources and unclipped native shocks

Accepted on **2026-11-08** after independent source/shock, credit-model
and consumer/host reviews. Runtime/caller/host reconciliation and independent
financial qualification remain pending.

1. **Keep base-correlation meaning and financial association explicit.** A node
   rho(K) is the selected model parameter for the base-equity tranche [0,K].
   It is not a compound correlation for [A,D], a factor loading, default-indicator
   correlation or a matrix element. In the homogeneous one-factor Gaussian
   model, asset correlation rho uses loading sqrt(rho); other copulas require
   their actual qualified interpretation. Copying parameters between models is
   not convention conversion or evidence of retained fit.

   The active financial owner binds the source to the actual reference index/
   series or portfolio/weights, contractual maturity and market origin, hazard/
   recovery basis, selected copula/loss model and original quote/construction
   policy. Require that association for use; an ID, constituent count or a
   stored credit-index reference alone does not prove compatibility. Numeric
   model inputs need no invented calibration history; actual fits preserve their
   accepted lossless provenance without metadata alone certifying current fit.
   Do not add a generic correlation-convention framework or duplicate fit state.

2. **Retain literal percentage coordinates and a closed correlation domain.**
   Source/query detachment K is percent of original pool notional in [0,100]:
   3 means 3%, 0.5 means a legitimate 0.5%, and the loss fraction is K/100.
   Correlation is a dimensionless fraction in [0,1], including exact endpoints.
   Never guess fraction versus percent from magnitude or reject all sub-1-percent
   tranches as fraction-like input. Convert explicitly fraction-valued calibration
   fields by multiplying by 100 and integer detachment basis points by dividing
   by 100. Thus 0.03 fraction and 300bp both map to K=3. Do this exactly once at
   actual boundaries; do not confuse detachment bp with correlation-change units.

   K=0/100 may be deliberate source/model coordinates, but zero-width/full-pool
   expected loss does not generally identify correlation under fixed marginals.
   Do not manufacture these anchors or label them observed fitted correlations.
   The financial owner handles zero-width/full-pool branches and decides whether
   correlation is needed, rather than forcing an unused source lookup.

3. **Own strict immutable source state and a genuine constant case.** Require
   nonempty ID and nonempty aligned finite nodes in supplied strictly increasing
   detachment order, with the domains above. Admit nonmonotonic correlation
   levels; no sorting, deduplication, gap floor, default anchors or monotone
   projection. A singleton is an explicit constant source over the admitted K
   domain. Do not invent a second equal-correlation knot to satisfy an unrelated
   two-point kernel. The shared numeric Linear interpolator retains its own
   minimum count; the financial owner handles its constant case directly.

   Preserve private immutable nodes and derived interpolation state as one
   source. Restore rebuilds the actual checked evaluator from canonical raw
   nodes; no independently persisted cache, public mutation or unchecked twin.
   Remove calibration's synthetic second knot and tolerance-deduplicated source
   publication; actual solver/fit policies remain owned by the calibration record.

4. **Use checked bounded detachment lookup.** Replace `correlation` with
   `get_correlation -> Result`. Validate finite K in [0,100] before constant,
   exact-node or tail identities. Negative, above-100 and nonfinite requests
   error. Return exact stored levels at nodes, stable checked Linear interpolation
   between them and fixed flat levels outside the stored span within that
   financial domain. No epsilon denominator, artificial minimum spacing, unused
   slope requirement, error-to-NaN or query clipping.

   Reuse accepted numerical interpolation ownership and complete-result
   arithmetic. Preserve representable subnormal results and ordinary final
   rounding rather than lose an interior contribution through separately rounded
   intermediate multiplication. Propagate actual computation/range failures.
   This record selects the source's fixed linear/flat interpretation, not a
   time-axis model or an alternative arbitrage-calibrated loss surface.

5. **Keep shape diagnostics literal and nonblocking.** Replace the misleading
   validating accessor with `get_shape_report`: report each exact adjacent drop,
   actual correlations/detachments and largest drop. A boolean describing
   nondecreasing data must not hide decreases below 1e-9. Report actual rho=0/1
   endpoint presence as consumer-limit information; remove fixed 0.02/0.98
   warning bands as generic admissibility criteria. Numerical conditioning
   warnings belong to the actual model/error policy.

   This report does not block source construction or shocks, require increasing
   rho, repair it or establish positive-semidefinite matrices, price monotonicity
   or tranche no-arbitrage. A deliberately constrained fitter may have its own
   explicit monotonic parameter policy; it is not a financial theorem implied
   by this source report.

6. **Apply one native checked shock without clipping.** Support finite native
   Additive/Fraction delta b and Multiplicative/Factor k>=0. For example b=0.01
   adds one correlation percentage point, k=1.1 multiplies rho by 1.1, and k=0
   intentionally yields zero at selected nodes. Reject negative/nonfinite factors,
   unsupported units/styles and inadmissible final correlations, including
   overflow, without clipping, floors, smoothing or transformed-parameter repair.
   Genuine rho=0 or k=0 is valid; a strictly positive product lost to zero is a
   computation/range failure, not an intentional zero-correlation action.
   Generic RateBp/Percent interpretations are removed; financial adapters convert
   their explicitly declared correlation-point units to native values.

   Bucket amounts are the same native additive changes. Resolve each requested
   finite unique K to an exact stored knot; unmatched/out-of-domain/duplicate
   targets error, without tolerance matching, nearest selection or silent no-op.
   None selects all and [] is an intentional empty selection. One checked target
   resolver serves application and previews/reports; remove a public unchecked
   selector. Validate source, units, amount and all filters before empty/identity
   shortcuts. Empty requests are reported as intentional; invalid requests do
   not become valid because their amount is zero.

   Construct and admit the entire selected replacement atomically. Preserve
   source ID, coordinates, interpretation and actual metadata unless an explicit
   separate rename is requested. Rebuild derived state once and reconcile actual
   context/composite rebinding, without stale mixed dependencies. A direct
   parameter change is not a tranche-quote-preserving recalibration; historical
   construction records remain available without automatic current-fit authority.

7. **Consume the actual source parameter or refuse unsupported use.** Preserve
   rho endpoints in the selected loss model. Gaussian rho=0/1 limits have their
   own supported mathematical/numerical branches; other copulas need their own
   limit contracts. Source endpoint admission does not certify every integrator.
   Implement a qualified exact limit or refuse it explicitly, rather than map
   source parameters into an interior 0.01/0.99 band or quietly change the model.
   The same applies to valid near-boundary interior correlations: smoothing is
   a different source/model operation, not validation of an unchanged parameter.

   A detachment-only curve does not select maturity interpolation, interim-date
   reuse, realized-loss/recovery erosion, original-versus-effective strikes or
   bespoke-pool mapping. Those remain explicit qualified financial model choices.
   Cloning/renaming/context roll does not create a new maturity/current fit.
   There is no annualization or expiry-clock retag for dimensionless correlations.

8. **Qualify loss consistency on one actual pricing basis.** For fixed-horizon
   portfolio loss L in original-pool fractions, equity capped loss is
   C(k;rho)=E[min(L,k)] with k=K/100. Base-correlation tranche loss uses
   C(d;rho(D))-C(a;rho(A)) on the same pool basis; divide by d-a only when the
   requested result is a tranche-notional loss fraction. Do not directly subtract
   separately normalized equity-loss fractions or quoted spreads. Full PV/leg
   subtraction also needs common contractual dates, cashflow and notional bases.

   For one common loss law, C(k) is nondecreasing, concave and 1-Lipschitz.
   Varying rho with k does not automatically preserve these properties; even an
   increasing rho curve can yield a negative tranche loss. Actual loss/calibration
   owners qualify their consistency/error budgets and report or refuse financial
   failures under their selected model. Core shape/storage bounds are not that
   validation. Existing interim/recovery floors, maturity tests and outstanding-
   notional repairs remain Draft methods, not approved evidence of consistency.

9. **Keep risk units and boundary geometry honest.** Direct correlation-point
   risk is different from market-tranche-quote rebootstrap or another copula's
   parameter risk. Correlation01's reporting move is delta rho=0.01, not one
   detachment bp or automatically one credit-spread bp. Base and bumped PVs must
   use the same qualified financial evaluator/dependencies, or refuse unsupported
   metric/model combinations instead of substituting the default Gaussian pricer.

   At bounds, the risk owner selects an actually supported central/reduced-step/
   one-sided scheme or returns an unsupported-request error. Use actual admitted
   actions and finite-difference widths. Do not clip individual nodes and retain
   the nominal 2h denominator, silently change requested geometry or claim
   central accuracy from an asymmetric boundary action. Risk acceptance, effective
   step reporting and nonlinear residuals remain with the actual risk owner.

10. **Reconcile strict state and existing exposed routes together.** Reuse
    canonical source admission for builder/restore/rename/shock/host/model paths,
    rejecting missing required/unknown/incompatible state without defaults,
    sorting, old readers, aliases or migrations. Reconcile actual core context/
    CreditIndexData/state/schema, calibration tranche-quote/CreditIndex MarketDatum and source publication, scenario
    adapters/fixtures/previews, tranche pricing/risk and existing Python/WASM
    classes, stubs/declarations/parity/examples. Rust owns checked financial
    arithmetic and mapped failures. No new host class, namespace, correlation
    matrix repair or generic pricing fallback is introduced.

## Observed implementation and discrepancies

Core admits finite K in [0,100], rho in [0,1] and decreasing levels, but requires
at least two nodes and sorts construction/restore. Shared interpolation imposes
its old minimum gap and uses the staged y0+w*(y1-y0) expression. Queries delegate
unchecked and extend flat beyond even the financial domain. Canonical private
state and rebuilt interpolation already prevent mutable-node/cache divergence.

Shape reporting ignores drops within 1e-9 and warns outside 0.02/0.98. Both shock
routes clip; generic resolution permits negative factors and hidden RateBp, and
bucket matching uses absolute 1e-10. Generic direct bumps change the ID before
context renaming. The actual scenario detachment conversion divides by 10,000;
calibration fraction-to-percent conversion is a different boundary and is already
explicit. Calibration also sorts/deduplicates nodes and adds a synthetic knot
for a singleton; its solver bounds/monotone-fit rule remain separate Draft policy.

Tranche pricing reads rho at original attachment/detachment then smooths near
bounds, including rho=0/1. Correlation01 clips shifted nodes, divides by nominal
central width and has specialized routes constructing a default Gaussian pricer.
Source and CreditIndexData state alone do not qualify maturity/series/model fit.
These are source/control-flow findings, not executed production pricing or risk
results. Full downstream loss/recovery/fit/risk methodologies remain Draft.

## Independent examples and qualification limits

On **2026-11-08**, sixteen standalone rational/200-digit Decimal/binary64 cases
were evaluated without production imports. Transcendental contrasts are bounded
expression references, not production accuracy certification:

- Scenario detachment 700bp maps to 7 percent; division by 10,000 gives 0.07
  percent instead. Separately, calibration detachment 0.03 fraction maps to
  3 percent. These are different declared input units, converted exactly once.
- Gaussian loading 0.5 gives latent asset rho=0.25; using rho as the loading
  instead gives 0.0625. Separately, with equal PD=0.5 and latent rho=0.5,
  default-indicator correlation is 2*asin(0.5)/pi=1/3, not 0.5.
- K=3/7, rho=0.25/0.45 gives rho(5)=0.35. Separately, minimum positive binary64
  ordinates m and 2m at the same midpoint have exact represented-input value
  1.5m, rounding to 2m; the source's staged expression returns m. This is source-
  path inspection plus standalone arithmetic, not execution of the Rust kernel.
- Knots 0.25 and 0.25+1.5e-10 pass the old spacing rule, but their unmatched
  midpoint falls within the bucket tolerance of both. Separately, an approximately
  5e-10 actual adjacent rho drop is hidden by the old 1e-9 diagnostic tolerance.
- rho=0.99,h=0.02 clips to up/down 1/0.97, reducing actual central width to
  75% of the request. Separately, multiplying rho=0.4 by -1 produces -0.4;
  clipping it to zero does not make the original action valid.
- For the stated linear proxy PV=rho at rho=0.995,h=0.01, clipped nominal-width
  Correlation01 gives 0.0075 instead of 0.01. Separately, the current smoother
  maps endpoints to approximately 0.01024893534/0.98975106466, changing parameters.
- In a stated two-name equal-weight Gaussian endpoint example, each PD=0.5 and
  recovery=0. Independence gives pool loss 0/0.5/1 with probabilities
  0.25/0.5/0.25; perfect correlation gives loss 0/1 with probabilities 0.5/0.5.
  With A=25%,rho(A)=0 and D=30%,rho(D)=1, capped equity ELs are 0.1875/0.15:
  their difference is -0.0375 despite increasing rho. Separately, both laws have
  capped EL=0 at k=0 and EL=0.5 at k=1; those endpoints do not identify rho.
- In a separate deterministic loss example L=0.05,a=0.03,d=0.07, pool-basis
  subtraction followed by width normalization gives tranche loss fraction 0.5.
  Directly subtracting separately normalized equity losses instead gives -2/7.
  This fifteenth case checks units, not a complete tranche PV implementation.
- For minimum positive binary64 m and factor 0.5, the exact product is positive
  but binary64 multiplication returns zero. In contrast, rho=0 or factor=0 gives
  genuine zero. This sixteenth case distinguishes shock range failure from an
  intentionally admitted zero.

Independent production qualification must cover strict order/domains/singletons/
restore, checked exact/nearby/subnormal queries and bounded tails; literal shape
reports versus financial loss tests; explicit shock units/exact targets/empty/
identity/range failure and atomic cache/state/composite replacement; actual
scenario/calibration unit conversion and financial maturity/pool/model/recovery
association; supported endpoint limits, pool-basis loss consistency and actual
risk widths/evaluator parity. Reconcile all host/schema/state routes and use
independent loss/market/fit/risk fixtures at the actual owners. Algebra, source
bounds, same-kernel round trips and sampled quotes do not establish these results.

## Evidence and alternatives

Owners: [canonical source](../../../../finstack-quant/core/src/market_data/term_structures/base_correlation.rs),
[source tests](../../../../finstack-quant/core/tests/market_data/curves/base_correlation.rs),
[context storage/rename](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs),
[CreditIndexData](../../../../finstack-quant/core/src/market_data/term_structures/credit_index.rs),
[scenario adapter](../../../../finstack-quant/scenarios/src/adapters/basecorr.rs),
[calibration publication](../../../../finstack-quant/calibration/src/targets/base_correlation.rs),
[loss evaluation](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/expected_loss.rs),
[smoothing/direct risk](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs)
and [metric dispatch](../../../../finstack-quant/valuations/credit/src/cds_tranche/metrics/mod.rs).
Existing source/shape/serde, context rename, scenario and calibration/tranche tests
were inspected or located, not executed.

Primary context: [Amato–Gyntelberg, BIS Quarterly Review March 2005, page 83](https://www.bis.org/publications/cds-index-tranches-and-pricing-credit-risk-correlations_1.pdf)
explains base-equity correlation construction; its historical market tables are
not adopted as current universal conventions. [Gibson, Federal Reserve, section 3
and equation 1](https://www.federalreserve.gov/pubs/feds/2004/200436/200436pap.pdf)
distinguishes factor loadings from latent asset correlation and conditional pool
loss inputs. Canonical [Gibson](../../../REFERENCES.md#gibson-2004-synthetic-cdos)
and [Laurent–Gregory](../../../REFERENCES.md#laurent-gregory-2005-factor-copulas)
entries identify further model provenance. These sources do not qualify this
repository's pricing, risk, fit budgets or this record's architectural choices.

Transformed shocks, monotone/loss-space projections, term surfaces, bespoke-pool
mapping and alternative copula recalibration retain explicit financial owners
rather than becoming hidden source clipping, defaults or fallback models.

## Follow-up tracking

Runtime reconciliation and independent qualification remain pending and are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
