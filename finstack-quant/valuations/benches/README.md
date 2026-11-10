# Valuations Benchmarks

Criterion benchmarks for valuation pricing, cashflows, and risk. The 34 targets
are declared in [`Cargo.toml`](../Cargo.toml). With `autobenches = false`, adding a
source file does not register a benchmark target.

## Run

Run commands from the repository root. Prefer the scoped task while iterating:

```bash
# Measure one target
mise run rust-bench-crate -- finstack-quant-valuations bond_pricing

# Select case IDs with a regular expression
mise run rust-bench-crate -- finstack-quant-valuations bond_pricing --filter '^bond_pv/5Y$'

# Exercise selected cases in Criterion correctness/smoke mode
mise run rust-bench-crate -- finstack-quant-valuations bond_pricing --filter '^bond_pv/' --test

# Save and compare measurements for the same selected cases
mise run rust-bench-crate -- finstack-quant-valuations bond_pricing --filter '^bond_pv/' --save-baseline before
mise run rust-bench-crate -- finstack-quant-valuations bond_pricing --filter '^bond_pv/' --baseline before

# Measure every registered benchmark across the workspace
mise run rust-bench
```

`--filter` matches Criterion case IDs, including the group name; it does not
select Cargo targets. `--test` checks execution and assertions without collecting
a timing baseline. It cannot be combined with either baseline flag, and
`--save-baseline` and `--baseline` are mutually exclusive. Save and compare with
the same selection, fixtures, settings, and machine conditions. Criterion reports
statistical changes; these tasks do not enforce a percentage regression gate.

Both tasks pass these reduced timing defaults to Criterion:

| Environment variable | Default | Meaning |
| --- | --- | --- |
| `FQ_BENCH_SAMPLE_SIZE` | `10` | Number of measurement samples; Criterion requires at least 10 |
| `FQ_BENCH_WARM_UP_TIME` | `1` | Warm-up duration in seconds |
| `FQ_BENCH_MEASUREMENT_TIME` | `1` | Target measurement duration in seconds; costly cases can take longer |
| `FQ_BENCH_NRESAMPLES` | `1000` | Bootstrap resamples for statistical estimates |

For a longer measurement:

```bash
FQ_BENCH_SAMPLE_SIZE=100 FQ_BENCH_WARM_UP_TIME=3 FQ_BENCH_MEASUREMENT_TIME=5 FQ_BENCH_NRESAMPLES=100000 \
  mise run rust-bench-crate -- finstack-quant-valuations bond_pricing --filter '^bond_pv/'
```

The valuation targets inherit these task settings without group-level timing
overrides. Other packages included by `rust-bench` may override Criterion
defaults in their benchmark sources. Environment variables above are interpreted
by the mise tasks, not directly by Criterion.

When running a copied or prebuilt Criterion 0.8 binary directly, pass `--bench`
to collect timings. `cargo bench` supplies this flag automatically. Without it,
the binary defaults to correctness/test mode even when timing or baseline
options are present. Supply the timing arguments explicitly as well; a direct
binary invocation does not interpret the `FQ_BENCH_*` task variables.

## Suite

This table inventories registered targets and their operations; it is not a
claim that every fixture already passes validation. Model and fixture repairs
are tracked separately from performance measurements.

