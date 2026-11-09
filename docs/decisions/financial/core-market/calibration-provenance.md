# CORE-MARKET-06: Lossless construction records and qualified calibration replay

- Decision status: **Accepted — complete construction-record/replay package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; complete recipe and request-time qualification reconciliation remain unimplemented.
- Validation status: **Source/reference evidence and independent recipe/lifecycle/consumer reviews inspected; standalone exact/high-precision evaluator/clock counterexamples executed 2026-11-08; production qualification pending.** Structural counterexamples below are source-inspected, not production validator tests. No replay/calibration or Rust/host suite was run.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This decision owns calibration construction-record fidelity, core transport
versus execution validation, historical provenance, exact quote/dependency
binding and request-time replay qualification. It preserves accepted source,
clock, state, shock and probability contracts in
[records01](discount-evaluation.md), [02](forward-projection.md),
[03](rate-shocks.md), [04](hazard-survival.md) and
[05](hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery).
An inherited recipe cannot change their financial meaning or grant an
unqualified source a different risk operation.

Calibration targets own materialization, solver/residual/fit policy and numerical
qualification; products own contractual and model assumptions. Their detailed
[replay record](../calibration-targets/17-calibration-replay.md),
[framework](../calibration-framework/README.md) and
[ADR0032](../calibration-targets/0032-curve-interpolation-and-calibration.md)
retain their remaining Draft choices. This package requires complete capture
and explicit acceptance of whichever supported policies were used; it does not
select those policies, a universal fit tolerance or a historical pricing model.

## Foundational findings

Current rate recipes omit original quote IDs, resolved dates, calendar/convention
state and full numerical configuration. Replay invents IDs and combines stored
quotes with current curve settings/default config. Hazard recipes store more,
but independently sorted original/risk arrays are paired only by count/ID in
core, while upper-layer date/time checks do not prove cross-view CDS identity.

Recipe presence and successful serialization are data-availability facts, not
proof of current fit. Transforms currently retain or delete recipe sidecars
inconsistently across source families. Existing hazard replay checks some source
parameters, but several actions use the target market for the source identity
gate; rate replay overlays a fit discrepancy instead of returning the accepted
recalibrated source. This requires one explicit fidelity and qualification contract.

## Accepted decision: lossless construction records and request-time replay qualification

Accepted on **2026-11-08** after independent recipe, lifecycle and consumer
review. Runtime/caller/host reconciliation and independent production
qualification remain pending; observed implementation evidence stays separate below.

1. **Use immutable construction history and qualified replay as separate facts.**
   Retain one immutable construction record for a calibrated source, containing
   the effective inputs, resolved financial/numerical choices, baseline output
   binding and outcome described below. A manually specified source may have
   no record; do not invent observations or a calibration history. Stored data
   can support audit or an explicitly requested new calibration. It does not
   authorize automatic replay as the current fitted source. Qualification is
   computed by the existing recalibration provider for its complete bound
   request; no persisted current-fit Boolean, automatic trust on attachment,
   or new provider/event-log framework is introduced.

2. **Keep executable semantics in calibration and storage in core.** Use
   calibration's existing canonical quote, target-parameter, convention and
   configuration types for per-family payloads. Core retains narrow immutable
   RateCalibrationRecipe/HazardCalibrationRecipe transport envelopes and
   structurally checked identity/binding data; it does not import calibration
   solvers or valuation conventions. Remove the duplicated incomplete core rate
   quote representation rather than maintain a parallel quote conversion model.
   Opaque payload storage never proves semantic validity: calibration must
   strictly decode its complete supported payload and perform domain/contract
   validation before use. Preserve schema-generated/documented host payload types
   from their semantic owner; an opaque core storage field is not permission
   to publish an unchecked host API. No new common quote or dependency framework.

