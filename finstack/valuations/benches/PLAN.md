# Benchmark Expansion Plan

Adds dedicated Criterion benchmarks for hot paths that currently lack coverage.
Each new file follows the same conventions as existing benches: Criterion groups,
`black_box`, `BenchmarkId`, `test_utils` include, `harness = false`.

---

## Priority 1: Global Calibration Solver

**File:** `benches/global_calibration.rs`
**Cargo.toml:** `[[bench]] name = "global_calibration" harness = false`

Benchmarks the `GlobalFitOptimizer::optimize` and `optimize_with_multi_start`
paths that are absent from the existing `calibration.rs` (which only exercises
sequential bootstrap plans).

### Scenarios

| Group | Scenario | Description |
|-------|----------|-------------|
| `global_discount_curve` | `8_quotes` | LM fit of discount curve (8 deposit/swap quotes) |
| `global_discount_curve` | `16_quotes` | LM fit of discount curve (16 quotes) |
| `global_discount_curve` | `22_quotes` | LM fit of discount curve (22 quotes — full strip) |
| `global_discount_analytical_jac` | `16_quotes` | Same as above but with `use_analytical_jacobian: true` |
| `global_hazard_curve` | `6_tenors` | LM fit of hazard curve (6 CDS par spreads) |
| `global_multi_start` | `16_quotes_5_restarts` | Multi-start LM with 5 restarts |
| `global_multi_start` | `16_quotes_10_restarts` | Multi-start LM with 10 restarts |
| `global_condition_number` | `small_8` / `medium_16` | Isolate the power-iteration condition-number diagnostic |

### Setup pattern

Reuse the `CalibrationPlan` / `CalibrationStep` envelope from `calibration.rs`
but set `method: CalibrationMethod::GlobalSolve { use_analytical_jacobian }`.
Build rate quote vectors of varying size and run `engine::execute_plan`.

---

## Priority 2a: Merton MC Bond Engine

**File:** `benches/merton_mc_pricing.rs`
**Cargo.toml:** `[[bench]] name = "merton_mc_pricing" harness = false required-features = ["mc"]`

Benchmarks the Merton structural MC pricer in
`instruments/fixed_income/bond/pricing/engine/merton_mc.rs`.

### Scenarios

| Group | Scenario | Description |
|-------|----------|-------------|
| `merton_mc_paths` | `1K` / `5K` / `10K` / `50K` | PV scaling with path count (5Y PIK bond, monthly steps) |
| `merton_mc_tenor` | `3Y` / `5Y` / `10Y` | PV scaling with bond tenor (10K paths) |
| `merton_mc_antithetic` | `on` / `off` | Antithetic variates cost vs. benefit (10K effective paths) |
| `merton_mc_features` | `plain` / `pik_toggle` / `first_passage` | Feature flag impact on per-path cost |

### Setup pattern

Construct a bond with `PricingOverrides` that enable Merton MC
(`model_key = MertonMC`, `mc_paths`, `mc_seed`). Use `Instrument::value` or
the `MertonMcPricer` directly.

---

## Priority 2b: Stochastic Revolving Credit MC

**File:** `benches/rcf_mc_pricing.rs`
**Cargo.toml:** `[[bench]] name = "rcf_mc_pricing" harness = false required-features = ["mc"]`

Benchmarks the 3-factor MC engine for revolving credit facilities
(utilization OU + HW1F short rate + CIR credit spread + Cholesky
decorrelation), currently absent from `fi_misc_pricing.rs` which only
benches the deterministic path.

### Scenarios

| Group | Scenario | Description |
|-------|----------|-------------|
| `rcf_mc_paths` | `1K` / `5K` / `10K` | PV scaling with path count (5Y facility) |
| `rcf_mc_tenor` | `3Y` / `5Y` / `7Y` | PV scaling with facility tenor (5K paths) |
| `rcf_mc_factors` | `1_factor` / `3_factor` | Cost of correlated vs. single-factor model |

### Setup pattern

