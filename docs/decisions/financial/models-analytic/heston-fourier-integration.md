# MODELS-ANALYTIC-09: Heston probability inversion, strip caching and convergence budget

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

European calls/puts, positive S and K, finite time (nonpositive time returns intrinsic after validation), constant Heston/rate inputs.

## Choice and rationale

Composite Gauss–Legendre integrates P1/P2 and caches CF values across strikes. Tail and raw-bound diagnostics precede price acceptance; repeated-grid stability is mandatory even for explicit settings.

## Algorithm

Compute Pj=1/2+π⁻¹∫Re[exp(−iφ ln K)ψj/(iφ)]dφ. Cache both ψj/(iφ); call=S exp(−qT)P1−K exp(−rT)P2; put via parity. Record absolute integrand mass and max CF magnitude in the last 10% of the integration interval. Reject any overflow node or all-underflow grid, tail CF>1e−8, price-scaled tail mass>1e−9 discounted S, or probability-clamp excursion beyond that tolerance. Check intrinsic/upper price bounds before clamping accepted roundoff. Repeat up to six grids, doubling upper limit and quadrupling panels until prices differ by≤1e−9 discounted S.

## Selection and controls

Explicit settings support GL orders 2,4,8,16, positive panels/upper limit and ≤1,048,576 nodes. Plain settings default 100/100/16. Omitted settings instead choose upper=max(80,√[2×27.631/(min(v0,θ)T)],27.631/c∞), c∞=√(1−ρ²)(v0+κθT)/σv, panels=ceil(upper), order16. Work-budget/overflow failure errors; no BS substitution.

## Conventions

CF integrates log spot, price uses continuous domestic discount and dividend carry. Settings are initial grids, not a promise that exactly those nodes will be used. Strip comparison requires all requested strikes to converge.

## Limitations, failure and fallback

Tail tests are finite diagnostics, not a mathematical tail bound. A difficult strike can make an entire strip fail; explicit coarse settings can be expanded. Probabilities and final prices are clamped only within tested tolerances, whereas parity puts additionally floor at zero.

## Alternatives and tradeoffs

Adaptive integrators/COS are possible alternative numerical inversions; no alternate Heston kernel is automatically selected. Cached strips trade memory for fewer CF evaluations.

## Implementation and validation evidence

[fourier_prices.rs](../../../../finstack-quant/models/src/closed_form/heston/fourier_prices.rs), [strip_pricer.rs](../../../../finstack-quant/models/src/closed_form/heston/strip_pricer.rs) and [params.rs](../../../../finstack-quant/models/src/closed_form/heston/params.rs). [tests.rs](../../../../finstack-quant/models/src/closed_form/heston/tests.rs) contains short-expiry reference numbers, unsupported-order/corruption/unresolved-grid rejection and scalar/strip consistency. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) uses separately generated QuantLib Heston values. Reference: [Heston](../../../REFERENCES.md#heston-1993).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
