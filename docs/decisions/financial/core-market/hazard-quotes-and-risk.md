# CORE-MARKET-05: CDS quote meaning, explicit hazard risk, and recovery changes

- Decision status: **Accepted — complete quote/risk/recovery package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; quote removal and explicit risk/caller reconciliation remain unimplemented.
- Validation status: **Source/reference evidence and independent quote/risk/recovery reviews inspected; standalone rational/high-precision expression checks executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Expression checks do not qualify CDS pricing, market fit or risk accuracy.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This decision owns removal of ambiguous core CDS quote views, explicit direct
intensity versus quote-recalibrated risk, hazard parameter targeting, and
frozen-intensity recovery replacement. It preserves accepted
[CORE-MARKET-04](hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation):
nonnegative ending-interval intensities, explicit clock, constant last-intensity
tail, probability domain and checked state. It reuses the exact bucket geometry
and unit discipline in accepted [CORE-MARKET-03](rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning),
without changing hazard parameter shocks into continuous-zero factor shocks.

[Record06](calibration-provenance.md) retains recipe representation, historical
provenance and replay-state design. [Record22](market-change-measurement.md)
retains comparison aggregation/date-alignment choices. Full CDS contracts,
recovery consistency, par/upfront conversion, solver/fit policy and independent
market qualification remain with [ADR0030](../models-and-valuations-credit/0030-cds-survival-and-integration.md),
[hazard calibration](../calibration-targets/05-hazard-calibration.md),
[calibration replay](../calibration-targets/17-calibration-replay.md) and
[credit sensitivity owners](../valuations-framework/README.md).
This package does not adopt their remaining Draft calculations. Accepted
[bond CS01 without a hazard curve](../valuations-bonds/0011-bond-cs01-without-hazard-curve.md)
retains its z-spread contract.

## Foundational findings

A nonnegative intensity source determines survival probabilities. It does not
by itself determine a par CDS quote: the latter also depends on a contract,
discounting, recovery and valuation conventions. Current cds_quote_bp silently
switches from stored quote interpolation to local lambda*(1-R) when insufficient
quotes remain. Core market differences then label both outputs as par spreads.

Generic hazard bumps silently use the same credit-triangle approximation, while
explicit scenario quote replay and direct intensity APIs already exist. Some
direct-risk metrics publish the same cs01 keys as quote recalibration, including
a near-zero-LGD fallback that changes units. A recovery metadata change also
preserves stale quote arrays and overclaims unchanged pricing. The revised
contract removes these implicit substitutions.

## Accepted decision: explicit CDS observations, hazard risk and recovery

Accepted on **2026-11-08** after independent quote, risk and recovery review.
Runtime/caller/host reconciliation and independent production qualification
remain pending; observed implementation evidence stays separate below.

1. **Keep intensity sources and CDS quotes at their actual owners.** Remove
   HazardCurve::cds_quote_bp, ParInterp, standalone par-tenor/par-spread arrays,
   par_interp fields, builders, accessors, re-exports and wire/host parameters.
   Do not replace them with another generic interpolated quote mode or a
   local-hazard fallback. Preserve actual observations and replay inputs through
   existing lossless calibration quote/recipe records with quote identity,
   entity, currency/convention, resolved pillar date, quote form and recovery
   assumption. A single observation is valid data; missing observations are
   unavailable rather than fabricated. Record06 still owns recipe validation
   and representation. Core does not acquire a dependency on calibration or CDS
   pricing merely to expose a quote.

2. **Compute model-implied quotes through the existing CDS pricer.** A request
   for a model-implied par spread supplies the actual CDS contract, valuation
   date, discount and hazard sources, recovery and applicable conventions to
   CreditDefaultSwap::par_spread/the canonical pricer. That public entry point
   returns basis points. Typed par spreads and running coupons also use bp
   (100 means1% annually); the existing upfront_pct field is a signed decimal
   fraction of notional (.01 means1%), with payment orientation owned by the
   CDS contract. Convert units explicitly once. Upfront amounts, fixed running
   coupons, par spreads and native intensity units remain distinct.
   Require checked dependency/contract validation and propagate failures; no
   interpolation-style fallback, extrapolated quote guess, triangle approximation
   or core reimplementation. A model-implied par spread can be zero even when
   the calibration market-input schema requires a positive quote. Preserve
   accepted04's zero-intensity/R=1 source domain without silently extending
   calibration quote admission or promising CDS identification at zero LGD.

