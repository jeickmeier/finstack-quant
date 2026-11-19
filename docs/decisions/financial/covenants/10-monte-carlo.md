# COVENANTS-10: Path-consistent Monte Carlo, antithetic units and sampling error

**Decision status:** Accepted 2026-11-17 for fixed-grid Monte Carlo paths, explicit replay inputs and sampling uncertainty.

**Implementation status:** Brownian increments and pair-level error estimates are implemented. Fixed-grid stream assignment, explicit seed, exact antithetic counts, checked log-space comparison and sampling disclosure remain pending.

**Validation status:** All 115 Covenants Rust tests and five focused Monte Carlo and antithetic tests passed on 2026-11-17 against current behavior. Cross-window consistency, numerical extremes and the accepted sampling rules still need qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Apply the [accepted lognormal model](09-analytic-probability.md) on one chronological simulation grid per exact metric binding. Carry each Brownian path across the selected dates, including threshold changes and waivers. Require an explicit seed and stable metric and sampling-unit stream assignment using the [core RNG contract](../core-numerics/pseudorandom-and-bridge.md#accepted-decision-checked-random-draws-bridge-and-poisson-counts). Replay requires the same grid, inputs and numerical implementation. Published probabilities are marginal to one test date.
2. Use the requested path count and require at least two independent sampling units. Antithetic mode requires an even count, with each pair's indicator average treated as one unit. For example, 10,000 antithetic paths use 5,000 pair averages to estimate standard error.
3. Apply the analytic clock, domain and boundary rules. Compare simulated values with positive thresholds in log space. An unresolved numerical failure produces a computation error.
4. Report probability, standard error, executed path count and independent sampling-unit count. Flag samples with all hits or no hits. A zero standard-error estimate at those endpoints describes the observed sample and conveys no certainty about the model probability.

## Observed simulation and controls

With stochastic mode and `num_paths>0`, eligible dates are active, finite-positive metric projections with positive sigma and horizon. All other dates retain deterministic decisions. Simulate independent units `N=num_paths` without antithetics or `N=ceil(num_paths/2)` with them. Validation requires at least two units; antithetic requests of three paths therefore simulate four paths. Antithetic true with zero paths errors only in stochastic mode. No adaptive stopping or error target is implemented.

For unit `u`, initialize core `Pcg64Rng::new_with_stream(seed,u)`, with absent seed resolved to zero. Core draws normals by cached Box–Muller over PCG64 uniforms. Starting at `T0=0,W0=0`, at eligible date `k` draw `Zk` and accumulate `Wk=W(k-1)+sqrt(Tk-T(k-1))*Zk`; the paired path accumulates its negative. Values are `bk*exp(-0.5 sigma²Tk + sigma Wk)`. The existing breach comparator produces each indicator. This preserves Brownian increments across the eligible dates of a single spec forecast while allowing the supplied base projection to vary arbitrarily by date.

A plain unit contributes `Yu=Iu`; an antithetic unit contributes `(Iu+ + Iu-)/2`. For each date, compute `p=sum(Y)/N`, unbiased unit variance `s²=max((sum(Y²)-N*p²)/(N-1),0)`, then standard error `sqrt(s²/N)`. Counting paired paths as independent samples is deliberately avoided. A zero-hit/all-hit simulation reports zero estimated error without a confidence interval or rare-event upper bound.

## Observed reproducibility and interpretation

Repeated identical inputs with the same grid/configuration use the same stream and traversal order. Each batch spec group restarts these streams; equal grids share random numbers, while different grids/group boundaries consume them differently. Adding an intermediate eligible date changes later same-seed Brownian realizations. Consequently source wording about independence of evaluation order is limited to fixed traversal inputs: it does not establish grid-invariant samples or joint paths across window replacements, metrics or covenants. Output contains only marginal probabilities, not a joint portfolio/default/first-passage estimate.

Finite inputs can produce nonfinite drift/shock arithmetic or zero/infinite simulated values; the sampler has no explicit finite-intermediate guard. Internally NaN counts as breach; zero/infinity use the comparator. Sample agreement in one moderate case does not establish equivalence to the analytic branch in these regimes.

## Rationale, alternatives and evidence

Source comments explain Brownian accumulation and antithetic variance reduction for a monotone indicator. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) and the repository [PCG reference](../../../REFERENCES.md#o-neill-2014-pcg) supply mathematical/generator background; no independent stream or borrower-model qualification was performed. A shared event-grid process, explicit cross-metric correlation, confidence intervals and stable log-space comparison are meaningful alternatives requiring separate decisions.

Owning [MC and validation](../../../../finstack-quant/covenants/src/forward.rs) and inspected [core PCG/normal implementation](../../../../finstack-quant/core/src/math/random.rs). Inline tests `mc_converges_to_analytic_lognormal_probability`, `mc_is_deterministic_for_a_fixed_seed`, and `antithetic_reduces_standard_error_here` use one moderate near-threshold case; convergence uses absolute 0.005 rather than an uncertainty-scaled acceptance rule. [Audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) reject fewer than two independent units.

The focused simulations passed against current behavior. The convergence fixture checks agreement with the crate's analytic result. Qualify the accepted shared grid across windows, explicit seed replay, odd-path rejection, stable tails and all-hit or zero-hit reporting independently.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
