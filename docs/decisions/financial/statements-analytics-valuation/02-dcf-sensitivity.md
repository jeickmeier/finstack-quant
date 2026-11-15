# STATEMENTS-ANALYTICS-VALUATION-02: Cached-statement DCF parameter sensitivities

- **Decision status:** Accepted for the full Statements DCF sensitivity contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for Statements DCF sensitivity

1. Use one verified Corporate DCF baseline and evaluate the statements once. Reuse the statement snapshot, cutoff, required market inputs, effective valuation date, currency, forecast cashflows, timing and terminal basis. A WACC shock reprices every discount factor and any growth capitalization. A stable-growth shock holds H-model high growth and half-life fixed. An exit-multiple shock holds its matched terminal metric fixed. [Statements corporate DCF](01-corporate-dcf.md#accepted-decision-for-statements-corporate-dcf).
2. Explicitly select WACC and stable-growth shock sizes independently, in basis points or decimal rates. Specify exit-multiple shocks in absolute turns or a proportion of the baseline multiple. Use absolute turns at a zero baseline multiple. Require positive finite sizes and distinct represented levels. Low and high shocks may differ when specified.
3. Apply the direct DCF domain and any elected ceiling to each requested level. Vary only the selected parameter and use the exact specified shocks. If an endpoint is outside the domain, report that endpoint as unavailable with its requested level and reason, and retain any valid opposite endpoint. A malformed request, invalid baseline or pricing failure within the admitted domain produces an error.
4. Report endpoint levels, enterprise values and signed same-currency changes from baseline. Low and high name parameter directions. Rank complete pairs by the absolute difference between high- and low-endpoint enterprise values, with ties ordered by stable parameter ID. `dcf::wacc01` remains the separate centered common-equity measure in currency per positive basis point and requires both stencil endpoints. [Valuations corporate DCF](../valuations-equity-and-composition/17-corporate-dcf.md#accepted-decision-for-checked-corporate-dcf-and-dilution).

For example, the Gordon baseline in the corporate DCF record has enterprise value of USD 1,249.78 at 10% WACC and 2% stable growth. At 9% and 11% WACC, the changes are USD +178.56 and USD −138.88, respectively. With an elected 2.5% ceiling and stable-growth shocks of 100 basis points in each direction, the 1% endpoint has a change of USD −138.86 and the 3% endpoint is unavailable because it exceeds the ceiling.

## Audited choice and input domain

[`dcf_sensitivity`](../../../../finstack-quant/statements-analytics/src/analysis/valuation/corporate.rs) reuses one evaluated statement result and varies DCF parameters only. It does not rerun operating forecasts for each capital-cost shock. It accepts the same DCF monetary and terminal-value requirements as [01](01-corporate-dcf.md), but exposes neither market context nor an explicit as-of argument; baseline and shocks use the DCF date default.

## Audited algorithm, controls and conventions

1. Evaluate the original model once without market data. Compute baseline enterprise value from cached results.
2. Set `b=abs(wacc_sensitivity_bump)`, default `.01` (100 bp), and `epsilon=max(wacc_denominator_epsilon,0)`, default `.005` (50 bp). WACC up is `w+b`; down is `max(w−b,g+epsilon,epsilon)` for growth models and `max(w−b,epsilon)` for exit multiples.
3. Gordon and H-model stable-growth shocks are `g−b` and `min(g+b,w−epsilon)`. The growth-up clamp only uses the denominator ceiling, not the separate maximum-stable-growth policy, so ordinary DCF validation can still reject a sensitivity point. H-model high growth is raised if necessary to remain at least shocked stable growth; half-life is unchanged.
4. Exit-multiple shocks use absolute distance, default `1.0×`, or configured relative proportion; the down multiple is floored at zero. The observed baseline terminal metric is held fixed.
5. Recompute EV for each point and return changes from baseline. Sort entries by absolute up-minus-down span. `downside` and `upside` refer to parameter direction; for WACC the economic EV directions usually reverse. Clamp flags use an absolute `1e−12` comparison.

No finite-difference division is applied: entries are currency EV changes, not derivatives, normalized elasticity or per-bp risk. No parallel operating-model perturbation, calibration or covariance aggregation occurs.

## Audited limits, alternatives and evidence

A zero or asymmetric/clamped bump can make ranges unsuitable for comparing local sensitivity slopes. With a `.05` stable-growth cap, an otherwise valid baseline near the cap can fail on its requested growth-up shock. This is observed control flow; its numerical consequences were not experimentally measured. Market-dependent capital-structure statements may require the market-aware evaluation available in direct DCF but absent from this helper.

Colocated `dcf_sensitivity_ranks_wacc_and_growth_as_deltas`, `dcf_sensitivity_clamps_shocks_against_the_growth_denominator` and `dcf_sensitivity_shocks_the_exit_multiple` cover internal repricing and clamp semantics. They do not provide independent economic sensitivity calibration. Supported absolute/relative multiple shocks differ from unimplemented central derivatives or joint scenario reforecasting.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