3. **Use two explicit risk operations.** Direct hazard risk shocks canonical
   intensity parameters; CDS par-spread risk shocks declared par-risk quote
   inputs and recalibrates through the existing provider. The selected quantity
   must survive scenario dispatch, metric keys, reports and attribution. Remove
   the implicit generic hazard Bumpable/CurveStorage ParSpread conversion and
   FirstOrderShift scenario mode, including delta_lambda=delta_spread/(1-R).
   An explicit direct-intensity entry point remains available for sources with
   no replay recipe. An ambiguous generic spread request must require the caller
   to select its intended operation rather than reinterpret its bp units.
   Keep the triangle, if used by calibration, solely as that owner's separately
   selected initialization heuristic; it is not an adopted risk approximation.
   No missing-recipe, near-zero-LGD or failed-solve fallback to another risk basis.

4. **Define direct hazard shocks as parameter operations.** Support additive
   RateBp (value/10000), percentage points (value/100) and annual decimal Fraction;
   reject multiplicative/factor shocks. Require finite request inputs, checked
   nonzero normalization and exact shared bucket geometry before zero shortcuts.
   A parallel bump adds b to every lambda_i. A triangular parameter bump adds
   b*w(t_i) to each existing ending parameter t_i, using original source-clock
   coordinates and accepted03's exact-width weights. This produces a piecewise-
   constant parameter perturbation, not a continuous triangular hazard function.
   Do not insert new intensity breakpoints or apply a different weight at each
   query time. The last parameter controls its ending interval and the entire
   constant tail; its sampled weight therefore persists beyond the last knot.
   This operation is distinct from both a continuous-zero factor shock and a
   fitted CDS quote bump, even when their ordinary outputs look similar.

5. **Target parameters and quotes exactly.** Direct individual targets name
   exact existing positive ending parameters, scoped to the selected source.
   Reject off-grid/missing/nonfinite/duplicate targets, including requests beyond
   the final end; remove 1e-6 matching, containing-segment selection, ignored
   tail targets and the singleton special case. A singleton's sole parameter
   naturally controls its complete constant-intensity source. Quote-risk targets
   identify exact stored quote IDs and resolved bindings, using the existing
   exact quote-index route only with a verified matching recipe/order. Reject
   missing/ambiguous/duplicate quote targets; remove the current abs(tenor-target)
   <0.1 matching and any nearest-tenor/no-op fallback. A pillar date is a target
   only when it resolves uniquely. Quote-calendar tenors and source numeric
   parameter ends are not interchangeable identities.

6. **Validate whole replacements atomically and preserve valid identities.**
   Compute all candidate parameter changes from the unchanged source; require
   every resulting intensity finite and nonnegative under accepted04, without
   cap, negative floor or clipping. Validate the complete request/target list
   before publishing changes. Reject duplicate targets rather than depend on
   sequential intermediate states. Empty or exact-zero valid direct-parameter requests preserve
   source state and metadata; zero requests still reject malformed units or
   missing targets. A nonzero unit normalization lost to0 is a range error;
   ordinary parameter addition can round unchanged and cannot certify an
   effective finite-difference step. Required arithmetic failures propagate.
   An actual direct change disables automatic reuse as evidence of current
   fit or replay equivalence for the old quote set; historical construction records may remain only as historical
   provenance under record06. Do not turn clearing quote state into a switch to
   an approximate quote evaluator. Source and context remain unchanged on failure.

7. **Recalibrate quote risk through explicit dependencies.** Use the existing
   recalibration provider and lossless quote/replay inputs, with explicitly
   resolved source/target markets, discount dependency, recovery, contractual
   conventions and selected bumped par-risk observations. Do not infer discount
   dependencies from currency prefixes or a sole available curve. Distinguish
   original observations from par-space risk inputs derived from upfront quotes;
   their derivation remains the CDS/calibration owner's full instrument
   calculation. Require zero-shock reproduction and the selected fit acceptance;
   missing recipes/dependencies, invalid bumped quotes and failed fits are
   errors. Never silently clip a bumped quote or substitute a hazard shift.
   This boundary selects the operation, not its solver, acceptance tolerances
   or market-data sufficiency policy.

