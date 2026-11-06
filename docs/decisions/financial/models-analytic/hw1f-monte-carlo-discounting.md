# MODELS-ANALYTIC-25: HW1F event grid, antithetic streams and bank-account estimator

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Rate-dependent payoffs implementing Payoff; supplied HW1F process parameters, initial rate, positive increasing event times and currency. Payoff returns time-zero path value using supplied bank-account state.

## Choice and rationale

Exact marginal HW rate stepping is combined with trapezoidal bank-account integration. Antithetic partners are one estimator for standard-error purposes; this does not make the integrated-rate discount exact.

## Algorithm

For each event gap, subdivide n=max(min_steps,ceil(12gap)) and hit event exactly; min_steps floors1. Optional terminal segment extends to maturity. For each base path use Philox(seed).substream(path_id); antithetic replays identical stream with z sign reversed. Step short rate with ExactHullWhite1F, update bank by exp[.5(rstart+rend)dt], expose r and bank, invoke payoff.on_event then callback. Collect payoff.value at end. Average complete antithetic pairs before OnlineStats; with OOS mode only odd base streams contribute; construct MoneyEstimate from independent estimator stats.

## Selection and controls

Embedded JSON defaults:10000 base paths,seed42,antithetic=true,min_steps4,basis_degree2; oos_lsmc=false in Rust config. Overrides floor supplied paths1 and always derive seed from instrument ID/scenario (default base), replacing configured seed. At least2 independent pricing estimators required; multiplicity only1/2 and complete pairs enforced. `accumulate_bank_factors` exposes the same trapezoid accumulator separately.

## Conventions

Rate decimal per year; bank starts1, represents accumulated money-market account. Callback payoffs own conversion to PV; harness aggregation uses scale1, avoiding another discount. Paths count base streams; simulated paths double with antithetics.

## Limitations, failure and fallback

Trapezoid bank integration retains time-discretization bias. OOS mode on nonexercise MC still discards half streams. Low-level bank accumulator trusts path/grid length compatibility. Input process/r0/payoff finite-value contracts rely partly on downstream components. Standard error measures sampling error, not grid/parameter/model error.

## Alternatives and tradeoffs

Exact joint short-rate/integral sampling would reduce discount discretization error but is not used here. Independent or antithetic streams are supported; QMC is not selected by this harness.

## Implementation and validation evidence

[mc.rs](../../../../finstack-quant/models/src/rates/hw1f/mc.rs), [bank_account.rs](../../../../finstack-quant/models/src/rates/hw1f/bank_account.rs), [config.rs](../../../../finstack-quant/models/src/rates/hw1f/config.rs) and [embedded defaults](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json). Constant-rate bank exponential and pair-aggregation/singleton tests establish deterministic/statistical bookkeeping; full stochastic grid convergence is separate evidence. Reference: [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
