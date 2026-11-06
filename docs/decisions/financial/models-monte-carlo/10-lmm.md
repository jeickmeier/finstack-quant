# MODELS-MONTE-CARLO-10: Displaced terminal-measure LMM predictor-corrector

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/lmm.rs and discretization/lmm_predictor_corrector.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

The LMM evolves n forward rates with two or three Brownian factors under the terminal-bond measure. Rates/displacements are decimals, accrual factors are year fractions, and tenor/fixing times are years. Inputs require positive displaced forwards and positive `1+tau*L`, finite factor loadings, consistent shapes and ordered knots. This is a terminal-measure path model, not automatic money-market discounting.

## Algorithm and controls

For each alive forward i, displaced drift rate is `-sum(j>i) tau_j*(L_j+d_j)/(1+tau_j*L_j) dot(lambda_i,lambda_j)`. Multiply by `(L_i+d_i)` for level drift. Fixing equality freezes the forward. Predict all alive displaced levels with a lognormal step using current drift; recompute drift using the predicted vector at the same left endpoint time; correct with the average of current/predicted drift rates and the same Gaussian factors. The last forward has zero terminal-measure drift. The scheme requires every fixing/volatility knot inside the horizon to occur exactly on the grid; a direct crossing step fills state with NaNs. Volatility equality selects the next period.

State metadata publishes each forward plus `lmm_numeraire=product(1+tau_i L_i)^(-1)` over remaining periods. At a fixing boundary the numeraire includes that fixing period via a strict boundary comparison, while dynamics have already frozen it. Generic scalar engine discounting does not automatically consume this value.

## Failure behavior and alternatives

Drift terms with |1+tau*L|<1e-15 are skipped; nonpositive displaced states are left unchanged and predicted invalid displaced levels receive zero corrected drift. Initial validation does not guarantee positive accrual denominators on all simulated paths for every displacement. Log-Euler/frozen-drift LMM is an unselected alternative; the public dispatcher accepts only the dedicated predictor-corrector scheme.

## Evidence

[Process/drift/grid](../../../../finstack-quant/models/src/monte_carlo/process/lmm.rs), [predictor-corrector](../../../../finstack-quant/models/src/monte_carlo/discretization/lmm_predictor_corrector.rs). Located tests cover caplet comparison to displaced Black, fixed-forward freezing, terminal-forward martingale, and endpoint volatility handling. [Non-Markov simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests/non_markov.rs) include direct moments and production-loop equality. [Brigo–Mercurio](../../../REFERENCES.md#brigo-mercurio-2006-interest-rate-models) and [Andersen–Piterbarg](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling) provide model context; the located tests do not establish broad nonterminal option/grid accuracy.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