8. **Give risk outputs truthful quantities and numerical policies.** Quote-space
   CDS risk retains cs01::<curve> / bucketed_cs01::<curve> labels and per-bp par
   spread units. Direct intensity sensitivity uses distinct hazard01::<curve> /
   bucketed_hazard01::<curve> labels and currency per intensity bp; remove the
   spread-equivalent LGD scaling and reuse of cs01 keys in direct-hazard
   calculators. Preserve accepted no-hazard bond z-spread CS01 at its owner.
   The sensitivity owner must explicitly select/qualify its finite-difference
   scheme and effective bump. A central down-bump leaving the intensity domain
   is a domain boundary, not permission to catch any error and silently switch
   to a forward difference. Computation/market failures propagate. No arbitrary
   small-bump zero result, guaranteed portfolio sign or exact sum of separately
   repriced finite bucket changes is approved. Coherent bucket weights support
   first-order parameter additivity; calibration and valuation remain nonlinear.

9. **Separate frozen-intensity recovery replacement from recalibration.** A
   frozen request validates finite R in[0,1], changes only recovery metadata,
   and preserves lambda, source clock and J/S/PD. Exact unchanged R preserves
   state after validation. An actual recovery change disables automatic current-fit/replay
   authority; it does not rewrite historical observed quote recovery assumptions
   or certify agreement with a contract's historical calibration assumption.
   Retained historical typed inputs can still support an explicitly requested
   new calibration with caller-selected recovery and dependencies. Remove the claim that no priced quantity changes: valuation, LGD, consistency
   guards and implied quotes can change. A quote-preserving recovery request
   instead invokes the existing recovery-recalibration operation with explicit
   quote inputs/dependencies and its own admitted recovery/fit domain. Missing
   inputs, zero-LGD identification failures and failed calibration are errors,
   not frozen-intensity or triangle fallbacks. Contractual recovery replacement
   and Recovery01 remain explicit product operations.

10. **Keep market comparisons on the same declared basis.** Replace core
    measure_par_spread_shift/per-tenor quote views that infer par spread from
    HazardCurve. A quote comparison supplies matching typed observations or
    explicitly repriced contract quotes on a common financial basis, with units,
    identity/date/convention/recovery and source provenance available. Core may
    compare supplied values or native intensity parameters; full CDS pricing
    stays above core. Do not pair an intensity/triangle move with quote-space
    CS01 or compare historical quotes with stressed implied quotes merely
    because their curve IDs match. Reconcile credit cascade/attribution and
    generic credit-curve dispatch accordingly. Record22 still owns aggregation,
    weighting, date alignment and availability policy; this decision removes the
    silent basis substitution without adopting that record's remaining choices.

11. **Reconcile one Rust-owned state and host boundary.** Apply this package
    across core quote/state/re-export/schema removal, direct hazard/context APIs,
    scenario quote delivery, calibration replay, valuation sensitivity registries,
    attribution, Python/WASM constructors/accessors, generated declarations,
    documentation and parity in coherent slices. Reject retired quote arrays,
    ParInterp, FirstOrderShift and ambiguous old fields/labels without aliases or
    migration. Existing lossless provenance stays at its owner; no duplicate
    current-quote cache is added. Checked errors must reach callers instead of
    successful unchecked numeric wrappers, missing-as-zero or an approximation
    warning followed by a different financial calculation. Runtime reconciliation
    and independent financial qualification remain pending after adoption.

## Rationale and alternatives

An intensity source has one mathematical role. Removing its incomplete quote
view is simpler than specifying separate fallbacks for zero/one/many stored
quotes, interpolation positivity, extrapolation, recovery and stale fit state.
Existing quote records, CDS pricing and recalibration already own the required
information and computations; no new quote/provider framework is needed.

Local lambda*(1-R) is exact only in restricted stylized cases, not a general CDS
quote. Keeping it as an implicit quote or risk fallback would preserve the very
basis ambiguity this package resolves. Parameter-weighted hazard buckets remain
simple and preserve accepted04's piecewise-constant model; an exact continuous
triangular hazard shock would be another intensity model and is not added here.

## Observed choice and rationale

Core exposes an interpolated stored CDS quote or a local credit-triangle approximation; its generic hazard bump shifts hazard by spread divided by loss-given-default. It does **not** replay CDS calibration in this path. That distinction is material to CS01 and market-change measurements.

## Observed algorithm, selection, and conventions

`cds_quote_bp(t,method)` uses stored par quotes only when at least two aligned tenor/quote entries exist. Linear is the default metadata choice. Explicit LogLinear attempts a log-linear quote strategy; if construction fails it silently uses Linear. Both request FlatForward, which extends the linear spread slope or log-spread slope respectively, not a flat endpoint spread. With insufficient quotes the output is `max(lambda(max(t,0))*(1-R)*10000,0)`. It uses the **local segment hazard**, not a fitted CDS par spread or average hazard.

