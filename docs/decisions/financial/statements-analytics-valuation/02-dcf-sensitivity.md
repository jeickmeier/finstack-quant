# STATEMENTS-ANALYTICS-VALUATION-02: Cached-statement DCF parameter sensitivities

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice and input domain

[`dcf_sensitivity`](../../../../finstack-quant/statements-analytics/src/analysis/valuation/corporate.rs) reuses one evaluated statement result and varies DCF parameters only. It does not rerun operating forecasts for each capital-cost shock. It accepts the same DCF monetary and terminal-value requirements as [01](01-corporate-dcf.md), but exposes neither market context nor an explicit as-of argument; baseline and shocks use the DCF date default.

## Algorithm, controls and conventions

1. Evaluate the original model once without market data. Compute baseline enterprise value from cached results.
2. Set `b=abs(wacc_sensitivity_bump)`, default `.01` (100 bp), and `epsilon=max(wacc_denominator_epsilon,0)`, default `.005` (50 bp). WACC up is `w+b`; down is `max(w−b,g+epsilon,epsilon)` for growth models and `max(w−b,epsilon)` for exit multiples.
3. Gordon and H-model stable-growth shocks are `g−b` and `min(g+b,w−epsilon)`. The growth-up clamp only uses the denominator ceiling, not the separate maximum-stable-growth policy, so ordinary DCF validation can still reject a sensitivity point. H-model high growth is raised if necessary to remain at least shocked stable growth; half-life is unchanged.
4. Exit-multiple shocks use absolute distance, default `1.0×`, or configured relative proportion; the down multiple is floored at zero. The observed baseline terminal metric is held fixed.
5. Recompute EV for each point and return changes from baseline. Sort entries by absolute up-minus-down span. `downside` and `upside` refer to parameter direction; for WACC the economic EV directions usually reverse. Clamp flags use an absolute `1e−12` comparison.

No finite-difference division is applied: entries are currency EV changes, not derivatives, normalized elasticity or per-bp risk. No parallel operating-model perturbation, calibration or covariance aggregation occurs.

## Limits, alternatives and evidence

A zero or asymmetric/clamped bump can make ranges unsuitable for comparing local sensitivity slopes. With a `.05` stable-growth cap, an otherwise valid baseline near the cap can fail on its requested growth-up shock. This is observed control flow; its numerical consequences were not experimentally measured. Market-dependent capital-structure statements may require the market-aware evaluation available in direct DCF but absent from this helper.

Colocated `dcf_sensitivity_ranks_wacc_and_growth_as_deltas`, `dcf_sensitivity_clamps_shocks_against_the_growth_denominator` and `dcf_sensitivity_shocks_the_exit_multiple` cover internal repricing and clamp semantics. They do not provide independent economic sensitivity calibration. Supported absolute/relative multiple shocks differ from unimplemented central derivatives or joint scenario reforecasting.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
