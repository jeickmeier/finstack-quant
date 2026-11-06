# MODELS-NUMERICAL-017: Black–Scholes and Heston coefficient bridges

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and equations

[bridge.rs](../../../../finstack-quant/models/src/pde/bridge.rs) maps Black–Scholes with x=ln S to a=σ²/2, b=r−q−σ²/2, c=−r and zero source. Terminal payoff is max(exp(x)−K,0) for calls or max(K−exp(x),0) for puts. The in-the-money edge uses `LinearInExp`; the out-of-the-money edge is Dirichlet zero. This preserves affine-in-spot behavior at truncated log-spot edges.

[bridge2d.rs](../../../../finstack-quant/models/src/pde/bridge2d.rs) uses y=v, v+=max(v,0): a_xx=v+/2, a_yy=σ_v²v+/2, a_xy=ρσ_v v+, b_x=r−q−v+/2, b_y=κ(θ_v−v), c=−r. The drift uses raw v while diffusion uses v+. Terminal payoff and x boundaries match the BS bridge. Both lower and upper variance boundaries use `Linear`, including the degenerate v=0 edge; the bridge does not solve the limiting boundary PDE explicitly.

## Input conventions and selection

Rates and yield are continuous annual decimals; σ is annual lognormal volatility, v and θ_v are annual variance, κ is inverse years, σ_v is variance-process volatility and ρ is correlation. Both structs expose public fields and implement no validation. The generic solver does not enforce positive strike, valid correlation, positive parameters or a nonnegative variance grid. Model homogeneity permits operator reuse. The 1D solver may attach an obstacle; the 2D solver remains European. Grid/domain, maturity and damping are caller choices, not fields of the bridge.

## Validation

BS call/put tests compare with the repository analytical formula (for example 0.1% call tolerance). Heston `heston_pde_vs_fourier_coarse_anchor` checks K=100 and 120 at 2.5% relative tolerance against the repository Fourier engine; it is numerical-method cross-validation, not an independent vendor source. The full-resolution ATM test is ignored as slow and allows 2%. Put-call parity alone cannot identify correlation or variance-dynamics errors. Feller-violation/refinement and high-κ cases exercise selected parameter regimes, not the whole exposed domain.

A degenerate-PDE boundary closure or validated model-parameter constructor would be alternative implementations. [Black–Scholes](../../../REFERENCES.md#black-scholes-1973), [Heston](../../../REFERENCES.md#heston-1993), and [MCS](../../../REFERENCES.md#in-t-hout-welfert-2009) identify model/method provenance; none approves the selected grid and boundary policy.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
