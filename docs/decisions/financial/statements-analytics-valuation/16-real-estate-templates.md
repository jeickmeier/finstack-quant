# STATEMENTS-ANALYTICS-VALUATION-16: Lease rent-roll projection, renewal attribution and property cashflow templates

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice, domain and selection

[Real-estate templates](../../../../finstack-quant/statements-analytics/src/templates/real_estate/mod.rs) generate scalar statement nodes from period-indexed leases and property assumptions. Lease start/end must resolve to model periods and end cannot precede start. Rent/step values and growth must be finite; negative rent and growth below −100% are not independently prohibited. Occupancy is `[0,1]`. Renewal requires positive term/rent factor and probability `[0,1]`; renewal is applied only when the original lease has an explicit end.

## Lease algorithm and timing controls

1. Infer periods per year from the **first period ID**, defaulting to four when no period exists; the later start/end lookup still requires actual periods. No mixed-frequency/day-length validation or day-count prorating is performed.
2. Build sorted rent steps, including opening base rent. The last step at/before a period resets both base rent and escalation age; duplicate dates resolve to the last applicable step. `rent=base×(1+g)^n`, with `n=elapsed periods` for `PerPeriod`, or `floor(elapsed/periods_per_year)` for default `AnnualEscalator`. Exponents above 10,000 and nonfinite resulting rents error.
3. Initial lease includes both start and end periods. Free-rent windows mark whole periods, combine as Boolean coverage and clip at model end. Windows with zero length do nothing.
4. Renewal begins `end_index+1+downtime`, lasts the configured number of periods, and starts at ending initial contractual rent times rent factor. Its own rent steps can reset that base; escalation restarts by the same convention. Outside the original/renewal interval, rent is zero.
5. Let contractual rent be `R`, full free-rent amount `F` (either `R` or zero), occupancy `o`, and renewal probability `p` (one for the original lease). Effective rent is `(R−F)op`; physical vacancy is `(R−F)p(1−o)`; renewal probability loss is `(R−F)(1−p)`. Total vacancy sums the two losses. Full `F` is attributed to free rent even when renewal probability is below one, preserving `R−F−vacancy=effective` under this attribution convention.
6. Insert per-lease scalar series and aggregate generated node IDs. Duplicate lease IDs overwrite the same generated series through builder semantics, then the aggregation list repeats those IDs, so distinct leases are not preserved. Fixed aggregate IDs such as `vacancy_loss_physical` and `renewal_prob_loss` can also collide with caller nodes.

## Property cashflow formulas

Property EGI is effective rent plus optional other income. Operating expenses combine supplied expense nodes and optional management fee, with a finite nonnegative decimal rate but no upper cap; fee base is EGI by default or effective rent when selected. NOI equals EGI minus operating expenses; NCF equals NOI minus capital expenditure. Optional node IDs select inclusion; formulas do not infer missing categories, inflation, reimbursements, taxes, depreciation, financing or terminal value. The standalone NOI builder requires nonempty revenue and expense node lists; the standalone NCF builder copies NOI when its capex list is empty, otherwise subtracts the summed capex nodes. Generated values are scalar: currency is a caller convention, not enforced money metadata.

## Alternatives and evidence

Period-indexed expected rent is not a stochastic renewal path, partial-period lease accrual or market-rent re-leasing model. Negative rents/very negative growth can produce alternating signs under integer compounding; whether permitted for stress cases requires an explicit domain. The free-rent versus renewal-loss attribution is one exact reconciliation, not uniquely determined economics.

[Template tests](../../../../finstack-quant/statements-analytics/tests/integration/real_estate_template_tests.rs) and [end-to-end statement cases](../../../../finstack-quant/statements-analytics/tests/integration/real_estate_statements_e2e.rs) provide located arithmetic/integration evidence. No lease-administration or independently priced property reference reconciliation was established. Calendar-prorated rent, stochastic vacancy and currency-enforced templates are unimplemented alternatives.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