3. **Capture effective choices as executed.** Preserve original canonical
   quote IDs, forms, units and meaningful optional fields, together with the
   effective adjustments/overrides actually used. Keep original Tenor/Date
   requests and exact resolved contractual fixing/effective/maturity/payment
   bindings and source coordinates needed to reconstruct the priced contracts.
   Record calibration/source base and valuation/reference dates separately,
   complete source clock/calendar, curve role/currency/index association,
   dependencies, recovery where applicable, OIS observation rules, futures
   convexity treatment and resolved convention inputs. Preserve source
   representation, ordered parameter/pillar layout, interpolation/tails and
   all effective solver/Jacobian/bounds/weighting/acceptance settings.
   Do not reconstruct missing fields from the current curve, current registry
   or CalibrationConfig::default. A field may be resolved by a target before
   solving, but its effective value is then explicit in the record. In
   particular, absent legacy futures adjustment is not replayed as0, IDs are
   never minted as curve-REPLAY-index, and an original meaningful None is not
   silently changed to Some(0). Do not invent pre-canonical user intent that
   was never supplied to the calibration owner.

4. **Freeze reference data and bind required dependency state.** Capture the
   resolved convention/calendar data actually used, including business-day
   coverage and rules needed by the materialized contracts. A registry key
   alone is insufficient: the same key can later resolve to different economics
   even if maturity dates happen to agree. Record the financial state of the
   required external dependencies used for construction, with explicit role/ID
   bindings with nonblank required dependency/output identifiers; reject blank
   identities without trimming or renaming them. The replay request supplies
   immutable source and target market
   snapshots and validates those bindings. Use existing canonical financial
   source/state forms, excluding their recipe sidecars/private caches to avoid
   recursive market/recipe graphs. Whole-market embedding, cryptographic lineage
   services and automatic convention migrations are unnecessary. Unavailable
   required external state makes original-source replay unavailable, not an
   invitation to use a live provider/default. Record the implementation and
   supported schema identity as provenance; this does not bundle an old engine
   or promise cross-version/compiler bitwise reproduction.

5. **Persist executed order and atomic quote bindings.** A target may resolve
   a deterministic order before its solve; persist that executed order and its
   unique nonblank original quote IDs, without trimming or renaming them.
   Canonical restore validates rather than sorting,
   deduplicating or independently re-pairing collections. Duplicate maturity
   admissibility remains target/method-specific; it is not a universal recipe
   restriction or permission to drop an observation. For CDS, use one atomic
   binding containing original quote, derived par-risk quote and their shared
   resolved date/time identity. Require equal ID/entity/currency/convention/
   recovery and shared contractual maturity/binding; only the intended quote
   form/value changes in the derived risk view. Preserve the original upfront
   and fixed-running-coupon inputs. Derived par values remain outputs of the
   original bound pricer/market, not observed quotes. Shared times are the same
   canonical values from one resolution, not independently sorted approximate
   matches. Validate full typed fields, finite numbers, units, chronological
   periods and clock consistency at the semantic owner. Remove ID-only pairing,
   approximate-tenor selection and silent 1e-12 binding repair; no new generic
   time tolerance is selected.

6. **Bind the output and report without confusing evidence levels.**
   Record the original output's canonical financial evaluator state and
   calibration outcome, excluding the output's recipe sidecar/caches. Include
   quote-keyed finite residuals with their units/normalization, the effective
   acceptance policy and actual outcome. Data/contract checks, solver convergence,
   fit acceptance, source reconstruction and independent financial qualification
   are distinct facts. An exploratory calibration may return a curve with an
   unsuccessful report, but fail_on_bad_fit=false cannot turn it into qualified
   quote-risk replay. A fitted record remains historical evidence rather than
   current authority. Matching pillar values alone does not establish the same
   interpolation, tails, clocks, index role or retained transform.

7. **Qualify same-source replay on the explicit source market first.** For
   every automatic replay action, including exact quote/index, center and zero
   requests, validate the selected source/view, complete recipe, semantic
   bindings and original dependency state against the supplied immutable source
   snapshot. Reconstruct without a quote shock and require the recorded source
   policy's explicit fit and source-reproduction acceptance. Compare the full financial
   contract: role/IDs/index association, base/reference, clock/calendar, recovery,
   representation, interpolation/tails, retained transform and parameter layout,
   alongside numerical reconstruction and quote residuals. Use financial-owner
   numerical tolerances in their stated units, not approximate identity of IDs,
   dates or categorical choices. There is no universal bitwise, ULP or residual
   bound. Missing/incomplete data, semantic drift, unsupported quote methods,
   failed reproduction and failed acceptance error without aliases, defaults,
   sampled-fit authority or fallback to another risk basis.

