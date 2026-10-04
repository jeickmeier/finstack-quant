# Monte Carlo source layout

Navigation aid for people working *inside* this crate. It maps directories and
files to responsibilities and records which items are public.

Crate purpose, usage examples, conventions, determinism guarantees, runtime
validation, and verification commands are in [`../README.md`](../README.md).
Per-item detail is in the rustdoc (`cargo doc -p finstack-quant-models --open`).

## Directories

| Path | Role | Public modules |
|------|------|----------------|
| `barriers/` | Brownian-bridge hit probability and continuity corrections for discretely monitored barriers | `bridge`, `corrections` |
| `discretization/` | Time-stepping and exact transitions | all except `qe_common` (`pub(crate)`) |
| `engine/` | Generic simulation loop, config, path capture | files are private `mod`s; `McEngine`, `McEngineConfig`, `PathCaptureConfig`, `PathCaptureMode`, `MAX_NUM_PATHS`, `MAX_CAPTURED_PATHS` are re-exported from `engine/mod.rs` |
| `greeks/` | Sensitivity estimators | `pathwise`, `lrm`, `finite_diff`, `gbm_european` |
| `payoff/` | Contract definitions evaluated on `PathState` | `vanilla`, `asian`, `barrier`, `lookback` |
| `pricer/` | Higher-level workflows over `McEngine` | `european`, `path_dependent`, `lsmc`, `heston`, `basis`, `lsq` |
| `process/` | SDE definitions and process metadata | all 12 model modules plus `metadata` |
| `rng/` | Random and quasi-random generation | `philox`, `sobol`, `fbm`, `volterra`; `brownian_bridge`, `poisson`, and `BrownianBridge` live in `finstack_quant_core::math::random` |
| `variance_reduction/` | `control_variate` only — antithetic pairing lives in the engine loop, not here | `control_variate` |

## Top-level files

| File | Role |
|------|------|
| `lib.rs` | Module declarations, crate docs, `prelude` |
| `traits.rs` | `RandomStream`, `StochasticProcess`, `Discretization`, `Payoff`, `PathState`, `StateKey`, `state_keys`, `ProportionalDiffusion` |
| `engine_fractional.rs` | `simulate_path_fractional` — per-path loop that injects pre-generated fractional noise, for rough-volatility processes the generic engine rejects |
| `estimate.rs` | `Estimate`: raw f64 mean / stderr / CI / optional distribution statistics |
| `results.rs` | `MoneyEstimate`, `MonteCarloResult`, `RunMetadata` — currency-tagged outputs |
| `paths.rs` | `PathDataset`, `SimulatedPath`, `PathPoint`, `PathSamplingMethod`, `ProcessParams`, `CashflowType` |
| `registry.rs` | Embedded runtime defaults from `../data/defaults/pricer_defaults.v1.json`; `MONTE_CARLO_DEFAULTS_EXTENSION_KEY` |
| `seed.rs` | `derive_seed` — FNV-1a seed derivation from instrument id + scenario name |
| `gbm_paths.rs` | Private module; `simulate_gbm_paths`, `GbmPathConfig`, `GbmPathSummary` are re-exported at the crate root |
| `captured_path_stats.rs` | Private: folds captured-path distribution statistics into an `Estimate` |
| `indexed_spot_table.rs` | Private: static `spot_0` … `spot_127` key table backing `traits::state_keys::indexed_spot` (higher indices fall through to a cached overflow path) |
| `mc_process_params_serialization.rs` | `#[cfg(test)]` only |

## Where behavior lives inside `engine/`

| File | Contents |
|------|----------|
| `mod.rs` | Engine-module docs plus the `pub use` list that forms the crate's `engine` surface |
| `config.rs` | `McEngineConfig`, `MAX_NUM_PATHS`, `MAX_CAPTURED_PATHS` |
| `path_capture.rs` | `PathCaptureConfig`, `PathCaptureMode`, the deterministic hash-based sampling rule |
| `pricing.rs` | `McEngine`, `price`, `price_with_capture`, all runtime validation, `adaptive_chunk_size`, result finalization |
| `simulation.rs` | Engine-internal per-path loops, none re-exported publicly: `run_path_loop` and `NoiseHook` (`pub(crate)`, used by `engine_fractional`), `simulate_path` / `simulate_path_with_capture` / `simulate_antithetic_pair` (engine-private) |
| `tests.rs` | Engine unit tests, including the bit-identical serial/parallel and thread-pool-invariance pins |

## Adding to the crate

Adding a process, scheme, or payoff means adding a leaf file plus a `pub mod`
line in the matching `mod.rs`. Route public re-exports through that `mod.rs` so
each item has one canonical path; the `prelude` in `lib.rs` is a curated list on
top of those, not a second source of truth.