Build a `RevolvingCredit` with `PricingOverrides` that activate the MC engine
(supply `mc_seed_scenario`). Provide utilization / HW / CIR parameters. Build
market with discount + forward + hazard curves.

---

## Priority 3: PE Fund Waterfall

**File:** `benches/pe_fund_pricing.rs`
**Cargo.toml:** `[[bench]] name = "pe_fund_pricing" harness = false`

Benchmarks the `WaterfallEngine` and Brent-based IRR solves in
`instruments/equity/pe_fund/waterfall.rs`.

### Scenarios

| Group | Scenario | Description |
|-------|----------|-------------|
| `pe_fund_pv` | `example` | `PrivateMarketsFund::example()` end-to-end PV |
| `pe_fund_waterfall` | `4_events` / `20_events` / `100_events` | Waterfall engine scaling with event count |
| `pe_fund_irr` | `simple` / `multi_tier` | IRR solver (2-tier vs. 4-tier waterfall) |
| `pe_fund_style` | `european` / `american` | Waterfall style impact (aggregate vs. per-deal) |

### Setup pattern

Use `PrivateMarketsFund::example()` for small case. Build larger funds with
`WaterfallSpec::builder()` chains and `FundEvent::contribution` /
`FundEvent::proceeds` / `FundEvent::distribution` sequences.

---

## Priority 4: Cross-Currency Swap

**File:** `benches/xccy_pricing.rs`
**Cargo.toml:** `[[bench]] name = "xccy_pricing" harness = false`

Benchmarks `XccySwap` pricing (two-currency floating legs, notional exchange,
FX conversion), missing from both `swap_pricing.rs` and `linear_rates.rs`.

### Scenarios

| Group | Scenario | Description |
|-------|----------|-------------|
| `xccy_swap_pv` | `2Y` / `5Y` / `10Y` / `30Y` | PV scaling with tenor |
| `xccy_swap_notional_exchange` | `none` / `final` / `both` | Impact of notional exchange legs |
| `xccy_swap_metrics` | `dv01` / `bucketed_dv01` | Risk metric overhead |

### Setup pattern

Build a EUR/USD XCCY swap with `XccySwap::builder()`. Provide USD-OIS,
EUR-OIS discount curves, USD-SOFR-3M and EUR-EURIBOR-3M forward curves, and
an `FxMatrix` with EUR/USD spot. Tenor-parameterized via `BenchmarkId`.

---

## Priority 5: Exotic Instruments

### 5a: CMS Instruments

**File:** `benches/cms_pricing.rs`
**Cargo.toml:** `[[bench]] name = "cms_pricing" harness = false`

| Group | Scenario | Description |
|-------|----------|-------------|
| `cms_swap_pv` | `5Y` / `10Y` / `30Y` | CMS swap with convexity adjustment |
| `cms_option_pv_convexity` | `3M` / `6M` / `1Y` | CMS option (Hagan convexity-adjusted Black) |
| `cms_option_pv_replication` | `3M` / `6M` / `1Y` | CMS option (static replication — Gauss-Legendre) |
| `cms_option_greeks` | `6M` | Delta, Gamma, Vega via FD bump-reprice |

### 5b: Range Accrual

**File:** `benches/range_accrual_pricing.rs`
**Cargo.toml:** `[[bench]] name = "range_accrual_pricing" harness = false required-features = ["mc"]`

| Group | Scenario | Description |
|-------|----------|-------------|
| `range_accrual_static_replication` | `1Y` / `2Y` / `5Y` | Digital call-spread replication PV |
| `range_accrual_mc` | `1K` / `5K` / `10K` | MC path-dependent pricing |

### 5c: FX Exotic Options

**File:** `benches/fx_exotics_pricing.rs`
**Cargo.toml:** `[[bench]] name = "fx_exotics_pricing" harness = false required-features = ["mc"]`

