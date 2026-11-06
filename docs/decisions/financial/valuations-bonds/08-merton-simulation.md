# VALUATIONS-BONDS-08: Structural GBM default simulation, PIK elections and loss outputs

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and supported domain

`MertonMc` is explicit and requires `merton_mc_config`. Bond adaptation rejects floating and amortizing specs and all call/put/return-floor rights. Fixed coupons are supported; StepUp is accepted but every coupon uses its initial rate, an explicit approximation. Custom schedule amounts are not replayed: the adapter extracts dates/accrual factors and scalar coupon/notional, so a custom fixed placeholder can lose its actual coupon economics. This is not general custom-schedule valuation.

## Algorithm and controls

Use `ceil(T·steps_per_year)`, at least one step, with `dt=T/n`; embedded defaults are 5,000 independent estimators, antithetic on, 12 steps/year. PCG64 streams are indexed by seed and estimator. Per path draw all Gaussian steps, bridge uniforms and coupon-toggle uniforms. Antithetic legs negate Gaussians and complement bridge uniforms; toggle uniforms are reused.

Evolve `V_next=V exp[(r_model−payout−σ²/2)dt+σ√dt Z]`. The drift uses `config.merton.risk_free_rate`, not the separate `discount_rate` argument (some surrounding docstrings say otherwise). Terminal default occurs only at final step if `V<B`. First-passage uses growing barrier `B exp(g t)`; endpoint crossing defaults, or Brownian-bridge probability `exp(−2 log(V_prev/B_prev) log(V/B)/(σ²dt))` when both log distances are positive. Constructor default is bridge for first-passage and discrete for terminal. Recovery is paid at the detecting step end on then-current notional; dynamic recovery overrides flat recovery. Default is checked before coupon processing, so intrastep coupon/default ordering is an approximation.

Process coupons when step time reaches coupon time minus half a step; discount at the exact coupon time. Coupon is current notional times scalar coupon rate times contractual accrual. Cash pays it, PIK capitalizes it, Split divides it, and Toggle delegates a credit-state decision; absent toggle model falls back to cash. State contains leverage, coupon due, asset value, distance to default and either endogenous leverage hazard or equivalent Merton PD hazard. Endogenous hazard affects this toggle state only; structural barrier default is unchanged. PIK changes recovery/redemption notional but does not itself change the configured debt barrier. Default Uniform(Cash) config is replaced by the bond's cash/PIK type; a nondefault config schedule wins.

Survivors repay accreted principal. DF inputs use log-linear interpolation with endpoint-flat-zero extrapolation; absent/empty knots use `exp(−discount_rate·t)`. Registry pricing derives the fallback discount rate from the curve/horizon and populates dated curve DF knots, without changing structural-model drift.

## Outputs, fallbacks and limitations

Dirty price is mean PV/initial face×100; clean subtracts contractual accrued. Expected loss is `1−meanPV/risk_free_PV` (zero if denominator nonpositive); baseline follows PIK schedule but treats Toggle as Cash. Unexpected loss is physical-path PV sample standard deviation/face. `expected_shortfall_95` is mean of the lowest `ceil(5%·physical_paths)` PVs as percent of face, not a tail loss. Average PIK fraction counts coupon periods with any PIK, not capitalized amount fraction. Terminal notional averages survivors; when none survive it returns initial face. Default time/recovery average defaulters, otherwise zero. Standard error uses antithetic pair means and is in price percentage points.

Effective spread Newton-solves a nonnegative continuous spread on the risk-free baseline, finite-difference derivative at 1 bp, 50 iterations and residual `1e−10·face`. Nonpositive targets, nonpositive maturity or price above baseline return zero; a tiny derivative or iteration exhaustion returns the last iterate without convergence error.

## Evidence

Owning code: [engine](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/engine.rs), [bond adapter](../../../../finstack-quant/valuations/fixed-income/src/bond/types/traits.rs), [registry adapter](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/pricer.rs), [sampling/result types](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/types.rs), [config/defaults](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/merton_mc.rs), [default data](../../../../finstack-quant/valuations/core/data/defaults/product_mc_defaults.v1.json). [Tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/tests.rs) inspect bridge/terminal branches, seeded determinism, pair-mean stderr and toggle fallback; `endogenous_hazard_lowers_pik_price` only asserts nonincrease and permits equality, so does not prove hazard feedback. References: [Merton 1974](../../../REFERENCES.md#merton-1974), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). No independent structural-credit accuracy qualification was executed.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
