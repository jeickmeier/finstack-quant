# ATTRIBUTION-010: Metric spot, FX, volatility, inflation and mixed interactions

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and algorithm

Each available opening sensitivity multiplies its specifically normalized observed move; unavailable sensitivities generally leave zero. This route has no general correlation-repricing factor and no dividend-specific attribution stage despite Dividend01 being in the default requested metric menu.

- Spot uses only the first declared scalar and absolute price change: `Delta*dS + 0.5*Gamma*dS²`. Gamma is applied only when Delta exists. Missing primary movement with Delta flags invalid. Additional underlyings do not receive per-source risk allocation.
- FX uses joint `Fx01` in money per 1% move times the percentage-point change of the instrument's primary FX pair. Missing measurable pair yields zero with a note. Multiple declared pairs add an approximation note because differential pair movements are not resolved. Reporting-currency translation is separate.
- Volatility uses Vega in money/vol point times measured vol-point move, plus `0.5*Volga*move²` when Vega exists. The shift contract rejects unresolved twists/multiple unequal source moves as described in record 8. Moves beyond 5 vol points add a warning, not a quantitative bound. Missing measurable shift leaves zero with a note.
- Inflation01 in money/bp multiplies the average move of declared inflation curves/indices only. Any failed declared source or an empty declaration with Inflation01 flags invalid, although available sources still contribute to the mean. Optional InflationConvexity adds `0.5*raw_dollar_second_derivative*(move_bp/10000)²`; twist diagnostics are notes.
- Seven mixed metrics contribute `CrossGamma_AB*move_A*move_B`, without a half factor: Rates×Credit, Rates×Vol, Spot×Vol, Spot×Credit, FX×Vol, FX×Rates and Credit×Vol. Rate/credit axes are bp; spot/FX axes are percentage points; vol axis is vol points. Missing metric or either move omits that pair. Vanna is deliberately not substituted for Spot×Vol because its spot axis is absolute price, not percentage points. Finite pair values smaller than 1e-12 currency units are omitted; remaining pair totals use f64 addition.

## Selection, failure and limits

Nonfinite calculated money uses zero sentinel and invalid status. Several spot/inflation finiteness checks are debug assertions; native debug callers can panic while JSON execution catches panic as Internal. Rate/credit cross terms use unweighted aggregate mean moves even when first-order risk is bucketed. There are seven metric pairs versus nine default repricing pairs; source comments saying they are the same set are stale. Inflation/rate and correlation/credit interactions are absent here, along with higher-order/time interactions.

## Rationale and alternatives

Explicit units prevent spot-price and percentage normalization errors; omitted decomposition remains residual rather than invented detail. Supported alternatives are Taylor or full repricing. Located tests verify primary spot selection, spot/credit vol cross units, Vanna non-substitution, declared inflation dependencies and nonfinite sensitivity handling. These establish targeted contracts, not broad pricing accuracy.

## Source and evidence

- [metrics_based/equity.rs](../../../../finstack-quant/attribution/src/metrics_based/equity.rs) — owning implementation.
- [metrics_based/fx.rs](../../../../finstack-quant/attribution/src/metrics_based/fx.rs) — owning implementation.
- [metrics_based/volatility.rs](../../../../finstack-quant/attribution/src/metrics_based/volatility.rs) — owning implementation.
- [metrics_based/inflation.rs](../../../../finstack-quant/attribution/src/metrics_based/inflation.rs) — owning implementation.
- [metrics_based/cross_factor.rs](../../../../finstack-quant/attribution/src/metrics_based/cross_factor.rs) — owning implementation.

## Relationship to the accepted comparable-measurement package

[CORE-MARKET-22's accepted package](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows)
was accepted on **2026-11-08**. It preserves exact native monetary movements, explicit directed
FX queries/per-pair changes and distinct projected-CPI annual-growth versus
observed-period level quantities. Require actual producer quantity/coordinate
pairing; equal bp or percent labels do not identify one factor, and required
source failure cannot become a successful partial mean. This does not adopt
primary-underlying/pair proxies, joint sensitivity allocation, mixed-gamma
approximations or remaining inflation/volatility/status methodology.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
