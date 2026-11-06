# CORE-NUMERICS-18: Finite-period Sobol points and prefix-dependent scrambling

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/random/sobol.rs`: dimensions 1..40, 32-bit direction numbers, deterministic point filling, optional seeded scrambling and inverse-normal conversion. This API does not estimate uncertainty or enforce a randomized-QMC replicate design.

## Choice and rationale

Stored Joe–Kuo-labelled direction data avoid repeated parameter generation. A prefix-dependent bit-flip transform scrambles coordinates deterministically. Midpoint conversion keeps all coordinates strictly inside (0,1), avoiding infinite normal quantiles. The reference label alone does not certify the chosen hash scrambler's theoretical randomization properties.

## Algorithm

1. First coordinate direction bit i is `1<<(31−i)`. Remaining coordinates use stored polynomial degree/coefficient/initial odd direction values and the Sobol XOR recurrence to fill 32 columns.
2. Point integer value is XOR of direction columns selected by binary digits of the index. Incremental advancement flips every direction column changed by binary addition (all lower bits through the first new one); it is checked against the same direct definition, not a Gray-code reorder.
3. Seed 0 disables scrambling. Otherwise derive 32 per-dimension seeds with `hash_combine`; process coordinate bits MSB to LSB, derive the hash input from already-scrambled higher bits XOR that bit's seed, and flip the current bit when the final mixed hash low bit is one.
4. Convert uint32 value v to `(v+0.5)/2^32`. Standard normals apply the shared inverse normal CDF to these coordinates.
5. Index starts at 0 and wraps to 0 after 2^32 points. `skip(n)` advances modulo that period and recomputes coordinate state directly. `reset` restores index/state but preserves seed configuration.

## Selection, controls and conventions

Dimension and scramble seed are explicit. Buffer fills consume successive complete points; buffer length multiple-of-dimension is guarded only by `debug_assert`. No scalar RNG-trait adapter, random start, independent-replicate standard error, or automatic power-of-two count is supplied here.

## Limitations, failure and fallback

Unsupported dimension errors. The finite midpoint map truncates normal tails at the two extreme midpoint quantiles. A partial final buffer chunk is accepted in release builds and still advances one point. Repeated period points are deterministic repeats, not additional information. Dimension/seed equality implies matching implementation output; no cross-library sequence equivalence or scramble distribution guarantee is established.

## Alternatives and tradeoffs

Unscrambled and seeded-scrambled modes are implemented. [PCG](pseudorandom-and-bridge.md) is a separate pseudorandom alternative. Higher-dimensional direction sets or other scrambling algorithms would change the sequence and require evidence; they are not fallbacks above 40 dimensions.

## Evidence

- [Implementation, direction table and inline tests](../../../../finstack-quant/core/src/math/random/sobol.rs).
- `incremental_state_matches_from_scratch_definition` proves kernel consistency at inspected indices. Seed, reset, skip, period-wrap and finite-normal tests cover contracts. Those tests do not independently authenticate all table entries or prove discrepancy/error-rate properties.
- [Joe–Kuo](../../../REFERENCES.md#joe-kuo-2008-sobol), [Sobol](../../../REFERENCES.md#sobol-1967), [Owen](../../../REFERENCES.md#owen-1995-scrambling). Exact table-file provenance and correspondence of this hash implementation to the cited randomization remain evidence gaps.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
