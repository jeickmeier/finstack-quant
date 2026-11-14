# VALUATIONS-FRAMEWORK-01 — Selected-model dispatch, valuation lifecycle, and metric enrichment

- Decision status: **Accepted — complete checked valuation dispatch and lifecycle package, 2026-11-13**.
- Implementation status: Accepted target. Observed behavior is recorded below.
- Validation status: Source and independent arithmetic examples inspected and checked. Runtime and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked valuation dispatch and lifecycle

1. **Select one registered product model.** Resolve the exact
   `(InstrumentType, ModelKey)` in the explicitly supplied registry
   or the product family registry. A named model must match that
   registry entry. When the caller omits the model, use a documented
   product-owned selection from admitted contract terms, or require
   an explicit model when the product has no such selection.
   Retire the blanket trait Discounting default and divergent
   direct-instrument dispatch in checked pricing. A stable product
   default can depend on authored product features. Market
   availability or a failed calculation does not choose another
   model or registry. Retain duplicate-registration rejection and
   explicit replacement. A missing pair returns an error with the
   supported alternatives. Retain the actual selected model and
   registry for dependent reprices.

2. **Validate the complete request before model execution.** Public
   native, registry, envelope, typed host and standalone export
   entries validate the instrument and all applicable instrument,
   metric and scenario controls through the Rust owner. For
   metric-only overlays, preserve absent fields, merge supported
   nested fields, and let explicit null clear only a nullable
   field. Reject unknown, malformed or inapplicable overrides.
   The metric convenience path cannot rewrite contract, model or
   scenario inputs. Apply the shared strict raw-text admission
   before map materialization so duplicate JSON keys cannot
   silently win. Product construction and structural restore
   establish their own invariants. A restored instrument still
   passes the complete post-merge pricing gate. Remove the public
   trusted-validation boolean and unchecked constructor. Any
   internal reuse of admission is private and bound to the
   identical validated input. An export capability check is
   independent of pricing validation.

3. **Resolve and retain the effective valuation date.** Use the
   requested valuation close for ordinary pricing. A documented
   product-owned date rule may select a different effective close
   when the contract specifies that basis, such as an authored
   valuation date for a dated DCF. Require checked resolution and
   record the requested date, effective date and rule. A curve
   base, settlement date or incidental market lookup does not
   reset the valuation close. The pricer's result, explanation,
   market access and metrics use the resolved close. Keep it fixed
   across ordinary market reprices. An explicitly selected
   horizon experiment owns its new date and required market
   information. Preserve distinct fixing and settlement
   eligibility under [ADR 0045](../conventions/0045-financial-clocks-and-date-boundaries.md).

4. **Keep one economic valuation and declare its arithmetic basis.**
   Product and registered low-level hooks return the selected
   value before the generic price multiplier. A true raw
   pre-Money result includes native currency and checked finite
   arithmetic. A risk operation that requires raw repricing
   must have it at every endpoint and center. A Money-only
   model remains valid for Money valuation. Removing the
   Money-to-float fallback from the raw path prevents a
   finalized Money result from being mistaken for a raw price.
   Use the same economic cash and model basis on every
   comparable endpoint. Keep Money-only valuation in checked
   Money arithmetic and finalize raw-backed reporting amounts
   under [CORE-CONVENTIONS-012](../core-conventions/012-money-decimal-precision.md).
   Product-specific pricing and numerical accuracy remain with
   the selected product owner.

5. **Apply a price multiplier once and require an explicit spread
   consumer.** For finite decimal price shock `s>=−1`, apply
   `V(1+s)` once at the shared boundary. Zero shock is a
   literal identity. `s=−1` gives zero after admission and
   base valuation. A smaller shock is outside this sign-preserving
   multiplier domain. Use checked raw multiplication before
   Money finalization for a raw-backed mark and checked Decimal
   scaling for Money-only marks. Detect unusable factors, overflow
   and loss of nonzero required values. Where result details
   reconcile to the adjusted headline, apply the same adjustment
   to the matching components with their basis recorded. Quote,
   accrual and other standalone measures retain their own
   definitions. A measure of the adjusted mark must use that
   same adjustment at its baseline and endpoints. A spread shock
   in bp must be consumed by the selected product and model
   on a supported spread basis, or return an error. The scenario
   owner retains shock composition and targeting.

6. **Distinguish market declarations from active bindings.**
   Keep ordered declarations for planning and invalidation.
   Deduplicate only identical role and descriptor records.
   Preserve distinct leg, issuer, expiry, strike, quote
   coordinate, spot or FX orientation. Capture the exact
   source namespace, type, identity and applicable basis
   from the actual product and metric resolutions under
   [CORE-MARKET-20](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).
   The active binding describes the consumed value, including
   relevant coordinate, unit, clock, role and override or
   recipe provenance. Several exposures can use one source.
   Rebuild or shock that source once only where its full
   binding agrees, preserving the exposures separately.
   A scalar volatility and complete model convention can
   bypass market volatility entirely. Any intentionally
   selected market convention or displacement has its own
   active binding. A same-ID surface, cube and FX-delta
   surface remain distinct source types. An absent required
   selected source errors. A declared but inactive source
   does not cause a missing-data failure or imply exposure.
   Use actual bindings for risk and attribution, including
   extra sources a metric requires. Record 02 owns volatility
   and fitted model resolution. Later risk records own
   individual shock and replay rules.

