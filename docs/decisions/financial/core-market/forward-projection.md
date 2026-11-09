# CORE-MARKET-02: Index-specific projection factors and contractual forward rates

- Decision status: **Accepted — complete revised factor-based package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; the accepted representation is not implemented.
- Validation status: **Source/reference evidence and two independent architectural challenges reviewed; standalone exact-rational expression checks executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Expression checks do not certify production behavior or a domain-wide accuracy bound.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and existing authority

This decision owns the projection source's representation, index association,
checked factor and contractual interval queries, and its construction boundary.
It preserves [ADR 0045's source-clock/contractual-accrual distinction](../conventions/0045-financial-clocks-and-date-boundaries.md),
accepted [scalar conversion](../core-numerics/compounding.md),
[scalar interpolation](../core-numerics/value-interpolation.md),
[DF-specific interpolation](../core-numerics/forward-interpolation.md) and
[CORE-MARKET-01 discount evaluation](discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction).
Their accepted contracts are not reopened. Discount and projection sources
share numerical factor mathematics, while their financial uses remain distinct.

Contractual tenor, fixing/effective/maturity/payment dates, observation schedules,
fixing eligibility and coupon accrual belong to product/index/schedule owners.
[Record03](rate-shocks.md) retains shock selection/composition and aging economics;
[record06](calibration-provenance.md) owns replay provenance.
[ADR 0032](../calibration-targets/0032-curve-interpolation-and-calibration.md) and
[forward calibration](../calibration-targets/04-forward-calibration.md) retain
market fit, quote selection and acceptance. This decision defines the source
those owners must reconcile to; it does not approve their remaining
Draft methodologies or stochastic index models.

## Foundational re-review

The earlier draft retained a scalar seed-rate function plus a mandatory reset
grid. Re-review rejected that recommendation as the canonical foundation.
A constant 5% seed gives 5% one-year growth with one annual cell, but 5.0625%
with two half-year cells. Current calibration builds the compounding grid from
the union of all quotes' reset boundaries. Adding a quote boundary can therefore
change the evaluated source before any rate parameter changes. This is a model
choice, not merely a more accurate way to evaluate a fixed index curve.

The earlier package also needed public seed means, partial-cell conditioning,
a separate grid horizon and lossy discount-to-seed reconstruction. Internal
algebraic consistency did not establish that these were the right permanent
financial abstractions. The accepted revised package replaces that representation rather
than retaining it as a second mode. The observed runtime below is unchanged.

## Accepted decision: explicit index projection factors and contractual rates

Accepted on **2026-11-08** after foundational re-review. Runtime, caller and
host reconciliation and independent production qualification remain pending.
The observed implementation below remains separate from the accepted contract.

1. **Use one projection-factor representation.** Retain `ForwardCurve` as a
   positive normalized auxiliary factor function P(t), with P(0)=1. Its pillars
   are factors, not simple rate levels. For a selected index x, contractual
   growth over its effective/maturity dates S,E is P_x(S)/P_x(E). P_x is not a
   tradable discount bond or a general PV discount source. Keep public discount
   and projection roles distinct; `ForwardCurve` does not implement Discounting
   merely because both use factor mathematics. Reuse the smallest private
   positive-factor evaluator with accepted01 rather than duplicate interpolation,
   clocks or error rules, or add a general public curve framework.

2. **Preserve an explicit index-to-source association.** Use the existing
   typed IndexId and CurveId at market resolution, with an authoritative mapping
   from each requested index to its projection source. Index identity includes
   the applicable term tenor or overnight category and currency through explicit
   index conventions. Reject missing associations and inconsistent requested
   index/currency metadata; do not parse a curve name to infer conventions or
   choose the nearest tenor. Deliberately sharing a numerical source across
   indices is an explicit model selection, not a default. Keep numerical source
   state and contractual index conventions separate. Existing upper-layer
   RateIndexConventions supplies resolved conventions; core and cashflows must
   not gain dependencies on valuations. Pass the resolved contractual data
   downward and enforce identity when the index source is selected. Fixing-series
   identity remains tied to the configured index observations, not silently
   renamed whenever the selected projection CurveId changes.

3. **Adopt accepted01's positive-factor source contract.** Require explicit
   base date, complete supported source day count/calendar, interpolation and
   Flat/Extend/Error tails. Require finite strictly increasing nonnegative
   pillar times and finite positive factors, including positive subnormals and
   factors above one. Apply the same documented constructor anchor (0,1),
   strict already-anchored canonical restore and at least one positive-time
   pillar. No sorting, snapping, rate floors, monotonicity preset or minimum
   tenor. Retain all five accepted factor interpolation methods: Linear,
   CubicHermite/PCHIP, LogLinear, MonotoneConvex and PiecewiseQuadraticForward.
   Their working variables and literal tails are those of accepted01 and the
   numerical owners. A strategy admissible for positive factors is not thereby
   approved for every index calibration. Unusable intermediate or queried
   factors fail; positive pillars do not prove positivity everywhere.

4. **Remove the independent reset-compounding grid.** Interpolation pillars
   and the chosen evaluator define P. Product observation dates are evaluation
   dates, not an additional chain of simple accrual cells. Delete the seed
   function, mandatory/optional projection grid, numeric tenor stepping,
   implicit continuation and grid extension. Remove curve-level reset lag and
   identifier-derived defaults. Actual calendar tenor and business-day lag stay
   in the index contract. With the evaluated P fixed, introducing intermediate
   query dates cannot change its endpoints or interval growth. Changing pillars,
   interpolation or fitted parameters can change P and must remain explicit.
   This does not promise invariance to changing the calibration problem.

5. **Project a resolved index observation, not a coupon proxy.** Resolve the
   fixing date, effective date S, maturity E and finite positive index accrual
   alpha before projection. Require E>S. The simple annual decimal index rate
   is L_x(S,E)=(P_x(S)/P_x(E)-1)/alpha. Every simple-rate query requires its
   annualization fraction explicitly, including numeric interval queries;
   source-time differences do not supply an implicit contractual alpha.
   Fixing date is not automatically effective date, and coupon start/end are
   not automatically the index period. Apply the projected index rate to the
   separately resolved coupon accrual and spread/gearing rules. Stubs, in-period
   resets and interpolated-tenor fixings require their actual contractual rules;
   arbitrary intervals on one factor source do not authorize every tenor or
   stub convention. Observed fixings retain their own availability/eligibility
   rules and bypass projection when the product requires them.

6. **Use the source clock once and annualize once.** Map S and E with the
   source's complete stored clock; reject clocks needing absent coupon or
   termination context, and require the resolved calendar for Bus/252. Keep
   signed pre-origin factor queries under the selected tails, as in accepted01;
   do not clamp dates to zero or infer fixing eligibility from the curve base.
   A spot-based origin is distinct from the valuation/fixing cutoff. Dated
   projection requires finite mapped a<=b, admitted endpoint factors and
   explicit alpha>0; distinct dates with a=b give zero growth after admission.
   Numeric simple-rate requests require finite a<b and explicit alpha>0.
   Historical projection permission belongs to the product, not this numerical
   boundary. Do not multiply a source-clock annualized rate by a second
   accrual conversion; divide the interval's growth increment by alpha directly.

7. **Evaluate checked endpoint growth stably.** Require both absolute endpoint
   factors to be representable, positive and finite, including before equal-
   coordinate shortcuts; do not add a log-only source domain that bypasses
   accepted01/ADR0045 factor admission. Compute ell=log(P(a))-log(P(b)) using
   accepted01's stable positive-factor log difference, not subtraction of rounded
   cached logs or a potentially overflowing factor quotient. Compute the
   requested simple rate as expm1(ell)/alpha, with the accepted scalar owner's
   stable equivalent scaling where an unnecessary intermediate overflows or
   underflows. A finite rate can be returned when a separately requested growth
   factor is unrepresentable. Check the returned rate's finite value and
   mathematical positive simple-gross domain without requiring a materialized
   finite gross factor; a result rounded onto the zero-gross boundary is a
   computation error. Equal evaluated factors can yield zero; ordinary
   endpoint rounding is not recoverable from nominal inputs. Required nonzero
   log/rate results lost to zero must be reformulated or fail. No arbitrary
   short-tenor cutoff, clipping, NaN sentinel or high-precision runtime fallback.

8. **Remove ambiguous seed-rate and arithmetic-mean APIs.** Remove raw rate(t),
   rate_period and their date variants instead of reinterpreting their outputs
   as instantaneous or period rates under the same names. The source exposes
   checked factors and explicit interval rates. A model needing an instantaneous
   projection quantity must request the named derivative -P'(t)/P(t), with
   accepted01's analytic complete-evaluator and one-sided conventions; that
   quantity is neither an observed term fixing nor an OIS short rate by default.
   Overnight owners project the actual resolved overnight observations and
   combine them with known fixings under the contractual weighting/compounding,
   lookback, observation-shift and lockout rules. Endpoint telescoping is an
   optimization only when those rules establish its equivalence. A whole-coupon
   arithmetic source mean is not a replacement for that calculation. No new
   quadrature or public seed integration machinery is needed for this source.

9. **Calibrate the factor source using actual instrument contracts.** Reconcile
   forward calibration's parameterization, initialization, dependency resolution
   and repricing to factor pillars and the same index-observation projector used
   by valuation. Direct tenor-forward quotes are observations with explicit
   periods and alpha; they are not factor ordinates and are not automatically
   chained across the union of quote dates. The current scalar-rate fit, reset-
   date parameter placement and union-grid builder must be replaced coherently.
   Quote sets, pillar selection, interpolation, solver and acceptance tolerances
   remain explicit calibration-owner decisions. Independent repricing and tenor
   basis evidence are required; factor algebra alone does not establish a market
   fit, absence of arbitrage or appropriate model dynamics.

10. **Make factories explicit and avoid lossy conversion.** Generic builders
    take factor pillars and the required source choices; dated construction uses
    the same mapping/admission. A named flat factory means the accepted01
    two-knot LogLinear+Extend recipe (0,1),(1,exp(-r)) for an explicit signed
    continuously compounded source rate and clock, with its same factor/rate
    rounding and range limits. This is not a flat simple quote for every tenor.
    Remove the fixed-tenor DiscountCurve::to_forward_curve sampling recipe.
    Reusing a discount evaluator for projection requires an explicit index/source
    model selection and preserves the complete evaluator, clock, tails and any
    admitted retained transform; do not refit only reporting pillars or invent
    terminal seeds. Financial role assignment does not itself prove that the
    discount source fits that index. No compatibility path for old rate pillars.

11. **Separate factor conditioning from risk and future dynamics.** For one
    fixed dated source, normalized factors at an admitted reference v are
    P_v(T)=P(T)/P(v), retaining the complete original evaluator and mapping both
    dates on its original clock. Admit both original absolute factors even
    when T=v, and require the requested normalized quotient to be positive
    and finite. This is a dated relative-factor identity, not authorization
    to construct a new source by changing its base date or applying a time
    offset, and not a forecast of future calibrated curves. Do not substitute
    t_old(T)=t_old(v)+t_new(T) unless that
    identity holds for the selected clock; non-additive day counts require the
    actual original date mapping. Record03 must select supported roll operations,
    retained state, backward/expired-horizon behavior and shock order. The old
    seed bump and fresh-grid roll have no unchanged meaning on factor pillars.
    Record03 must define the shocked financial quantity and units explicitly;
    a basis-point shock must not become an additive bump to factor values.
    Stochastic consumers retain their model measure, discount source and drift/
    centering assumptions; a projection factor is not an OIS short-rate process.

12. **Reconcile one strict Rust-owned boundary across all callers.** Canonical
    persisted state contains explicit factor source choices and retained state,
    not seeds, grids, inferred conventions or supplied numerical caches. The
    explicit index association and resolved contractual inputs must also survive
    their owning serialization/host boundaries. Reject missing/unknown/retired
    fields and labels; no aliases or migrations. Rebuild/validate private caches
    atomically and propagate checked input/domain versus computation errors.
    Reconcile Rust construction, market selection, calibration, cashflows,
    valuations, scenarios, Python/WASM parameters, schema, docs and parity in
    coherent slices. Removing mean and raw-rate callers requires
    the financial substitutions below, not mechanical renaming. Do not expose
    partially reconciled source semantics as a working replacement. Production
    implementation and independent financial qualification remain pending.

## Rationale and rejected alternatives

Positive factor sources provide a single interval-growth function independent
of the set of evaluation dates. They reuse the already accepted numerical
foundation and support explicitly distinct discount and index projection roles.
The ratio is a representation for one chosen index; it does not erase tenor
basis or authorize cross-index substitution. Products still own how projected
observations become payments.

A direct fixed-tenor quote curve can be a legitimate separately selected model,
but does not by itself determine arbitrary-period growth. It is not retained as
an interchangeable second ForwardCurve mode. The seed/reset-grid construction
is also a possible specialized model, but its schedule-dependent compounding,
non-quote ordinates and extra integration/reconstruction contracts are not
justified as this library's canonical projection source.

## Observed choice and rationale

The forward curve exposes three different quantities: point rate, arithmetic time-average rate, and simple forward implied by chained projection DFs. Keeping them separate avoids treating an average as a contractual compounded return. The actual construction is explicit left-endpoint simple compounding, not integration of an instantaneous forward.

## Observed algorithm, domain, and conventions

The builder requires a base date, finite positive tenor, nonnegative reset lag, at least two already-ordered knots and finite rate values. It defaults to Linear interpolation and FlatZero (constant boundary forward). It rejects MonotoneConvex; positive-domain interpolation styles still reject nonpositive source rates. An optional projection grid must start within 1e-12 of zero, be finite, nonnegative, strictly increasing, contain at least two entries and cover the last interpolation knot.

For `df(t)`, start at zero and accumulate `logDF -= log1p(F(start)*dt)` for successive grid boundaries, truncating the final interval to `t`. After the explicit grid, continue in numeric `tenor` steps; without a grid all steps use numeric tenor. Each simple-compounding denominator `1+F(start)*dt` must be finite and positive. Failure to advance time is an error. Exponentiation must produce a positive finite DF. `rate_between(t1,t2)` computes `expm1(logDF(t1)-logDF(t2))/(t2-t1)` with finite, strictly ordered, nonnegative projection times.

`rate_between_dates(start,end,accrual_year_fraction)` rejects historical starts before base and returns projection growth divided by the **contractual accrual fraction**, converting from the curve clock. By contrast, `curve_time` clamps dates at or before base to zero, and `rate_period_between_dates` uses that conversion for an arithmetic average. Reset lag is stored convention metadata; these lookup methods do not apply business-day reset adjustments themselves.

`rate_period(t1,t2)` accepts finite ordered times, including negative times. Intervals up to 1e-12 use `rate(t1)`. Otherwise composite Simpson integration uses 8 panels through 5 years, 16 through 20 years, and 32 above 20, divided by interval length. For transformed curves, the source interpolator is integrated on the shifted clock, multiplied by its scale, and the piecewise-linear shock area is integrated exactly. There is no tolerance-driven refinement or split at source interpolation knots.

## Observed selection and controls

Identifier inference uppercases/trims and matches substrings. SOFR/FedFunds/EFFR/ESTR/EURIBOR/SARON select Act/360; SONIA/TONA(R)/TIBOR/CORRA/CDOR/AONIA/BBSW/BKBM select Act/365F; recognized currency prefixes supply a fallback, otherwise Act/365F. OIS/overnight names imply reset lag zero; supported term-index substrings imply two business days. Explicit builder fields override inference. Name inference is a convenience policy, not a registry-backed convention guarantee.

`DiscountCurve::to_forward_curve` samples `(DF(t)/DF(t+tenor)-1)/tenor` at source knots and builds a new forward curve (Linear unless overridden), preserving base and day count. This is resampling into a new projection representation; exact reconstruction away from compatible reset grids is not guaranteed. Rebuilding forward knots extends any existing projection grid by numeric tenor and discards the old evaluation transform. Rolling shifts the rate function and surviving grid; it does not assert conditional projection-DF identity within a partially elapsed simple-compounding reset interval.

## Observed failure, alternatives and evidence

Point-rate lookup may propagate NaN from no-extrapolation or nonfinite queries. Finite negative rates are allowed subject to the selected kernel and positive projection denominators. Supported explicit grids represent contractual reset dates more faithfully than a fixed numeric tenor; a discount-factor-native projection curve or adaptive knot-split integration would be different, unimplemented choices here.

Owning code: [builder](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/builder.rs), [convention inference](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/conventions.rs), [projection/average kernels](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/curve.rs), [transforms](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/transform.rs), and [wire validation](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/wire.rs). Inspected [tests](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/tests.rs) independently multiply simple accrual factors, check off-grid contractual resets, verify decimal contractual growth across Act/360 and Act/365F, distinguish all three rates on a steep curve, and verify exact triangular shock area. These are formula/invariant evidence; no convergence study for the fixed Simpson grid was located. References: [multi-curve construction](../../../REFERENCES.md#ametrano-bianchetti-2013), [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes).

## Source/reference evidence and caller boundaries

The following are substantive reconciliation obligations:

| Owner / inspected source | Current interpretation | Required reconciliation |
| --- | --- | --- |
| [Forward calibration](../../../../finstack-quant/calibration/src/targets/forward.rs) | Fits scalar rates and unions every quote's reset boundaries into an economic compounding grid | Fit factor state and reprice actual index observations; qualify quote/pillar/solver choices separately. |
| [Cashflow rate helper](../../../../finstack-quant/cashflows/src/builder/rate_helpers.rs), [coupon contract](../../../../finstack-quant/cashflows/src/builder/specs/coupon.rs) and [pipeline](../../../../finstack-quant/cashflows/src/builder/pipeline.rs) | Raw point rate at reset, coupon-accrual rescaling and numeric-tenor compatibility checks | Resolve index identity and actual index period; preserve coupon accrual, stubs, reset timing and fixing eligibility separately. |
| [Swap forward](../../../../finstack-quant/valuations/rates/src/forward_swap_rate.rs) and [IRS pricing](../../../../finstack-quant/valuations/rates/src/irs/pricer.rs) | Factor-implied coupon growth and raw fixing-date projection coexist | Use the same observation projector for the same contractual fixing; coupon boundaries need explicit contractual justification. |
| [Structured-credit period helper](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/period_helpers.rs) | Curve tenor/reset lag plus weekends-only date inference | Supply the resolved index effective/maturity dates and alpha; remove curve metadata and calendar inference. |
| [Overnight valuation](../../../../finstack-quant/valuations/core/src/instruments/pricing/overnight.rs), cashflow overnight helper | Forward-source arithmetic mean versus discount-source growth | Project resolved observations on the source-owned clock; preserve actual fixing/weighting/compounding rules. |
| [Bond asset-swap annuity](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/annuity.rs) | Tiny numeric tenor selects an overnight whole-coupon mean | Use explicit index kind and leg contract; reconcile seasoned coupons' known/projected observation split. |
| [Revolver path generator](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_generator.rs), [cashflow engine](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/cashflow_engine.rs) and [simulation utilities](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/utils.rs) | Raw forward nodes become rate paths; raw rates center simulated/additive changes | Keep factor values out of rate paths. Define the contractual index baseline, perturbation units and distinct OIS short-rate bridge at the model owner. |
| [Term-fixing helper](../../../../finstack-quant/valuations/core/src/instruments/pricing/term_fixings.rs), [TARN](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/pricer.rs) and [Snowball](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/pricer.rs) | Raw fixing-date rates supply deterministic fixings and a basis over Hull–White term forwards | Project the actual index observation; reconcile the model term-forward period/alpha at its owner. An instantaneous projection derivative is not a term fixing. |
| [Statement curve binding](../../../../finstack-quant/scenarios/src/adapters/statements.rs), [scenario bumps](../../../../finstack-quant/scenarios/src/adapters/curves.rs) and [recalibration overlays](../../../../finstack-quant/calibration/src/recalibration/rates.rs) | Numeric tenor and raw rates drive growth conversion and basis-point overlays | Resolve explicit index periods and output convention; record03 must define the shocked rate/quote quantity rather than add bp to factor nodes. |
| [Python](../../../../finstack-quant-py/src/bindings/core/market_data/curves/forward.rs), [WASM](../../../../finstack-quant-wasm/src/api/core/market_forward.rs), [discount conversion](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs) | Defaulted rate pillars, old queries and fixed-tenor resampling | Expose the canonical factor state and checked contractual operations; remove obsolete fields and behavior together. |

The existing [RateIndexConventions](../../../../finstack-quant/valuations/core/src/market/conventions/defs.rs)
already separates currency, term/overnight kind, tenor, day count and calendar
conventions. Reuse that ownership without reversing the dependencies documented
in [ARCHITECTURE.md](../../../../ARCHITECTURE.md). [Bermudan tree valuation](../../../../finstack-quant/valuations/rates/src/swaption/pricing/tree_valuator.rs)
explicitly supports a single-curve model; this decision does not silently turn
that model into a multi-curve stochastic model.

[Ametrano–Bianchetti's author-hosted multiple-curve paper](https://www.bianchetti.org/finance/bootstrappingtheilliquidity-v1.0.pdf)
supports distinct discounting and tenor-specific projection sources; it does not
justify one universal source across tenors. [Strata's primary index-rate implementation](https://raw.githubusercontent.com/OpenGamma/Strata/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/rate/DiscountIborIndexRates.java)
binds a specific index to a factor curve and computes projected rates using the
observation's effective date, maturity date and index year fraction.
Its [resolved observation contract](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/index/IborIndexObservation.html)
separately carries fixing and effective/maturity dates. These are precedents for
the proposed separation; they do not qualify this implementation or establish
universal market suitability for the retained interpolation methods.

## Independent examples and required verification

On 2026-11-08 standalone standard-library Fraction arithmetic checked these
exact relationships. No library module was imported or production suite run.

| Example | Independent result / implication |
| --- | --- |
| Identical flat seed r=1/20, grids [0,1] and [0,1/2,1] | Growth increments are 1/20 and 81/1600=0.050625: a 6.25 bp difference solely from the compounding grid. This rejects grid insertion as an evaluation-only operation. |
| Fixed factors P(0)=1, P(1/2)=100/103, P(1)=20/21 | Direct growth 21/20 equals (103/100)*(105/103) exactly. Adding an evaluation date leaves the fixed source growth unchanged; this algebra is not a rule for combining different tenor indices. |
| Growth increment 1/80 with index alpha=1/4 and source-time span=18/73 | The contractual rate is 1/20=5%; annualizing on the source clock instead gives 73/1440≈5.069444%. Alpha is independent of source time. |
| Same factors, normalize at v=1/2 | P_v(1)=P(1)/P(1/2)=103/105; P_v(v)=1 and the interval growth remains 105/103. This is fixed-source conditioning, not a future-curve forecast. |
| Factors P(S)=1, P(E)=100/99, alpha=1/4 | Growth increment -1/100 gives rate -1/25=-4%, with positive gross factor 99/100. Negative rates need no factor monotonicity floor. |

These checks establish algebraic counterexamples and definitions, not production
correctness, empirical accuracy or market fit. Required qualification includes:

- All accepted01 factor construction, interpolation, clock/calendar, literal-tail,
  derivative-side, state and numerical-range checks on the shared owner; exact
  untransformed stored factors and strict malformed-state rejection.
- Independent factor-ratio rates, negative/zero rates, close/wide endpoint factors,
  extreme alpha, normal/subnormal results, positive-gross rounding boundaries,
  mapped-time collisions and identities that cannot bypass domain admission.
- Evaluation-date partition invariance for a fixed source, with numerical error
  tolerances; no assertion that inserting interpolation pillars or recalibrating
  must leave a curve unchanged.
- Wrong-index/tenor/currency rejection, deliberate shared-source selection,
  fixing versus effective dates, coupon versus index accrual, stubs, in-period
  resets, spot origins and historical-fixing cutoffs at the actual consumers.
- Overnight known/projected mixes, weekends, observation shift/lookback/lockout
  and non-telescoping schedules against independent contractual examples;
  remove both overnight mean paths and asset-swap tenor heuristics.
- Independent calibration instrument repricing and tenor basis benchmarks with
  the new factor parameterization; factor-state shocks versus quote-recalibrated
  risk remain separately selected and qualified by their owners.
- Explicit discount-source assignment preserving off-pillar evaluation; fixed-
  source date conditioning across non-additive clocks; model-owned deterministic
  index baselines, OIS short-rate centering and rate-path units.
- Rust/host error and behavioral parity, complete schema/state/association round
  trips, removed old fields/queries, and matching calibration/valuation contracts.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
