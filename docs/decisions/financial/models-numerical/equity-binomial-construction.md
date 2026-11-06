# MODELS-NUMERICAL-001: CRR and strike-centered Leisen–Reimer construction

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice, scope and rationale

The engine supports multiplicative recombining vanilla lattices, with caller-supplied constant continuously compounded rate and continuous yield. `OptionType::intrinsic` is max(S−K,0) or max(K−S,0); `ExerciseStyle` defaults to European. Spot and strike are positive price levels, volatility is annual decimal lognormal volatility, and time is years. LR is documented as improving convergence; that motivation is observed, not an approved accuracy guarantee.

## Algorithm and selection

For N steps, dt=T/N. CRR uses u=exp(σ√dt), d=1/u, p=(exp((r−q)dt)−d)/(u−d). Its helper rejects u−d<1e−14 and probabilities outside [0,1]. `with_drift` reconstructs probabilities with the supplied annual drift and dt; it is not a separate interpolation method.

LR uses d1,d2 from the shared Black–Scholes kernel and c=(N+1/6)/(N+1/3+0.1/(N+1))². The inversion is PP(z)=1/2+sign(z)√(1−exp(−cz²))/2. Set p=PP(d2), p′=PP(d1), u=exp((r−q)dt)p′/p, d=exp((r−q)dt)(1−p′)/(1−p). The implementation evaluates these ratios in log space with `expm1`/`ln_1p` branches to avoid cancellation in deep tails. The constructor rounds a positive even N upward; pricing rejects an even LR grid created by mutating public fields. At σ=0 both factors equal exp((r−q)dt), with p=1/2.

The [equity caller](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/black.rs) explicitly chooses LR and defaults to 201 steps. This is a caller default, not a global engine default. There is no automatic convergence refinement or fallback to CRR. Discounted induction is [record 003](tree-discounting-and-induction.md); dividends and exercise are [record 002](dividends-and-lattice-exercise.md).

## Failure behavior and alternatives

Pricing requires finite positive T and N>0; LR checks finite positive factors. The shared `OptionMarketParams::validate` requires strictly positive σ and allows T=0, whereas tree pricing allows σ=0 and rejects T=0. Supported CRR/LR are alternatives; adaptive refinement or alternative tree constructions would be new work.

## Evidence

[binomial_tree.rs](../../../../finstack-quant/models/src/trees/binomial_tree.rs) owns construction, including `calculate_parameters`; [tree_framework.rs](../../../../finstack-quant/models/src/trees/tree_framework.rs) owns `EvolutionParams`; [types.rs](../../../../finstack-quant/models/src/types.rs) owns payoff and validation. Inline `test_golden_crr_atm_vs_black_scholes` uses the numeric 10.4506 anchor (500 steps, 0.1% tolerance); `test_golden_lr_odd_converges_faster` allows 0.05 absolute price error at 101 steps. Other convergence tests compare the repository BS implementation, and bit pins only detect changes. [Black–Scholes reference](../../../REFERENCES.md#black-scholes-1973) supplies the analytical comparator, not LR adoption evidence.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
