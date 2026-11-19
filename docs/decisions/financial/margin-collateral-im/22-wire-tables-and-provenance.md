# MARGIN-COLLATERAL-IM-022 — Sensitivity table conversion and margin wire boundaries

**Decision status:** Accepted 2026-11-17 for checked sensitivity tables, margin restoration and result provenance.

**Implementation status:** Existing behavior is described below. Typed sensitivity bases, row admission, nested restoration, provenance and schema and host reconciliation remain pending.

**Validation status:** Source, transport fixtures and primary references inspected. Independent examples checked contribution versus snapshot identity, explicit zero and paired curvature. Runtime, schema and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked margin tables, wire state and provenance

1. **Carry the calculation basis with every batch.** Require a typed header
   for detached rows and tables. Include wire version, SIMM or FRTB family,
   methodology edition and effective profile, valuation date and information
   cutoff, calculation currency, sensitivity units and applicable scope, side
   and product partition. An empty batch retains its schema and basis. A
   complete zero-risk result requires explicit coverage evidence.
   [Arrow metadata contract](../arrow-interchange/02-schema-order-and-metadata.md).
2. **Give each financial coordinate its own field.** Publish a fixed column
   schema per family with required and nullable fields for each row kind. Keep
   factor currency, pair direction, curve, issuer, bucket, option expiry,
   underlying tenor and basis distinct where applicable. Reject missing
   required dimensions, populated inapplicable fields, unknown coordinates
   and nonfinite amounts. The financial owners define each factor and unit.
   [Basel MAR21 factor definitions](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Declare contributions or snapshot state.** Add repeated contribution
   keys only where the elected method allows them, with checked accumulation
   in the declared order. Two qualified contributions of `+20` and `−5`
   give `+15`. A canonical snapshot carries one sorted net value per ordinary
   factor and rejects duplicates after the owner's normalization. Keep explicit
   zero rows and retain multiplicity and meaningful order for position records.
   [SIMM inputs](03-sensitivity-inputs.md#accepted-decision-for-checked-simm-sensitivity-identity-scaling-and-netting) ·
   [Accepted FRTB input contract](../margin-regulatory-xva/01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance).
4. **Preserve curvature, default and residual-risk evidence.** Keep SIMM
   curvature contributions with their separate risk tenor, option expiry and
   vega basis before prescribed weights. Carry FRTB curvature as an explicit
   up/down pair with both values supplied, including literal zero. Keep DRC
   and residual-risk positions individually. Require any applicable DRC P&L
   adjustment explicitly, including zero, and retain residual-risk
   classification and its notional basis.
5. **Validate the whole payload before restoring state.** Require the exact
   supported version and one closed payload variant. Reject duplicate JSON
   keys, unknown fields and invalid nested terms, dates, amounts or rows before
   publishing a result. Apply the same Rust admission rules to JSON, tables,
   constructors and nested restoration. Restore collateral metrics from their
   primitive inputs and check derived result claims where the required evidence
   is present. Economic replay requires original qualified inputs.
   [Bounded contract loading](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics) ·
   [Collateral metrics](21-collateral-metrics.md#accepted-decision-for-checked-collateral-metrics).
6. **Require explicit result provenance.** Keep wire version, methodology
   edition and effective data identity distinct. Retain source authority,
   scope, currency, cut, horizon, coverage and qualification. Distinguish
   reported amounts, calculated amounts and indicative estimates. The
   historical SIMM profile retains its approximation status and limitations.
   Remove the default `approximation=false` and generate examples through
   validated constructors.
   [Parameter and source authority](01-registry-and-defaults.md#accepted-decision-for-explicit-margin-parameter-snapshots-and-contractual-terms) ·
   [SIMM result evidence](11-simm-total-and-mpor.md#accepted-decision-for-product-class-simm-assembly-and-supported-horizons).
7. **Publish one Rust-owned contract through existing host surfaces.** Preserve
   dimensions, nulls, zeros, exact Money and errors through Python, WASM and
   table exports. Restore tabular state only with its required basis. Generate
   schemas and examples from the owning Rust types. Publish the format as
   Finstack-native. ISDA CRIF interoperability requires an edition-specific
   mapping and separate validation.
   [ISDA CRIF specification](https://www.isda.org/a/owEDE/risk-data-standards-v1-36-public.pdf).

## Choice and rationale

The observed adapters export fixed long-format columns using overloaded factor labels. The accepted wire contract keeps typed factor and basis evidence so each host can reconstruct validated Rust state.

## Input domain and conventions

SensitivityRow carries risk_class, kind, issuer, bucket, tenor, optional expiry and DRC metadata plus an amount. The table envelope does not include the sensitivity container's base currency; callers must retain it and provide it again to from_rows. Amount units depend on risk class/kind.

## Ordered algorithm

1. Export all populated SIMM/FRTB maps to rows. Sort by class, kind, issuer, bucket, tenor, expiry, sector, seniority, optional numeric metadata and amount total order. Zero rows are retained. FRTB up/down curvature becomes two rows; SIMM curvature retains a separate expiry column and pre-SF volatility-weighted vega.
2. IR currency lives in issuer; CQ sector or FRTB bucket in bucket; FX currency/pair in issuer. FRTB GIRR vega encodes option/underlying tenor as A/B; credit/commodity delta encodes tenor/basis as A/B. DRC exports rating bucket, sector, seniority, maturity and P&L adjustment; RRAO distinguishes exotic/other notional.
3. Import matches exact class/kind strings, requires used dimensions, parses currencies/buckets/pairs and calls the financial container's adders. Duplicate map keys sum; curvature halves add; DRC positions remain separate. Missing DRC P&L defaults to 0, while maturity/sector/seniority are required.
4. SIMM curvature rows validate immediately, but ordinary SIMM and all FRTB rows do not get a final container validate call in from_rows. Unused populated dimensions are ignored. Later model engines enforce their own input domains.
5. Tables use stable dimension/measure columns, nullable absent dimensions and an empty metadata map; even empty exports retain schema. SIMM adds expiry, FRTB adds DRC fields.
6. MarginEnvelope requires the exact version tag and one OTC/CSA/call shape via Serde; from_slice performs shape parsing only, not CsaSpec/OtcMarginSpec::validate. Feature-gated schema generation registers input/output/component types, examples and externalized common definitions using the shared core generator.

## Selection, controls and limitations

ImResult constructors default approximation=false; calculators set appropriate provenance explicitly. SIMM detail retains unscaled10-day numbers with separate horizon scale. Many monetary output structs deserialize public derived fields without recalculation; utilization is a notable validated-wire exception. Schema examples are illustrations, sometimes direct constructed outputs, not model-validation evidence.

## Alternatives and tradeoffs

The accepted typed header supplies the basis for restoring detached tables. Closed factor coordinates remove delimiter ambiguity. Contribution feeds retain their admission order and canonical snapshots retain their unique net keys.

## Source and validation evidence

Inspected row conversion code and located roundtrip tests for all FRTB adders/DRC and SIMM maps/curvature. Schema parity tests are structural evidence; they do not validate financial claims in prose or show that direct deserialization applies business validation.

Implementation and data: [src/table.rs](../../../../finstack-quant/margin/src/table.rs), [src/schema.rs](../../../../finstack-quant/margin/src/schema.rs), [src/calculators/traits.rs](../../../../finstack-quant/margin/src/calculators/traits.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [src/types/serde_validation.rs](../../../../finstack-quant/margin/src/types/serde_validation.rs).

Existing evidence: [src/table.rs](../../../../finstack-quant/margin/src/table.rs), [tests/schema_parity.rs](../../../../finstack-quant/margin/tests/schema_parity.rs), [tests/simplification_contracts.rs](../../../../finstack-quant/margin/tests/simplification_contracts.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
