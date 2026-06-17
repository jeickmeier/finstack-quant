# Valuations Benchmarks

Criterion benchmarks for the maintained valuation hot paths. The suite is
manifest-driven (`autobenches = false`) so new files do not expand benchmark
runtime unless they are deliberately added to `Cargo.toml`.

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
| `cashflow_generation` | Bond/swap cashflow generation, schedule build, and summation examples |
| `calibration` | Plan-driven discount and forward calibration |
| `global_calibration` | GlobalSolve discount and hazard calibration examples |
| `credit_factor_calibration` | Representative credit factor calibration panel |
| `linear_rates` | Deposit, FRA, basis swap, cap/floor, repo, and futures examples |
| `df_bootstrap` / `fwd_curve` | Focused discount and forward curve bootstrap examples |
| `mc_pricing` | Public Monte Carlo pricing path |
| `mc_exotics_pricing` | Asian, lookback, autocallable, and cliquet Monte Carlo examples |
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
- HTML: `target/criterion/<bench>/<group>/report/index.html`

## Portfolio benches

Portfolio-scale benches live in `finstack-quant/portfolio/benches/`.

## Notes

- Benches use release builds.
- Latency tables in older docs are indicative only; re-measure on your hardware after material changes.
