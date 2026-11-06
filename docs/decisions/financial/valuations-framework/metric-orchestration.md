# VALUATIONS-FRAMEWORK-08 — Metric dependency resolution, qualified identities, storage and aggregation contracts

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and algorithm

Metrics use an instrument-specific calculator when registered, otherwise a default calculator. Requests already present in the computed cache are accepted without requiring a registry entry; other unknown/not-applicable requests fail. Resolve static or context-dependent dependencies by depth-first topological traversal with temporary marks and an explicit cycle path. Already computed values are not recalculated. **Missing or inapplicable dependencies are skipped**, while explicitly requested missing metrics fail. A downstream calculator therefore owns whether an absent dependency errors or activates a documented fallback, such as degenerate carry without YTM. Calculation errors are wrapped with the metric ID.

Metric context preserves the selected pricing dispatch for bumps and supports externally supplied recalibration providers. Rate-market/hazard rebuild methods delegate to that provider and fail when absent; no calibrator is embedded here. Typed extensions and cashflow caches reduce repeated work. A scratch market is retained only after successful work; errors discard it so unreverted mutations cannot contaminate subsequent bumps. Replacing an instrument clears typed extensions but leaves computed metrics and cashflow caches; direct users must manage those caches consistently with the new instrument.

## Identity, reporting and units

Standard IDs are typed constants. Strict parsing rejects unknown names; registry resolution additionally permits explicitly registered custom IDs. Permissive `FromStr`/`TryFrom<String>` construct custom names, so parsing alone is not proof of a supported calculation. Composite keys append `::`-separated components and inherit base metric units. Suggestions trim/case-fold the request and rank case-folded exact, prefix, substring, then other matches; ties use Levenshtein distance and original name. A two-row dynamic program computes insertion/deletion/substitution distance, and the default limit is five. This is diagnostic selection, not a substituted calculation. Composite components encode colon and a literal `_x` escape prefix as lowercase `_xHH`, encode empty as `_empty`, and escape literal `_empty`; parsing decodes then re-encodes to reject noncanonical spellings.

Bucket series store both an ordered series and flattened qualified metrics. Duplicate labels overwrite a flattened scalar while the ordered series still retains both entries. Matrix storage checks nonempty rectangular shape but not unique row/column labels; duplicates can similarly collide. Requested results are combined with custom/bucketed and explicit diagnostic side outputs in deterministic name order. Unit classification marks currencies, decimals, bp, years, percent and dimensionless values; custom/calculator-dependent units remain Unknown. Bump reporting units such as currency per 1bp are part of the metric definition, not dimensional enforcement by `MetricUnit::Currency`.

Aggregation deliberately rejects unqualified delta/gamma/vega/vanna/volga/bucketed-vega as additive, while allowing them when factor-qualified. It allows the enumerated PV/cash/accrued/rate/credit/FX/inflation/cross sensitivities. Ratios/yields and unknown custom metrics are not blindly summed. This is an explicit name-based aggregation policy; it cannot prove that identically named results share currency, bump conventions or model basis.

Flat result export preserves measure insertion order. Long rows decode the first composite component as `curve`, join every remaining component with `::` into `bucket`, and leave scalar coordinates absent. Series export retains only composite rows with the exact requested base. This representation intentionally compresses higher-dimensional coordinates; it is not a typed factor schema. Result construction/stamping and composite result structs carry already computed values rather than rerunning portfolio aggregation.

## Evidence, alternatives

[registry.rs](../../../../finstack-quant/valuations/core/src/metrics/registry.rs), [traits.rs](../../../../finstack-quant/valuations/core/src/metrics/traits.rs), [universal.rs](../../../../finstack-quant/valuations/core/src/metrics/universal.rs), [IDs](../../../../finstack-quant/valuations/core/src/metrics/ids/mod.rs), [unit mapping](../../../../finstack-quant/valuations/core/src/metrics/ids/unit.rs), [aggregation.rs](../../../../finstack-quant/valuations/core/src/metrics/aggregation.rs), and [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs) own the inspected behavior. Tests cover dependency order/cycles, registration, parsing, unit mappings and series/matrix shape; they are behavioral evidence, not executed.

Strong typed factor/currency dimensions or strict dependency failure are alternatives requiring an API/method decision; this documentation does not introduce them. No external source establishes acceptance of the registry's aggregation policy.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