8. **Solve the selected action on the explicit target market.** After source
   qualification, apply the exact requested quote/recovery/dependency action
   and solve with its explicitly selected target inputs/policy. Distinguish
   source-market identity validation from target-market calibration for every
   provider action; never substitute target_market for source_market merely
   because the action selects an exact quote index or the risk center. Target
   policy changes are explicit new-calibration choices, not silent replay
   defaults. Require an accepted target report before publishing a replay result.
   Return the actual accepted recalibrated source with a new construction record;
   remove discount ratio/raw-rate delta overlays and invalid-factor-to-ratio1
   substitutions that preserve a source discrepancy while attaching shocked
   quotes. A discrepancy-preserving financial operation would need its own
   separately adopted definition, not the ordinary replay path.

9. **Retain history uniformly without preserving automatic qualification.**
   Discount/projection/hazard direct shocks, source replacements, frozen dated
   conditioning and recovery metadata changes retain the immutable original
   construction record for audit. An actual change to financial parameters,
   clock/reference, role/binding, policy or dependencies invalidates reuse of
   prior replay qualification. Core attachment, cloning and deserialization
   never requalify fit; exact validated no-ops can preserve unchanged state.
   Pure renaming/rebinding retains history but requires explicit correct output/
   dependency bindings for any later replay. Historic inputs remain available
   for an explicitly requested new calibration, including after a synthetic
   change; this is a different request from replaying the changed source as
   though it still represented the old fit. Remove family-specific clearing
   as the sole indicator of validity, without making retained presence a
   current-fit flag. Accepted03/04 conditioning stays frozen-source evaluation.

10. **Keep derived horizon calibration and cache identity explicit.** Horizon
    requoting is a new calibration with model-derived quotes and a new record
    that identifies their originating source/market/horizon and conventions.
    It is not observed market data or accepted frozen-source conditioning.
    Use the existing batch-local replay cache over immutable requests; bind
    selected source/view financial state, complete construction payload,
    resolved source/target dependencies, policy, exact action/quote bindings
    and requested output identity. IDs or quote counts alone are not cache
    identity. Cache only successful qualified results; failed calculations and
    prior qualification from a changed request cannot be reused. No global
    persistent fit-certificate or historical execution service is introduced.

11. **Reconcile strict state, errors and host claims coherently.** Require the
    complete supported envelope at construction/restore; reject unknown/missing
    envelope fields, and reject unknown/missing/retired payload semantics at
    calibration decode. Do not accept an obsolete incomplete recipe as an
    executable fallback or infer supported meaning from JSON shape alone.
    Preserve source state on decode/qualification/fit failure and propagate
    checked stage-specific errors, not NaN, missing-as-zero, partial fit or
    unchanged-source success. Reconcile core/target adapters, providers/caches,
    transforms, scenario quote actions, CS01 availability checks, schema,
    Python/WASM getters/input docs and parity in coherent slices. Getters expose
    provenance/replay-input availability, not a promise of complete current fit
    or independent qualification. Runtime and independent numerical/financial
    qualification remain pending after methodology adoption.

## Rationale and alternatives

Complete effective inputs plus request-time qualification address two different
problems: loss of construction information and misuse of historical information
as current authority. Existing typed calibration inputs, immutable source/state
forms, provider and local cache are sufficient. A trusted serialized fit flag,
new generic recipe framework, mirrored core quote hierarchy or mutable global
registry lookup would add complexity without proving financial equivalence.

A successful JSON round-trip can preserve the wrong or incomplete economics.
Likewise a close fit at a handful of pillars does not establish the complete
evaluator or original contractual inputs. Explicit new calibration remains
available when a caller wants changed assumptions; it must be labeled and
qualified as that operation rather than silently presented as lossless replay.

## Observed choice and rationale

Core stores the recipe needed by downstream calibration/risk code without depending on that code. These files implement recipe typing, ordering and partial validation; no bootstrap or global inverse solver runs here. Storing provenance supports replay, but presence of a recipe is not proof that current curve values still fit it.

## Observed algorithm and controls

