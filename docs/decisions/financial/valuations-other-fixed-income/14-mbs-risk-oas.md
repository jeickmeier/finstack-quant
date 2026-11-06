# VALUATIONS-OTHER-FIXED-INCOME-14: MBS effective rate risk and Monte Carlo OAS

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

MBS duration/convexity reproject cashflows after rate shocks, while OAS fits a spread on stochastic Hull–White paths. The common coefficient `β=ln(2)/.01` encodes a doubling/halving rule for a 100 bp move; source comments give a rule-of-thumb rationale, not pool calibration. These are separate approximations and have not been adopted as one calibrated borrower model.

## Algorithm and selection

**Effective risk:** [duration.rs](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/duration.rs) reprices at zero and ±25 bp by default. A positive finite shock is required. For any rate-risk rebuild, infer one rate move from the DF change at remaining WAM: `Δr=(ln DF_base − ln DF_bumped)/T`. Multiply the PSA speed or CPR-like model parameter by `exp(−βΔr)`; bounded probability variants clamp to `[0,1]`. Rebuild and price the pool. Report `(P_down−P_up)/(2P0h)` and `(P_up+P_down−2P0)/(P0h²)`; near-zero base PV yields zeros. Bucketed DV01 uses the same WAM-endpoint response to a local curve bump.

**OAS:** [mc_oas.rs](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/mc_oas.rs) does the following:

1. Convert the clean quote per 100 current face to dirty money with settlement accrual and a quote-basis pool excluding seller-owned delayed P&I.
2. Fit the Hull–White drift to the discount curve using caller scalar mean reversion/sigma or defaults `.05` and `.01`; a partial pair or sigma schedule is rejected. No swaption-volatility calibration occurs. Default paths are 512; deterministic seed comes from instrument/model configuration.
3. Generate Philox substream paths on a monthly `dt=1/12` grid. Each transition is `r_next=r exp(−κdt)+theta(t)(1−exp(−κdt))+step_stddev z`.
4. At each month, multiply the **base SMM**, not CPR, by `exp(−β(r−r0))`, clamp to `[0,.9999]`, and use the shared level-pay pool step. Discount with accumulated left-endpoint `exp(−(r+oas)dt)` and an additional same-rate factor for the exact agency payment-date offset on the curve day count.
5. Reuse the same simulated paths for every candidate spread. Average path PVs and solve average minus dirty target with Brent, tolerance `1e-7`, 200 iterations, bounds `[-.10,.20]`, initial bracket size `.05`. Return decimal OAS; convergence failure is a validation error.

## Scope, conventions and limitations

Default pricing remains deterministic; requesting OAS selects this MC calculation. The duration path scales CPR/speed and the OAS path scales SMM, so their refinancing responses differ despite sharing β and source comments calling them consistent. Exponential response, hard SMM cap, monthly left-endpoint rate integration and frozen-rate payment-delay adjustment are material approximations. OAS reports no path standard error or confidence interval; its root tolerance is not a statistical price-accuracy guarantee. Native parallel and WASM sequential paths retain deterministic path aggregation order.

## Alternatives and tradeoffs

Deterministic z-spread is cheaper but holds projected prepayments fixed; the registered OAS deliberately uses stochastic rates. An empirical S-curve with burnout and calibrated mortgage-refinancing spread is an unimplemented alternative. Retaining monthly paths is efficient, while finer integration and joint delay-period simulation could reduce discretization bias.

## Evidence

Colocated tests include `payment_offset_uses_the_curve_day_count`, `test_mc_oas_deterministic_with_seed`, `mbs_clean_quote_reprices_at_zero_oas_in_zero_vol_limit`, and `mc_oas_is_stable_across_the_in_flight_payment_date`. The zero-vol comparison allows 0.5% relative PV error; it is a limiting-case comparison to the deterministic kernel, not a calibrated-market benchmark. [Metric tests](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/mod.rs) verify OAS dispatch and override plumbing. [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives), [Philox](../../../REFERENCES.md#salmon-2011-philox) and [SIFMA](../../../REFERENCES.md#sifma-mbs-standard-formulas) are reference context.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
