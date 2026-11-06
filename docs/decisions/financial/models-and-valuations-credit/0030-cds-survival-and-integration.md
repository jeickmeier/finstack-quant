# ADR 0030: CDS survival integration and valuation conventions

**Decision status:** Draft — observed implementation; adoption and rationale
pending agreement.

**Implementation status:** Observed for single-name CDS pricing and its reuse
by CDS indices; index composition and valuation have separate owners below.

**Validation status:** Evidence located and inspected; tests and external
reference calculations not rerun for this documentation audit.

**Audit date:** 2026-11-05

## Observed choice and rationale

`CreditDefaultSwap` selects a reduced-form hazard engine with conditional
survival and piecewise analytical protection and accrual-on-default integration.
`CdsValuationConvention` bundles premium, step-in, clean/dirty and par-spread
policies; its observed default is `BloombergCdswClean`.

The source rationale is screen reconciliation for named convention bundles,
with both curve knot grids retained and discount/hazard clocks kept distinct.
The [CDS leg record](12-cds-legs.md) owns those conventions, integration details
and source evidence. These explanations do not establish acceptance of every
bundle, default or approximation.

Proposed adoption rationale, awaiting agreement: a common reduced-form engine
keeps CDS calibration, pricing and spread risk on compatible cashflow and
recovery assumptions; explicit convention bundles make comparisons with named
reference systems reproducible. This proposed rationale does not establish
a library-wide default.

## Scope and selection

The scope is single-name running/upfront CDS, including supported forward
protection starts, and the analytic leg engine reused by `CdsIndex`. Discount
and hazard curves are required; contract and bootstrap recovery must satisfy
the agreement check in the [leg input contract](12-cds-legs.md#scope-and-input-domain).

`Instrument::default_model()` and the registered CDS pricer select
`ModelKey::HazardRate`; `base_value` uses `CdsPricer::npv_full`. The
[typed convention bundles](12-cds-legs.md#selection-and-controls) are explicit
alternatives. Individual Boolean mixtures and alternative numerical integrators
are not exposed selections. Regional `CdsConvention` schedule presets are
distinct: an ISDA regional constructor does not select `IsdaDirty` automatically.

Protection ends at the premium leg's contractual end. Separate protection
termination, stochastic recovery, default contagion and counterparty credit
adjustments are outside this engine. Index weighting and defaulted-name
treatment, hazard calibration, and quote-risk replay are separate choices.

## Methodology ownership

| Choice or algorithm | Detailed owner |
| --- | --- |
| Convention bundles, units, conditional legs, grid, settlement, lifecycle and failure behavior | [CDS legs](12-cds-legs.md) |
| Hazard representation and conditional default probability | [Hazard survival](../core-market/hazard-survival.md) |
| Analytic index aggregation versus informational projected cashflows | [Index valuation](14-index-valuation.md) and [index inputs](../core-market/credit-index-composition.md) |
| Quote-space credit/rate/recovery risk and margin fallback | [CDS/index risk](13-cds-and-index-risk.md) |
| Quote contracts and fixed-recovery hazard fitting | [CDS materialization](../calibration-framework/07-cds-materialization.md) and [hazard calibration](../calibration-targets/05-hazard-calibration.md) |

The owners retain numerical formulas, settings, source/test/reference evidence
and material limitations. They remain Draft; this selection record does not
adopt adjacent calibration, index or risk policies.

## Alternatives and tradeoffs

The named convention variants support different reference comparisons. Their
outputs are not interchangeable clean/dirty labels: schedule and accrual
policies can change the legs. Midpoint integration is simpler but loses
within-window discount/default timing; adaptive integration can estimate error
for smooth interpolants but costs more and needs calendar-discontinuity
handling. Neither is an alternative registered CDS engine. A structural default
model changes calibration assumptions and is not a missing-input substitute.

## Limitations and evidence

The fixed grid, whole-date inversion and absent-calendar settlement remain
observed approximations; the [date-inversion fallback](12-cds-legs.md#limitations-failure-and-fallback)
is not approved by this draft. Missing curves and inconsistent recovery error;
no hazard curve is inferred from missing inputs.

The named QuantLib and Bloomberg fixtures retain provenance limitations in the
[leg evidence](12-cds-legs.md#evidence); labels alone do not certify equivalence
under [ADR 0014](../../project/0014-financial-reference-evidence.md).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