`RateCalibrationRecipe` carries currency, curve day count, method (`Bootstrap` or `GlobalSolve { use_analytical_jacobian }`), curve role (discount with projection-curve ID, or projection with discount-curve ID), optional OIS compounding, and quotes. OIS modes are Simple, CompoundedInArrears with lookback, observation-shift compounding, and cutoff compounding. Deposits, FRAs, futures, swaps and basis quotes carry typed date/tenor pillars and identifiers; futures carry price and optional convexity adjustment, swaps optional spread. Rates/spreads are decimal inputs. This is a public data struct; core does not validate quote finiteness, duplicate maturities, consistency of IDs, or solver feasibility here.

`HazardCalibrationRecipe` stores JSON hazard parameters/configuration and two input sets: original calibration quotes and par-spread risk quotes. Construction and deserialization independently sort each set by numeric pillar time, date, quote ID and finally serialized quote text. Both sets must be nonempty, have finite nonnegative strictly increasing times, nonempty IDs, and recognized `cds_par_spread` or `cds_upfront` types; the risk set must contain par-spread quotes only. Their lengths and each sorted position's quote ID must match. Dates/times across the paired sets are not directly checked for equality, and quote numeric fields/configuration remain opaque JSON at this layer.

Discount/forward builders and wire forms preserve attached rate recipes; their shocks, rolls and replacement-knot rebuilds can retain the same recipe. Hazard synthetic shocks, recovery changes and rolls clear their hazard recipe; a pure rename/rebuild preserves it. Recipe invalidation policy therefore differs between rate and hazard curves.

## Observed limitations, alternatives

No recipe field selects or overrides evaluation of an already-built core curve. Downstream consumers must distinguish original calibration provenance from a replay recipe representing the current curve, validate raw JSON, and resolve dependent curves. A cryptographic quote snapshot, typed hazard quote/config storage, or invalidation on all synthetic transforms would be possible design changes; none is adopted by this audit.

## Observed evidence

