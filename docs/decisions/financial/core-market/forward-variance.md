# CORE-MARKET-17: Explicit forward-variance sources and qualified model consumption

- Decision status: **Accepted — complete forward-variance source/consumer package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted admission, clock, checked-query and consumer reconciliation is unimplemented.
- Validation status: **Primary/source evidence and independent source/numerical, model/measure and consumer/host reviews inspected; standalone rational/high-precision/normal-price examples executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Examples do not certify rough-model simulation, calibration or arbitrage freedom.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns `ForwardVarianceCurve` as an expected instantaneous variance-rate
source, its numerical point/interval representation and actual model-consumer
compatibility. It preserves accepted [ADR0045's clock/source-resolution boundary](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility)
and [record 14's distinct sigma-grid domain](volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).
A variance rate is different from volatility amplitude, implied sigma and total
integrated variance. A variance source need not have been calibrated.

[Rough Bergomi dynamics/discretization](../models-monte-carlo/14-rough-bergomi.md),
[rough Cheyette amplitudes/state](../models-monte-carlo/16-rough-cheyette.md)
and the [equity ATM-strip producer/pricer](../valuations-equity-and-composition/11-rough-volatility-adapters.md)
retain their remaining Draft choices. This source contract does not adopt their
full Gaussian driver, compensation, measure, fit, simulation, grid, error budget
or financial pricing methodology.

## Foundational findings

Current core admission rejects genuine zero variance, sorts point input/restore,
extends negative-time queries left-flat and returns NaN for nonfinite queries.
Its ordinary staged linear blend can lose representable subnormal information.
Point samples and ending-interval averages already have different meanings;
identical arrays do not define the same function in these two representations.

Rough Bergomi consumes the curve as variance. Rough Cheyette stores the same type
as sigma_base but deliberately consumes it directly as a volatility amplitude.
Its current example 0.005 means 50bp sigma, whose square is 0.000025. This is a
financial type/contract misuse, not proof that today's Cheyette prices are wrong
or permission to reinterpret those numbers as variance.

The actual equity producer differences sampled ATM-forward implied total
variance. That is a declared approximation, not an exact expected-variance
identity, variance-swap bootstrap or rough-model calibration. Even an exact
source interval-area identity does not make a simulation grid that crosses its
breaks integrate those areas exactly.

## Accepted decision: nonnegative variance sources and explicit model-time semantics

Accepted on **2026-11-08** after independent source/numerical, model/measure
and consumer/host reviews. Runtime/caller/host reconciliation and independent
financial qualification remain pending.

1. **Own variance-rate meaning at the actual financial boundary.** Interpret
   xi_0(t)=E^Q[V_t|F_0] under the consuming model's declared underlying variance
   role, pricing measure and original information/conditioning basis. The owner
   supplies that association; a numeric curve alone does not attest it. For
   rough Bergomi, values are squared decimal returns per source/model year:
   0.04 matches 20% annual relative volatility. Other variance roles require their own explicit units/qualified model association, not shape reuse.
   This is neither sigma nor a sigma quote/total-variance observation. Retain
   actual construction/proxy/fit provenance at its producer, without inventing
   a new source ID, context namespace or core measure/convention framework.

2. **Admit exact nonnegative canonical data without repair.** Require nonempty
   finite source coordinates and finite values>=0. A flat source stores its
   explicit value at time 0, including zero. Point times are nonnegative and
   strictly increasing in supplied order; ending-interval coordinates are
   strictly positive increasing from implicit initial boundary 0. Singletons
   are valid. Reject unordered/duplicate/nonfinite/misaligned state rather than
   sorting, merging, tolerance snapping or filling. No tiny variance floor,
   strictly-positive global restriction or automatic vol-to-variance conversion.

   Require the representation explicitly through the existing point/interval
   constructors and mandatory stored mode. The same times/values cannot infer
   their own mode. Reuse full validation in restore and embedded process data;
   preserve the canonical representation rather than reconstructing a default.

3. **Retain literal point and ending-interval behavior.** Point sources return
   exact stored levels at knots and interpolate variance levels linearly between
   them. The first point may be later than 0: its level applies flat from 0 to
   that first point. The final level extends flat for later finite times. These
   are fixed source rules, not hidden pre-origin historical extrapolation.

   Ending-interval values apply on [previous_end,end); the first begins at 0.
   At an exact interior end, select the next interval. The last level extends
   flat beyond the final end, with no zero tail or extrapolated slope. Thus
   interval areas are sums of xi_i*duration; a boundary's single-point convention
   does not change an integral. Do not reinterpret interval averages as point
   observations or interpolate sigma and square it. Source area identities do
   not require adding an unused generic core integration API.

4. **Make level queries checked and numerically faithful.** Replace unchecked
   `value` with `get_value` returning `Result`. Validate finite nonnegative time
   before exact-knot, constant, zero or tail identities. Negative/nonfinite
   queries error; no historical left-clamp, NaN sentinel or error-to-zero path.
   Use stable complete convex interpolation and checked coordinate arithmetic,
   without an unnecessary slope, epsilon denominator, clipping or floor.
   Preserve representable subnormals and ordinary final rounding; avoid losing
   a required contribution through a separately rounded intermediate product.
   Propagate computation/range failures through actual consumers and hosts
   rather than retaining an unchecked twin or substituting a model value.

5. **Name the variance clock and retain the original conditioning origin.**
   Reuse the accepted data-only `VolatilityClock` representation as optional
   `variance_clock`, explicitly a variance-density time role rather than an
   `expiry_clock` retag. Numeric model construction may omit it; the caller then
   supplies coordinates and parameter units on its declared numerical clock.
   Dated producers record the actual origin/day count/required context; dated
   active consumers resolve/agree with that mapping under ADR0045's source
   admission rules, without a universal Act365F or borrowed coupon/discount clock.
   Preserve the metadata through clone, state and actual derived process inputs.

   Ordinary current-source use requires origin=valuation as_of unless an
   explicit owned frozen/conditioned/recalibrated model applies. Merely shifting
   numeric time or copying old levels does not produce E[V_(a+u)|F_a]. Restarting
   rough dynamics with zero Volterra history is a separate model, not a core roll.
   Do not add context-roll machinery for this currently embedded model input.

   Actual source/model clocks must agree or use a supported owned conversion
   preserving integrated variance. For a positive linear time change t'=c*t,
   xi'(t')=xi(t'/c)/c. Renaming knots or applying the endpoint sigma variance
   bridge alone does not define a density conversion or transform eta, Hurst,
   Gaussian history or the full rough model. An arbitrary dated mapping is not
   automatically a supported stochastic clock.

6. **Qualify zero at the actual model operation.** Zero means zero variance
   contribution at that time/interval, not an absorbing future curve or permission
   to discard later positive values or fractional history. A consumer supports
   the mathematical zero case or refuses unsupported use explicitly. After
   validating actual required controls/history, avoid computing an unnecessary
   overflowing exponential and then multiplying it by zero. Do not mask invalid
   active state or globally stop the driver because one source sample is zero.
   Source zero admission does not adopt a simulation scheme, remove unrelated
   eta/Hurst bounds or certify deterministic-limit pricing.

7. **Remove the Cheyette volatility/variance type misuse.** Give sigma_base an
   explicitly named model-owned volatility-amplitude representation with its
   actual units and selected sampling contract; remove its `ForwardVarianceCurve`
   use and obsolete wire path. Preserve the intended amplitude values and
   interpolation semantics while reconciling the input. Do not silently take
   square roots, relabel ordinates, or square linear-amplitude knots and then
   interpolate variance: those operations change the represented function.
   Existing numeric curve utilities may support implementation where their
   actual semantics fit; their availability does not justify replacing a linear
   amplitude with a constant one. No new generic core curve family is required.
   Cheyette's full amplitude/model/state policy remains with its Draft owner.

8. **Keep ATM strips and simulation sampling separately qualified.** The equity
   producer's ATM-forward strip is an explicitly chosen proxy for expected
   variance. Select/qualify the active scalar or actual Black/absolute-strike
   surface before resolving its dependencies. An active explicit scalar/model
   contract needs no unused surface; do not evaluate an unused strike-specific
   quote merely to construct the separately selected ATM strip. Retain source
   clock, original strip coordinates, actual forward/strike mapping, supported
   interpolation/tail operation and construction policy at this producer.

   Reconcile exact ordered strip nodes without fixed 1e-12 merging/filtering,
   checked complete sigma^2*time differences/division, zero slopes and full
   failure propagation. Genuine negative interval variance cannot be admitted;
   refuse an unsupported proxy without clipping or repairing the source.
   Rejecting a decreasing moving-ATM strip is a proxy-admission condition, not
   proof of general calendar arbitrage. Positive strips also do not establish
   a calibrated rough model or an arbitrage-free smile. A flat scalar's square
   needs the actual representation/range checks; no sign bypass is inferred
   from the private builder when the real caller already validates sigma.

   Interval-source area conservation does not certify a coarse left-sampled MC
   grid. The actual simulation owner qualifies break alignment, quadrature and
   path bias. Preserve adaptedness; do not switch to current-step end variance
   merely to force source totals. Fit, Gaussian covariance, path budgets and
   financial precision remain independent model acceptance work.

9. **Reconcile the actual exposed paths together.** Strict state preserves
   representation, raw ordered data and optional variance clock, rejecting
   unknown/missing required/incompatible fields without old readers, aliases,
   defaults, repairs or migrations. Reconcile canonical Rust source queries,
   rough-model process/stepper/dispatch, the actual equity proxy, existing
   PathSimulationSpec/ProcessSpec plain data, Python/WASM simulation functions,
   stubs/generated declarations/model schemas/parity and examples. Use named
   params/builders where additional controls would exceed the positional limit.
   Rust owns all financial arithmetic/errors.

   There is no current standalone host class or ForwardVarianceCurve entry in
   MarketContext/calibration/scenario dispatch. This contract does not add one,
   generic curve bumping or sigma-grid-vega fallback. Direct parameter stress,
   expected-variance fitting and quote-recalibrated risk remain separate owners.

## Observed implementation and discrepancies

Core stores only mode/times/values. Both constructors reject zero; points sort,
so point restore also repairs source order. Interval restore keeps ordered ends.
Exact interval ends already select the next value and last levels extend flat.
Finite negative queries get the first level; nonfinite queries return NaN. The
literal v0+w*(v1-v0) blend can lose a representable subnormal midpoint. No date,
clock, financial association, integration or core roll is stored/computed here.

Rough Bergomi's dedicated scheme uses left-endpoint xi(t) before the current
fractional increment, multiplies by its stochastic exponential and then takes
a square root for spot diffusion. It currently has no admitted zero source.
Cheyette's scheme instead multiplies sigma_base directly as sigma and squares
that amplitude in the auxiliary-state drift. Its present 50bp amplitude values
are intentional; the shared variance type is the discrepancy.

The equity adapter samples clamped ATM-forward sigma, differences total variance,
rejects nonpositive strip slopes and uses a uniform simulation grid. It filters
nearby times with fixed thresholds and uses flat effective rates for its forward
strikes. The actual shared caller validates scalar overrides and may resolve a
strike-specific quote later discarded by strip construction. These are source/
control-flow findings, not executed production pricing or simulation results.
The wider adapter/model methods remain Draft.

## Independent examples and qualification limits

On **2026-11-08**, fourteen standalone rational/200-digit Decimal and standard-
library normal-price examples were evaluated without production imports.
Normal CDF/quantile contrasts are approximate binary64 references, not high-
precision normal-function or production accuracy certification:

- Point levels 0.04/0.08 at times 1/3 give midpoint variance 0.06. Separately,
  ending intervals (0.25,0.04),(1,0.12) select 0.12 exactly at time 0.25.
- Those interval arrays integrate to 0.10 over [0,1]; treating them as point-
  linear data with the declared left-flat rule gives 0.07. Separately, squaring
  interpolated endpoint sigmas for variance levels 0.04/0.08 gives approximately
  0.05828427124746 rather than the variance midpoint 0.06.
- For minimum positive binary64 m, point levels m and 2m at a midpoint have exact
  represented-input value 1.5m, which rounds to 2m. The old staged expression
  returns m. This checks interpolation arithmetic, not production accuracy.
- Zero variance on [0,0.25), followed by 0.12 on [0.25,1), has area 0.09;
  the source is not globally zero. Separately, 0*exp(1000)=0 mathematically,
  whereas the avoidable binary64 0*infinity is NaN.
- ATM sigmas 0.20 at time 1 and 0.10 at time 2 give a total-variance difference
  slope -0.02. Separately, an equal-weight mixture of Black terminal laws with
  F=1,T=1 and sigmas 0.1/0.3 has ATM implied sigma approximately 0.1997501572244
  and implied variance 0.0399001253112, versus expected log-contract variance
  0.05. This stated risk-neutral terminal-mixture example demonstrates the
  nonidentity; it does not adopt a variance-swap replication/fitting method.
- Cheyette amplitude 0.005 has squared amplitude 0.000025; interpreting 0.005
  itself as variance changes that input by 200 times. Separately, linear
  amplitudes 0.01/0.03 have midpoint squared 0.0004, while interpolating their
  squared endpoints gives 0.0005. No current pricing error is reproduced here.
- A time change from 30/365 to 30/360 multiplies time by 365/360; dividing a
  constant variance rate by that factor preserves its area. Separately, three
  equal left steps over the 0.25 interval break above give 0.0933333333333 rather
  than 0.10. These are density/grid contrasts, not full rough-model conversions.
- In a stated two-branch conditional-mean example with variance levels 0.01/0.09
  equally weighted, the original mean is 0.05. Revealing the 0.09 branch gives
  conditional mean 0.09, not a shifted copy of 0.05. This fourteenth case checks
  the meaning of conditioning, not a new stochastic aging model.

Independent production qualification must cover strict order/mode/restore,
zero/nonfinite/negative/identity domains, exact boundaries/subnormal blends,
actual unit/measure/clock/information association and dated model admission;
zero-safe active consumers and Cheyette amplitude semantics; scalar/surface
selection, checked proxy arithmetic/strip coordinates and admitted zero slopes;
model-grid breaks, covariance, history, path error and actual pricing accuracy.
Reconcile all exposed plain-data/host/schema paths and compare selected financial
models with independent references. Source bounds, algebra, same-kernel tests
and interval identities do not establish these results.

## Evidence and alternatives

Owning [core source and tests](../../../../finstack-quant/core/src/market_data/term_structures/forward_variance.rs),
[rough Bergomi process](../../../../finstack-quant/models/src/monte_carlo/process/rough_bergomi.rs)
and [stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/rough_bergomi.rs),
[Cheyette process](../../../../finstack-quant/models/src/monte_carlo/process/cheyette_rough.rs)
and [stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/cheyette_rough.rs),
[simulation specs/dispatch](../../../../finstack-quant/models/src/monte_carlo/simulate.rs)
and [equity strip/adapter](../../../../finstack-quant/valuations/equity/src/equity_option/rough_bergomi_mc_pricer.rs)
were inspected. Existing source boundary/serde, model martingale/limit and proxy
strip tests were inspected or located, not executed. Existing host owners are
[Python simulation](../../../../finstack-quant-py/src/bindings/models/monte_carlo/engine.rs)
and [WASM simulation](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs).

The [Bayer–Friz–Gatheral January 2015 preprint](https://www.realvol.com/RVOL2Paper.pdf),
sections 1 and 3–4, defines conditional forward variance and its integrated role,
then separately treats stochastic dynamics and simulation. The canonical
[published-reference entry](../../../REFERENCES.md#bayer-friz-gatheral-2016)
identifies the 2016 paper; the inspected preprint is not a claim to have checked
all published revisions. These sources provide model context, not proof of the
proposed zero-source extension or this repository's numerical quality. Strict
source/clock/consumer boundaries are this record's architectural choices.

Expected-variance calibration, direct variance-swap inputs, explicit sigma/variance
conversion, alternative interpolation and dynamic conditioning retain their
financial owners rather than becoming hidden source repairs/defaults.

## Follow-up tracking

Runtime reconciliation and independent qualification remain pending and are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