7. **Scope enrichment and seeds to the owning valuation basis.**
   Native metrics use the resolved original product and
   selected model. An equivalent instrument is permitted
   only for a product-authorized measure with a declared
   transformation, valuation target, model, date and cash
   basis. A qualified metric identity has the same basis whether
   requested alone, with other metrics, or as a dependency.
   For example, the bond owner may select an explicitly
   labelled cash-pay equivalent for a defined yield or spread
   comparison. Retire generic inference from a non-Discounting
   model and a list of metric names. Evaluate the equivalent
   in its own validated context with its own seeds and caches.
   Keep original pricer measures, cashflow caches and typed
   extensions with the original run. A required seed or
   calculator failure propagates with its identity. Scope
   any instrument or market replacement to a fresh context
   unless the complete binding is unchanged. Record 08
   owns dependency graph and metric storage rules.

8. **Publish a single result per qualified metric identity.**
   Under [ADR 0010](0010-metric-identifiers.md), a pricer-published
   value can satisfy a requested metric when its full basis
   matches and the pricer owns that result. In that case reuse
   it without a second calculation. Admit declared qualified
   side outputs and diagnostics. If an alternative basis
   is supported, give it its own approved identity. Conflicting
   producers for the same identity are an error, including
   collisions with side outputs. Retire silent merge-order
   overwrite and accidental cache hits across contexts.
   Explicitly requested unsupported measures and failures
   propagate. Menu narrowing for a mixed portfolio remains
   an explicit consumer step. Record 08 owns key encoding,
   output ordering and aggregation detail.

9. **Use one selected child valuation for composite headline and details.**
   Each child uses its own admitted product model and the
   resolved registry rules. A parent model selection does not
   substitute for a child model. Reuse the child's valuation
   basis for headline aggregation, leg details and applicable
   requested measures. Reuse cached values only for matching
   effective date, instrument state, registry/model, market,
   controls and cash basis. Apply each admitted child multiplier
   once and the parent multiplier once on the parent value.
   Parent scenario scaling of metrics requires the corresponding
   adjusted mark basis. Report a metric from an applicable
   child only under the composite owner's aggregation contract.
   Reject an unsupported composite measure, a requested
   aggregate with no eligible child, or an applicable required
   child that fails. Composite quantity, reporting FX and
   economic aggregation remain with the composition owner.

