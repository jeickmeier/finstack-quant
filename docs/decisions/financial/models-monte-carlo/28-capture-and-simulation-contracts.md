# MODELS-MONTE-CARLO-28: Captured-path diagnostics and public process/scheme dispatch

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in paths.rs, captured_path_stats.rs, results.rs, engine/path_capture.rs and simulate.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and algorithm

Captured diagnostics are separate from full-population price moments. `Sample(count,seed)` hashes path IDs and compares to a probability threshold count/requested_paths; count is an expected count, not a fixed-size sample. Final paths are sorted by ID. Median/quartiles use core sorted quantiles and min/max use only captured final discounted values. Point cashflows retain time/type; totals simply sum amounts. Captured IRR, however, extracts amounts only and calls periodic `cashflow::irr` when at least two flows exist, silently omitting IRR on solver error. Cashflow times and simultaneous-flow aggregation do not enter that calculation.

## Dispatch and controls

`simulate_paths` returns raw process coordinates for every time, no payoff/discounting, with 1–100,000 streams, optional adjacent antithetic paths, and a 64-million-value allocation budget. It revalidates parameters and finite states at each recorded step. Defaults: exact GBM/multi-GBM/dividend GBM/Hull–White/Schwartz–Smith; Euler Brownian/multi-Brownian/multi-OU; log-Euler local vol; QE CIR/CIR++/Heston; dedicated LMM and rough schemes. Euler is an allowed override for ordinary diffusions, log-Euler for GBM/local vol, Milstein only proportional GBM. Unsupported combinations error; credit-line and piecewise-GBM are lower-level models outside this serializable enum. CIR++ input/output uses actual shifted rates while internal state is unshifted. Schwartz–Smith output is X/Y; Cheyette output is x/y, not projected short rate.

Metadata validates square correlation dimensions and factor-name counts, not correlation numerical admissibility or complete model provenance. Run metadata records seed only where the caller adds it. Monetary estimates wrap mean/CI in currency units and retain optional subset diagnostics; relative stderr returns infinity for |mean|<1e-10.

## Limitations and alternatives

Expected-count capture can exceed its requested count and nominal 100,000 capture budget because the budget validates the request, not realized hash hits. Capture memory has no general points-times-dimension cap in the pricing engine. Auto-stopping samples against the original requested denominator, so its captured count may be smaller than expected for actual completed paths. Fixed-size reservoir/hash-ranked samples, actual-time IRR and richer provenance are unimplemented alternatives. Do not interpret captured quantiles as guaranteed full-population risk metrics.

## Evidence

[Paths/metadata](../../../../finstack-quant/models/src/monte_carlo/paths.rs), [hash sampling](../../../../finstack-quant/models/src/monte_carlo/engine/path_capture.rs), [quantiles](../../../../finstack-quant/models/src/monte_carlo/captured_path_stats.rs), [result conversion](../../../../finstack-quant/models/src/monte_carlo/results.rs), [simulation dispatcher](../../../../finstack-quant/models/src/monte_carlo/simulate.rs); [engine tests](../../../../finstack-quant/models/src/monte_carlo/engine/tests.rs) inspect cashflow preservation, subset statistics and auto-stop counts. [Simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs) cover every ordinary process/scheme pair, direct Gaussian/CIR moments, validation and serialization; [non-Markov tests](../../../../finstack-quant/models/src/monte_carlo/simulate/non_markov_tests.rs) cover LMM/rough selection and shared-kernel equivalence. [Serialization tests](../../../../finstack-quant/models/src/monte_carlo/mc_process_params_serialization.rs) are roundtrip/schema evidence only. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) supplies statistical background; it does not justify periodic IRR for arbitrary dated diagnostic cashflows.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
