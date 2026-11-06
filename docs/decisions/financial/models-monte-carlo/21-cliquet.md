# MODELS-MONTE-CARLO-21: Cliquet local/global clamps and seasoned return aggregation

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in payoff/cliquet.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice, domain and algorithm

Cliquet observes strictly increasing reset times and positive finite event spot. At every reached reset (1e-6-year tolerance), append the current spot. Compute consecutive simple returns from initial spot, clamp each to local floor/cap, then either sum from the prior locked sum or multiply `(1+return)` from prior locked growth. Apply global floor/cap to the resulting total return; final long-call payoff additionally floors the result at zero and multiplies notional. No principal redemption is included. Additive is the default payoff type; multiplicative is explicit.

## Selection, failure and alternatives

Coarse grid crossings reuse one endpoint at multiple resets. Caps/floors are checked for ordering, but finite dates/levels/initial spot and seasoned values are not fully validated. If no reset spots have been seen, `compute_return` returns zero before considering prior locked history, so a fully seasoned/no-future-reset payoff loses prior accumulated return. Public payoff type and scalar fields are caller-controlled. Alternatives include contractual per-reset payment, exact reset-aligned grids and principal-inclusive redemption, none implicitly added here.

## Conventions and evidence

Caps/floors and returns are decimal fractions; notional scales payoff; times are years and scalar engine discounting handles payment horizon. [Implementation/tests](../../../../finstack-quant/models/src/monte_carlo/payoff/cliquet.rs) locate local/global clamp, coarse-step, negative-global-floor and reset tests. These specify examples; independent seasoned contract validation is missing. [Overhaus](../../../REFERENCES.md#overhaus-2007-equity-derivatives) is product context only.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
