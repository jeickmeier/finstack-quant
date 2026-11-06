# MODELS-MONTE-CARLO-02: Philox normals and stable stream seeds

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in rng/philox.rs, rng/sobol.rs and seed.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice, domain and rationale

The production pseudorandom stream is counter-based Philox4x32 with ten rounds. A root 64-bit seed and 64-bit path stream ID determine draws independently of scheduling. This supports reproducible common random numbers and parallel path simulation. Sobol is a delegated core generator with a special adapter; it advertises no splitting and quasi-random dependence.

## Algorithm and controls

Philox packs the 64-bit counter into the first two counter words and stream ID into the last two, uses the two seed words as keys, and executes ten multiply/high-low/XOR rounds with Weyl key increments. Uniforms take 52 upper random bits and add one half, yielding values strictly inside (0,1); two uniforms feed core Box–Muller, with the unused normal cached across odd requests. `with_stream_u01_offset` selects the exact block and word for a uniform offset. Splitting resets a stream to its beginning; it does not inherit parent consumption. `derive_seed` hashes instrument bytes, delimiter 0xff, and scenario bytes with stable FNV-1a-64.

## Conventions, failure and alternatives

Normal draws are unit variance; scaling by square-root time belongs to the scheme. A zero seed is valid. FNV is an identity-to-seed convention, not collision-free identity or a cryptographic guarantee. Counter overflow wraps. Generic engine QMC rejection prevents treating dependent Sobol points as independent samples. Alternative normal transforms or RNGs would change reproducibility and Greek covariance even with the same seed. Statistical tests do not prove independence of every stream.

## Evidence

[Philox implementation and tests](../../../../finstack-quant/models/src/monte_carlo/rng/philox.rs) include `test_philox_matches_random123_reference_vector` (external reference-vector comparison), stream correlation, open-interval boundary and spare-normal tests. [Seed hashing](../../../../finstack-quant/models/src/monte_carlo/seed.rs) tests deterministic identity/scenario changes. [Sobol adapter](../../../../finstack-quant/models/src/monte_carlo/rng/sobol.rs) rejects splitting. Canonical references: [Salmon/Philox](../../../REFERENCES.md#salmon-2011-philox), [Box–Muller](../../../REFERENCES.md#box-muller-1958), [Joe–Kuo](../../../REFERENCES.md#joe-kuo-2008-sobol). Core direction numbers and scrambling are dependencies, not implemented in these files.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
