# CORE-CONVENTIONS-013: FX source authority, single-pivot conversion, state and relative shocks

- Decision status: **Accepted — complete FX resolution/state/shock package, 2026-11-07**.
- Implementation status: **Partly reconciled 2026-11-11 (F6)**: config and policy items. `FxConfig.pivot_currency` is a required `Option<Currency>` (`null` disables routing, a missing key errors) replacing `enable_triangulation` and the implicit USD pivot; there is no `FxConfig::default`, `FxMatrix::default` or `FxConversionPolicy` default, and `FxQuery.policy` has no serde default; `FxMatrix::new`/`from_quotes` and the Python/WASM constructors take the pivot explicitly; `PeriodAverage` is removed (variant, label and host members); host `FxMatrix.rate` requires its policy; `FxPolicyMeta` has no `Default`. Routing behaviour itself is unchanged (F7). Resolution order, routing on `NotFound` only, caching, state and shocks (items 3-12) remain, scoped to all six `money/fx/` implementation files.
- Validation status: **Evidence inspected; independent arithmetic examples passed; production qualification pending**. Documentation links/status/count checks and independent exact-average, cross-rate, shock and binary64 boundary examples passed. The linked Rust/host tests were located or inspected, not run; this is not implementation or methodology certification.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

This record owns point-date FX source selection, one-pivot derivation, cache/state
semantics and relative pair shocks. [Accepted 011](011-fx-date-and-quote-conventions.md#accepted-decision-explicit-fx-quote-metadata-and-value-dates)
owns ordered quote units and contractual value-date boundaries;
[accepted 012](012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
owns Decimal Money conversion and financial rounding. Those choices are not
reopened. The package below replaces unadopted source-observed alternatives; its
source hierarchy and shock model are library decisions, not universal market or
accounting rules.

## Accepted decision: FX resolution, state and relative shocks

Accepted on **2026-11-07** after whole-record review. Runtime reconciliation
and independent qualification remain pending. Observed implementation is
retained separately below so accepted behavior is not mistaken for current behavior.

1. **Resolve an explicit ordered point-date query.** `FxQuery` contains
   `from`, `to`, the actual applicable date `on`, and a required explicit
   `FxConversionPolicy`. Keep `CashflowDate` and `PeriodEnd` as distinct
   provider-selection purposes, not instructions to calculate dates or infer
   spot versus forward. Remove the policy `Default`, serde omission fallback
   and host optional-policy shortcuts. `FxPolicyMeta.strategy` is likewise
   explicit rather than defaulted. The owner supplies the correct date,
   economic quote type, source edition/publication cutoff and policy under
   0045; labels alone do not certify these facts. Do not silently substitute
   another date/policy or project a missing fixing.

2. **Remove `PeriodAverage` from the point resolver.** A single `on` date
   cannot distinguish, for example, March and first-quarter averages sharing
   March 31. Remove the variant, wire label and wrappers; do not reinterpret
   old average queries as point queries. Actual reporting/transaction owners
   select interval, observations, missing-data policy and time/amount weights,
   calculate the required oriented aggregate and apply it through 012's
   explicit resolved-rate conversion. No new average-query registry or
   generic accounting engine is introduced here. An average of reciprocals
   is not generally the reciprocal of an average, and a mean of cross
   products is not generally a product of means. A qualified accounting
   approximation requires its own owner decision; the existing test and
   “standard reporting approximation” comment do not adopt one.

3. **Use the same explicit authority and specificity order throughout.**
   Matrix-held quotes outrank the provider. Within each matrix/provider
   quote-store layer, try the exact date/policy pin first, then a pair-global
   constant. Within the winning scope, prefer supplied direct orientation,
   otherwise use a checked reciprocal of its opposite. Thus a matrix pin's
   inverse beats a matrix global direct, and a matrix global inverse beats a
   provider scoped direct. Align `SimpleFxProvider` with this rule. Global
   quotes are deliberate all-date/all-retained-policy constants, not inferred
   pegs, closing rates or fixings. Invalid authoritative data/reciprocals fail;
   never fall through to a lower authority. Provider implementations own
   their documented internal source selection and must honor query units and
   purpose. A cache is an optimization of that provider response, not an
   independent authority above explicit matrix inputs.

4. **Preserve directed inputs without automatic arbitrage repair.** All
   stored/provider/derived rates must be finite and strictly positive, in
   units of `to` per unit of `from`. Identity queries return exactly 1 with
   `triangulated=false` without provider/revision lookups. Reject all redundant
   stored identity quotes, even unit ones, rather than silently ignoring
   nonunit identity data. Distinct supplied forward and reverse orientations
   may coexist and need not be reciprocal; each direct wins in its scope.
   This does not model bid/ask sides or certify consistency. A reciprocal is
   required and validated only when requested by the resolution path. A
   tiny positive direct rate may succeed even when its unrequested inverse
   would overflow. No tolerance reconciliation, clipping or hidden repair.

5. **Require explicit single-pivot routing.** Replace redundant
   `enable_triangulation` plus unconditional pivot with one required
   `pivot_currency: Option<Currency>` in `FxConfig`: explicit `None` disables
   routing, `Some(currency)` selects exactly that pivot. No implicit USD,
   omitted routing choice or financial `FxConfig::default`. Matrix constructors
   take this explicit choice/config; remove default-routing constructor paths.
   After an exact provider query returns specifically `InputError::NotFound`,
   and only then, an enabled resolver may request `from -> pivot -> to` using
   the same date/policy and authority rules. Legs cannot recursively triangulate.
   Identity legs are 1. No alternative pivot, multi-hop search or market-order
   inference is attempted. Outages, unsupported policies, invalid values,
   numerical failures and other provider errors propagate without routing.
   A missing leg reports its ordered query; retain any failed leg's original
   error category/cause with route context instead of relabeling it missing.

6. **Keep calculation and provenance literal.** Return supplied direct
   binary64 rates unchanged. Compute a requested reciprocal as `1/r` and a
   selected cross as the two-leg binary64 product, checking the actual result
   is finite and positive; zero underflow and infinity are errors. Do not
   require unrequested reciprocal/intermediate-normalization representability.
   `FxRateResult.triangulated` means this matrix used its pivot fallback; false
   does not certify that an opaque provider used a direct market observation.
   Preserve that flag in cached results. No decimal/minor-unit quantization,
   arbitrage claim, universal pricing tolerance or lossless Decimal admission
   follows from an FX result. The explicit 012 float-to-Decimal boundary may
   reject a rate that is otherwise valid for this model resolver. Quote kind,
   vendor, time and publication/edition provenance remain owner metadata.

7. **Cache exact ordered queries.** Key observed entries by the full ordered
   `(from,to,on,policy)` and store the returned rate bits and triangulation
   flag unchanged, together with matrix/provider generations. Remove canonical
   currency-order storage, opposite-direction cache reuse and double-reciprocal
   normalization. Cache history/pressure cannot choose one of two supplied
   directed rates. Explicit pins/globals are never evicted and are checked
   before observations. Retain a technical default capacity of 256; explicit
   capacity must be a positive exact integer representable on the supported
   host, without float truncation. Authoritative mutation invalidates dependent
   observations; provider generation changes invalidate its observations.

8. **State consistency must match the provider's guarantees.** A provider
   generation identifies a coherent published quote state and is never reused;
   immutable providers can report generation 0. Mutable providers must publish
   updates atomically or make in-progress updates detectable, not expose mixed
   values under a stable generation. Matrix/provider generation exhaustion
   errors before mutation rather than wrapping. Retain at most three complete
   stable-state attempts for a rate or snapshot, then return a consistency
   error; do not serve stale results. An unversioned provider may answer a
   single uncached point request. A candidate resolution attempt needing more than one provider
   call, including failed calls within that attempt, requires a stable provider generation or an
   already frozen coherent provider. A single unversioned missing response
   followed by wholly matrix-held legs is permitted under a stable matrix,
   without claiming broader feed coherence. Identity/entirely explicit results
   do not require unrelated provider access. Live revisions are consistency
   tokens, not serializable market-data provenance.

9. **Capture complete supported state or fail explicitly.** Existing provider
   snapshot hooks become fallible and unsupported by default; silent empty
   snapshots are removed. Explicitly supported empty means a complete empty
   quote-only provider. Validate captured config, policies, keys, identity
   restrictions and rates before returning `FxMatrixState`, not only at eventual
   serialization. Full snapshots require coherent matrix and provider
   generations and faithful coverage of all behavior being restored; arbitrary
   live/calculated feeds must be frozen into a supported complete quote state
   by their owner or reject capture. Preserve config, authority layers, ordered
   global/scoped quotes and both supplied orientations in deterministic order;
   omit transient LRU entries and live generation counters. Restoration through
   the existing quote-only provider is valid only for faithfully representable
   behavior. Do not claim to serialize a callback, future feed updates or a
   shock that the quote state cannot reproduce. Unsupported capture is an
   ordinary documented error, never a successful partial snapshot.

10. **Restore fresh validated state; make writes atomic.** Remove
    `load_from_state` and its merge/ignored-provider semantics. Use one fresh
    `try_from_state` path that validates all config, currencies, required fields,
    policies, dates, identity restrictions, rates and parsed keys before
    publishing an object. Reject duplicate identical ordered keys within each
    authority/scope collection, including duplicates created by case-normalized
    currency parsing; opposite orientations and different authority layers are
    distinct valid keys. Apply the same no-duplicate admission and all-or-nothing
    validation to bulk updates; separate deliberate updates may replace an
    existing key. Preserve explicit overwrite intent without last-row-wins
    ambiguity inside a batch. Restore required routing/policy choices exactly;
    no defaults, compatibility aliases, migrations or partial writes. Persist
    the chosen cache capacity as well as routing. Serialized/restored
    `FxRateResult` values also require finite positive rates and a well-formed
    triangulation flag; deserialization is not an alternate invalid-rate entry
    path or independent proof of market provenance.

11. **Make a relative pair shock an explicit frozen scenario.** Retain
    `with_bumped_rate`, taking an explicit reference `FxQuery` plus the
    fractional decimal binary64 bump `b` (`0.01` means +1%). The query supplies
    pair/date/policy; it replaces the implicit CashflowDate validation. Require
    distinct currencies, finite `b > -1`, and finite positive `f=1+b`; validate
    these before provider access. Capture a coherent complete baseline through
    point 9, then validate the reference lookup against that frozen baseline.
    Source mutations after construction must not alter the scenario; unsupported
    freeze is an error, not a live-sharing fallback. The shock applies to this
    pair across all dates and retained policies, not just the reference scope.
    Reference validation does not certify all future scopes. `b=0` is an
    intentional identity scenario after ordinary admission/lookup validation.

12. **Define shock direction, dependencies and representability precisely.**
    For the target `A -> B`, resolve that ordered query on the frozen unshocked
    baseline and multiply once by `f`; for `B -> A`, resolve its own baseline
    ordered query and divide once by `f`. This preserves independently supplied
    reverse rates rather than replacing them with the inverse of forward.
    Apply exactly once whether the target was supplied, inverse-derived or
    pivot-derived. Unrelated supplied/provider quote responses stay fixed;
    unrelated matrix-derived crosses are recomputed using shocked target legs
    where selected by the same routing/authority rules. Do not treat opaque
    provider crosses as known dependencies. Clear derived observations and keep
    pins/globals in their original scope/authority. Reject nonpositive/nonfinite
    outputs; also reject nonzero bumps whose factor rounds to 1 or whose
    requested nonzero target rate rounds unchanged. Check all known stored
    target scopes at construction and every subsequently requested shocked
    rate. Never replace a failed shocked value with unshocked data. This is
    a pair-local finite-difference scenario, not a currency-wide/arbitrage-free
    recalibration, forward-curve reconstruction or automatic monetary rounding.
    Snapshot a shocked object only if the existing supported state can reproduce
    its entire behavior faithfully; otherwise report unsupported capture rather
    than flattening away shock direction, dependency or scope.

13. **Reconcile Rust, hosts, state and financial callers in one working slice.**
    Keep logic in Rust and propagate policy/config/constructor/shock/state
    changes through Python/WASM errors, stubs/facade, schemas, parity, examples
    and actual statements/valuations/scenario consumers. Hosts reject missing
    required choices, unknown/retired policy labels, malformed pairs/dates,
    booleans in numeric positions, nonfinite rates/bumps and invalid capacities;
    map errors through the existing public exception boundaries. Remove obsolete
    branches and misleading approximation/current-behavior claims without
    compatibility paths. Accounting-average eligibility and actual date/source
    selection remain with the relevant financial owner; don't copy the removed
    core aggregate inversion/cross heuristic into those callers. No new provider
    registry, serialization framework or general FX scenario engine is adopted.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Query policy omitted; `period_average` supplied | Error; no CashflowDate or point-query substitution. |
| March versus Q1 aggregate with the same March 31 end | Owner retains distinct intervals/weights; neither is a core point-date average query. |
| Rates 1 and 3 with equal weights | Mean is 2; mean reciprocal is 2/3, not 1/2. |
| Paired leg samples `(1,3)` and `(3,1)` | Mean cross is 3; product of means is 4. |
| Matrix scoped reverse 0.5; matrix global direct 3 | Scoped reciprocal 2 wins. |
| Matrix global reverse 0.5; provider scoped direct 3 | Matrix reciprocal 2 wins. |
| Same winning scope has A/B=2 and B/A=0.6 | Each direct is retained; opposite cache warm-up cannot change either. |
| Direct requested rate `1e-320` | Valid positive direct result; requesting its unrepresentable inverse separately errors. |
| Same currency query; stored same-currency quote; identity shock | Exactly 1 without provider; stored quote rejected; shock rejected. |
| Provider NotFound with explicit pivot and legs 2,3 | Cross 6; same query date/policy on both legs; triangulated true. |
| Provider outage/invalid value; failed leg | Propagate original error, adding route/leg context where relevant; no alternate source repair. |
| Routing None, pivot equals an endpoint, or unavailable pivot leg | No fallback / identity leg 1 / missing-leg error; no recursive pivot search. |
| Nonfinite, zero/negative result, product overflow or zero underflow | Error without clipping or arbitrary quote substitution. |
| Cache hit after matrix/provider mutation | Old generation cannot serve; re-resolve or bounded consistency error. |
| Revisionless feed requires a second provider call | Consistency error; freeze/version it first. |
| Provider snapshot unsupported versus complete supported empty provider | Capture error versus faithful empty provider state; do not conflate. |
| Batch/state contains duplicate normalized ordered keys or a late invalid quote | Reject whole operation; no prefix writes or last-row-wins restoration. |
| Target baseline A/B=2 and B/A=0.6; b=0.1 | About 2.2 and 0.6/1.1 respectively; do not replace reverse with 1/2.2. |
| Target pair itself derived through a pivot | Apply target multiplier once to its baseline result. Other matrix crosses move only if a selected leg is the target/reverse; a nonpivot target cannot be such a leg under the same single pivot. |
| b=-1, NaN, infinity, or a nonzero b with `1+b==1` | Error before querying provider. |
| b=0; later mutation of original live source | Valid frozen identity scenario; later source mutations do not change it. |
| Requested shocked quote overflows, underflows to zero or is unchanged for nonzero b | Error in that scope; no unshocked fallback. |
| Shock cannot be faithfully represented by the supported snapshot format | Explicit capture error; no incomplete successful round trip. |

## Observed algorithm: rate resolution

Identity queries return 1. For other pairs, try up to three complete resolutions while checking matrix/provider revisions before and after. Within each resolution: pair-global direct or reciprocal first; matrix pinned `(pair,date,policy)` direct or reciprocal second; a current observed LRU cache entry third; provider fourth. Direct orientation wins over inverse **within** each authority layer, while a higher-authority inverse wins over a lower-authority direct. All served rates must be positive finite and inverted rates must have representable reciprocals.

If provider returns `InvalidFxRate`, propagate it. Other provider errors trigger triangulation when enabled, even if not specifically a missing-quote error. Resolve from->pivot and pivot->to by the same authoritative layers/provider, without recursively searching other pivots, multiply, and validate the product. A failed leg is surfaced as `FxTriangulationFailed` naming the leg, potentially losing its original error type. Identity pivot legs are 1. Triangulated provenance is retained through cached repeat queries.

Observed quotes use a canonical currency ordering distinct from market-quotation rank, with reciprocal storage and reciprocal serving as needed; this preserves cold/warm bit consistency for the normalized result, but can change direct source bits, reject an unrequested overflowing inverse and let call order override an independently supplied opposite quote. LRU capacity defaults 256 and must be positive. Cache keys include date/policy; values include matrix/provider revision. Providers with no revision do not have observations cached. Explicit global/pinned quotes are never evicted by LRU pressure; updates invalidate observations.

`SimpleFxProvider` itself serves identity 1, scoped pinned direct/inverse, then global direct/inverse. Global quotes ignore query date and policy. `FxConversionPolicy` defaults CashflowDate; PeriodEnd and PeriodAverage are carried as keys to providers. There is **no averaging or period-end-date calculation** in this layer: PeriodAverage reciprocates/triangulates supplied rates like other policies.

## Observed algorithm: persistence and risk shocks

Snapshots preserve config, matrix globals/pinned quotes and provider global/pinned quote snapshots, with deterministic ordering; provider hooks default to empty even for behavior that cannot be faithfully restored. Rate-result deserialization derives no positive-rate validation. Snapshots omit transient observed cache entries. Capture retries up to three revision-stable attempts and rejects nonempty provider snapshots without a revision. Reconstruction builds a SimpleFxProvider and restores authority layers. `load_from_state` merges matrix global/pinned values into an existing object; it does not replace provider/config, and a later invalid pinned quote can leave earlier writes applied. It is distinct from fresh `try_from_state`.

`with_bumped_rate(from,to,b,on)` validates the effective rate at `on`/CashflowDate and requires a nonidentity pair and positive finite bumped rates. Matrix-held direct target quotes multiply by `1+b` and reverse quotes divide by it. The provider wrapper instead resolves its baseline forward and returns the reciprocal of the bumped forward for a reverse query, so an independently supplied reverse response can be lost. Unrelated supplied responses stay unchanged; matrix-derived crosses using shocked legs may move. A wrapper resolves a derived target cross before shocking, preserves the original provider's date/policy structure, and normalizes snapshots of the shocked orientation. Static global/pinned scopes are checked for shock overflow, fresh observations are built, and future live provider changes can still affect rates. The bump is fractional relative change, not percentage points or basis points.

## Observed limitations and rejected alternatives

The observed implementation lets callers disable triangulation/change pivot/cache or explicitly pin/global-override quotes. No bid/ask spread, arbitrage reconciliation, multi-hop graph search or stochastic FX projection is done here. Conflicting direct/inverse authoritative inputs can remain direction-specific; direct wins. Average of reciprocals generally differs from reciprocal of average, and average cross rates generally differ from product of averages; the code chooses the latter operations on supplied aggregates. Revisionless mutable providers cannot provide the same consistency guarantee as versioned snapshots.

## Evidence

[Python FX bindings](../../../../finstack-quant-py/src/bindings/core/market_data/fx.rs),
[WASM FX bindings](../../../../finstack-quant-wasm/src/api/core/market_fx.rs),
[statement conversion](../../../../finstack-quant/statements/src/evaluator/capital_structure_runtime.rs)
and [scenario FX adapter](../../../../finstack-quant/scenarios/src/adapters/fx.rs)
show actual host/caller boundaries requiring reconciliation. Their
existing defaults do not establish financial approval here.

[AASB 121, October 2023 compilation, paragraphs 22 and 40](https://standards.aasb.gov.au/aasb-121-oct-2023),
which incorporates IAS 21, permits context-qualified average-rate approximations
and warns against their use with significant rate fluctuations. That does not
establish reciprocal/product-of-average equivalence. It is background for
owner-specific accounting selection, not adoption of a reporting regime or
universal average method in core. The algebraic counterexamples above are
independent of any accounting standard.

[Matrix](../../../../finstack-quant/core/src/money/fx/matrix.rs), [provider contracts](../../../../finstack-quant/core/src/money/fx/provider.rs), [simple/bumped providers](../../../../finstack-quant/core/src/money/fx/providers.rs), [policy/config](../../../../finstack-quant/core/src/money/fx/types.rs). Inspected inline test `period_average_quotes_invert_and_triangulate_like_other_policies` explicitly encodes aggregate inversion/crossing. [FX integration tests](../../../../finstack-quant/core/tests/money/money_fx.rs) locate source precedence, scope retention, revision races, shock snapshots, cache pressure and overflow cases; these are deterministic invariants, not independently observed FX-market prices. [Clark](../../../REFERENCES.md#clark-fx-options) is general financial background.

No independent accounting-rate or market-data provenance certification was found.
The 2026-11-07 whole-record review inspected source and primary accounting background.
Independent rational-average/directional-shock and binary64 boundary examples,
documentation links/anchors, status/count checks and whitespace checks passed.
Production Rust/host tests and provider/risk/snapshot qualification were not run.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
