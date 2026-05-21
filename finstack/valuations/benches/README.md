# Valuations benchmarks

Criterion benchmarks for pricing, metrics, calibration, and attribution hot paths.

## Run

```bash
# All benches
cargo bench -p finstack-valuations

# One bench
cargo bench -p finstack-valuations --bench bond_pricing

# Faster iteration
cargo bench -p finstack-valuations -- --quick

# Save / compare baseline
cargo bench -p finstack-valuations --bench bond_pricing -- --save-baseline main
cargo bench -p finstack-valuations --bench bond_pricing -- --baseline main
```

## Suite (high level)

| Bench | Focus |
|-------|--------|
| `bond_pricing` | PV, YTM, duration, DV01, tree/OAS, spreads |
| `swap_pricing` | IRS PV, DV01, par rate |
| `option_pricing` | Equity option PV and Greeks |
| `cds_pricing` / `cds_option_pricing` / `cds_tranche_pricing` / `cds_index_pricing` | Credit instruments |
| `swaption_pricing` | Black and SABR swaptions |
| `structured_credit_pricing` / `convertible_pricing` | Structured and hybrid FI |
| `calibration` | Curve and surface bootstrap |
| `bucketed_risk` | Key-rate vs parallel risk |
| `cashflow_generation` | Schedules and Kahan summation |
| `fx_pricing` / `linear_rates` / `equity_pricing` / `commodity_pricing` / `inflation_pricing` / `fi_misc_pricing` | Other asset classes |
| `attribution` / `attribution_scale` | P&L attribution |
| `metrics` | Metrics pipeline |
| `mc_exotics_pricing` / `merton_mc_pricing` | Monte Carlo paths (when MC pricers are enabled in the build) |

See each `benches/*.rs` file for scenario names.

## Output

- Terminal summary from Criterion
- HTML: `target/criterion/<bench>/<group>/report/index.html`

## Portfolio benches

Portfolio-scale benches live in `finstack/portfolio/benches/`.

## Notes

- Benches use release builds.
- Latency tables in older docs are indicative only; re-measure on your hardware after material changes.