Generic `Bumpable::apply_bump` and context curve bumps select `ParSpread`: additive decimal spread shift `s` becomes `delta_lambda=s/(1-R)`. Recovery must satisfy the stricter loss-given-default validation before division. Explicit direct-hazard APIs instead add basis points/10,000 to hazard. Parallel or triangular key-rate weights are sampled at the stored hazard ending knots, so bucket shapes are piecewise constant in hazard after rebuilding, unlike continuous discount/forward shocks.

Every resulting hazard must be finite, nonnegative, and within its configured cap. The mutation builds new survival before committing, so failure leaves source values intact. A successful shock clears stored par quotes and calibration recipe, even a zero-size generic shock. `with_recovery_rate` changes recovery and clears the recipe while preserving hazards and stored par quotes.

Direct tenor hazard bumps select a knot within 1e-6 years of requested tenor, otherwise the next ending knot (containing segment). Requests beyond last knot plus 1e-6 are ignored. Repeated targets accumulate sequentially and each intermediate negative result is rejected. For a single-point hazard curve all requested finite bumps are summed into one parallel shock regardless of target tenor.

## Observed domain, failures, and alternatives

Par-tenor ingestion sorts but only checks finite nonnegative tenors and finite spreads. It does not reject duplicate quote tenors or negative quote spreads; log-linear failure can therefore change the requested method silently. Stored quotes are not verified against hazards/recovery. A recovery override can consequently retain par quotes inconsistent with the changed recovery. These behaviors require explicit policy decisions.

Quote bump and full recalibration, supported by downstream recipe consumers, is a different alternative with more faithful instrument repricing but additional dependencies and solver costs. Direct hazard bump is explicitly supported and measures another risk. The credit-triangle approximation requires a stated intended range; code/tests alone do not establish its accuracy for term-structured or distressed names.

## Observed evidence

