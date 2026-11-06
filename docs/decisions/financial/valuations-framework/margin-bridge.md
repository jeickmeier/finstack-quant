# VALUATIONS-FRAMEWORK-18 — Repriced risk extraction, tenor allocation and margin identity

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in shared valuation-to-margin helpers; actual margin aggregation and product fallback proxies are separate owners.
- Validation status: Evidence inspected/located, not executed; no comparison to a current margin methodology performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, algorithm and input domain

The bridge first requests an instrument's default pricing path with the desired risk metric. A pricing failure or missing scalar becomes `None`; bucketed-DV01 failure becomes an empty list. Product `Marginable` implementations decide whether to use a proxy. The helper deliberately loses the error category, so missing data and unsupported calculations are not distinguished from other failures. Selected-model overrides from an unrelated valuation request are not carried into this default request.

Bucket extraction searches only the standard DV01 tenor labels for each supplied curve ID and returns every found `(years,value)` pair. It does not discover custom grids or aggregate duplicate curve IDs. Its documentation calls the curves unique, but the loop does not deduplicate them; callers own that precondition.

Credit tenor allocation selects the first upper bound among 1, 2, 3, 5 and 10 years, otherwise 15Y. IR allocation selects 0.5, 1, 2, 3, 5, 10, 15 or 20 years, otherwise 30Y. This is ceiling allocation, not linear interpolation. Nonpositive values enter the first bucket; NaN and positive infinity reach the final bucket because no explicit finite-domain check precedes comparisons.

Reference-entity extraction rejects an empty curve ID, then strips at most two rightmost suffixes recognized as an integer followed by uppercase Y/M/W/D or a supported uppercase curve type (`CDS`, `BOND`, `CURVE`, `OIS`, `SOFR`, `ESTR`, `EURIBOR`, `LIBOR`, `HAZARD`, `SPREAD`). Either suffix kind may occur on either iteration. Unknown suffixes stop stripping. If stripping removes the entire prefix, preserve the original ID and warn. An ordinary unknown suffix returns the original text without warning despite the broader Rustdoc statement.

Credit delta requires explicit qualifying-sector or nonqualifying classification from `OtcMarginSpec`; absence or an unsupported variant errors. The bridge does not infer a sector from entity, rating or currency. Netting-set identity uses the CCP for cleared trades and repeats the CSA ID as both bilateral counterparty and agreement key otherwise.

## Alternatives, limitations and evidence

Interpolated tenor allocation, explicit reference-entity IDs, error-preserving risk extraction, and an explicit requested model are possible alternatives, not current bridge behavior. The bridge alone is not evidence of compliance with any current SIMM version; its names and comments do not establish it.

[margin.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/margin.rs) contains every algorithm in this record. Its inline tests cover explicit qualifying/nonqualifying routing and absent-classification rejection. The [DV01 record](finite-difference-rates.md) defines upstream shock signs and units. Independent tenor-boundary, duplicated-curve and pricing-failure behavior tests were not established in this scope; no tests were run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
