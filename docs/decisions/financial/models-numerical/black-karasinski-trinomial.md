# MODELS-NUMERICAL-006: Black–Karasinski capped log-rate trinomial

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and domain

[black_karasinski.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/black_karasinski.rs) uses a recombining trinomial in x=ln r. Unlike BDT it represents mean reversion through node-dependent branch probabilities. The model selection in [tree.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tree.rs) requires κ>0 and σ>0; the κ=0 alternative is BDT, not an automatic fallback. Rates remain positive.

## Algorithm

Uniform dt gives dx=σ√(3dt), width cap jmax=max(ceil(0.184/(κdt)),1), and row width 2 min(n,jmax)+1. Interior probabilities are p_up=1/6+(j²m²−jm)/2, p_mid=2/3−j²m², p_down=1/6+(j²m²+jm)/2 with m=κdt. Thus the one-step mean offset is −jm and second moment is 1/3+(jm)². At the upper cap branches use offsets (0,−1,−2), and at the lower cap (+2,+1,0), with probabilities solved from those two moments. Nonfinite/negative probabilities fail; finite probabilities are normalized and checked to sum to one.

Each interval solves ΣQ_j df(exp(a+j dx),dt)=P_target with default Brent, seeded by ln(max(period forward rate,1e−8)). Store r=exp(a+j dx), propagate state prices with the same branches, and gate maximum relative DF error at the configured tolerance. The final a is repeated for terminal accessors. Dedicated rollback uses these capped branches and the OAS conventions in [record 003](tree-discounting-and-induction.md).

## Controls, limitations and alternatives

Step count, volatility, κ, compounding and fit tolerance are explicit. κdt can produce invalid boundary probabilities; the engine returns an error rather than clipping them. This is Euler-style OU mean/variance matching, not exact finite-step OU moments. The common probability normalization is borrowed from Hull–White. The maximum-error aggregation has the same per-row NaN visibility concern as BDT. Exact OU transitions or an uncapped nearest-mean lattice would be new variants.

## Evidence

[tests.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tests.rs) includes `bk_trinomial_reprices_curve_to_a_tenth_bp`, `bk_terminal_log_rate_dispersion_matches_ou_limit` (σ√((1−e^(−2κT))/(2κ)), and `bk_kappa_to_zero_converges_to_bdt` (cross-engine comparison). [Hull–White numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures) is the cited branching background; it does not independently qualify BK option prices.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
