# Valuations Benchmarks

Criterion benchmarks for the maintained valuation hot paths. The suite is
manifest-driven (`autobenches = false`) so new files do not expand benchmark
runtime unless they are deliberately added as a `[[bench]]` entry in
[`Cargo.toml`](../Cargo.toml).

## Run

```bash
# All benches
cargo bench -p finstack-quant-valuations

# One bench
cargo bench -p finstack-quant-valuations --bench bond_pricing

# Faster iteration
cargo bench -p finstack-quant-valuations -- --quick

# Save / compare baseline
cargo bench -p finstack-quant-valuations --bench bond_pricing -- --save-baseline main
cargo bench -p finstack-quant-valuations --bench bond_pricing -- --baseline main
```

Workspace-wide tasks run every crate's benches with reduced Criterion timing;
they are the form CI uses:

```bash
mise run rust-bench           # all workspace benches, short sampling
mise run rust-bench-baseline  # save baseline 'main'
mise run rust-bench-compare   # compare vs 'main', fail above 10% median regression
```

Sampling is tunable through `FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`,
`FQ_BENCH_MEASUREMENT_TIME`, and `FQ_BENCH_NRESAMPLES`.

## Suite

| Bench | Focus |
|-------|--------|
| `bond_pricing` | PV, YTM, duration, DV01, tree/OAS, spreads |
| `bond_future_bench` | Bond future conversion factor, NPV, invoice, and risk examples |
| `swap_pricing` | IRS PV, DV01, par rate |
| `option_pricing` | Equity option PV and Greeks |
| `cms_pricing` | CMS option and CMS swap quadrature examples |
| `cds_pricing` / `cds_option_pricing` / `cds_tranche_pricing` / `cds_index_pricing` | Credit pricing across single-name, option, tranche, and index instruments |
| `structured_credit_pricing` | Structured-credit waterfall and tranche valuation |
| `swaption_pricing` | Black and SABR swaption pricing |
| `fourier_var_pricing` | COS strip pricing, Heston Fourier scalar pricing, Taylor-approximation historical VaR |
| `cashflow_generation` | Bond/swap cashflow generation, schedule build, and summation examples |
| `calibration` | Plan-driven discount and forward calibration |
| `global_calibration` | GlobalSolve discount and hazard calibration examples |
| `linear_rates` | Deposit, FRA, basis swap, cap/floor, repo, and futures examples |
| `df_bootstrap` / `fwd_curve` | Focused discount and forward curve bootstrap examples |
| `mc_pricing` | Public Monte Carlo pricing path |
| `mc_exotics_pricing` | Asian, lookback, autocallable, and cliquet Monte Carlo; continuous/quarterly barriers with GBM, Heston and PDE |
| `rate_exotics_pricing` | Analytic and quanto range accrual (including GBM), callable range accrual, snowball and TARN |
| `merton_mc_pricing` | Structural credit Merton Monte Carlo examples |
| `fi_misc_pricing` | Miscellaneous fixed-income instruments not covered above |
| `pe_fund_pricing` | Private-equity waterfall, style, and full-pricing examples |
| `equity_pricing` | Equity spot, TRS, futures, variance swap, and vol-index examples |
| `convertible_pricing` | Convertible tree pricing, greeks, metrics, and parity |
| `inflation_pricing` | Inflation swap, cap/floor, and linked-bond examples |
| `commodity_pricing` | Commodity forward, swap, option, Asian, spread, and swaption examples |
| `bucketed_risk` | Key-rate vs parallel risk |
| `metrics` | Pricing metrics pipeline |
| `sabr_slice` | SABR slice calibration |
| `fx_pricing` | FX spot, forward, swap, NDF, option, and metrics |
| `fx_exotics_pricing` | FX barrier, touch, digital, variance swap, and quanto examples |
| `xccy_pricing` | Cross-currency swap tenor and notional-exchange examples |

See each `benches/*.rs` file for scenario names.

## Output

- Terminal summary from Criterion
- HTML: `target/criterion/<group>/<benchmark>/report/index.html`. The path keys
  off the `benchmark_group(...)` name, not the bench target file —
  `bond_pricing.rs` writes under `bond_pv/`, `bond_ytm_solve/`, `bond_dv01/`,
  and so on. Note the `mise run rust-bench*` tasks pass `--noplot`.

## Portfolio benches

Portfolio-scale benches live in [`../../portfolio/benches/`](../../portfolio/benches/).

## Notes

- Benches use release builds (`--profile bench`).
- Bench targets are skipped by `mise run rust-fmt` and `mise run rust-lint`.
  Compile and measure them with `mise run rust-bench`; they are not executed by
  `mise run rust-test`.
- Latency tables in older docs are indicative only; re-measure on your hardware
  after material changes.