10. **Stamp execution provenance and keep diagnostics honest.**
    Record the actual selected product model, requested and
    effective dates, scenario changes and applied numerical,
    currency and FX policies. Keep configured bump requests
    distinct from the actions and stencils executed by
    individual metrics. Source construction metadata stays
    with the source. An FX policy on a declared curve does
    not establish that pricing used it. Replay qualification
    requires coherent contract, market, model, calibration
    and provider state in addition to result provenance.
    If explanation is requested, use the selected pricer's
    trace or the generic reconciled trace on the same admitted valuation
    basis. Report truncation and unavailability explicitly,
    with a finite public row limit and a reason that survives
    a zero row allowance. An
    unavailable optional explanation leaves the valid price
    available. Standalone cashflow export requires qualified
    decomposability and reconciliation under record 15.
    An empty schedule is a successful no-residual result
    only when the owner establishes it. Unavailable contingent
    cash cannot be represented by an empty placeholder or an
    inferred day count. Reconcile native, Python and WASM
    envelopes, typed routes, portfolio, calibration, scenario,
    exports, declarations, schemas and parity to the Rust
    authority. [Strata's calculation function](https://strata.opengamma.io/apidocs/com/opengamma/strata/calc/runner/CalculationFunction.html)
    gives a separate example of explicit measures and market requirements.

## Observed behavior

### Choice, scope and selection

A price is selected by `(InstrumentType, ModelKey)`. `Instrument::price_with_metrics` uses an explicit request model or the instrument's default (trait default Discounting), and an explicit registry or the instrument family's registry. Unknown pairs fail with available alternatives; no next-best model fallback occurs. Registration rejects duplicates, while replacement is an explicit operation. The generic instrument adapter calls `base_value`, not the fully wrapped `value`, so lifecycle adjustments are applied once.

### Ordered algorithm

1. Validate instrument invariants and instrument, metric and scenario override blocks. An internal `instrument_validated` flag can skip this; the public instrument convenience entry resets it to false. Resolve the effective valuation date through the instrument's hook; its default is the requested date.
2. Invoke the selected base pricer at the effective date. Stamp the result date and request metadata. Apply scenario price shock as `V×(1+shock)` once; finite shocks below −1 are not forbidden by generic validation. Spread shock is only stored here and must be consumed by a supporting product; it is not a generic post-price transformation.
3. Record model, requested date, declared market dependencies and scenario price shock; when metrics are requested, record resolved configured bump sizes. These are request controls, not a complete executable model/calibration snapshot, and can differ from effective boundary-shrunk Greek stencils.
4. Preserve pricer-provided explanation or construct a bounded cashflow trace. An unavailable decomposition becomes an explanatory trace entry rather than a price failure. A composite instrument can supply its own leg metrics/details; leg requests intentionally clear the parent model override so child products choose their own model.
5. For non-Discounting models, requested spread-equivalent metrics may run on the instrument's `metrics_equivalent` representation, while remaining risk runs on the original instrument. Both use the same selected dispatch, but only original-instrument metrics receive the pricer seed. The product owns which equivalent transformation is valid.
6. Build metric context with selected dispatch, config, market history/provider, instrument overrides, pricer seed, instrument seed and selected-pricer seed hook. Standard pricer measures enter the computed cache. Compute requested metrics, include custom/bucketed and declared diagnostic side outputs, then merge original pricer measures last, so original values win duplicate keys.

Raw repricing follows validation/date resolution/scenario multiplication too, while using a raw floating-point PV path where implemented. The default raw adapter extracts a Money price; it is not automatically a separate high-precision numerical kernel. Family registration and typed downcasts constrain model ownership; model-key names alone do not establish algorithm or supported features.

Dependency merging deduplicates each curve role, scalar, series and FX pair while preserving first insertion order. A volatility descriptor is equal only when surface, optional spot and strike **bit pattern** agree; distinct strikes on one surface remain distinct, while unique-surface extraction preserves first-descriptor order. Present-surface filtering probes ordinary surfaces only, ignores missing candidates and does not discover cubes or FX-delta surfaces. This matters because shared pricing source resolution supports those other artifacts. Consumers must explicitly provide compatible risk resolution; dependency declaration alone is not proof that all volatility source types receive equivalent bumps.

The `validate_on_deserialize` macro invokes instrument invariants after serde parsing; ordinary constructor/builder validation and explicit override validation remain separate boundaries. The empty-cashflow-provider macro deliberately emits an empty classified schedule, defaulting to no notional and ACT/365F unless supplied; it does not synthesize an option payoff schedule.

### Limitations, alternatives and evidence

[registry.rs](../../../../finstack-quant/valuations/core/src/pricer/registry.rs), [enrichment.rs](../../../../finstack-quant/valuations/core/src/pricer/enrichment.rs), [instrument trait](../../../../finstack-quant/valuations/core/src/instruments/common_impl/traits/instrument.rs), [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs), [generic adapter](../../../../finstack-quant/valuations/core/src/instruments/pricing/generic.rs), and [family.rs](../../../../finstack-quant/valuations/core/src/pricer/family.rs) are the inspected execution owners. Inline tests provide dispatch/lifecycle behavior evidence, not model accuracy; none was run. This orchestration has no external quantitative standard to claim.

A complete replay artifact would additionally need market/model/calibration state; present provenance should not be described as that artifact. Alternate registries and explicit models are supported choices; automatic failover is not.

### Independent review evidence

- A child value of `100` from the direct kernel and `120`
  from the selected registry gives composite values `225`
  and `270` respectively under quantity `2`, FX `1.25`
  and a parent multiplier of `.9`. The source currently uses
  direct raw child values for the headline and registered
  child values for leg enrichment.
  [Composite owner](../../../../finstack-quant/valuations/src/instruments/composite/instrument.rs).
- Exact Money `10^16+1` survives an identity multiplier. A
  binary64 round trip loses the last unit. At a `.02bp`
  central step, raw prices `100.00005` and `99.99995`
  give `.0025` currency per bp even though the values
  rounded to cents coincide. A finite binary64 value times
  a finite price multiplier can also overflow.
- A raw metric patch with two `rate_bump_bp` entries
  retains only the later value under ordinary map parsing.
  A pair-preserving parse detects the duplicate before merge.
  Current standalone export can declare validation complete
  after a decomposition eligibility check.
  [Envelope and overlay](../../../../finstack-quant/valuations/src/pricer/json.rs)
  and [cashflow export](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs).
- With forward and strike `100`, unit discount, one year and
  scalar volatility `.31`, a Black call is about `12.317868`
  at displacement zero and `13.549655` at displacement ten.
  The existing scalar-volatility branch can inherit a shift
  from a present same-ID cube. This illustrates the economic
  effect of treating a potential source as active.
  [Volatility resolver](../../../../finstack-quant/valuations/core/src/instruments/common_impl/vol_resolution.rs).
- A model-published native spread of `25bp` and a computed
  equivalent spread of `70bp` currently collapse to `25bp`
  if they use the same key and the last merge wins. Qualified
  ownership avoids using either result to satisfy the other
  basis. Production qualification still needs dispatch and
  error parity across native, JSON, Python and WASM, and
  selected-source, composite, raw repricing and trace fixtures.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
