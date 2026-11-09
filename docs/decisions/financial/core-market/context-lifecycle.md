# CORE-MARKET-20: Typed market-context operations and atomic source lifecycle

- Decision status: **Draft — complete context identity/lifecycle/state package proposed 2026-11-08; adoption pending.**
- Implementation status: **Observed** in the owning paths below; the proposed targeting, mutation, scratch, dated-conditioning and state reconciliation is unimplemented.
- Validation status: **Primary/source evidence and independent selection, atomicity/scratch, financial-conditioning and snapshot/host reviews inspected; standalone state-machine/rational/high-precision examples executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Examples do not certify financial replay, horizon pricing, risk or production mutation safety.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns `MarketContext` identity, source dispatch, owned mutation,
registered composite consistency, scratch scope, dated source conditioning and
snapshot/restore semantics. It preserves [accepted core-market 01–19](README.md),
including each family's units/domains/clocks, exact targets, immutable sources
and financial association. Context plumbing must execute those contracts rather
than create alternate financial defaults.

Also preserve accepted [explicit FX queries/state/shocks](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks),
[Money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
[strict numeric ingress/content identity](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity)
and [bounded contract loading](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics).
These decisions are not reopened by context storage or serialization.

[Hierarchy expansion](hierarchy-selection.md),
[market-change measurement](market-change-measurement.md),
[scenario time roll](../scenarios/13-market-time-roll.md),
[fixing materialization](../cashflows/16-fixing-materialization.md),
[theta/carry](../valuations-framework/carry-theta-breakeven.md)
and [attribution restoration](../attribution/07-taylor-volatility-and-restoration.md)
retain their remaining Draft financial policies. Context integrity is not a book-
wide instrument dependency audit, a forecast of tomorrow's market or proof of fit.

## Foundational findings

Typed getters already enforce concrete kinds. Generic bumps instead probe curves,
surfaces, scalars and series in priority order; volatility lookup separately probes
cube/surface/FX-delta priority. Legitimate namespace-local IDs can therefore select
different data without an explicit financial choice. Bulk replacement also admits
external-key/embedded-ID mismatch and last-wins duplicate entries.

Current source insertion/removal can commit changes, then drop incompatible
credit indices with warnings. Several callers discard invalidated IDs. Public
cloneable scratch tokens blindly restore saved data, including whole composite
maps; stale/out-of-order/cross-context reversion can overwrite unrelated new state.
Existing staged bump batches and metric-error scratch discard are useful behavior
to preserve, not evidence that every public lifecycle path is already safe.

The context has no authoritative global valuation date. Adding the same day count
to different source bases does not produce one target reference. Cloning volatility
or observations does not age/qualify them, and cloning FX retains a possibly live,
date-sensitive provider rather than freezing its quotes. A valid serialized context
can restore its captured sources without reproducing arbitrary future provider
queries or proving a financially admissible horizon valuation.

## Proposed decision: typed targets and atomic context lifecycle

This complete recommendation remains **Draft** until explicit adoption.
Runtime/caller/host reconciliation and independent financial qualification
would remain separate work after acceptance.

1. **Keep namespace-local IDs and make active selection typed.** Retain the
   existing typed stores/getters; do not impose global ID uniqueness on unrelated
   observations, curves or volatility forms. Within the curve store one ID has
   one concrete variant. Financial lookup/mutation specifies the exact namespace,
   expected family/subkind and ID. Price versus VolIndex remains distinct even
   though both currently share one storage variant. Quote/convention/underlying
   compatibility remains with the accepted source/financial owner.

   Remove bare-ID multi-namespace `MarketBump::Curve` probing and cube-first
   volatility fallback. Reuse small typed operation variants/kind information
   at the canonical core boundary and adapt existing scenario/valuation targets;
   do not make core depend on scenario code or add a registry framework. Missing,
   wrong-kind and unsupported-operation requests error rather than inspect an
   alternate store. Explicit scalar/model overrides and inactive branches do not
   resolve unused same-ID sources. Bare `contains`/entry counts may remain
   diagnostic introspection; they do not establish a financial source's presence.

2. **Preserve canonical keys and explicit replacement scope.** Require nonblank
   exact keys and, where an artifact carries an ID, agreement between registered
   key and embedded ID. Externally keyed scalars/composites retain their actual
   key role without inventing duplicate internal IDs. Reject duplicate preserved
   keys in a bulk replacement before map collection, rather than first/last wins.
   Do not recover identity by parsing IDs or rounding financial coordinates.

   An ordinary source update targets its declared existing family/subkind; it
   cannot silently change a Discount into Hazard or Price into VolIndex. An
   intentional kind transition uses an explicit owned removal/insertion with
   final dependencies valid. Intentional sequential same-kind replacements are
   different from ambiguous duplicate records in one input. Calibration prior/
   data overlays must declare their actual replacement order or reject collisions.
   A partial restore declares its selected namespaces; clearing prices must not
   implicitly clear observations, CPI fixings or dividend events. Broad clearing
   operations must expose their full scope explicitly.

3. **Publish owned changes atomically and reject dangling composites.** Stage
   candidate sources, resolve registered composite references against that
   candidate and reuse canonical composition/source validation before committing.
   Apply this to single insertion/replacement, mutable helpers, bulk replacement,
   retention/removal/clear and snapshot/factor restoration. A failure returns an
   error and preserves the previously published context-owned state.

   An ordinary dependency-breaking update fails; do not warn and silently remove
   the credit index. If removal of both composite and dependency is intended,
   remove them explicitly in the same owned operation and validate the final
   candidate. Keep accepted19's mode/member/recovery/weight and actual reference
   associations through rebuild. This validates registered data-internal
   dependencies, not every external instrument or financial model. No automatic
   full repricing, fit solve or public transaction/dependency-engine framework
   is needed. External provider activity is governed by its own owner; context
   rollback does not promise to undo external calls or freeze provider state.

4. **Retain supplied-order batch semantics and family-owned financial actions.**
   Apply each action to a private candidate in the supplied order, validating its
   actual request/units/targets/domains before identity or empty shortcuts. Repeated
   targets compose; do not reorder, deduplicate actions or combine them into a net
   action that hides an inadmissible intermediate result. Any failed action aborts
   publication of the entire owned batch.

   Delegate actual quantities to accepted source owners: zero-rate parameter
   shocks, local-intensity risk, level/CPI parameters, native sigma/correlation
   and generic scalar/series rules are different operations. Remove old hazard
   spread-proxy dispatch and obsolete BasisSpread storage rather than preserve
   them as context fallbacks. Stored SABR/FX-delta/dividend data do not acquire
   generic bump support merely because they are in a context. Preserve source
   IDs/metadata and exact selections through replacement.

   Final-only composite rebinding is valid for private source-only staging. If
   an intervening action reads/prices a composite, it must see that stage's
   canonical references, not old embedded handles. Explicit context FX actions
   delegate pair/date/policy/unit semantics to accepted FX ownership, without a
   second percent/factor conversion or derived-source substitution.

5. **Scope every scratch operation and restore exact owned state.** Replace the
   public constructible/cloneable token enum and unchecked reversion with one
   opaque context-bound scoped/LIFO contract. Capture exact original sources and
   affected composite snapshots; point surface scratch restores its original
   artifact, not a nearest coordinate or one guessed scalar value. Reuse the
   same admitted family action as ordinary mutations.

   Reject stale, replayed, wrong-context and out-of-order reversion before writing.
   Never blindly restore an old whole composite map and delete unrelated new
   entries. Cheap whole-map Arc restoration is valid when exclusive scope/LIFO
   ownership proves no unrelated mutation occurred; otherwise restore only
   guarded owned state or refuse. Preserve exact metadata/IDs and source values,
   not an inverse bump that incurs a second rounding/model operation.

   Successful reusable scratch calculations restore before returning; on error,
   restore or discard the owned scratch without contaminating the base market.
   Existing `MetricContext::with_market_scratch` already discards scratch on Err;
   do not misreport later-metric error contamination. Reconcile successful/manual
   reversion and general public helpers as well as that protected error path.

6. **Condition to an explicit reference date through accepted source owners.**
   Replace signed per-source day-offset rolling with one canonical
   `condition(reference_date)` operation. No global context as_of is invented.
   The requested date is the common target reference for families whose accepted
   operation constructs a dated view; a source's actual clock/domain still owns
   admission, including supported backward targets. No universal negative-date
   ban, pre-origin clamp or offset addition replaces that family contract.

   Discount/projection/parametric factors retain complete original evaluators and
   normalization; hazards use retained-intensity local conditioning even if
   absolute survival underflows. Price/VolIndex/CPI retain absolute original
   levels and clocks. Do not realize a new spot/base CPI, prune expired reporting
   nodes, refit a parametric curve or sample it into another source. Preserve
   construction history without declaring a new current calibration.

   Build the dated result privately and rebind composites canonically before
   publishing it; unsupported requested family conditioning fails atomically.
   Unchanged data families retain their actual sources/metadata. The result is
   a context of selected source views, not a universally consistent future market.

7. **Keep horizon data/model eligibility explicit at active consumers.** Preserve
   observations and knowledge cutoff, event ex/payment dates, volatility clocks/
   quote profiles, correlation maturity/fit basis and any actual history. Moving
   valuation date alone does not publish a fixing, settle a dividend, age a smile,
   create a new conditional forward-variance curve or restart a rough driver.
   Active frozen-vol/history use must satisfy accepted source-clock/origin/model
   qualification or error; unused stored data does not force unrelated checks.

   Scenario tenor/date resolution, hypothetical fixing projection, held-spot
   product observations, theta/carry definitions and quote replay remain their
   financial owners' operations. Projected data cannot become actual publication
   knowledge through context assignment. Recipe presence alone must not trigger
   automatic hazard replay/current-fit claims; actual replay qualifies the
   explicit request under accepted06. Reconcile real scenario/theta/attribution
   producers without treating one context operation as a universal carry model.

8. **Delegate Money/FX queries without hidden policy.** Context conversion uses
   accepted Decimal Money arithmetic and a complete explicit ordered FxQuery
   with actual date/policy; validate currency agreement with amount/target.
   Remove the default-policy convenience path rather than add a second resolver.
   A valid same-currency identity needs no unused FX provider; required request
   controls and Money state still follow their accepted validation contracts.

   Shallow clone/conditioning may retain an FX matrix/provider capability. It
   is not held quotes or immutable replay: a date-aware or revised provider can
   answer differently at the horizon. A calculation requiring fixed FX data
   explicitly captures qualified immutable quotes/coverage at the FX owner.
   Do not add a context policy enum that pretends to forecast/freeze an arbitrary
   provider, or infer quote authority from a cache or retained Arc.

9. **Use the existing strict snapshot/restore contract and lossless ingress.**
   Retain `MarketContextState`'s owning versioned loader, required fields/explicit
   nullable state, source-family validators and shared resource/diagnostic limits.
   Apply the same identity rules as live operations: duplicates within a namespace
   error, cross-namespace IDs remain allowed, embedded keys agree, and registered
   references resolve to expected kinds. Build the complete candidate after its
   source records load, then commit once; do not expose a partial restored graph.

   Reconcile raw JSON object-name duplication at the existing contract-parser
   owner before generic Value/map materialization loses it, as accepted in core
   numerics24/25. Array-ID checks alone cannot catch overwritten `prices` keys.
   Prebuilt host dictionaries/Values cannot recover erased duplicate history.
   No second context codec, old reader, omission default, alias or migration is
   introduced. Reconcile schema/contract versions at their actual owners.

   Preserve full source modes/clocks/conventions/quote and construction metadata,
   not only sample values and IDs. Keep deterministic ordering of namespace
   arrays/maps and intentionally preserved hierarchy order. Determinism for a
   given state is not byte identity across reordered hierarchy trees or financial
   equivalence; canonical content hashing remains its accepted separate owner.

10. **Preserve captured FX state without claiming provider replay.** Existing
    FX snapshot capture checks matrix/provider revision coherence and retains
    serializable quote layers. Preserve that checked capture rather than assume
    independent reads form a coherent edition. Restore the actual quote-only
    state; arbitrary live provider functions, connections and future date/policy
    coverage are not serialized. Missing required restored queries error under
    the accepted FX contract, without an inferred live provider.

    Serialization success is structural capture, not proof that every original
    financial query can replay. Actual replay needs source/target/dependency and
    date/quote coverage qualification. A partial factor snapshot/restoration also
    owns its selected scope; do not call it a complete source/model reconstruction.

11. **Separate organizational hierarchy from concrete financial resolution.**
    Preserve validated hierarchy structure and its intended ordering. Bare leaf
    IDs/paths/tags are organizational selection data, not proof of a compatible
    active source. A resolved concrete financial action carries the required
    namespace/type and exact ID and must satisfy the context contract above.
    Hierarchy path/tag/overlap, missing/zero-match and leaf-association policy
    remain with Draft21; this record does not adopt them or certify all hierarchy
    references merely because a snapshot restores. It likewise does not adopt
    Draft22's market-change measurement conventions.

12. **Reconcile actual Rust and host callers through the same owner.** Update
    typed source/volatility dispatch, all context mutation/scratch/condition/state
    routes, calibration publication/context building, scenario adapters, actual
    theta/risk/attribution consumers, Python/WASM context APIs, pickle/JSON,
    stubs/declarations/schemas/parity and examples together. New fallible operations
    propagate errors instead of retaining an infallible warning/drop route.

    Attribution's clone-and-overwrite factor restore must stage its explicitly
    selected final market and propagate failure; do not publish sequential partial
    replacement/removal or suppress composite loss. Reuse existing core context/
    state owners and cheap COW storage; no host financial calculations, generic
    transaction framework, provider serialization fiction or parallel API family.

## Observed implementation and discrepancies

Core typed getters are exact, but bare-ID bump and volatility dispatch use
independent namespace priorities. Replacement helpers collect unchecked pairs,
allowing duplicate keys and external/embedded ID drift. Insertion/retention and
other mutation paths may drop composites after committing source changes, with
invalidated IDs discarded by several callers. Existing private source state,
staged bump clone and fully constructed JSON restore are useful foundations.

Scratch variants publicly expose saved state and are Clone; rollback reinserts
saved sources/maps without owner/generation/LIFO checks, and surface-point
reversion reselects coordinates. MetricContext's Err path already discards its
scratch, so this is a general lifecycle/source-path finding, not a reproduced
claim of failed-metric contamination of subsequent calculations.

Current roll applies days separately to source bases, reconstructs family-specific
origins and shallow-clones other maps. CPI/price rolling changes origin/base
metadata and retained interpolation offsets; it does not simply divide all CPI
levels by a new CPI. FX shallow cloning retains the actual provider. Scenario
roll may replay every recipe-bearing hazard; theta projects crossed fixings and
held-spot observations, while carry uses a different held-curve decomposition.
Their wider economic choices remain Draft.

Snapshot arrays/maps are deterministically ordered, with hierarchy insertion
order retained. Within-namespace array duplicates and typed composite references
are already checked, but shared decoding currently materializes generic JSON
Value before raw duplicate names can be diagnosed. FX capture already checks
revision coherence and restores quote-only state. These are source/control-flow
findings, not executed production mutation, replay, pricing or risk results.

## Independent examples and qualification limits

On **2026-11-08**, twenty standalone abstract-state/identity/rational/200-digit
Decimal cases were evaluated without production imports. State-machine examples
illustrate the selected contracts; they do not execute the Rust lifecycle:

- A bare X in curve/surface/scalar stores selects the curve under old priority;
  an explicit scalar target selects 100. Separately, an existing wrong-kind
  curve cannot be satisfied by an unrelated same-ID surface.
- External key LOOKUP with embedded ID ARTIFACT loses LOOKUP on value-based
  snapshot restore. Separately, duplicate replacement pairs X→0.2,X→0.3 collapse
  to one last-wins value. Both motivate admission before lossy collection.
- A staged hazard→discount replacement with an index still referencing that
  hazard fails and preserves the original graph. Separately, deleting a dependency
  alone fails, while explicitly deleting its index and dependency together succeeds.
- Ordered scalar actions give (100+10)*2=220 versus 100*2+10=210. Separately,
  correlation 0.9+0.2-0.2 has a forbidden intermediate even though its net change
  is zero. A third batch case discards an earlier price update when a later
  surface action fails, preserving the original context.
- Blind old composite-map rollback deletes a newly added unrelated composite.
  Separately, nested scratch 100→110→220 restores 110 then 100; wrong order does
  not own that state. A third scratch case shows why another context's value 200
  cannot be overwritten by a token holding the first context's previous 100.
- Bases January 1/January 2 plus one day become January 2/January 3, unlike one
  explicit January 3 target. Separately, in a stated January US30/360 case,
  original January29→30 and January29→31 times are 1/360 and 2/360, whereas
  fresh January30→31 time is 0. This checks original-clock interpretation.
- With intensity 1000, absolute survival to time 1 underflows binary64, while
  local survival from 1 to 1.1 is exp(-100), approximately 3.72e-44. Separately,
  total variance 0.04 over remaining time 0.75 gives sigma approximately 0.230940108
  if retained as total variance, versus 0.2 under constant variance density.
  Cloning alone selects neither volatility-aging model.
- A shared date-aware FX provider returns 1.1 at opening and 1.2 at the horizon;
  retaining its handle is not held quotes. Separately, valuation November 3
  does not advance a November 1 cutoff to admit a November 2 release.
- Raw JSON with duplicate prices.PX names loses the first value in an ordinary
  object decoder. Separately, price-only namespace restoration preserves the
  fixing series. This twentieth case checks explicit restoration scope.

Independent production qualification must cover typed identity/kind/duplicates,
key/embedded-ID consistency, family actions and same-ID collisions; single/batch/
remove/replace atomicity, actual composites and ordered failures; scratch ownership/
exact restoration/error paths; dated retained-clock conditioning, frozen active
source eligibility and actual horizon adapters; strict state ingress/versions/
metadata/references, coherent FX capture and required-query coverage; actual
partial restorers and host/schema/error parity. Reconcile all exposed routes and
use financial replay/valuation/risk fixtures at their owners. Abstract traces,
source bounds and deterministic serialization do not establish these results.

## Evidence and alternatives

Owners: [storage dispatch](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs),
[typed getters](../../../../finstack-quant/core/src/market_data/context/getters.rs),
[insertion](../../../../finstack-quant/core/src/market_data/context/insert.rs),
[composite binding/FX conversion](../../../../finstack-quant/core/src/market_data/context/mod.rs),
[ordered/scratch actions](../../../../finstack-quant/core/src/market_data/context/ops_bump.rs),
[date roll](../../../../finstack-quant/core/src/market_data/context/ops_roll.rs),
[state](../../../../finstack-quant/core/src/market_data/context/state_serde.rs),
[bulk/retention scope](../../../../finstack-quant/core/src/market_data/context/stats.rs),
[contract ingress](../../../../finstack-quant/core/src/contract/load.rs),
[FX capture/restore](../../../../finstack-quant/core/src/money/fx/matrix.rs),
[volatility resolver](../../../../finstack-quant/valuations/core/src/market/volatility.rs),
[metric scratch](../../../../finstack-quant/valuations/core/src/metrics/traits.rs),
[theta](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/theta.rs),
[scenario roll](../../../../finstack-quant/scenarios/src/adapters/time_roll.rs)
and [attribution factor restore](../../../../finstack-quant/attribution/src/factors.rs).
Existing [context](../../../../finstack-quant/core/tests/market_data/context.rs),
[state](../../../../finstack-quant/core/tests/market_data/serde.rs)
and [lifecycle](../../../../finstack-quant/core/tests/market_data/lifecycle.rs)
tests were inspected or located, not executed.

Primary [QuantLib term-structure date/time definitions](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructure.hpp)
distinguish reference dates and day-count time mapping. Its
[implied term-structure API](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/yield/impliedtermstructure.hpp)
uses an explicit reference date; this is architectural context, not adoption of
its scalar-time offset algorithm, observer behavior or this repository's financial
conditioning accuracy. Typed targeting, atomicity, snapshot and scratch policies
are this proposal's design choices, not rules mandated by a pricing standard.

Global ID uniqueness, warning-driven cascade deletion, default namespace priority,
generic full-market aging and arbitrary provider replay are not alternative
fallbacks selected by this package. Separately owned financial forecasting,
conditional history, scenario expansion and quote replay remain explicit operations.

## Follow-up tracking

Pending adoption, runtime reconciliation and independent qualification are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
