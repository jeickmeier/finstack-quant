# MODELS-MONTE-CARLO-17: Terminal payoffs and weighted historical Asian fixings

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in payoff/vanilla.rs, payoff/asian.rs and payoff/mod.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Terminal European, digital and forward payoffs store spot at an explicit maturity step and return undiscounted currency-tagged amounts. European uses notional times call/put positive part; digital pays only for strict S>K or S<K (zero at equality); forward is signed notional*(S−K). All require finite named spot at the relevant event. Constructors do not comprehensively validate strike/notional; engine maximum-step validation prevents events beyond its grid.

## Asian algorithm and controls

Default fixing indices are 1..=num_steps, excluding valuation spot. Arithmetic averaging uses compensated Kahan summation; geometric averaging stores a log product. Historical sum/log-sum and count are retained across resets. Repeated fixing steps remain repeated economic observations: a hash-map entry stores multiplicity, sum of positive fixing multipliers and sum of their logs. Arithmetic contribution is spot*sum(multiplier); geometric contribution is count*ln(spot)+sum(ln(multiplier)). Divide accumulated sum/log-sum by total historical+observed count, then apply call/put intrinsic and notional. Empty future fixing schedules are allowed only with history. Nonpositive geometric spot sets log sum to minus infinity, hence zero average.

## Failure behavior and alternatives

There is no validation that every fixing has actually fired before `value()`, and partial path snapshots intentionally produce partial averages. Historical counts/aggregates and public mutation of fixing steps/averaging can become inconsistent with cached weights. Low-level LRM support excludes step-zero fixings because payoff dependence on initial spot must be density-only. Weighted economic averaging is implemented as scaled observations divided by count, not normalized weight sum. Analytical geometric Asian controls are alternatives, not automatically used by this payoff or the default Asian convenience pricer.

## Evidence

[Vanilla](../../../../finstack-quant/models/src/monte_carlo/payoff/vanilla.rs), [Asian](../../../../finstack-quant/models/src/monte_carlo/payoff/asian.rs), [finite-state contract](../../../../finstack-quant/models/src/monte_carlo/payoff/mod.rs); located tests cover fixed paths, strict digital payout, repeated fixing scales, history/reset and geometric closed-form comparison. [Kemna–Vorst](../../../REFERENCES.md#kemna-vorst-1990) is geometric-Asian background; it does not independently validate contract-specific schedules and multipliers.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
