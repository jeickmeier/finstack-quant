# MODELS-MONTE-CARLO-20: Autocall observation ordering, coupon memory and redemption

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in payoff/autocallable.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

The path payoff supports scheduled autocall/coupon barrier ratios relative to initial spot, per-observation coupon ratios, optional memory coupons, seasoned state and three final-redemption formulas. Parallel vectors must match and observation dates must strictly increase. Positive finite event spot is required. Economic domains of all barriers, caps, payment ratios and notionals are not comprehensively checked by this constructor.

## Algorithm and conventions

For every observation reached by current time (tolerance 1e-6 years), use the current endpoint spot, update the minimum only at that scheduled observation, then process coupons before autocall. If coupon barrier is met, pay current coupon plus accumulated unpaid memory (when enabled), scaled by its `payment_df_ratio`; reset memory. Otherwise accumulate the missed coupon only in memory mode. Meeting the autocall barrier terminates observation processing and redeems principal using that same payment ratio. Final spot is frozen at the first event reaching the last observation.

Without autocall, capital protection pays `max(floor,participation*min(S_final/S_initial,cap))`; participation pays `1+participation*max(min(ratio,cap)-1,0)`; knock-in put pays `max(1-max(strike_ratio-ratio,0),0)` if the observation minimum touched initial_spot*final_barrier, otherwise one. Sum redemption and previously accumulated payment-adjusted coupon ratios, then multiply by notional. Ratios must be relative to the terminal discount convention used by the caller; final redemption is not separately adjusted here.

## Failure behavior and alternatives

A coarse step spanning several dates reuses one endpoint for all dates, and final knock-in monitoring is discrete. Unpaid memory at final maturity is not automatically redeemed. Public vector mutation can invalidate cached cursor/shape assumptions. Continuous knock-in and exact observation-grid construction are distinct alternatives, not implemented by this payoff alone.

## Evidence

[Implementation/tests](../../../../finstack-quant/models/src/monte_carlo/payoff/autocallable.rs) cover coarse steps, discrete knock-in, ratio strikes, cap behavior, coupon memory without autocall and contractual payment ratios. These are contractual examples, not an external vendor validation. [Overhaus](../../../REFERENCES.md#overhaus-2007-equity-derivatives) is broad product background; a canonical term-sheet/adoption record for each final-payoff formula is absent.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
