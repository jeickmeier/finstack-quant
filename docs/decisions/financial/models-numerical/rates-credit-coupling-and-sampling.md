# MODELS-NUMERICAL-009: Bernoulli correlation coupling and reproducible lattice paths

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice, scope and algorithm

[sampling.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/sampling.rs) preserves each fitted marginal while coupling one-step up/down indicators. Set a=p_r p_h+ρ√(p_r(1−p_r)p_h(1−p_h)). Joint probabilities are (a,p_r−a,p_h−a,1−p_r−p_h+a). Clamp a to the Frechet interval [max(p_r+p_h−1,0),min(p_r,p_h)] after a debug assertion; calibration is expected to have rejected economically infeasible ρ.

[calibration.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/calibration.rs) scans each row's minimum/maximum nondegenerate marginal probabilities, tests their four combinations and intersects attainable correlation intervals. Degenerate p=0/1 nodes are skipped. No scan is needed for ρ=0 or two zero mean-reversion factors (p=1/2). `max_feasible_correlation` returns the symmetric magnitude min(|lower|,|upper|), not the complete asymmetric interval.

## Sampling, controls and conventions

Use `PhiloxRng::with_stream_u01_offset(seed,path_index,checkpoint.step)` and consume one uniform per transition. Cumulative intervals select up/up, up/down, down/up, down/down; antithetic sampling replaces U by 1−U. Segment resumption preserves random-coordinate alignment. Each returned node includes raw short rate, floored hazard, exp(−r dt), exp(−h dt), and stable default probability −expm1(−h dt); terminal values use (DF,survival,default)=(1,1,0). These are weighted survival states: no default time is drawn by this method.

`BandTable` stores cached terms around step/2 with half-width 2 ceil(√step)+2. Zero-volatility rows cache one identical term. Out-of-band access evaluates the identical formula; this is an acceleration cache, not lattice truncation or discarded probability mass. Invalid checkpoint/segment geometry fails.

## Alternatives, evidence

Correlation here is Bernoulli innovation correlation, not terminal levels or realized default correlation. Degenerate marginal tails cannot express arbitrary innovation correlation. Gaussian/copula or exact continuous factor simulation would be different methods. [tests.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/tests.rs) contains `extrema_correlation_scan_matches_exhaustive_node_pairs`, `sampled_joint_moves_preserve_skewed_correlated_marginals`, `sampled_paths_are_seeded_weighted_and_antithetic`, and `sampled_states_match_direct_node_terms_bit_for_bit`. These are implementation/invariant evidence; external market calibration is absent. [Philox reference](../../../REFERENCES.md#salmon-2011-philox) supports the RNG family, not the coupling model. [band_table.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/band_table.rs) is storage plumbing and adds no probability approximation.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