| Target | Coverage and model |
| --- | --- |
| `bond_pricing` | Bond PV, yield, duration, DV01 and spreads; callable tree/OAS, call windows, and stochastic-hazard LSMC |
| `bond_rates_credit` | Callable bond PV and OAS through explicit `RatesCredit` dispatch with nonzero rate volatility; joint rates-credit LSMC |
| `bond_future_bench` | Conversion factor, model price, NPV, instrument value, DV01, bucketed DV01, and metric package |
| `swap_pricing` | IRS/OIS discounted cashflows, compensated summation stress, DV01, par rate, and standard metrics |
| `option_pricing` | Black-Scholes equity PV/Greeks, analytical barrier repricing for FD Greeks, and bucketed equity vega |
| `cms_pricing` | Default Black-76/Hagan CMS cap and swap pricing; an explicit static-replication cap uses smile quadrature |
| `cds_pricing` | Single-name CDS PV, calibrated parallel/bucketed CS01, and par spread |
| `cds_option_pricing` | Single-name and index CDS option PV, Greeks, and implied volatility |
| `cds_tranche_pricing` | Gaussian and Student-t copula tranche pricing, heterogeneous baskets, CS01, correlation delta, jump-to-default, and par spread |
| `cds_index_pricing` | Single-curve and constituent-curve index PV, par spread, CS01, risky PV01, and metrics |
| `structured_credit_pricing` | Deterministic pool/waterfall valuation, cashflows, tranche/pool metrics, and pool-size scaling |
| `swaption_pricing` | Black-76 PV, SABR-implied volatility, Greeks, forward swap rate, and annuity |
| `fourier_var_pricing` | Taylor-approximation historical VaR for a bond with 250/1,000 scenarios; no Fourier or COS pricing |
| `cashflow_generation` | Bond/swap cashflows, fixed schedule construction, and compensated summation |
| `linear_rates` | Deposit, FRA, basis swap, cap/floor PV and Greeks, repo, and interest-rate future |
| `mc_pricing` | Hull-White Bermudan swaption LSMC and tree pricing, with per-price preparation versus a prepared tree |
| `mc_exotics_pricing` | Explicit GBM Asian MC, lookback/autocallable/cliquet MC, and continuous/quarterly barriers with GBM, Heston, and PDE |
| `merton_mc_pricing` | Merton structural-credit MC: path count, tenor, antithetics, PIK, and terminal versus first-passage barriers |
| `fi_misc_pricing` | Term loans and callable trees, deterministic/MC revolvers, agency MBS/TBA and OAS, fixed-income TRS, CMO waterfalls, and dollar rolls |
| `pe_fund_pricing` | PE waterfall event/complexity scaling, European/American styles, and full pricing |
| `equity_pricing` | Spot, TRS, futures, variance swaps, volatility-index futures, and Black-76 future options |
| `convertible_pricing` | Binomial convertible PV, contract features, Greeks, metric packages, parity, and step convergence |
| `inflation_pricing` | Inflation swaps, cap/floor, inflation-linked bond PV, real yield/duration, and metrics |
| `commodity_pricing` | Forwards/swaps, Black-76 options/swaptions, arithmetic moment-matching and geometric Asian pricing, and Kirk spread approximation |
| `bucketed_risk` | Bond/swap parallel and bucketed DV01, CDS parallel and bucketed CS01, and combined metrics |
| `metrics` | Bond metric packages, requested-metric scaling, and sequential metrics across five bonds |
| `fx_pricing` | FX spot, forward, swap, NDF, vanilla option PV, and option Greeks |
| `fx_exotics_pricing` | Analytical barriers/touch/digital/quanto options and variance-swap replication |
| `composite_pricing` | Fixed-quantity equity composite value and primitive-exposure decomposition over 2/8/32/64 legs |
| `future_option_pricing` | European Black-76 PV; American future-delivery lattice PV and cash delta at 201/401 steps; raw terms at 401 steps with an untimed 801-step convergence check; American cash-settled closed-form control |
| `rate_exotics_pricing` | Analytical and quanto range accrual, explicit quanto GBM MC, Hull-White callable range-accrual LSMC, snowball, and TARN |
| `xccy_pricing` | Cross-currency swap tenor and notional-exchange variants |
| `fd_greeks` | Delta/gamma/vega/vanna/volga packages for equity and barrier options; Asian Greeks explicitly select `MonteCarloGBM` with 200 paths and 12 monthly fixings |
| `structured_credit_stochastic` | Deterministic waterfall control and stochastic MC over 100/1,000 paths with antithetics on/off |

The table describes the current source configuration, not a runtime validation
result. Smoke-check each case before saving a successful-pricing baseline and
repair any failures first. Error timings are not pricing measurements.

See each `benches/*.rs` file for case IDs, fixture sizes, requested metrics, and
timing boundaries. In particular, preparing a model outside the timer and
preparing it on every price are different operations.

Calibration targets (`calibration`, `credit_calibration`, `global_calibration`,
`df_bootstrap`, `fwd_curve`, `sabr_slice`) belong to
[`finstack-quant-calibration`](../../calibration/Cargo.toml). Fourier/COS model
benchmarks belong to [`finstack-quant-models`](../../models/benches/README.md).
Portfolio-scale risk benchmarks live in
[`../../portfolio/benches/`](../../portfolio/benches/).

## Output and execution

Criterion prints a terminal summary and writes measurement data below its output
root. By default this is the workspace Cargo target directory's `criterion/`
subdirectory, normally `target/criterion/` from the repository root. Criterion
honors `CRITERION_HOME` first, then `CARGO_TARGET_DIR`, then Cargo metadata's target
directory.

Paths follow the Criterion group/case ID, not the target filename. For example,
`bond_pricing` writes the five-year PV case under `bond_pv/5Y/`. Current estimates
and samples are under `new/`; a named saved baseline uses its own directory such
as `before/`; comparisons write under `change/`.

Both mise tasks pass `--noplot`, which disables plots and HTML reports. A direct
Criterion run with plotting enabled writes reports under
`<criterion-root>/<group>/<case>/report/index.html`. Smoke mode does not produce
fresh timing data; old files may still be present.

- Cargo's built-in `bench` profile inherits the workspace `release` profile:
  optimization level 3, fat LTO, and one codegen unit. Smoke runs through the same
  benchmark build profile.
- Rust lint/fix and normal test tasks omit benchmark targets. Use the scoped
  benchmark task to compile and smoke-check or measure a changed target.
- Benchmarks are not run by CI or `mise run all`. They are explicit local runs;
  see [the workflow documentation](../../../.github/workflows/README.md).
- Retain machine, revision, fixture/model, sample settings, and raw output when
  reporting timings. Historical latency tables do not establish current
  performance.
