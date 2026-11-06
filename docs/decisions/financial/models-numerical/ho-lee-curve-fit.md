# MODELS-NUMERICAL-004: Ho–Lee additive lattice and curve-fitting drift

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice, scope and rationale

[ho_lee.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/ho_lee.rs) fits each grid discount factor using additive normal moves and a deterministic row shift. This permits negative rates and exact recombination without mean reversion. [config.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/config.rs) defaults to Ho–Lee, N=100, absolute normal σ=0.01 per sqrt(year), continuous compounding and a 0.1 bp relative-DF fit tolerance. These are observed defaults, not adopted calibrations.

## Algorithm and controls

Use uniform dt=T/N. Set r0 by inverting the configured compounding against P(0,dt). From each row propagate Arrow–Debreu prices with probability 1/2 and configured node DF, and build next base rates r±σ√dt. To fit the following maturity, continuous compounding uses θ=(ln ΣQ_next exp(−r_base dt)−ln P_target)/dt. Other conventions solve ΣQ_next df(r_base+θ,dt)=P_target with core `BrentSolver::new`, initialized by the continuous expression. Add θ to the whole next row. The terminal row has θ=0 because no post-expiry discount interval is used.

Recompute all maturity DFs by forward induction on the stored lattice. Error is |modelDF−targetDF|/targetDF×10000, not yield basis points. Reject error above configured tolerance or any node DF outside [1e−30,1e6], including terminal-row diagnostic DFs. [tree.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tree.rs) validates finite positive targets and horizon, nonnegative volatility, positive steps/tolerance, and calibrates a candidate before replacing previous state. Ho–Lee rejects mean reversion with magnitude above 1e−12.

## Alternatives and limitations

The supported normal mean-reverting alternative is the separate Hull–White engine. Fitting the discount curve does not calibrate σ to option prices or validate callable-option accuracy. `TreeCalibrationResult::is_acceptable` uses a separate hardcoded <1 bp threshold, distinct from the configured default 0.1 bp. Core Brent tolerances are inherited rather than exposed in this tree config.

## Evidence

[tests.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tests.rs): `ho_lee_stored_lattice_prices_zero_coupon_to_calibration_curve` checks calibration/rollback duality to 1e−8; `ho_lee_noncontinuous_compounding_reprices_curve` tests Simple and several periodic conventions to 0.1 relative-DF bp; failure tests verify preservation of old state. [Brent](../../../REFERENCES.md#brent-1973) is the root-method reference and [Brigo–Mercurio](../../../REFERENCES.md#brigo-mercurio-2006-interest-rate-models) is background. These are not independent option-price benchmarks.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
