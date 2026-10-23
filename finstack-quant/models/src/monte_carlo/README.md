# Monte Carlo source layout

Navigation aid for people working *inside* this crate. It maps directories and
files to responsibilities and records which items are public.

Crate purpose, usage examples, conventions, determinism guarantees, runtime
validation, and verification commands are in [`../README.md`](../README.md).
Per-item detail is in the rustdoc (`cargo doc -p finstack-quant-models --open`).

## Directories

| Path | Role | Public modules |
|------|------|----------------|
| `discretization/` | Time-stepping and exact transitions | all except `qe_common` (`pub(crate)`); no flat re-exports |
| `engine/` | Generic simulation loop, config, path capture, fractional-noise path loop | files are private `mod`s; `McEngine`, `McEngineConfig`, `PathCaptureConfig`, `PathCaptureMode`, `MAX_NUM_PATHS`, `MAX_CAPTURED_PATHS`, `simulate_path_fractional` are re-exported from `engine/mod.rs` |
| `greeks/` | Sensitivity estimators | `lrm`, `finite_diff`, `gbm_european` |
| `payoff/` | Contract definitions evaluated on `PathState`; `barrier` also owns the Brownian-bridge crossing test and `GOBET_MIRI_BETA` | `vanilla`, `asian`, `barrier`, `lookback`, `autocallable`, `cliquet`, `range_accrual` |
| `pricer/` | Higher-level workflows over `McEngine` | `european`, `path_dependent`, `lsmc`, `heston`, `basis`, `lsq`, `polynomial` |
| `process/` | SDE definitions | all model modules (including Dupire `local_vol`); no flat re-exports |
| `rng/` | Random and quasi-random generation | `philox`, `sobol`, `fbm`, `volterra`; `brownian_bridge`, `poisson`, and `BrownianBridge` live in `finstack_quant_core::math::random` |

## Top-level files

| File | Role |
|------|------|
| `mod.rs` | Module declarations and Monte Carlo usage documentation |
| `traits.rs` | `RandomStream`, `StochasticProcess`, `ProcessMetadata`, `Discretization`, `Payoff`, `PathState`, `StateKey`, `state_keys`, `ProportionalDiffusion` |
| `control_variate.rs` | `apply_control_variate` — antithetic pairing lives in the engine loop, not here |
| `estimate.rs` | `Estimate`: raw f64 mean / stderr / CI / optional distribution statistics |
| `results.rs` | `MoneyEstimate`, `MonteCarloResult`, `RunMetadata` — currency-tagged outputs |
| `paths.rs` | `PathDataset`, `SimulatedPath`, `PathPoint`, `PathSamplingMethod`, `ProcessParams`, `CashflowType` |
| `registry.rs` | Embedded runtime defaults from `../data/defaults/pricer_defaults.v1.json`; `embedded_defaults()` and `MonteCarloDefaults` |
| `seed.rs` | `derive_seed` — FNV-1a seed derivation from instrument id + scenario name |
| `simulate.rs` | `simulate_paths`: one entry point returning a compact `PathSummary` for any built-in process (Markov, LMM or rough-volatility) and scheme named by a serializable `PathSimulationSpec` (`ProcessSpec`, `SchemeSpec`, `TimeGridSpec`) |
| `captured_path_stats.rs` | Private: folds captured-path distribution statistics into an `Estimate` |
| `mc_process_params_serialization.rs` | `#[cfg(test)]` only |

## Where behavior lives inside `engine/`

| File | Contents |
|------|----------|
| `mod.rs` | Engine-module docs plus the `pub use` list that forms the crate's `engine` surface |
| `config.rs` | `McEngineConfig`, `MAX_NUM_PATHS`, `MAX_CAPTURED_PATHS` |
| `path_capture.rs` | `PathCaptureConfig`, `PathCaptureMode`, the deterministic hash-based sampling rule |
| `pricing.rs` | `McEngine`, `price`, `price_with_capture`, all runtime validation, `adaptive_chunk_size`, result finalization |
| `fractional.rs` | `simulate_path_fractional` — per-path loop that injects pre-generated fractional noise, for rough-volatility processes the generic engine rejects |
| `simulation.rs` | Engine-internal per-path loops, none re-exported publicly: `run_path_loop`, `NoiseHook`, and `fill_shocks` (`pub(crate)`, shared with `fractional.rs`), plus private uniform-draw handling |
| `tests.rs` | Engine unit tests, including the bit-identical serial/parallel and thread-pool-invariance pins |

## Adding to the crate

Adding a process, scheme, or payoff means adding a leaf file plus a `pub mod`
line in the matching `mod.rs`. Each item has one public path: in `process/`,
`discretization/` and `payoff/` it is the child module path (no flat
re-exports); in `engine/` the files are private and `engine/mod.rs` re-exports.
Import types from their owning domain modules; no wildcard prelude is provided.
