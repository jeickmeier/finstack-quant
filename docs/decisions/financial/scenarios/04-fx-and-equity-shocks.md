# SCENARIOS-004: FX percentage shocks, cross-rate diagnostics and price scalars

- Decision status: **Accepted 2026-11-15 — complete frozen FX pair shock, cross-rate advisory and checked equity scalar package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent directional FX, cross-rate, equity compounding and multiplier boundary checks completed.** Production execution, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for frozen FX and checked equity scenario shocks

1. **Apply one frozen shock to an ordered FX pair.** Require distinct
   currencies, finite percentage `p > −100` and an explicit point-date FX
   policy at the scenario's effective date. Convert `p/100` once to the
   core fractional bump. Reject a nonzero input lost to zero on conversion.
   Validate a literal zero through the same lookup and frozen admission.
   Apply the pair shock across retained dates and policies, multiplying
   the requested direction and dividing an independently supplied reverse
   from its own baseline. Hold unrelated supplied quotes fixed. Use the
   same admitted frozen result for diagnostics and publication.
   [Accepted FX shock rules](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks).

2. **Compare affected direct crosses in the reference scope.** With
   a configured pivot, check each affected directed non-pivot pair once,
   in deterministic order. Its own pair or one of its two pivot legs must
   be the shocked pair in either orientation. Compare the winning
   explicitly supplied directed rate at the reference date and policy
   with the two-leg product on the same candidate. Include eligible
   matrix and frozen provider quotes and their scoped pins. Exclude
   shadowed and synthetic fallback quotes. Use same-scope core authority
   for the pivot legs and require a finite positive product.

3. **Warn on a dimensionless discrepancy.** For direct rate `D` and
   implied cross `I`, warn when `|D−I|/max(D,I)>1e−8`. A missing or
   unrepresentable comparison has an unavailable outcome. No configured
   pivot or eligible comparison makes the check inapplicable. Preserve
   the reported post-shock observation and leave quotes untouched.
   A failed required FX lookup, freeze or application aborts the scenario.
   [Relative-difference formula](https://www.itl.nist.gov/div898/software/dataplot/refman2/auxillar/reldif.htm) ·
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution).

4. **Require exact price-qualified equity targets.** Use one typed path
   for direct and hierarchy actions. Admit Money prices and explicitly
   price-qualified numeric scalars on the required security, share and
   currency basis. Require nonnegative spot. Reject missing or wrong-kind
   targets and protected fixing keys even for zero shocks or wipeouts.
   Reject duplicates within a target list. Separate authored operations
   compound in order.
   [Accepted equity spot rules](../valuations-equity-and-composition/02-spot-and-carry.md#accepted-decision-for-checked-cash-equity-valuation-and-dividend-carry) ·
   [Accepted hierarchy binding](03-hierarchy-resolution.md#accepted-decision-for-typed-scenario-hierarchy-expansion-and-overlap).

5. **Apply a checked equity factor and report the actual effect.** Require
   finite `p ≥ −100`. Literal `−100` gives zero in the original type and
   currency. Near the lower boundary form the positive factor as
   `(100+p)/100`. Use the accepted checked Decimal multiplication for Money
   and checked finite arithmetic for numeric scalars. Carry the requested
   percent, exact target and typed before-and-after values. Distinguish
   a valid unchanged quote from a failed action. Record 017 owns the
   report representation.
   [Accepted scalar arithmetic](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) ·
   [Accepted Money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- Starting from EUR/USD `1.20` and USD/JPY `150`, a `+10%` EUR/USD
  shock gives an implied EUR/JPY `198`. A supplied direct `180` stays
  fixed and gives a relative discrepancy of `1/11`. A `−5%` shock gives
  `171` and discrepancy `1/20`.
- Starting from independent A/B `2` and B/A `.6`, a `+10%` A/B
  shock gives `2.2` and `6/11` respectively.
- Two `−60%` equity shocks produce `100 → 40 → 16`. Decimal
  `123.4567 × 1.10` is `135.80237`. The represented percentage
  immediately above `−100` gives a positive factor near `1.4211e−16`
  through the near-boundary formula.

## Observed choice and rationale at the audit revision

Observed choice: explicit FX pair and price-scalar operations scale the selected quote by `1+pct/100`. They do not translate a currency-wide macro shock into a globally consistent FX system or generate factor covariance. Consecutive percentages compound, for example two -60% price changes retain 16% of the original level.

## Domain and ordered algorithm

FX pairs require distinct currencies and finite `pct>-100`. Emit `MarketBump::FxPct` with the execution as-of date; the core context converts percentage to decimal before calling the FX matrix. Before emission, when an FX matrix has triangulation enabled, create the corresponding bumped matrix for diagnostics. Iterate its original explicit quotes touching either shocked currency, excluding identity and pivot-leg pairs. Compare the direct rate to `rate(from,pivot)*rate(pivot,to)` at the scenario date. Emit a warning when the absolute difference exceeds `1e-8*max(1,|direct|,|implied|)`. Missing direct/pivot query results are skipped. Serialization or bump-preview failures propagate.

The diagnostic does not alter other direct quotes, choose an arbitrage-free spanning tree, or certify every cross. With no matrix, effect generation still emits the bump and the core application decides the missing-data error. With triangulation disabled, no consistency diagnostic runs.

Price-scalar operations first look up each exact ID in the price collection, then emit `PriceBump`; the effect path uses collection-specific in-place percentage scaling. Missing IDs produce `EquityNotFound` warnings. `MarketScalar::Price` retains currency and `Unitless` retains scalar representation. The operation accepts multiple IDs in order; duplicate IDs create repeated compounded effects. Exactly -100% is valid for prices and yields zero; the FX lower boundary is strictly positive.

## Controls, alternatives and evidence

FX snapshot/quote resolution, reciprocal handling and checked scaling are shared core behavior documented in [FX resolution and shocks](../core-conventions/013-fx-resolution-and-shocks.md). The engine's typed price path prevents an identically named curve from receiving the equity shock.

Owning source: [adapters/fx.rs](../../../../finstack-quant/scenarios/src/adapters/fx.rs), [adapters/equity.rs](../../../../finstack-quant/scenarios/src/adapters/equity.rs), [effects.rs](../../../../finstack-quant/scenarios/src/engine/effects.rs). Inspected FX tests construct EUR/USD, USD/JPY and EUR/JPY and assert warnings at implied 198 after +10% and 171 after -5%; these are arithmetic/diagnostic fixtures. [Namespace regressions](../../../../finstack-quant/scenarios/tests/senior_review_market_regressions.rs) locate colliding-ID protection. No independent market scenario calibration is established.

The live adapter still previews its FX diagnostic on a separate capture
from the applied shock and scans matrix global quotes only. The live
equity wipeout uses a raw price insertion, bypassing the fixing-key check
used by the ordinary price bump.

## Relationship to the accepted generic scalar boundary

[CORE-MARKET-12](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings)
owns checked generic transforms. The scenario adapter converts its relative
price intent to a literal factor and retains the price source's financial
domain and accepted Money Decimal arithmetic.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
