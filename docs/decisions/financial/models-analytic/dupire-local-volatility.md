# MODELS-ANALYTIC-19: Finite-difference Dupire extraction, smoothing and local-vol interpolation

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Black-lognormal strike-axis implied surface, at least2 expiries/3 positive strikes and one positive forward per expiry. Output grids accept nonnegative local vols and nonnegative expiries.

## Choice and rationale

Extract local variance at fixed log-moneyness from total variance derivatives. Optional Gaussian smoothing acts on implied vol in cash-strike units before differentiation; it changes the input smile.

## Algorithm

At each node k=ln(K/F(T)),w=σimp²T; reject w<1e−14. Compute time derivative with neighboring expiries at matched k (Kneighbor=Fneighbor exp k): central secant inside, one-sided at boundaries. Strike derivatives use unequal-grid finite differences in K, then wk=K wK and wkk=K²wKK+wk. Density=1−k wk/w+.25(−.25−1/w+k²/w²)wk²+.5wkk. Local σ=√(max(wT,0)/density). Require finite positive density and wT≥−1e−10; finite result. Smoothing averages input σ using weights exp[−.5((Kj−K)/bandwidth)²], reconstructs same-grid implied surface then extracts. Query bilinearly interpolates local σ with clamped axes.

## Selection and controls

Extraction uses clamped matched-moneyness lookup; shared density diagnostics instead request strict lookup and may omit unavailable time derivatives. Bandwidth finite≥0, <1e−14 means unsmoothed. Negative calendar slope within tolerance becomes zero; no implied-vol substitution on invalid density.

## Conventions

Forwards determine fixed-moneyness time motion; rates are not guessed from strikes. Bandwidth is absolute strike units, not log-moneyness or vol points. Output is instantaneous diffusion volatility per√year.

## Limitations, failure and fallback

Interior time secant on an irregular expiry grid is not the full unequal-grid second-order formula; edges have lower accuracy. Matched-moneyness clamping can distort wing time derivatives. Local smoothing is not arbitrage repair; extraction still errors. `value` rejects NaN but clamps infinities to endpoint values.

## Alternatives and tradeoffs

Analytic parametric smile derivatives or price-space Dupire would be alternatives; neither is selected here. Smoothing reduces numerical noise at the cost of repricing the smoothed rather than original surface.

## Implementation and validation evidence

[dupire.rs](../../../../finstack-quant/models/src/volatility/dupire.rs) and [local_vol.rs](../../../../finstack-quant/models/src/volatility/local_vol.rs). `local_vol_matches_the_exact_ssvi_local_vol` compares to analytic SSVI derivatives on a fine grid and excludes boundary rows/columns; tolerance5e−4 within three standard deviations. Flat-vol/forward-dependence/arbitrage rejection tests supplement it. Reference: [Dupire](../../../REFERENCES.md#dupire-1994).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
