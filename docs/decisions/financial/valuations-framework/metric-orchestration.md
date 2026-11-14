# VALUATIONS-FRAMEWORK-08 — Metric dependency resolution, qualified identities, storage and aggregation contracts

- Decision status: **Accepted — complete checked metric orchestration and reporting package, 2026-11-13**.
- Implementation status: **Observed, with reconciliation pending** across producers, registry, aggregation, reports and hosts.
- Validation status: Source and primary references inspected. Independent bounded graph, identity, reporting and aggregation examples checked. Runtime and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked metric orchestration and reporting

1. **Select one authority for each metric.** Resolve the exact qualified
   request on the admitted pricing basis under
   [record 01](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).
   Use the applicable instrument-specific calculator or, in its absence,
   an explicitly registered default applicable to that instrument and basis.
   A pricer value or prior computed value can satisfy a request only from
   its declared authority on the identical basis. Treat that admitted
   value as a resolved graph node, with no unrelated calculator inputs.
   If the selected source fails or fails to publish a required value, keep
   the actual error. Require a separate approved identity for a different
   financial basis, and reject conflicting producers for the same key.

2. **Resolve a checked dependency graph.** Require explicit static or
   context-dependent edges. Resolve the applicable graph against the frozen
   request, independent of which other metrics are requested or cached.
   Plan declared side outputs through their authoritative producer. Reject
   unknown or unsupported explicit requests before calculating. Traverse in
   deterministic request and declaration order with dependencies first, one
   evaluation per node, an explicit cycle path, and checked graph work and
   allocation. Edges are required unless the producer expressly declares
   the optional dependency and the allowed absence reason. Permit an
   optional branch only for a known inapplicable or explicitly unavailable
   measure on that basis. Unknown IDs, missing required data or providers,
   nonfinite outputs, cycles and calculation failures still propagate with
   context. The product owner selects any financial fallback, including
   the carry policy in [record 13](carry-theta-breakeven.md).
   [Strata derived calculation dependencies](https://strata.opengamma.io/apidocs/com/opengamma/strata/calc/runner/DerivedCalculationFunction.html).

3. **Bind caches and publication to one prepared request.** Hold the
   instrument, effective date and phase, market/history, registry and model,
   overrides, amount basis and applicable provider state fixed. An equivalent
   instrument or changed basis gets a fresh scoped context with its own
   metrics, cashflows, schedules, typed extensions and seeds. Access only
   declared dependencies. Stage validated scalar, structured and diagnostic
   outputs and publish them as a complete batch. A failed request may reuse
   earlier compatible validated state and publishes no partial measures.
   A scratch market is reusable only after restoration to the bound
   baseline. Isolate it from the source market and discard it on failure.
   Delegate recalibration to the explicitly bound provider for operations
   that need it. Preserve failure causes and actual execution provenance.

4. **Keep exact canonical keys and separate lexical admission from use.**
   Retain standard base IDs and [ADR 0010 attribution](0010-metric-identifiers.md),
   including `bucketed_dv01::USD-OIS::10y`, hazard-curve `cs01` and
   bond z-spread `cs01::BOND_A`. Require exact case and spelling for use.
   Admit requestable custom IDs only under declared registry or selected
   producer authority. A wire parser can read a well-formed qualified
   result key without promising a calculator. Preserve the base and every
   ordered coordinate. Keep `::` framing, `_x3a` for a colon, `_x5f` for
   the underscore before literal `x`, `_empty` for an empty component and
   `_x5fempty` for literal `_empty`. Decode and re-encode to reject
   noncanonical wire forms. Require a nonempty base and validate the
   owner's coordinate roles and arity in addition to the encoding. An
   attributed factor requires a nonempty identity. Use lossless key labels
   for numeric buckets and strikes, with rounded labels reserved for display.
   An exact curve ID `10y` remains an identifier. Renames and attribution
   changes retain ADR 0010's explicit agreement requirement.

5. **Carry the metric's actual financial basis.** Bind the result to its
   definition, factor coordinates and roles, unit and normalization, and
   actual currency when monetary. Retain the valuation information phase,
   applicable horizon and clock, model or comparable calculation basis,
   quantity basis, units per bump or denominator, and sign convention where
   they determine use. Keep a static catalog description distinct from the
   facts of a specific calculation. Avoid a generic Currency default or
   inferred tenor semantics from a label. For example, financing annuity
   is currency per unit decimal spread under
   [record 06](trs-legs.md#accepted-decision-for-checked-trs-legs-and-par-spread).
   An unresolved unit can remain Unknown for display. Arithmetic requiring
   the missing metadata is unavailable.
   A hazard curve and a bond with the same identifier can both produce a
   `cs01` key with that suffix. Their source roles and quote experiments
   govern compatibility, while the accepted key spelling is preserved.

6. **Store finite unique results and preserve absence.** Use one checked
   authority for scalars, ordered series and matrix cells. Validate an
   entire batch before insertion. Require unique complete qualified keys
   across scalar, seeded and side outputs, and unique labels within each
   declared axis. Validate shape, indices, finite values and checked work.
   A dense matrix has nonempty rectangular axes. A producer with irregular
   support publishes only actual cells with their full coordinates. Preserve
   real zero and distinguish a qualified empty series from invalid shape or
   unavailable output. Missing surface strikes stay absent. Read the
   structured and scalar forms from the same authority. Allow reuse of a
   single admitted value on the same basis. Conflicting writes error even
   if their numbers happen to agree. Validate and reject duplicate keys
   when restoring raw wire maps.

7. **Publish one lossless view with deterministic order.** Emit requested
   results in first-request order, once per exact key. Follow with declared
   public side outputs in canonical key order. Keep internal dependencies
   private unless explicitly requested or declared public. Preserve axis
   order in structured views. Each long row carries the full key, base,
   ordered decoded coordinates, financial metadata and value. Use an
   empty coordinate list for a scalar and preserve an actual empty
   coordinate where the owner admits it. Select series by exact root or
   qualified prefix. Return strict descendants with their full keys and
   coordinate paths. Use role-aware curve/bucket display only where known.
   Put metric values in a separate map from wide-row identity and PV
   fields. Preserve transferable row order through arrays rather than
   requiring JSON object order. Suggestions are diagnostic only. Keep up
   to five names by trimmed case-folded match, prefix, substring and then
   other, breaking ties by edit distance and exact name. Bound diagnostic
   work and preserve the original request and error.
   [JSON object and array semantics](https://www.rfc-editor.org/rfc/rfc8259#section-4).

8. **Aggregate only linear and compatible measures.** Require the owner
   to declare linear quantity scaling and compatible economic meaning.
   Match full measure and factor identity, axis, units and normalization,
   applicable model or comparable shock basis, information phase, horizon
   and valuation basis. Thus two different instruments may contribute to
   the same curve bucket on an admitted common basis. Factor-qualified
   Greeks still require the actual factor and denominator. Candidate
   additive names alone are insufficient. Ratios, yields and undeclared
   custom values remain per position unless the owner defines a distinct
   aggregate. Require declared row currency. Use an explicitly qualified
   normalization when different units represent the same measure. Keep
   structurally inapplicable contributions, unavailable data, failures and
   true zeros distinct. A strict requested total needs applicable eligible
   contributions and every required applicable contribution to succeed. If
   an owner admits a partial aggregate, label its coverage explicitly.
   [Strata sensitivity compatibility](https://strata.opengamma.io/apidocs/com/opengamma/strata/market/param/CurrencyParameterSensitivity.html#plus(com.opengamma.strata.market.param.CurrencyParameterSensitivity)).

9. **Scale and translate once on the selected basis.** Identify whether
   a contribution is per unit or already signed and sized. Apply the
   admitted signed position scale once. Use actual measure currency and
   explicitly qualified FX for monetary results. A frozen reporting
   translation of native sensitivity and sensitivity from repricing the
   converted amount are separate bases, especially if the shock moves FX.
   Preserve exact Money aggregation where required and use checked finite
   scaling and ordered compensated sums on admitted raw results. Keep
   representable complete contributions through intermediate scaling. Fail
   unusable required conversions and nonrepresentable results. Count each
   admitted contribution once. A headline and its decomposition, or structured
   and flattened views of one cell, share the same contribution. Preserve
   portfolio eligibility, FX policy, quantity and reporting selection in
   the [portfolio record](../portfolio-execution/10-metric-aggregation-and-reporting.md).

10. **Reconcile every existing output path with the shared rules.** Use
    canonical Rust for native and registry results, composite and portfolio
    consumers, attribution and diagnostics. Carry the same key, basis,
    errors and row order through Python tables, WASM result views, schemas,
    declarations, parity and UI. Reconcile model-published metrics, theta
    and carry horizon writers, z-spread bucket labels, surface grids and
    custom outputs at the actual producers. Keep `theta_period_days`
    owned by theta. A different carry horizon belongs in its own
    owner-approved identity or execution provenance. Preserve owner decisions for
    pricing formulas, shock definitions, account reporting and precision.
    Qualify graph and cache reuse, identity round trips, unit and currency
    errors, atomic output, sparse reporting and signed aggregation.

## Observed behavior

### Choice and algorithm

Metrics use an instrument-specific calculator when registered, otherwise a default calculator. Requests already present in the computed cache are accepted without requiring a registry entry; other unknown/not-applicable requests fail. Resolve static or context-dependent dependencies by depth-first topological traversal with temporary marks and an explicit cycle path. Already computed values are not recalculated. **Missing or inapplicable dependencies are skipped**, while explicitly requested missing metrics fail. A downstream calculator therefore owns whether an absent dependency errors or activates a documented fallback, such as degenerate carry without YTM. Calculation errors are wrapped with the metric ID.

Metric context preserves the selected pricing dispatch for bumps and supports externally supplied recalibration providers. Rate-market/hazard rebuild methods delegate to that provider and fail when absent; no calibrator is embedded here. Typed extensions and cashflow caches reduce repeated work. A scratch market is retained only after successful work; errors discard it so unreverted mutations cannot contaminate subsequent bumps. Replacing an instrument clears typed extensions but leaves computed metrics and cashflow caches; direct users must manage those caches consistently with the new instrument.

### Identity, reporting and units

Standard IDs are typed constants. Strict parsing rejects unknown names; registry resolution additionally permits explicitly registered custom IDs. Permissive `FromStr`/`TryFrom<String>` construct custom names, so parsing alone is not proof of a supported calculation. Composite keys append `::`-separated components and inherit base metric units. Suggestions trim/case-fold the request and rank case-folded exact, prefix, substring, then other matches; ties use Levenshtein distance and original name. A two-row dynamic program computes insertion/deletion/substitution distance, and the default limit is five. This is diagnostic selection, not a substituted calculation. Composite components encode colon and a literal `_x` escape prefix as lowercase `_xHH`, encode empty as `_empty`, and escape literal `_empty`; parsing decodes then re-encodes to reject noncanonical spellings.

Bucket series store both an ordered series and flattened qualified metrics. Duplicate labels overwrite a flattened scalar while the ordered series still retains both entries. Matrix storage checks nonempty rectangular shape but not unique row/column labels; duplicates can similarly collide. Requested results are combined with custom/bucketed and explicit diagnostic side outputs in deterministic name order. Unit classification marks currencies, decimals, bp, years, percent and dimensionless values; custom/calculator-dependent units remain Unknown. Bump reporting units such as currency per 1bp are part of the metric definition, not dimensional enforcement by `MetricUnit::Currency`.

Aggregation deliberately rejects unqualified delta/gamma/vega/vanna/volga/bucketed-vega as additive, while allowing them when factor-qualified. It allows the enumerated PV/cash/accrued/rate/credit/FX/inflation/cross sensitivities. Ratios/yields and unknown custom metrics are not blindly summed. This is an explicit name-based aggregation policy; it cannot prove that identically named results share currency, bump conventions or model basis.

Flat result export preserves measure insertion order. Long rows decode the first composite component as `curve`, join every remaining component with `::` into `bucket`, and leave scalar coordinates absent. Series export retains only composite rows with the exact requested base. This representation intentionally compresses higher-dimensional coordinates; it is not a typed factor schema. Result construction/stamping and composite result structs carry already computed values rather than rerunning portfolio aggregation.

## Evidence, alternatives

[registry.rs](../../../../finstack-quant/valuations/core/src/metrics/registry.rs), [traits.rs](../../../../finstack-quant/valuations/core/src/metrics/traits.rs), [universal.rs](../../../../finstack-quant/valuations/core/src/metrics/universal.rs), [IDs](../../../../finstack-quant/valuations/core/src/metrics/ids/mod.rs), [unit mapping](../../../../finstack-quant/valuations/core/src/metrics/ids/unit.rs), [aggregation.rs](../../../../finstack-quant/valuations/core/src/metrics/aggregation.rs), and [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs) own the inspected behavior. Tests cover dependency order/cycles, registration, parsing, unit mappings and series/matrix shape; they are behavioral evidence, not executed.

The observed code uses name, unit family and caller policy in places where
the proposed package requires resolved ownership or basis. The external
references illustrate dependency checks, sensitivity compatibility and
interchange behavior.

- [Result projections](../../../../finstack-quant/valuations/core/src/results/dataframe.rs), [metric catalog](../../../../finstack-quant/valuations/src/pricer/catalog.rs), [bucket labels](../../../../finstack-quant/valuations/core/src/metrics/buckets.rs), [vega storage](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/vega.rs), [theta](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/theta.rs) and [carry](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/carry_decomposition.rs) show the live reporting and producer boundaries.

### Independent review evidence

- A diamond dependency graph evaluates its shared leaf once. A closed
  cycle provides a concrete error path. In a separate synthetic producer
  example, theta and carry attempt to write `theta_period_days` as `1`
  and `91`. Selecting the last write makes publication order affect the
  result. The proposed single-owner rule rejects the conflict.
- Qualified tuples `[USD,a::b,c]` and `[USD,a,b::c]` have distinct
  canonical keys. Joining their suffixes creates the same display bucket
  `a::b::c`. Numeric labels `1.1y` and `1.4y` also collide when rendered
  as `1y`. A custom measure named `pv` can coexist with headline PV when
  it stays in a distinct map. Numeric-looking object keys can enumerate
  in a different order than the producer's result list.
- Two surfaces with different strike grids can have a true zero at one
  actual node and an absent cell elsewhere. A dense union filled with
  zero erases the difference. Duplicate series labels and conflicting
  scalar/cell keys must fail before any part of the batch is visible.
- Two long units of `EUR 10` converted at `1.25 USD/EUR`, together
  with a signed `USD −5`, sum to `USD 20` on the same additive basis.
  `USD 1` per bp and `USD 10,000` per unit decimal rate agree after
  declared normalization. Frozen translation of a zero native FX
  sensitivity remains zero. Repricing a fixed `EUR 100` in USD as its
  translation rate moves gives USD sensitivity `100` per unit of that
  quote. Checked ordered compensation gives `1` for `[1e16,1,−1e16]`.

Production qualification requires actual metric owner and seed tests,
shape and wire round trips, producer reconciliation, mixed-currency
aggregation and existing host and reporting evidence.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
