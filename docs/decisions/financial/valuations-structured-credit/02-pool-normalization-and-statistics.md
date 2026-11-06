# VALUATIONS-STRUCTURED-CREDIT-02: Collateral normalization and pool statistics

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: asset rows, representative lines, instrument collateral and concentration reporting.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The engine converts representative lines to asset rows and builds one pool view for statistics and simulation. Instrument-backed pools follow a separate projection source. This preserves one effective representation but embeds approximation and missing-data choices that require adoption.

## Scope, algorithm and conventions

Rows, representative lines and instrument collateral are mutually exclusive. Representative lines carry weighted age/maturity/coupon/default attributes; normalization anchors origination at closing minus age and expands their configured terms into a row. Original balance is explicit when supplied, otherwise reconstructed from current balance plus cumulative defaults, prepayments and scheduled principal. A supplied original balance cannot be below the reconstructed amount within tolerance.

Pool WAC weights fixed-rate assets only; weighted spread weights floating assets only and remains in basis points. Performing/defaulted treatment differs by statistic and is explicit in each iterator. WAM uses balance-weighted remaining Act/365F years, flooring already matured remaining term at zero. WARF uses canonical rating factors, treating missing ratings as `NR`; callers such as the metric registry return zero when no weighted statistic is available. Diversification score is inverse concentration, `1 / sum(w_obligor²)`, over identified obligors, not a rating-agency diversity-score table. Reports aggregate exposure by asset type, rating, sector, region and obligor, and sort concentrations deterministically.

Attachment/detachment percentages describe the note capital structure. If absent, they are derived from original note balances in reverse payment-priority order; explicit points must tile [0,100] and their thickness must agree with balance share within 0.5 percentage points. Payment priority is seniority then input order, with distinct sequential ranks. These points are not independently calibrated portfolio-loss boundaries.

## Controls, limitations and alternatives

Pool balance, currencies, representation exclusivity, original balance and representative-line shape are validated. Statistics of a representative pool inherit aggregation bias; instrument pools and unrated reinvestment cohorts can lack the row metadata used by some metrics. Empty/absent statistic fallbacks differ by public caller and must not be read as measured zero risk. `senior_balance` silently returns zero if checked currency summation fails; validation is therefore a prerequisite, not an interchangeable reporting policy.

Exact loan-by-loan rows are the supported alternative to representative lines. Exposure-weighted versus equal-name concentration, rating-agency diversity tables, and fully marked asset market values would be distinct methodologies; they are not interchangeable labels for the implemented inverse HHI.

## Implementation and validation evidence

- [structured_credit/types/pool.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool.rs)
- [structured_credit/types/pool_report.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool_report.rs)
- [structured_credit/types/pool_state.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool_state.rs)
- [structured_credit/types/tranches.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/tranches.rs)
- [structured_credit/metrics/pool.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pool.rs)

Existing evidence: [structured_credit/unit/components/pool_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/components/pool_tests.rs); [structured_credit/unit/attachment_derivation_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/attachment_derivation_tests.rs); [structured_credit/academic_golden.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/academic_golden.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [rating nomenclature](../../../REFERENCES.md#moodys-rating-symbols). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Existing hand-formula WARF/WAS checks do not establish current agency parameter licensing, provenance or fit.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
