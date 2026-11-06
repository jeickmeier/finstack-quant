# MODELS-MONTE-CARLO-15: Rough Heston Volterra-history hybrid discretization

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/rough_heston.rs and discretization/rough_heston.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Rough Heston carries spot and variance and stores the complete past drift/noise history in workspace. The dedicated hybrid scheme requires H in (0,1/2), a finite increasing grid beginning at zero, and three independent normals per step. Process validation requires finite rates/yields, positive finite kappa/theta/sigma_v/v0 and rho strictly inside (-1,1), through ordinary Heston parameter checks; the wider process Hurst type can accept values that the hybrid constructor rejects. The simulator requires initial variance to equal model v0 bit-for-bit.

## Algorithm and controls

Set alpha=H+1/2 and kernel normalization 1/Gamma(alpha). Precompute triangular weights `K_ij=((t_next-t_j)^alpha-(t_next-t_(j+1))^alpha)/alpha` and noise weights K_ij/dt_j. At each step save `kappa(theta-max(v,0))` and `sigma_v sqrt(max(v,0))*sqrt(dt)*Z_v`. Recompute the Volterra sum from all saved terms; add a newest-cell independent bridge correction `sigma_v sqrt(v)*dt^H*(1/2-H)/(alpha sqrt(2H))*Z_bridge`. Set next variance to `max(v0+sum/Gamma(alpha),0)`. Advance spot by left-variance log-Euler with `rho Z_v+sqrt(max(1-rho²,0))Z_independent`.

Storage/work is quadratic for precomputed weights/per-path history evaluation, respectively; warning threshold is 200 steps, and public simulation limits precomputation to at most 8,000 steps. Work contains 2n history entries plus a counter, reset every path. The scheme exposes expected step count; generic engine checks count only, not equality of actual grid coordinates.

## Failure behavior and alternatives

Variance truncation changes boundary behavior. A same-count grid or Hurst parameter mismatch between a directly constructed scheme and process is not rejected by the generic pairing check. Memory truncation, multi-factor Markovian approximation and exact conditional methods are unimplemented alternatives here.

## Evidence

[Process](../../../../finstack-quant/models/src/monte_carlo/process/rough_heston.rs), [hybrid](../../../../finstack-quant/models/src/monte_carlo/discretization/rough_heston.rs); tests locate exact newest-cell covariance, fractional-mean comparison, `mc_atm_price_matches_fourier_pricer`, history reset and positivity. Fourier agreement is a distinct production-method comparison; not an external vendor benchmark. [El Euch–Rosenbaum](../../../REFERENCES.md#el-euch-rosenbaum-2019) and [Bennedsen–Lunde–Pakkanen](../../../REFERENCES.md#bennedsen-lunde-pakkanen-2017) describe model/kernel background; parameter-wide accuracy remains unestablished by this audit.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
