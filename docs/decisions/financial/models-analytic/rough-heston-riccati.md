# MODELS-ANALYTIC-12: Fractional Riccati rough-Heston characteristic function and Lewis pricing

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

European calls/puts with positive Heston parameters and H in (0,.5), α=H+.5. At positive expiry the price function requires finite positive spot/strike and finite rate, yield and expiry. Before those checks, every t≤0—including negative infinity—returns undiscounted intrinsic provided spot and strike are finite; that branch permits negative spot/strike and does not inspect rate/yield. The public characteristic function instead rejects negative/nonfinite expiry and nonfinite frequency/rate/yield before returning one at t=0. Standalone fractional solver constructor does not validate step count/H/maturity.

## Choice and rationale

An implicit product-integration fractional Riccati solve feeds a fixed Lewis contour quadrature. Time refinement is tested; frequency truncation is fixed. This is distinct from classical Heston integration.

## Algorithm

Solve fractional D with forcing a=−(u²+iu)/2, b=iuρσv−κ,c=σv²/2. Precompute convolution weights ∝h^α/Γ(α+2) from differences of powers. Each step combines all history then solves the implicit quadratic with the rationalized root 2constant/[1−bweight+√((1−bweight)²−4cweight constant)]. Nonfinite trajectory fills remaining values with NaN. C=κθ∫D ds uses trapezoids; v0 multiplies a piecewise-linear fractional integral I^(1−α)D. CF adds iu(r−q)T. Lewis call integrates Re[exp((.5+iu)ln(F/K)) φdemeaned(u−i/2)]/(u²+.25), then C=discounted S−discounted K×integral/π.

## Selection and controls

Riccati steps start200, double to1600 for price and3200 for public CF. Acceptance requires successive difference≤1e−6×max(S exp(−qT), K exp(−rT)) for prices or absolute1e−6 for CF, and call bounds within tolerance. Both spot and strike are discounted in the price stopping scale. GL order16 with one panel on [1e−8,1], one on [1,5],16 on [5,200]. Exponent real>700 rejects. Put via parity; IV uses shared Black inversion and maps failure to None.

## Conventions

H is dimensionless roughness, σv and variance inputs follow Heston convention; time in years. Rates contribute deterministic drift/discount only. Fourier upper limit200 is not chosen from maturity/variance.

## Limitations, failure and fallback

Failed convergence returns NaN, not Result; accepted prices may retain tiny bound excursions (no clamp). Refinement compares the same quadrature/tail cutoff, so it cannot measure omitted frequency-tail error. O(N²) history work has fixed caps. Classical H=.5 is excluded rather than dispatched to classical Heston.

## Alternatives and tradeoffs

Classical Heston is an implemented separate alternative. Adaptive contour/tail integration, fast convolution or a Markovian approximation are unimplemented here and require independent tradeoff decisions.

## Implementation and validation evidence

[rough_heston.rs](../../../../finstack-quant/models/src/volatility/rough_heston.rs). `stiff_rough_heston_price_matches_refined_native_solver` compares against the same numerical kernel; deterministic-variance limit is an analytic Gaussian/Black check. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) has classical-limit comparisons at H=.499, not independent finite-H rough-Heston references. Reference: [El Euch–Rosenbaum](../../../REFERENCES.md#el-euch-rosenbaum-2019).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
