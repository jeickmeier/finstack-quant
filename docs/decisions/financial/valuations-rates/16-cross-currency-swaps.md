# VALUATIONS-RATES-16-CROSS-CURRENCY-SWAPS: Cross-currency coupons and resetting notionals

- ID: VALUATIONS-RATES-16-CROSS-CURRENCY-SWAPS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

Cross-currency swaps project each floating leg in its native currency, discount with that leg's curve, then convert PV at valuation-date FX. Principal exchange is None, Final, InitialAndFinal, or MtMResetting; the constructor selects InitialAndFinal. Resetting notionals use deterministic covered-interest-parity forwards. Stochastic FX/rate covariance is not calculated.

## Algorithm and controls

1. Require different leg currencies, finite positive notionals and nonnegative payment/reset lags. Separate fixing calendars and simple-average compounding are unsupported. Explicit `allow_calendar_fallback=true` permits missing/unresolvable calendars to become weekends-only; otherwise they error. Simple overnight identifiers are rejected by shared validation.
2. Build each leg schedule from its actual tenor, stub, day count, payment and reset lags. For simple coupons, reset strictly before valuation requires the exact fixing; same-day/future resets project from the forward curve. Compounded coupons delegate daily observations, lookback/lockout handling and exposures to the shared overnight projector, after adjusted accrual boundaries.
3. Coupon is `side*N*((compound_factor-1)+spread_decimal*alpha)`; the spread is simple outside compounding. Constant-notional principal uses the opposite sign at start and coupon-side sign at end. Include only payments strictly after valuation, discount native cashflows and convert their PV at as-of FX. Neumaier accumulation reduces cancellation.
4. MtM requires common raw start/end dates but permits distinct coupon schedules. Orient FX as constant currency per unit resetting currency. Future FX is `X(t)=X(as_of)*D_reset(as_of,t)/D_constant(as_of,t)` and resetting notional `N_reset(t)=N_constant/X(t)`.
5. Historical resets needed by an unpaid coupon, future rebalancing or terminal redemption require exact FX observations. At initial start equal to valuation an absent fixing uses the supplied resetting notional. Irrelevant settled periods may use the supplied notional only as bookkeeping. Later valuation-date resets use the forward formula. Future notional changes create principal rebalancing flows with the initial-principal sign; the final flow returns the last resetting notional.
6. Initial-reset historical/same-day selection and fixing lookup use the raw common contractual start. A future initial CIP reset instead uses the constant leg's adjusted start, even if the resetting leg adjusts differently. Subsequent resetting notionals/rebalancing use the resetting period's raw accrual start. MtM PV discounts each resulting native flow then converts at as-of FX. Projected FX and rate observations are included in cashflow metadata.

## Risk and limitations

Registered risks are generic FX01, combined rates DV01, per-curve PV01, triangular bucketed DV01 and mixed FX/rates cross-gamma; their finite-difference kernels belong to valuations-core. The actual curve IDs and currency FX dependencies determine the factors. The constant-notional PV branch warns on forwards below -5% without rejection; this warning is absent from MtM projection. MtM discount factors and FX resets must be finite positive. Missing FX paths error. The common helper's curve conventions remain material dependencies.

## Alternatives and tradeoffs

Native discount plus spot conversion is the implemented valuation basis; payment-date conversion with a consistent reporting discount curve is an equivalent reconciliation only under matching curve/FX assumptions. A stochastic reset adjustment or collateral-currency/basis model is not selected here. Exact historical resets avoid replacing contractual state with current spot.

## Evidence

- [XCCY projection and PV](../../../../finstack-quant/valuations/rates/src/xccy_swap/types.rs), [MtM reset engine](../../../../finstack-quant/valuations/rates/src/xccy_swap/pricing_mtm.rs), [metric selection](../../../../finstack-quant/valuations/rates/src/xccy_swap/metrics.rs).
- [CIP invariance](../../../../finstack-quant/valuations/rates/tests/xccy_swap/cip_invariance.rs), [cashflow reconciliation](../../../../finstack-quant/valuations/rates/tests/xccy_swap/reconciliation.rs), [pricing tests](../../../../finstack-quant/valuations/rates/tests/xccy_swap/pricing.rs): independent formula/invariant checks where specified by each test; not a broad market benchmark.
- [MtM-reset reference](../../../REFERENCES.md#mtm-resetting-cross-currency-swaps) and [Fujii, Shimada and Takahashi](../../../REFERENCES.md#fujii-shimada-takahashi-2010).

## Evidence limits

No stochastic FX/rate reset-convexity validation is established.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
