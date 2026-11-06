# MODELS-ANALYTIC-03: Black and Bachelier volatility inversion

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Positive expiry and finite nonnegative premium; Black requires positive forward/strike, normal finite forward/strike. Adapters accept discounted spot or forward prices.

## Choice and rationale

Price inversion uses parity to move to the OTM side, an analytical seed, bracketing/bisection, then Halley refinement with Newton fallback. The implementation is not a generic unlimited implied-vol solver.

## Algorithm

Check intrinsic bounds; raw Black returns zero at intrinsic and rejects prices at its finite upper bound. Raw normal returns zero within machine-epsilon scaled intrinsic. Transform ITM premium to OTM by subtracting intrinsic. Black seed blends √(2π/T) premium/F with √(2|ln(F/K)|/T), weight=min(5|ln(F/K)|,1). Normal seed is premium√(2π/T). Double the upper bracket up to 64 times, then 20 bisections. Perform up to four Halley steps σ−2fv/(2v²−f volga); use Newton when denominator is too small, stop when vega underflows or step<1e−14. Clamp iterates. Reprice and require residual≤max(target×1e−8,scale×1e−14).

## Selection and controls

Vol floor=1e−16; Black ceiling=10; normal ceiling=10 max(|F|,|K|,1). BS adapter maps price to Black with discounted S and K; Black76 adapter uses dfF and dfK, avoiding dividing by tiny df. Those adapters require strictly positive premium above discounted intrinsic, so they reject the zero-IV solution raw solvers accept. Failures return explicit conversion/validation errors.

## Conventions

Time is years; returned Black IV is decimal annual volatility, normal IV in underlying units/√year. Adapters preserve price scaling; the normal scale floor is 1e−10.

## Limitations, failure and fallback

Ceilings can reject valid very-high-volatility inputs. Bracket expansion has a fixed work budget, and final residual—not termination alone—governs acceptance. Near intrinsic a zero-volatility answer is convention-dependent and differs between entry points.

## Alternatives and tradeoffs

A full Brent solve or specialized rational inversion could avoid some fixed-budget behavior; neither is selected here. Existing parity transformation reduces cancellation for ITM inputs but cannot recover premium information already rounded away.

## Implementation and validation evidence

[implied.rs](../../../../finstack-quant/models/src/volatility/implied.rs) and [implied_vol.rs](../../../../finstack-quant/models/src/closed_form/implied_vol.rs). `solver_only_returns_ok_for_genuinely_converged_solutions` reprices accepted inversions and `sub_intrinsic_prices_are_rejected` checks bounds. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) uses external premiums plus IV round trips; round trips primarily test inversion consistency. References: [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900). No canonical reference for this precise hybrid/seed/ceiling policy was established.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
