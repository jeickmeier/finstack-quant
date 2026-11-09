# CALIBRATION-FRAMEWORK-017: Forward-moneyness calendar and call-spread checks

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

Surface checks use the financial forward associated with each expiry. They compare total variance at fixed forward moneyness and validate undiscounted call prices, rather than comparing volatility at fixed absolute strike. These choices are coherent diagnostics for the stated positive lognormal domain.

## Inputs and stages

Require one positive finite forward per stored expiry and positive finite strikes, even if arbitrage checks are disabled. Validate numeric configuration. Zero-expiry nodes are allowed by the surface and correspond to zero total variance/intrinsic calls; the checker does not invent a positive-time epsilon.

For adjacent expiries t1,t2, take each later-row strike K2 and compute `k=ln(K2/F2)` and `K1=F1 exp(k)`. Skip the pair if mapped K1 lies outside the common surface strike bounds. Evaluate the surface using the volatility resolver, require finite values and `w=σ²t`, and flag `w2 < w1−tolerance`.

At each expiry, evaluate undiscounted Black calls at all stored strikes. Set price tolerance `max(tolerance,1e-12) × forward`. Every adjacent vertical spread must lie between negative tolerance and strike width plus tolerance. For three strikes, require the middle call at or below the linear chord between outer calls plus tolerance; the interpolation weight accounts for unequal strike spacing. Fewer than two strikes skip call-spread checks; exactly two still check vertical monotonicity/upper bound. Always test every stored vol is positive, finite and≤configured max, even with arbitrage checks disabled. Gather errors; calendar/call-spread summaries include counts and at most five details, while volatility bound messages are collected directly.

## Limitations and alternatives

This checks the published sampled surface, including interpolated K1 values, but not every strike/expiry or wings. No-overlap calendar pairs are skipped without an explicit untested-region summary. It assumes positive forwards/strikes and Black calls; normal or shifted surfaces need a different validator. A finite set of convex call nodes does not certify all interpolated values or densities.

Alternatives are analytic parameter restrictions (for example SVI density), constrained fitting, or adaptive call-grid checks including wings and extrapolation. No vendor or theorem-based qualification was executed here.

## Source and validation evidence

- Implementation: [validation/surfaces.rs](../../../../finstack-quant/calibration/src/validation/surfaces.rs) — fixed-moneyness probes, vertical/butterfly inequalities, bounds and finite checks.
- Implementation: [validation/config.rs](../../../../finstack-quant/calibration/src/validation/config.rs) — tolerance/max-vol policy.
- Test evidence: [tests/calibration/validation.rs](../../../../finstack-quant/calibration/tests/calibration/validation.rs) — explicit call-spread violations, fixed-forward-moneyness example, overflow rejection and zero-expiry preservation; inspected or located, not executed.
- Reference context: [Gatheral–Jacquier 2014](../../../REFERENCES.md#gatheral-jacquier-2014-svi) — no-arbitrage surface context; not proof these finite probes suffice; a catalogue entry is not proof of implementation equivalence.

## Relationship to the accepted volatility-source boundary

[CORE-MARKET-14's complete accepted package](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) separates valid finite
nonnegative sigma sources from price-model and arbitrage acceptance. Preserve
this validator's actual Black/absolute-strike assumptions; do not present it
as a universal Normal/Shifted/Tenor theorem or reject all zero-vol sources
without the actual model's limit contract. Explicit grid/clock/shift identity
and admitted shocks do not certify fit or no-arbitrage. Record14 does not adopt
this Draft diagnostic/repair/acceptance policy. Its source package was accepted
on **2026-11-08**, with reconciliation and independent qualification pending.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
