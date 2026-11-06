# MODELS-ANALYTIC-15: SABR delta strikes and discrete smile arbitrage repair

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Single-expiry SABR smile with user strike grid and continuous discount rate. Arbitrage grids must be finite and strictly ascending, but empty/singleton grids are accepted.

## Choice and rationale

Delta-to-strike uses ATM volatility as a proxy; discrete call prices are checked for monotonicity and convexity, optionally greedily reduced and reinverted. Repair returns a volatility vector and does not refit SABR parameters.

## Algorithm

For absolute delta δ, z=N⁻¹(δ); normal call strike=F−σATM√T z, put flips z. Lognormal/shifted K=(F+s)exp[−σATM√T signed_z+σATM²T/2]−s. Generate SABR quotes at each strike and price discounted Bachelier for β≈0 or shifted Black otherwise. Monotonicity fails if Ci>Ci−1+1e−6. Convexity uses unequal-grid linear interpolation of neighbor prices; violation=2(interpolated−Ci)<−1e−6. Repair passes first enforce decreasing prices by Ci=max(Ci−1−1e−8,0), then reduce each interior above its neighbor interpolation. Only modified nodes are inverted through shared IV solvers; all repaired prices are regenerated and revalidated.

## Selection and controls

Absolute delta must lie in (0,1). Repair iteration budget is caller-supplied; no hidden retries or alternative optimizer. `check_no_arbitrage` converts diagnostics into an error; `validate_no_arbitrage` returns violations. Severity percentage is butterfly magnitude/middle price×100 when middle price>1e−6.

## Conventions

Delta is premium-unadjusted forward delta in the ATM-vol proxy, not the delta of the fitted smile at its own strike. Arbitrage tolerance is absolute price units, not volatility/basis points.

## Limitations, failure and fallback

Delta strike is not solved self-consistently against strike-dependent SABR volatility. Greedy repair can move prices below attainable intrinsic, causing inversion failure; remaining arbitrage errors after budget exhaustion. Empty/thin grids passing says nothing about unobserved strikes. No calendar condition or global wing check is performed.

## Alternatives and tradeoffs

Self-consistent strike root finding or constrained convex regression would be alternative unimplemented policies. Existing grid repair is fast and local but not a minimum-distance calibration.

## Implementation and validation evidence

[smile.rs](../../../../finstack-quant/models/src/volatility/sabr/smile.rs) and [tests.rs](../../../../finstack-quant/models/src/volatility/sabr/tests.rs). Uneven-grid, two-strike monotonicity, empty-grid and post-repair revalidation tests exercise discrete mechanics; delta round trips use ATM-vol Black, so do not validate smile-consistent delta. Reference: [SABR](../../../REFERENCES.md#hagan-2002-sabr).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
