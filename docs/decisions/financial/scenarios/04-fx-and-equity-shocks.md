# SCENARIOS-004: FX percentage shocks, cross-rate diagnostics and price scalars

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: explicit FX pair and price-scalar operations scale the selected quote by `1+pct/100`. They do not translate a currency-wide macro shock into a globally consistent FX system or generate factor covariance. Consecutive percentages compound, for example two -60% price changes retain 16% of the original level.

## Domain and ordered algorithm

FX pairs require distinct currencies and finite `pct>-100`. Emit `MarketBump::FxPct` with the execution as-of date; the core context converts percentage to decimal before calling the FX matrix. Before emission, when an FX matrix has triangulation enabled, create the corresponding bumped matrix for diagnostics. Iterate its original explicit quotes touching either shocked currency, excluding identity and pivot-leg pairs. Compare the direct rate to `rate(from,pivot)*rate(pivot,to)` at the scenario date. Emit a warning when the absolute difference exceeds `1e-8*max(1,|direct|,|implied|)`. Missing direct/pivot query results are skipped. Serialization or bump-preview failures propagate.

The diagnostic does not alter other direct quotes, choose an arbitrage-free spanning tree, or certify every cross. With no matrix, effect generation still emits the bump and the core application decides the missing-data error. With triangulation disabled, no consistency diagnostic runs.

Price-scalar operations first look up each exact ID in the price collection, then emit `PriceBump`; the effect path uses collection-specific in-place percentage scaling. Missing IDs produce `EquityNotFound` warnings. `MarketScalar::Price` retains currency and `Unitless` retains scalar representation. The operation accepts multiple IDs in order; duplicate IDs create repeated compounded effects. Exactly -100% is valid for prices and yields zero; the FX lower boundary is strictly positive.

## Controls, alternatives and evidence

FX snapshot/quote resolution, reciprocal handling and checked scaling are shared core behavior documented in [FX resolution and shocks](../core-conventions/013-fx-resolution-and-shocks.md). The engine's typed price path prevents an identically named curve from receiving the equity shock.

Owning source: [adapters/fx.rs](../../../../finstack-quant/scenarios/src/adapters/fx.rs), [adapters/equity.rs](../../../../finstack-quant/scenarios/src/adapters/equity.rs), [effects.rs](../../../../finstack-quant/scenarios/src/engine/effects.rs). Inspected FX tests construct EUR/USD, USD/JPY and EUR/JPY and assert warnings at implied 198 after +10% and 171 after -5%; these are arithmetic/diagnostic fixtures. [Namespace regressions](../../../../finstack-quant/scenarios/tests/senior_review_market_regressions.rs) locate colliding-ID protection. No independent market scenario calibration is established.

A currency-factor shock followed by coherent cross reconstruction is an alternative economic definition; rejecting inconsistencies is another. The present warning-only cross policy is observed, not approved.

## Relationship to the accepted generic scalar boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) makes MarketScalar
construction checked and generic transforms native Additive/Fraction or literal
Multiplicative/Factor. This scenario's explicitly relative price operation
resolves its percentage at the financial owner and sends the checked intended
multiplier; it must not depend on generic Percent changing meaning between
Money and numeric storage. Preserve accepted Money Decimal admission/arithmetic
and typed FX ownership. Record12 was accepted on **2026-11-08**, with
reconciliation/qualification pending; this note does not adopt the
remaining scenario diagnostic/targeting choices above.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
