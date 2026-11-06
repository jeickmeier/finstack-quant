# MODELS-MONTE-CARLO-19: Lookback extrema and conditional lognormal bridge sampling

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in payoff/lookback.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Fixed-strike calls/puts use maximum/minimum spot; floating-strike calls/puts use terminal spot against minimum/maximum. Historical extrema are optional. The default monitoring label is `Continuous`, but without an enabled bridge it only inspects simulated nodes. `with_continuous_monitoring(vol)` explicitly enables constant-volatility bridge extrema; `with_monitoring(...)` clears the bridge.

## Algorithm and controls

At each observed event update the extreme. For bridge sampling store previous time/log spot and draw an interval extremum: `exp((logS_old+logS_new ± sqrt((logS_new-logS_old)²-2 sigma² dt ln(U)))/2)`. Plus samples maximum, minus minimum; U is floored at MIN_POSITIVE and must be in [0,1]. Zero variance selects endpoint extrema deterministically. Times must not decrease and lognormal spots must be positive. Reset restores history and clears bridge state. Fixed payoff uses positive part against strike; floating payoff uses positive part of terminal minus minimum (call) or maximum minus terminal (put).

## Limitations and alternatives

The bridge is exact for the conditional constant-volatility log process, not general local/stochastic-volatility paths. Empty/no-fired observations leave infinite sentinel extrema that may error in value construction. The default label alone does not promise continuous monitoring; node and bridge sampling have different contract meanings. LRM excludes the bridge and time-zero observations. Alternatives include analytic lookbacks or finer discrete monitoring, with different contract meanings and errors.

## Evidence

[Implementation/tests](../../../../finstack-quant/models/src/monte_carlo/payoff/lookback.rs) cover fixed-path extrema, seasoned values, discrete observation, bridge reset and `continuous_lookback_grid_and_seed_convergence_meets_one_bp_budget` (located numerical convergence evidence). [Conze–Viswanathan](../../../REFERENCES.md#conze-viswanathan-1991), [Goldman–Sosin–Gatto](../../../REFERENCES.md#goldman-sosin-gatto-1979) and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) supply analytical/bridge context. Accuracy of nonconstant-volatility use is not established.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
