# CORE-NUMERICS-19: Fractional Brownian increment covariance

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/fractional.rs`: validated Hurst exponent wrapper and covariance matrices for increments on caller-specified time grids. No sampler, factorization, financial drift or rough-volatility equation is implemented here.

## Choice and rationale

Use the fractional Brownian covariance directly, then take a four-corner difference to obtain increment covariance. `HurstExponent` admits finite 0<H<1 and labels H<0.5 as rough; the scalar/matrix helpers separately accept a raw h and do not enforce that wrapper.

## Algorithm

Set `C(t,s)=0.5[|t|^(2h)+|s|^(2h)−|t−s|^(2h)]`. For increments [ti,ti1] and [tj,tj1], compute `C(ti1,tj1)−C(ti1,tj)−C(ti,tj1)+C(ti,tj)`. A grid of n+1 times produces a dense n×n nalgebra matrix by evaluating this formula at each pair. Fewer than two times produces a 0×0 matrix.

## Selection, controls and conventions

H and the grid are caller-supplied; no approximation, jitter or matrix repair occurs. Time units determine covariance units of time^(2H). The same raw h must be used consistently with any downstream process normalization.

## Limitations, failure and fallback

Raw helper inputs are unchecked: nonfinite/nonmonotone/negative times and invalid h can produce mathematically unintended values or NaN. Absolute times in the formula permit negative arguments mechanically; that is not a financial negative-time endorsement. Four-term subtraction can lose precision for small increments far from zero. The output is not guaranteed PSD after floating-point error and is not automatically repaired.

## Alternatives and tradeoffs

Direct covariance and dense factorization in consumers support irregular grids but cost O(n²) storage. Toeplitz/FFT methods for uniform grids are separate sampler choices, not selected here. Stable increment-specific expansions are not implemented.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/fractional.rs), [Mandelbrot–Van Ness reference](../../../REFERENCES.md#mandelbrot-van-ness-1968).
- `fbm_cov_h_half_is_min` checks the H=0.5 Brownian formula; `increment_covariance_matrix_symmetric` checks symmetry and shape; `hurst_serde_round_trip_preserves_valid_exponents` checks the validated wrapper. They do not certify cancellation behavior at arbitrary scales or a downstream rough-volatility model.

Covariance construction alone does not validate a sampler or model.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
