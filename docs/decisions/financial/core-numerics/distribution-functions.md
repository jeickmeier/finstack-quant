# CORE-NUMERICS-12: Normal and Student-t functions and logarithmic Gaussian tails

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/special_functions.rs`: error function, log-Gamma, standard/general normal density/CDF/inverse, log normal CDF, and centered unit-scale Student-t CDF/inverse. The code supplies scalar functions, not a copula or financial model.

## Choice and rationale

Most distribution functions delegate to the existing `statrs` dependency. Normal density is evaluated directly. Log normal CDF has a special negative-tail expansion to retain representable log probabilities after the CDF itself underflows. No normal approximation replaces large-df Student-t calls.

## Algorithm

1. `erf`, `ln_gamma`, normal CDF/inverse, and Student-t CDF/inverse call statrs. A lazily initialized N(0,1) is shared. General normal functions standardize `(x−mean)/std_dev`, and PDF also divides by std_dev.
2. Normal PDF is `exp(−x²/2)/sqrt(2pi)` with a stored normalization constant.
3. For log normal CDF, x≥0 uses `log1p(−Phi(−x))`; −10<x<0 uses `log(Phi(x))`. At x≤−10 use `−x²/2−log(−x)−log(2pi)/2+log(S)`, with asymptotic sum S starting at 1 and successive term `−term*(2k−1)/x²`. Stop before terms increase, at epsilon relative to the sum, or after at most 99 updates.
4. Normal and Student-t inverse calls explicitly map p≤0 to −infinity, p≥1 to infinity, and NaN to NaN. Student-t validates finite df>0 first.

## Selection, controls and conventions

No numerical tolerance is exposed. General normal methods require finite positive standard deviation but do not independently reject nonfinite x or mean. Student-t scale is one, not variance one: finite variance, where defined, differs from that scale. Consumers must apply any variance normalization themselves.

## Limitations, failure and fallback

Out-of-domain finite p saturates rather than errors. Student-t invalid df errors; NaN x/p returns Ok(NaN). `ln_gamma` and erf inherit dependency domain behavior. The logarithmic tail uses an asymptotic series with a fixed switch at −10; it does not return an error estimate. No checked financial input domain is inferred.

## Alternatives and tradeoffs

Direct versus logarithmic CDF use is explicit; multiplying an underflowed CDF cannot recover information retained by log-space evaluation. A normal cutoff for large-df Student-t would change tail behavior and is absent. Fully log-domain Student-t tails are not supplied by these wrappers.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/special_functions.rs).
- `logarithmic_normal_cdf_preserves_underflowed_tails` uses tabulated log-erfc values at −10,−40,−100 with 2e-12 absolute log tolerance; generation/provenance of those values is not linked in the test. Normal/Student-t roundtrips share dependency kernels and are consistency evidence. Boundary and invalid-df tests establish saturation/error contracts.
- [Numerical Recipes context](../../../REFERENCES.md#press-numerical-recipes); dependency implementation provenance/version lives in Cargo.lock, not in these wrappers.

Existing tests do not demonstrate every representable input.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
