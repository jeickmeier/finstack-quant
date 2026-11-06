# COVENANTS-10: Path-consistent Monte Carlo, antithetic units and sampling error

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Simulation and controls

With stochastic mode and `num_paths>0`, eligible dates are active, finite-positive metric projections with positive sigma and horizon. All other dates retain deterministic decisions. Simulate independent units `N=num_paths` without antithetics or `N=ceil(num_paths/2)` with them. Validation requires at least two units; antithetic requests of three paths therefore simulate four paths. Antithetic true with zero paths errors only in stochastic mode. No adaptive stopping or error target is implemented.

For unit `u`, initialize core `Pcg64Rng::new_with_stream(seed,u)`, with absent seed resolved to zero. Core draws normals by cached Box–Muller over PCG64 uniforms. Starting at `T0=0,W0=0`, at eligible date `k` draw `Zk` and accumulate `Wk=W(k-1)+sqrt(Tk-T(k-1))*Zk`; the paired path accumulates its negative. Values are `bk*exp(-0.5 sigma²Tk + sigma Wk)`. The existing breach comparator produces each indicator. This preserves Brownian increments across the eligible dates of a single spec forecast while allowing the supplied base projection to vary arbitrarily by date.

A plain unit contributes `Yu=Iu`; an antithetic unit contributes `(Iu+ + Iu-)/2`. For each date, compute `p=sum(Y)/N`, unbiased unit variance `s²=max((sum(Y²)-N*p²)/(N-1),0)`, then standard error `sqrt(s²/N)`. Counting paired paths as independent samples is deliberately avoided. A zero-hit/all-hit simulation reports zero estimated error without a confidence interval or rare-event upper bound.

## Reproducibility and interpretation

Repeated identical inputs with the same grid/configuration use the same stream and traversal order. Each batch spec group restarts these streams; equal grids share random numbers, while different grids/group boundaries consume them differently. Adding an intermediate eligible date changes later same-seed Brownian realizations. Consequently source wording about independence of evaluation order is limited to fixed traversal inputs: it does not establish grid-invariant samples or joint paths across window replacements, metrics or covenants. Output contains only marginal probabilities, not a joint portfolio/default/first-passage estimate.

Finite inputs can produce nonfinite drift/shock arithmetic or zero/infinite simulated values; the sampler has no explicit finite-intermediate guard. Internally NaN counts as breach; zero/infinity use the comparator. Sample agreement in one moderate case does not establish equivalence to the analytic branch in these regimes.

## Rationale, alternatives and evidence

Source comments explain Brownian accumulation and antithetic variance reduction for a monotone indicator. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) and the repository [PCG reference](../../../REFERENCES.md#o-neill-2014-pcg) supply mathematical/generator background; no independent stream or borrower-model qualification was performed. A shared event-grid process, explicit cross-metric correlation, confidence intervals and stable log-space comparison are meaningful alternatives requiring separate decisions.

Owning [MC and validation](../../../../finstack-quant/covenants/src/forward.rs) and inspected [core PCG/normal implementation](../../../../finstack-quant/core/src/math/random.rs). Inline tests `mc_converges_to_analytic_lognormal_probability`, `mc_is_deterministic_for_a_fixed_seed`, and `antithetic_reduces_standard_error_here` use one moderate near-threshold case; convergence uses absolute 0.005 rather than an uncertainty-scaled acceptance rule. [Audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) reject fewer than two independent units.

Internal analytic agreement is distinct from independent validation; no simulations were executed.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
