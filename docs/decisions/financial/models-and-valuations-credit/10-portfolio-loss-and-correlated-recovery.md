# MODELS-AND-VALUATIONS-CREDIT-10: Portfolio loss simulation, tail statistics and correlated recovery

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Independent path streams simulate correlated obligor defaults and optional logistic-normal recoveries; empirical loss VaR/ES and tranche clipping summarize simulated distributions.

## Scope and input domain

Nonnegative finite exposure notional, probability/LGD in [0,1], unique trimmed IDs, equal nonempty factor-loading dimensions and squared norm≤1 (with epsilon tolerance). Positive bounded path count and confidence strictly between zero and one. RFL is rejected; MultiFactor requires two loadings.

## Algorithm

1. Each path uses Philox(seed,path index), one shared Student-t mixing draw where selected and independent standard-normal systematic factors. For name i form normalized loading dot product, rho=loading squared norm, add idiosyncratic copula shock, and default when latent≤marginal threshold.
2. Loss sums notional×LGD for defaulted names. Without a recovery model use exposure LGD; with it sample recovery using normalized systematic projection (or first factor for zero loading) and an independent recovery normal when needed. Recovery shocks are drawn consistently for every name before checking default.
3. Correlated recovery is sigmoid(logit(m)+[vol/(m(1−m))]×[corr×Z+√(1−corr²)ε]). Endpoints/zero volatility are deterministic. The supplied mean parameter is the zero-shock center; actual expectation is numerically integrated and cached. Conditional expectation integrates residual normal risk. Adaptive Simpson uses tolerance 1e−12/depth24 with a normal correction on [0,10] for small scaled variance and a logistic-density transformation on [0,36] for large variance.
4. Loss mean uses scaled compensated summation. VaR is the ascending order statistic ceil(confidence×N)−1. ES averages exactly (1−confidence)×N upper-tail mass, including fractional weight on the boundary observation. Tranche losses clip `(loss/poolNotional−A)` to [0,D−A] and divide by width; attachment breach uses >A and full writedown ≥D.
5. Native parallel paths preserve deterministic indexed streams and report the lowest-index failure; WASM fills serially. Deserialization rebuilds and exactly compares carried aggregate statistics.

## Selection and controls

Caller supplies copula, path count, seed, confidence, factor loadings and optional RecoverySpec. RecoverySpec default is constant .40; named stochastic preset centers .40, vol .25, correlation .40. No confidence-interval calculation or automatic simulation convergence control is selected.

## Conventions

Exposure losses are money amounts in a common external unit; portfolio tranche A/D are fractions, unlike CDS tranche percent inputs. Positive recovery correlation lowers recovery in adverse negative systematic states; logistic-normal volatility parameter is a local scale, not exact output standard deviation.

## Limitations, failure and fallback

No temporal default schedule, discounting, contagion or obligor-state migration is modeled. Unconditional expected recovery differs from recovery conditional on default. Copula MultiFactor in this simulator uses supplied loadings, not the kernel’s fixed 60/40 decomposition. Zero-loading exposures can still have common recovery-factor dependence.

## Alternatives and tradeoffs

Semi-analytic finite-pool convolution is supported elsewhere for tranches; importance sampling, QMC and rare-event estimators are unimplemented here. Mean-centered calibrated beta recovery is a separate LGD model, not the same stochastic law.

## Evidence

Owners: [portfolio_loss.rs](../../../../finstack-quant/models/src/correlation/portfolio_loss.rs) and [Recovery implementations](../../../../finstack-quant/models/src/correlation/recovery). [portfolio_loss.rs tests](../../../../finstack-quant/models/tests/portfolio_loss.rs) and inline tests locate deterministic streams, formulas/moments and fractional tail-mass checks; they are not a portfolio loss model validation. References: [QRM](../../../REFERENCES.md#mcneil-frey-embrechts-qrm), [Recovery dependence](../../../REFERENCES.md#altman-et-al-2005-recovery).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
