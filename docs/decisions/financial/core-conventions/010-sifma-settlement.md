# CORE-CONVENTIONS-010: Published TBA settlement dates and explicitly estimated projections

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to SIFMA functions in `dates/imm.rs` and `generated/sifma_settlements_generated.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Exact SIFMA class dates and a business-day-index estimate have separate public entry points. The exact path fails closed when no maintained class/month value exists. This distinction prevents an internal projection formula from being represented as a published settlement date; adopting estimates for valuation remains a user decision.

## Algorithm, selection and conventions

`SifmaSettlementClass` has A/B/C/D, default A. `from_agency_term` uppercases agency text and treats any string containing `GN` as GNMA: GN +30Y -> C; any 15Y -> B; non-GN +30Y -> A; all other combinations -> D. This is a string/term heuristic, not validated agency reference data.

`sifma_settlement_date` linearly finds `(year,month)` in the generated table, selects the class column and constructs the date; absent row or class -> `None`. The inspected table contains Class A for 2025 and all four classes for 2026–2027. `next_sifma_settlement` searches Class A only, starting with the input month, requiring a date strictly later; it checks at most 14 months and immediately returns `None` on any missing month's lookup rather than skipping gaps.

`estimated_sifma_settlement_date` counts business days from the first of the requested month on the `sifma` calendar and returns the 9th/11th/14th/16th business day for A/B/C/D. It errors if that many business days cannot be found within the month. It is never an automatic fallback from the exact lookup.

## Limitations, failure and alternatives

Exact data coverage is finite and includes class gaps in 2025. No published dates beyond 2027 exist in the inspected table. The estimate inherits the holiday calendar's own full-day and future-exception limitations. Calling `next_sifma_settlement` does not select a class from an instrument. Explicit estimates are supported; refreshed published tables or a richer agency mapping would be separate changes.

## Evidence

[Lookup/classification/estimate](../../../../finstack-quant/core/src/dates/imm.rs), [generated table](../../../../finstack-quant/core/src/generated/sifma_settlements_generated.rs), [maintained CSV](../../../../finstack-quant/core/data/sifma_settlements.csv). Inline tests `sifma_uncovered_year_class_fails_closed`, `sifma_estimate_uses_business_day_of_month_anchors`, `sifma_class_a_2025_uses_published_dates` and `next_sifma_settlement_fails_closed_beyond_coverage` are stored-date and algorithm examples. The generator consistency check passed; Rust tests and external publication refresh were not run. [SIFMA TBA Good Delivery](../../../REFERENCES.md#sifma-tba-good-delivery) is the canonical reference entry.

The estimator has no observed error-bound or comparison calibration against all published classes/months.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
