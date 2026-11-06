# MODELS-MONTE-CARLO-18: Barrier observation, Brownian-bridge crossing and rebate timing

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in payoff/barrier.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Barriers support up/down knock-in/out, inclusive endpoint touch, optional rebate, and explicit discrete step lists or continuous monitoring from a selected start step. Discrete steps are sorted/deduplicated. The bridge formula assumes positive lognormal spot/barrier and uses supplied scalar volatility unless path variance is available, in which case sqrt(current endpoint variance) is used.

## Algorithm and controls

Before monitoring starts there is no hit detection. At the start event inspect only the endpoint. Later continuous intervals first test endpoint crossing, then use `p_hit=exp(-2 ln(S_old/B)ln(S_new/B)/(sigma² dt))`, returning zero when the exponent is below −20 and when dt or sigma is nonpositive. A dedicated uniform determines crossing; no barrier displacement is applied despite the exported continuity-correction constant. Each interval takes its own grid dt. Invalid supplied path variance errors. Historical breach seeds survive reset.

At maturity, active options pay ordinary intrinsic. Inactive options pay the rebate (default zero). Optional hit-time rebate handling grows the rebate by `exp(rate*(T-hit_time))` so a terminal scalar discount recovers earlier payment value; the hit time is the detected interval endpoint, not an exact sampled first passage. Knock-in expiry rebates are not accelerated without a recorded hit. LRM is permitted only for discrete monitoring with no step-zero observation.

## Limitations and alternatives

Endpoint variance is a local bridge approximation under Heston, not an exact stochastic-volatility crossing law. Tiny crossing probabilities are dropped. Constructor fields do not enforce all positive/lognormal domains. Discrete-only monitoring and independent continuity-shift pricing are alternatives; combining a shifted barrier with this bridge would double-correct.

## Evidence

[Implementation/tests](../../../../finstack-quant/models/src/monte_carlo/payoff/barrier.rs) include unshifted-bridge, irregular-dt, initial-touch, discrete-observation, path-variance, zero-volatility and rebate tests. These are mostly formula/contract examples; no broad independent MC barrier price benchmark is claimed. [Broadie–Glasserman–Kou](../../../REFERENCES.md#broadie-glasserman-kou-1997) concerns a related continuity correction and must not be mistaken for this sampled bridge; [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) is the bridge reference family.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
