# CORE-NUMERICS-17: PCG normal draws, Brownian bridge ordering and Poisson counts

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/random.rs`, `random/brownian_bridge.rs`, and `random/poisson.rs`: seeded pseudorandom uniforms/normals, Brownian paths on increasing time grids, and Poisson quantile counts. Generic process and product selection remains outside this slice.

## Choice and rationale

PCG64 is the supplied pseudorandom generator; Box–Muller produces two normals per two uniforms and caches one. Brownian bridge changes the mapping of independent normals to path time levels to prioritize long-horizon variation. Poisson inversion preserves a single-uniform count mapping across low/high intensities. These are independently selected mechanisms, not automatically enabled together.

## Algorithm

1. `Pcg64Rng::new(seed)` delegates to `seed_from_u64`. `new_with_stream(seed,stream)` supplies state `(stream<<64)|seed` and sequence argument `(stream<<1)|1` to `Pcg64::new`.
2. Uniform uses rand's f64 generation. Normal consumes a cached standard normal if present, otherwise draws U1,U2 and calculates `r=sqrt(−2 log(clamp(U1,MIN_POSITIVE,1−EPSILON)))`, angle=2pi U2, pair `(r cos(angle),r sin(angle))`; second normal is cached. Requested mean/std multiply only at output.
3. A bridge precomputes midpoint indices recursively, terminal first. Set W0=0, Wn=sqrt(tn) z0. For midpoint m between known l,r, conditional mean is `Wl+(tm−tl)/(tr−tl)(Wr−Wl)` and variance `(tm−tl)(tr−tm)/(tr−tl)`. Use successive normals in depth-first left/right midpoint order.
4. Poisson validates finite lambda≥0 and u∈[0,1). Zero lambda/u returns count 0. For lambda<30, accumulate P0=exp(−lambda) and recurrence `Pk=Pk−1 lambda/k` through at most 200 counts.
5. If low-intensity accumulation does not reach u, or lambda≥30, use statrs CDF for u≤0.5 or survival for u>0.5. Double an integer upper bound from 1 until its CDF reaches u, then binary search the smallest satisfying count; detect usize/range overflow and invalid dependency probabilities.

## Selection, controls and conventions

Seeds and stream IDs are explicit. Bridge construction fixes number of steps; supplied times include t0=0, use consistent year units, and must be finite strictly increasing. Bridge output is Brownian levels, so increments require subtraction. Poisson lambda is expected count for the caller's interval, not an annual intensity automatically multiplied by dt.

## Limitations, failure and fallback

Normal parameters and Box–Muller uniforms are not validated beyond U1 clamping. Bridge validates dimensions/times but not shock finiteness. Poisson uses exact-distribution numerical inversion rather than a normal fallback; the low-intensity recursion is only its fast branch. PCG stream independence is not proven by different seeds or the deterministic stream construction. Side effects from normal caching make draw-order changes alter subsequent paths.

## Alternatives and tradeoffs

Direct increments versus bridge ordering and PCG versus the separate [Sobol API](sobol-sequences.md) are explicit caller choices. Inverse normal mapping is used by Sobol, not PCG. A generic RNG trait does not promise quasi-Monte Carlo error estimates.

## Evidence

- [RNG and inline tests](../../../../finstack-quant/core/src/math/random.rs), [bridge](../../../../finstack-quant/core/src/math/random/brownian_bridge.rs), [Poisson](../../../../finstack-quant/core/src/math/random/poisson.rs).
- `test_pcg64_rng_normal_statistics` and uniform-statistics cases are sampled-moment evidence; seed/stream cases establish repeatability and distinguish sampled sequences without proving independence. `test_brownian_bridge_irregular_grid_construction` checks supplied conditional values, while `precomputed_brackets_match_per_path_lookup_bit_for_bit` compares two implementations of the same mapping. Poisson `quantile_brackets_cdf_on_both_sides_of_normal_approximation_threshold` checks the statrs quantile inequalities at lambda 29.9–100; its name is historical and no normal approximation remains. These are not all-platform bit-identity or empirical independence proofs.
- [PCG](../../../REFERENCES.md#o-neill-2014-pcg), [Box–Muller](../../../REFERENCES.md#box-muller-1958), [Monte Carlo](../../../REFERENCES.md#glasserman-2004-monte-carlo).

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