Owning code: [quote evaluation](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/curve.rs), [quote ingestion](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/builder.rs), [shock/recovery/tenor dispatch](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs), [context dispatch](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs). Inspected [tests](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/tests.rs) verify lambda shift equals `spread/(1-R)`, fallback quote changes from 60 to 70 bp, stale quote removal, direct-hazard additivity, segment targeting, atomic failed bumps and partition of knot shocks. Those validate the approximation's implementation, not CDS-repricing accuracy. [O'Kane](../../../REFERENCES.md#o-kane-2008) and [ISDA CDS model](../../../REFERENCES.md#isda-cds-standard-model) are comparison references.

## Caller and primary-reference evidence

| Inspected owner | Required reconciliation |
| --- | --- |
| [Hazard quote evaluator](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/curve.rs), [builder](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/builder.rs), [state](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/wire.rs) | Remove cds_quote_bp, par arrays/interpolation and associated defaults; retain canonical intensity/recovery and separately owned lossless provenance. |
| [Hazard transforms](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs), [curve dispatch](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs) | Remove implicit spread/LGD risk, approximate target selection and misleading recovery/price invariance; publish explicit parameter shocks and atomic identity behavior. |
| [Scenario adapter](../../../../finstack-quant/scenarios/src/adapters/curves.rs), [scenario spec](../../../../finstack-quant/scenarios/src/spec.rs), [hazard replay](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) | Remove FirstOrderShift and inferred dependencies; use exact quote bindings and explicit full replay/recovery operations. |
| [Quote CS01](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01.rs), [direct hazard CS01](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01_direct_hazard.rs), [z-spread dispatcher](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01_z_spread.rs) | Distinct risk keys/units; no recipe-missing substitution, near-zero-LGD unit fallback, catch-all down-bump fallback, synthetic zero-anchor hazard bucket or exact finite bucket-sum claim. |
| [Core market differences](../../../../finstack-quant/core/src/market_data/diff.rs), [credit attribution cascade](../../../../finstack-quant/attribution/src/credit_cascade.rs) | Stop labeling mixed quote/triangle/intensity results as par-spread differences; supply a matching risk/market basis. |
| [CDS par spread](../../../../finstack-quant/valuations/credit/src/cds/types.rs), [recovery consistency](../../../../finstack-quant/valuations/credit/src/cds/pricing/helpers.rs), [Recovery01](../../../../finstack-quant/valuations/credit/src/cds/metrics/recovery01.rs) | Keep full instrument quotation and contractual recovery changes distinct from source metadata replacement; preserve each selected valuation/recalibration operation. |
| [CDS quote schema](../../../../finstack-quant/calibration/src/quotes/cds.rs), [recipe](../../../../finstack-quant/core/src/market_data/term_structures/hazard_calibration.rs), [Python](../../../../finstack-quant-py/src/bindings/core/market_data/curves/hazard.rs), [WASM](../../../../finstack-quant-wasm/src/api/core/market_hazard.rs) | Preserve quote-form/domain/provenance distinctions; remove obsolete core quote fields/APIs and reconcile strict error/state/host parity. |

The [ISDA CDS Standard Model](https://www.cdsmodel.com/) describes standardized
CDS upfront/spread conversion as a full calculation with agreed inputs.
QuantLib's [CDS calibration helpers](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/credit/defaultprobabilityhelpers.cpp)
carry recovery, discounting and contract conventions into pricing; its
[CDS engine](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/pricingengines/credit/midpointcdsengine.cpp)
uses recovery in the default claim. These are primary evidence for the separation
of intensity, recovery and instrument quotation, not approval of this library's
calibration, approximation domain, sensitivity scheme or exact target policies.

## Independent examples and required verification

On 2026-11-08 standalone Fraction arithmetic and Decimal at precision200 checked
the following bounded relationships. The CDS example is explicitly a stylized
continuous-premium, zero-discount calculation, not an ISDA benchmark. No library
module was imported and no production suite was run.

| Example | Independent result / implication |
| --- | --- |
| Additive intensity1bp, .01 percentage points, or decimal.0001 | All normalize to delta_lambda=1/10000, without LGD conversion. |
| Two-year model: lambda=.01 for year1, .03 for year2, R=.4, zero discount, continuous premium | With A1=(1-exp(-.01))/.01 and A2=exp(-.01)*(1-exp(-.03))/.03, par spread is .6*(.01*A1+.03*A2)/(A1+A2), approximately119.401019891bp, strictly between60 and180bp. The local final-segment triangle returns180bp. |
| Ends1,3; triangular parameter weight p=0,k=1,n=2; b=1bp | First-parameter shift is1bp throughout (0,1], so delta_J(0,1)=1e-4. A continuous triangular intensity shift would give5e-5; they are different models. |
| Last end3 with weight p=2,k=3,n=4; b=1bp | Last parameter and its constant tail both shift1bp. Delta_J(4,5)=1e-4 even though continuous w(t) is zero there. |
| Quote bindings Q1 at1year and Q2 at1.05years; target Q1 | Exact identity selects only Q1. The observed abs(tenor-target)<.1 test selects both. |
| Flat lambda=.02, R changes.4 to.5 at fixed intensity | S/PD are unchanged; stylized triangle/par spreads change120 to100bp. A fixed-intensity protection claim using the new R scales by.5/.6=5/6, disproving general price invariance. |
| Flat continuous-premium model R=.4 | A1bp par-spread move requires delta_lambda=1/6000; a direct1bp intensity move is1/10000. Their units/sensitivities are distinct even in this favorable stylized case. |
| Zero intensity, R=1, direct additive1bp hazard request | New intensity1e-4 remains admissible; a spread/LGD conversion would be undefined. Direct intensity risk does not need that conversion. |

Independent qualification must cover:

- Removal of data-count/style/triangle fallbacks and retired quote state; typed
  observation versus model-implied quote identity, units, availability and zero
  source probabilities without narrowing accepted04 through market-input rules.
- Exact positive ending/quote targeting, close bindings, missing/duplicate/zero
  requests, singleton/tail semantics, exact bucket widths and parameter-weighted
  versus continuous shapes; normalization and final-intensity range failures.
- Atomic parameter/recovery changes, exact no-op preservation and historical
  provenance versus disabled current-fit authority; strict malformed-state/host
  rejection and Rust/Python/WASM parity.
- Independent explicit quote replay with zero-shock reproduction, dependency/
  convention/recovery assertions and exact quote selection; failure propagation
  without floors, triangle conversion or fallback to direct risk.
- Distinct hazard01 versus CS01 labels and attribution bases, effective-step and
  finite-difference convergence/error behavior at zero intensity, finite bucket
  residuals, and accepted bond z-spread CS01 behavior.
- Frozen-intensity versus quote-preserving recovery changes, zero-LGD boundaries,
  contractual Recovery01, stale quote/fit refusal and independent CDS vendor
  repricing under the actual selected financial conventions.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
