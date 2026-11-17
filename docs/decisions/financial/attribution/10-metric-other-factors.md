# ATTRIBUTION-010: Metric spot, FX, volatility, inflation and mixed interactions

- Decision status: Accepted 2026-11-16 for checked metric spot, FX, volatility, inflation and mixed-factor P&L.
- Implementation status: Observed for the source scope listed below. Runtime, producer, arithmetic and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked spot units, directed and unequal FX moves, native Normal volatility, CPI growth, dividend yield, mixed terms and small Money amounts. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked metric spot, FX, volatility, inflation and mixed-factor P&L

1. Use absolute quote moves for spot delta and gamma. Match the exact underlying and quote units. With `m = S1 − S0`, calculate signed `Delta × m` and, when elected, `0.5 × Gamma × m²`. A zero delta leaves matching gamma eligible. Multiple underlyings require qualified per-source rows or a registered sensitivity on the complete matched direction. Retire first-scalar selection and preserve distinct spot, forward, futures and hedge-delta bases. [Accepted option sensitivities](../valuations-framework/option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).
2. Apply FX01 to its full directed experiment. Measure a directed quote move as `100(q1/q0 − 1)` in relative percentage points. Match the opening FX01 scope, eligible quote authority, lookup policies and authorized dependent transformations. Convert a reverse quote by the reciprocal transformation. A `+10%` direct move corresponds to about `−9.09%` in the reciprocal. Unequal moves across independently shocked pairs require elected compatible per-pair risk or a typed failure. Keep native pricing FX separate from [reporting FX](13-target-currency-and-scaling.md).
3. Normalize volatility by the producer's increment. With native annual sigma move `Δσ` and reporting increment `q`, set `m = Δσ/q`. Apply `Vega × m` and elected `0.5 × Volga × m²` on their matched directions. Black and shifted Black use `q = .01`. Normal uses an explicit increment in the underlying's units. Qualified node or per-source Vegas can explain unequal moves. Whole-scope Volga requires a match across its full direction. Preserve source clocks and distinguish scalar, grid and quote-replay risk. Retire reference-strike inference and universal `×100` normalization. [Accepted volatility grid rules](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).
4. Measure inflation on the original projected growth axis. Match the CPI index, original pillars and origin, source clock, and rebasing and seasonal basis. For verified annual decimal amplitude `d`, apply `Inflation01 × 10000 × d` and elected `0.5 × InflationConvexity × d²`. The convexity is already monetary. Use complete elected per-coordinate rows where supported. Retire sampled tenor and partial curve means. Published CPI releases and revisions require their own qualified effects. [Accepted inflation risk](../valuations-rates/13-inflation-swaps.md#accepted-decision-for-checked-inflation-swap-cashflows-and-risk).
5. Elect mixed pairs and apply each once. Keep Rates×Credit, Rates×Vol, Spot×Vol, Spot×Credit, FX×Vol, FX×Rates and Credit×Vol as the seven eligible families. Require a registered producer for the selected product and both complete observed directions on a compatible opening basis. Apply `K × a × b` once, with `a` and `b` in the producer's reporting units. Preserve applicable replay and held-input policies. Reject reversed duplicates and overlap with curvature already counted. Retire automatic Vanna substitution, mean-move proxies and borrowing the closing-marginal correction from Parallel attribution. [Accepted cross-factor risk](../valuations-framework/cross-factor-risk.md#accepted-decision-for-checked-cross-factor-risk).
6. Admit continuous-yield dividend risk explicitly. For a qualified `Dividend01`, multiply the signed holder sensitivity by `10000(q1 − q0)`, with `q` the actual annual decimal yield. Hold discrete cash events fixed and report the dividend contribution under Market Scalars. Discrete dividend events, CPI releases and correlation changes are outside this metric package. When required for complete coverage they need qualified effects under a separately elected repricing method. Request only metrics the selected calculation consumes. [Accepted dividend risk](../valuations-equity-and-composition/08-selected-risk-and-implied-vol.md#accepted-decision-for-checked-selected-model-equity-risk-and-implied-volatility) · [Accepted restoration scopes](02-market-factor-restoration.md#accepted-decision-for-checked-economic-factor-families-and-market-restoration).
7. Fail missing required inputs and publish checked working. Use checked scaled products and Money totals that reconcile to detailed rows. Preserve genuine zeros and small finite terms. Retire the `1e−12` mixed-term cutoff, five-vol-point warning threshold and zero sentinels. Elected advisory detail may be unavailable with a typed cause and stays outside the additive total. Publish matched axes, units, moves, producer identities and signed components through Rust for Python and WASM. Residual tolerance establishes neither coverage nor approximation accuracy. [Accepted execution](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints) · [Accepted arithmetic and diagnostics](09-metric-rates-and-credit.md#accepted-decision-for-checked-metric-rate-and-credit-pl).

Exact examples verified `+45` of P&L from independent FX-pair sensitivities `(2, 5)` with moves of `(+10%, +5%)`. Applying the first move to joint sensitivity `7` gives `+70`. A Normal sigma move from `0.0100` to `0.0105` at increment `0.0001` gives five units, `+40` of first order and `+25` of curvature for the example sensitivities.

## Observed choice and algorithm

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

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
