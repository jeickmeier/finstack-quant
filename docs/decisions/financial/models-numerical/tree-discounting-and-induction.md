# MODELS-NUMERICAL-003: Tree discounting, OAS conversion and backward induction

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and algorithm

[tree_framework.rs](../../../../finstack-quant/models/src/trees/tree_framework.rs) prices binary recombining claims with V(n,i)=df(n,i)·{p_up V(n+1,i+1)+p_down V(n+1,i)} before invoking the product callback. Terminal values come entirely from the callback. Spot nodes are S0 d^n (u/d)^i and use flat exp(−r dt); short-rate nodes and discount rates are supplied by closures. The callback owns coupon, recovery and exercise ordering beyond that continuation calculation.

`TreeDiscounting` delegates to core compounding: continuous exp(−r dt), simple 1/(1+r dt), or periodic (1+r/m)^(−m dt). Simple and periodic bases below 1e−15 are floored at that level. The inverse returns zero with a warning when |dt|<machine epsilon or DF≤0. Continuous conversion is −ln(df)/dt; degenerate dt returns the input rate and nonpositive DF logs and returns the raw rate.

## Selection, scope and conventions

[short_rate_tree/pricing.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/pricing.rs) uses equal binary probabilities for Ho–Lee/BDT and its own capped trinomial rollback for BK. OAS input is basis points. Convert the base node rate to continuous, then ask `oas_compounding.continuous_spread_shift(base,s/10000)` for the additional continuous rate. The callback receives that effective continuous OAS in basis points when the quote is noncontinuous. BK discounts by base convention DF times exp(−shift dt); binary models supply the equivalent combined continuous rate. This preserves the distinction between adding a spread to a periodic base rate and independently compounding a spread.

## Failure behavior and alternatives

Uncalibrated trees and malformed lattice geometry fail before pricing. Binary closure out-of-range zero fallbacks are guarded by geometry validation in this caller. BK skips invalid child indices; normal calibrated geometry should supply all branches. Terminal rates are populated without a post-maturity fit and may be used by terminal callbacks. Floors avoid invalid discount bases but materially alter extreme-rate economics; rejection instead is an unimplemented policy alternative. Compounding variants are supported, and their adoption per product remains separate.

## Evidence

[short-rate tests](../../../../finstack-quant/models/src/trees/short_rate_tree/tests.rs) include `tree_compounding_rate_from_df_inverts_df` (roundtrip) and `periodic_oas_is_added_to_each_node_rate_on_its_own_compounding` (independent deterministic formula). Framework state-handoff tests verify callback fields. [Hull](../../../REFERENCES.md#hull-options-futures) supports discount conventions, not the floor thresholds. Model calibration is separately recorded in records 004–007.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