| Group | Scenario | Description |
|-------|----------|-------------|
| `fx_barrier_option_mc` | `1K` / `5K` / `10K` | FX barrier (down-and-out) MC |
| `fx_barrier_option_analytical` | `3M` / `6M` / `1Y` | Analytical barrier pricer |
| `fx_barrier_option_vanna_volga` | `6M` | Vanna-Volga smile adjustment |
| `fx_touch_option_pv` | `3M` / `6M` / `1Y` | One-touch / no-touch analytical |
| `fx_digital_option_pv` | `3M` / `6M` | BS / Vanna-Volga / Replication |
| `fx_variance_swap_pv` | `3M` / `6M` / `1Y` | FX variance swap PV |
| `fx_quanto_option_pv` | `3M` / `6M` | Quanto-adjusted BS |

---

## Priority 6: FI Total Return Swap

**File:** Extend existing `benches/fi_misc_pricing.rs` (no new file needed)

| Group | Scenario | Description |
|-------|----------|-------------|
| `fi_trs_pv` | `1Y` / `3Y` / `5Y` | Fixed income TRS PV (bond + financing leg) |
| `cmo_pv` | `sequential` / `pac` | CMO waterfall PV (extends fi_misc coverage) |
| `dollar_roll_pv` | `1M` / `3M` | Dollar roll PV |

### Setup pattern

Append new benchmark functions and add them to the existing `criterion_group!`
macro in `fi_misc_pricing.rs`.

---

## Priority 7: Remaining Instruments

These are lower-priority but straightforward additions to existing bench files.

### Extend `commodity_pricing.rs`

| Group | Scenario | Description |
|-------|----------|-------------|
| `commodity_swaption_pv` | `3M` / `6M` | Black76 on commodity swap |
| `commodity_spread_option_pv` | `3M` / `6M` | Kirk's approximation |

### Extend `equity_pricing.rs`

| Group | Scenario | Description |
|-------|----------|-------------|
| `cliquet_option_mc` | `1K` / `5K` | MC periodic resets |
| `dcf_equity_pv` | `stable` / `growth` | Multi-stage DCF valuation |

### Extend `mc_exotics_pricing.rs`

| Group | Scenario | Description |
|-------|----------|-------------|
| `cliquet_mc` | `1K` / `5K` | Cliquet from exotics angle (if not in equity_pricing) |

---

## Implementation Checklist

For each new bench file:

1. Create `benches/<name>.rs` with `use criterion::{...}` boilerplate
2. Include `test_utils` via `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/test_utils.rs"))` where needed
3. Add `[[bench]]` entry to `Cargo.toml` with `harness = false` (and `required-features` for MC)
4. Add scenario documentation to `benches/README.md`
5. Run `cargo bench --bench <name>` to verify correctness and collect baseline numbers
6. Add typical latency numbers to README performance table

### New Cargo.toml entries (in order)

```toml
[[bench]]
name = "global_calibration"
harness = false

[[bench]]
name = "merton_mc_pricing"
harness = false
required-features = ["mc"]

[[bench]]
name = "rcf_mc_pricing"
harness = false
required-features = ["mc"]

[[bench]]
name = "pe_fund_pricing"
harness = false

[[bench]]
name = "xccy_pricing"
harness = false

[[bench]]
name = "cms_pricing"
harness = false

[[bench]]
name = "range_accrual_pricing"
harness = false
required-features = ["mc"]

[[bench]]
name = "fx_exotics_pricing"
harness = false
required-features = ["mc"]
```

### Existing files to extend

- `fi_misc_pricing.rs` — add FI TRS, CMO, dollar roll
- `commodity_pricing.rs` — add swaption, spread option
- `equity_pricing.rs` — add cliquet, DCF equity (non-MC scenarios only; MC in mc_exotics)

---

## Notes

- All bench functions use `black_box()` on both market data and `as_of` to prevent constant folding
- MC benches use a fixed `seed` for determinism across runs
- `Throughput::Elements(n)` set for MC benches so Criterion reports paths/sec
- Condition-number bench may need to extract the internal function; if not public, bench the end-to-end global calibration instead and note the diagnostic overhead
- CMS replication bench is the most latency-sensitive new scenario (quadrature × vol lookups); expect 50–200 μs per fixing period
