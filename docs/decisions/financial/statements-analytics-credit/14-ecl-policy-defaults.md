# STATEMENTS-ANALYTICS-CREDIT-14: Impairment policy snapshots, applicability and provenance

- Decision status: **Accepted for the full impairment policy selection contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for resolved impairment policy snapshots

1. Select one immutable policy snapshot identified by method family, policy ID and revision. Supply a complete snapshot directly or select an exact revision from a caller-supplied registry. Retain embedded calibrations as historical audit evidence.
2. Record the policy owner, source and vintage, exposure scope and effective interval. Verify that the policy applies on the reporting date and was available by the applicable cutoff. Retain parameter provenance, including staging thresholds, cure observation cadence, recovery timing and historical loss inputs where applicable.
3. Use one fallible validation contract across Rust, Python and WASM. Validate all required method-specific parameters and constraints. An override requires a distinct, fully resolved revision that records its base revision, changed values and rationale.
4. Carry the selected policy, effective dates, exact resolved parameter values and content identity in each output for replay.

For example, a revision effective July 1, 2026 is ineligible for a June 30, 2026 reporting date. The result records the eligible revision the caller selected.

## Audited choice and rationale

Default IFRS-style and CECL-style configurations are read from a versioned embedded registry. The implemented policy IDs select exact records; the metadata labels source/version/effective date but does not perform date-driven rule selection or establish regulatory approval.

## Audited scope and input domain

Embedded JSON schema `finstack_quant.ecl_policy/1`, default IFRS/CECL IDs, per-family records with aliases, source metadata, effective-date text and numerical parameters. Unknown fields are rejected.

## Audited algorithm

1. Load and cache the embedded registry. Validate schema string, unique record IDs/aliases within each family, existence of both defaults and each record's parameter/source metadata. Effective date is checked for nonblank text, not parsed or matched to an as-of date.
2. IFRS configuration defaults to .25-year buckets, point-in-time LGD and staging thresholds .01 absolute PD increase, 2.0 relative multiple, three rating notches, 30/90 DPD, enabled qualitative triggers and cure counts 3/12. No custom rating scale is supplied. TTC/downturn overrides are absent; Stage-3 recovery defaults separately to one year.
3. CECL defaults to .25-year buckets, two-year forecast horizon, immediate historical reversion, .02 annual historical PD and PD/LGD/EAD methodology. Omitted record fields deserialize to 90-DPD impairment, enabled default evidence, one-year recovery and discounted expected losses. WARM loss rate remains absent.
4. `EclConfig`, `StagingConfig` and `CeclConfig` defaults resolve these records. The internal fallible registry loader returns validation errors, but ordinary Default implementations use `expect` for the assumed-valid embedded artifact.

## Audited selection and controls

The registry records source version `2026-04` and effective-date text `2026-04-26`. No public runtime policy-selection API or as-of switch is implemented here; caller-created configs can override parameters and are validated by their runtime consumers.

## Audited conventions

Bucket/recovery/forecast durations are years, PD/LGD are decimals, DPD is days and cure periods are caller observations. Registry .01 absolute PD threshold is one percentage point. The default historical PD is an assumed scalar, not an inferred portfolio statistic.

## Audited limitations, failure and fallback

Source names cite broad IFRS 9/ASC 326 frameworks, not provenance for each numerical policy value. Results do not embed the registry policy ID, source version or effective date. Registry validation and runtime validation differ in places (positive registry bucket widths versus runtime minimum .0001). Invalid compiled data can panic at Default, despite a fallible internal loader.

## Audited alternatives and tradeoffs

Explicit policy/config selection at measurement and a resolved policy snapshot in results would improve reproducibility at a usability cost. Effective-dated policy selection would need real version rules and migration governance; this registry does not supply them.

## Evidence

[policy.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/policy.rs) and [ecl_policy.v1.json](../../../../finstack-quant/statements-analytics/data/accounting/ecl_policy.v1.json) are the inspected owners. Inline policy tests locate default preservation and enum-wire contracts. Named [IFRS 9](../../../REFERENCES.md#ifrs-9-impairment), [ASC 326](../../../REFERENCES.md#asc-326-cecl) references are conceptual/provenance pointers only; no external current-standard verification or policy-adoption evidence is claimed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
