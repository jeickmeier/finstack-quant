# MODELS-MONTE-CARLO-14: Rough Bergomi adapted variance and log-Euler spot

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/rough_bergomi.rs and discretization/rough_bergomi.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Rough Bergomi represents spot as the visible state and keeps the Volterra level in the scheme workspace. Rates/yields are finite decimals, eta>0, rho in [-1,1], and the input forward-variance curve supplies variance per year. The parameter constructor warns, but does not reject, H≥1/2. A dedicated scheme and externally injected noise are mandatory; generic drift/diffusion are deliberately zero placeholders.

## Algorithm and controls

At the left endpoint reconstruct `v_t=xi(t)*exp(eta*Y_t-eta²*t^(2H)/2)`. Form the spot shock `rho*Z_drive+sqrt(max(1-rho²,0))*Z_independent` from the Brownian driver associated with the fractional generator. Advance `S*=exp((r-q-v_t/2)dt+sqrt(v_t dt)*Z_spot)`, then update workspace `Y+=DeltaY`. This ordering preserves adaptedness: the new fractional increment does not enter the same step's variance. Finite negative variance is floored; nonfinite variance is preserved to surface downstream. Default simulation selects RL Volterra, with optional fBM choices in [record 13](13-fractional-noise.md).

## Failure behavior and alternatives

The compensation assumes Var(Y_t)=t^(2H); finite-window fBM and discrete RL approximations can deviate. A martingale spot test alone cannot establish correct volatility covariance or leverage. There is no upper variance cap or exponent clipping. fBM is an explicit alternative dynamics choice, not an accuracy-preserving switch. Euler price error and tail sampling remain separate from sampling CI.

## Evidence

[Process](../../../../finstack-quant/models/src/monte_carlo/process/rough_bergomi.rs), [stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/rough_bergomi.rs). Located tests include left-endpoint variance reconstruction, workspace reset, spot/forward-variance martingales and small-eta Black–Scholes convergence. [Non-Markov tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests/non_markov.rs) compare dedicated path loops and generator variants; loop equality is shared-kernel evidence. [Bayer–Friz–Gatheral](../../../REFERENCES.md#bayer-friz-gatheral-2016) and [McCrickerd–Pakkanen](../../../REFERENCES.md#mccrickerd-pakkanen-2018) are canonical background, with no recorded product adoption.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