Owning code: [rate recipe](../../../../finstack-quant/core/src/market_data/term_structures/rate_calibration.rs), [hazard recipe](../../../../finstack-quant/core/src/market_data/term_structures/hazard_calibration.rs), [forward wire](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/wire.rs), [hazard transforms](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs), and [discount metadata rebuild](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs). Inspected `full_rate_calibration_retains_one_day_cutoff` in [forward tests](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/tests.rs) proves round-trip preservation, not calibration correctness. Inspected `hazard_recipe_invalidation_covers_synthetic_transformations` in [hazard tests](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/tests.rs) proves invalidation behavior. Recipe unit tests are located in their owning files. [ADR 0032](../calibration-targets/0032-curve-interpolation-and-calibration.md) and [multi-curve bootstrapping](../../../REFERENCES.md#ametrano-bianchetti-2013) provide wider method context.

## Caller and primary-reference evidence

| Inspected owner | Required reconciliation |
| --- | --- |
| [Core rate recipe](../../../../finstack-quant/core/src/market_data/term_structures/rate_calibration.rs), [hazard recipe](../../../../finstack-quant/core/src/market_data/term_structures/hazard_calibration.rs) | Replace incomplete duplicate rate quote state with calibration-owned complete payloads; strict immutable transport and atomic CDS bindings without restore sorting/ID-only pairing. |
| [Rate recipe adapter](../../../../finstack-quant/calibration/src/targets/rate_recipe.rs), [hazard target bindings](../../../../finstack-quant/calibration/src/targets/hazard.rs) | Preserve original IDs/effective config and resolved reference data; remove regenerated IDs, absent-convexity-to0, current-registry dependence and independent-view identity checks. |
| [Rate replay](../../../../finstack-quant/calibration/src/recalibration/rates.rs), [hazard replay](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) | Original-source zero-shock gates, complete source contract and explicit policies; no current-default config, approximate quote targets, overlays or invalid-factor ratio substitutions. |
| [Provider](../../../../finstack-quant/calibration/src/recalibration/provider.rs), [fit gate](../../../../finstack-quant/calibration/src/recalibration/mod.rs), [cache](../../../../finstack-quant/calibration/src/recalibration/cache.rs) | Every action binds source versus target snapshots; unsuccessful exploratory reports do not qualify replay; bind complete immutable requests and cache only accepted results. |
| [Discount rebuild](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs), [forward transforms](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/transform.rs), [hazard transforms](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs) | Uniform history retention versus recomputed current-source replay qualification; retain accepted source/view semantics. |
| [CS01](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01.rs), [scenario adapters](../../../../finstack-quant/scenarios/src/adapters/curves.rs) | Recipe presence is input availability only; exact bindings and checked provider qualification remain required, including zero actions. |
| [Python discount](../../../../finstack-quant-py/src/bindings/core/market_data/curves/discount.rs), [forward](../../../../finstack-quant-py/src/bindings/core/market_data/curves/forward.rs), [hazard](../../../../finstack-quant-py/src/bindings/core/market_data/curves/hazard.rs), [WASM discount](../../../../finstack-quant-wasm/src/api/core/market_discount.rs), [forward](../../../../finstack-quant-wasm/src/api/core/market_forward.rs), [hazard](../../../../finstack-quant-wasm/src/api/core/market_hazard.rs) | Complete owner-generated payload schema/docs/parity; getters cannot certify replay completeness/current fit from sidecar presence. |

[Strata's rates calibrator](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/curve/RatesCurveCalibrator.html)
separately accepts curve definitions, market data, reference data, known dependent
curves and configured measures/tolerances. Its
[curve-group definition](https://strata.opengamma.io/apidocs/com/opengamma/strata/market/curve/RatesCurveGroupDefinition.html)
explicitly binds currencies/indices to curve roles. These support keeping
calibration inputs and dependencies explicit; they do not establish this
repository's persistence, source-qualification, strict-order or replay guarantees.

## Counterexamples and required verification

On 2026-11-08 standalone Fraction and Decimal at precision200 checked the three
numeric evaluator/clock examples below. Structural examples are source-derived
counterexamples, not executions of production validators or a replacement
recipe implementation. No library module or replay/calibration suite was run.

| Evidence | Example / implication |
| --- | --- |
| Numeric expression executed | Same factors (0,1),(1,.9) give Linear midpoint.95, versus LogLinear sqrt(.9)≈.94868329805. Matching pillar factors alone cannot establish the same complete evaluator. |
| Numeric expression executed | Same source endpoints with Flat versus Linear Extend tails give factors.9 versus.8 at t=2. Identical stored knots do not identify the tail policy. |
| Numeric expression executed | A91-day interval is91/360 versus91/365 under two source clocks. Matching dates/IDs or accepting close stored times does not establish clock identity. |
| Structural source inspection | Original upfront quote Q at date/time1 and derived par-risk quote Q at date/time5 can pass current count/ID pairing and separate per-view binding checks. One shared atomic CDS binding must reject the mismatch. |
| Structural source inspection | None versus Some(0) and original QuoteId versus curve-REPLAY-index serialize different information even when an ordinary price happens to agree. An omitted required effective adjustment is unavailable, not0. |
| Structural source inspection | Identical dependency ID D with different financial states in source/target markets is a changed dependency. Exact quote/index actions must not validate original identity on the target market. |
| Structural source inspection | A failed report with fail_on_bad_fit=false can be returned for inspection; it is not qualified quote-risk replay. A stored fit flag cannot repair that distinction. |
| Structural source inspection | A directly shocked/conditioned source retaining its old record still has historical inputs. Replay as the current fitted source requires a fresh source gate; explicit new calibration may reuse the history. |

Independent qualification must cover:

- Lossless IDs, quote forms/units/options, executed order, exact original/derived
  pair identity, duplicate/missing bindings, resolved calendars/conventions,
  source/role/clock/recovery and complete effective construction/fit policy.
- Strict core transport versus semantic decode, malformed/obsolete/unknown
  payloads, no default filling/sorting/repair, no recursive sidecar snapshots,
  and Rust/Python/WASM schema/behavior/error parity.
- Source/target dependency separation on every action, original-market zero-shock
  reproduction for rates and hazard, full selected evaluator binding, accepted
  target fit versus exploratory reports and independent quote repricing.
- Historical versus current replay meaning after direct changes, reference rolls,
  metadata/recovery changes, source replacement and rename; no-op preservation,
  horizon-derived quote provenance and missing original-state refusal.
- Actual accepted replay output versus removed overlays, cache isolation across
  request changes, uncached failures, atomic source preservation and actual
  metric/scenario/host availability/error behavior.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
