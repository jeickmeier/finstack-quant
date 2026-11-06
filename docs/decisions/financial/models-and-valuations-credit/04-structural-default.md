# MODELS-AND-VALUATIONS-CREDIT-04: Structural default, asset calibration, survival export and simulation

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Merton terminal-default, diffusion first-passage and CreditGrades laws are separately implemented, with explicit restrictions. Model-implied spreads, debt spreads and approximate CDS par spreads are distinct quantities.

## Scope and input domain

Positive finite asset value, volatility and barrier; finite risk-free/payout rates. Lognormal jumps require nonnegative intensity/volatility and terminal default. CreditGrades requires a flat first-passage barrier; physical-drift methods reject it.

## Algorithm

1. Diffusion terminal PD is `Φ(−[ln(V/B)+(r−q−σ²/2)T]/σ√T)`. First-passage subtracts barrier growth from drift and adds the reflected-normal term, computed using log-CDF; initial V≤B defaults immediately for positive T. Nonpositive horizons return zero PD/infinite distance.
2. Jump diffusion compensates drift by `λ(exp(μJ+σJ²/2)−1)` and sums normal terminal laws across Poisson counts through `ceil(λT+10√λT)`, at least 20. Weights begin at the Poisson mode and recurse outward.
3. CreditGrades uses `A=√(σ²T+λB²)`, `d=(V/B)exp(λB²)`, survival `Φ(−A/2+ln(d)/A)−dΦ(−A/2−ln(d)/A)`, clipped to [0,1]. Its helper derives V=equity+debt×meanRecovery and delta-scaled asset volatility.
4. Equity calibration alternates Black–Scholes asset-value and volatility updates, at most 100 iterations, relative tolerance 1e−8; tiny equity/N(d1) is rejected. CDS calibration scans 129 volatility points over [.01,2], skips unusable trials, requires one sign-changing bracket and Brent-solves to 1e−10. Exact grid roots return immediately. Target-PD calibration solves the terminal barrier algebraically; the KMV default point is short debt plus half long debt.
5. Hazard export sorts tenor points, requires strictly increasing positive time and nonincreasing positive survival, then uses segment `−ln(Si/Si−1)/Δt`. CreditGrades export requires its own mean recovery. Implied spread is `−ln(1−PD×LGD)/T`; terminal debt spread uses debt value as discounted assets minus option equity. CDS par spread builds approximately quarterly equal time intervals, ACT/365F→ACT/360 accrual, midpoint protection and half-period accrued-on-default.
6. Seeded PCG64 simulation uses exact lognormal steps and Poisson-normal jumps. Antithetics negate normals and invert count uniforms with U/1−U. CreditGrades path simulation only simulates asset diffusion; it does not sample uncertain barriers.

## Selection and controls

Constructors explicitly choose barrier/dynamics. Equity/CDS/target-PD inversion produces terminal GBM. Simulation takes path/step count, horizon, seed and antithetic flag; it returns assets, not default-time events.

## Conventions

Annualized decimal rate/volatility, years for horizon; market-risk-neutral drift defaults to r, physical drift is explicit. CDS input quote is bp and spread outputs decimal. The report maps model PDs through the registry-default master scale; this is not empirical KMV EDF calibration. Unsupported spread/equity measures become None with named per-measure errors, while invalid horizons or grade mapping abort the report.

## Limitations, failure and fallback

First-passage NaN is converted to zero PD; this can hide arithmetic failure. Poisson truncation has no surfaced omitted-mass diagnostic. A finite scan cannot guarantee global uniqueness between nodes, and terminal PD need not be monotone in horizon, causing hazard-export rejection. CreditGrades positive-time limit need not match the hard-coded zero-time PD.

## Alternatives and tradeoffs

Empirical KMV EDF mapping and full contractual CDS calibration are unimplemented alternatives. Brownian-bridge/default-time simulation would be required for continuous pathwise first passage; asset paths alone are insufficient.

## Evidence

Owners: [Merton module implementations](../../../../finstack-quant/models/src/credit/merton). [default_probability.rs](../../../../finstack-quant/models/src/credit/merton/default_probability.rs) includes first_passage_growing_barrier_matches_black_cox_1976 formula comparison and jump_diffusion_pd_matches_monte_carlo; [calibration.rs](../../../../finstack-quant/models/src/credit/merton/calibration.rs) includes equity/CDS roundtrips; [simulation.rs](../../../../finstack-quant/models/src/credit/merton/simulation.rs) includes GBM/jump mean and seed reproducibility cases; roundtrips and simulation comparisons establish internal consistency only. References: [Merton 1974](../../../REFERENCES.md#merton-1974), [CreditGrades](../../../REFERENCES.md#finger-2002-creditgrades), [KMV](../../../REFERENCES.md#crosbie-bohn-2003-kmv).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
