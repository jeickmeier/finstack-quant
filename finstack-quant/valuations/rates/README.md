# finstack-quant-valuations-rates

The interest-rate family of the valuations stack, a crate of its own since
the valuations split. It implements the framework in
[`finstack-quant-valuations-core`](../core/README.md) and is re-exported by
the umbrella crate [`finstack-quant-valuations`](../README.md) as
`finstack_quant_valuations::instruments::rates`, so every public path there
is unchanged. Depends on `finstack-quant-core`, `finstack-quant-cashflows`,
`finstack-quant-models`, `finstack-quant-margin` (the `Marginable` impls of
`InterestRateSwap` and `Repo`) and valuations-core only; no other instrument
family. The term-fixing helpers the TARN and snowball notes share with the
CMS products live in valuations-core (`instruments::pricing::term_fixings`).

Interest-rate and inflation instruments, from money-market deposits to Bermudan
swaptions, under the post-2008 multi-curve framework: an OIS discount curve plus
one or more projection curves per leg, and a normal or lognormal vol surface for
optionality. Fifteen instrument leaves plus one shared engine directory, about
45k lines total.

This file is an index — which leaf owns which product and which convention it
follows — plus the conventions the whole family shares. `irs/` carries its own
README with the full public surface and worked examples; this file links to it
rather than repeating it.

## Leaves

| Directory | Prices | Market convention / model | Own README |
|-----------|--------|---------------------------|------------|
| `irs/` | Vanilla fixed/float swaps and OIS-style compounded-RFR swaps | ISDA leg conventions; par rate from actual floating PV divided by fixed-leg unit annuity; `from_conventions` resolves per-index conventions from the registry | [yes](irs/README.md) |
| `basis_swap/` | Float-for-float basis swaps | Spread is added to the **primary** leg by convention; used to calibrate tenor and index basis (Ametrano–Bianchetti 2013; Fujii–Shimada–Takahashi 2010) | no |
| `xccy_swap/` | Cross-currency swaps | Per-leg projection and discount curves, explicit calendars with no implicit fallback, leg PVs collapsed to a reporting currency at spot FX. `NotionalExchange` = `None`, `Final`, `InitialAndFinal`, `MtmResetting`; `ResettingSide` picks the MtM-resetting leg. Reset lag is applied when building periods; overnight-RFR legs use the shared compounded coupon engine | no |
| `cap_floor/` | Caps, floors, caplets and floorlets | `CapFloorVolType::Auto` treats quotes as lognormal, using Black-76 for positive forwards/strikes and an equivalent-normal Bachelier fallback otherwise; normal surfaces must select `Normal`. Term schedules use the explicit calendar or the currency-standard rates calendar. Compounded RFR coupons distinguish lookback, observation shift and cutoff, and discount through payment delay. `expiry()` is the final contractual fixing date. An optional dated positive premium is paid by the holder and deducted from NPV while unsettled | no |
| `swaption/` | European and Bermudan swaptions | Black-76 / Bachelier / SABR for Europeans; HW1F tree, LSMC and LMM-BGM for Bermudans. Fixed and floating legs carry independent conventions. Cash settlement defaults to collateralized cash price; par-yield, ISDA par-par and zero-coupon methods remain explicit alternatives. Physical `value()` prices the pre-exercise option claim; the delivered swap lifecycle is external | no |
| `deposit/` | Money-market deposits | Simple (uncompounded) interest. Day count and spot lag come from the index convention registry, not from the instrument: ACT/360 and T+2 for USD, EUR and CHF; ACT/365F for GBP (T+0) and JPY (T+2). Calibrates the overnight-to-1Y end of the discount curve | no |
| `fra/` | Forward rate agreements | Settled at period start with the characteristic `1/(1 + F·τ)` adjustment; "3×6" names a 3-month rate fixing in 3 months | no |
| `repo/` | Term, open and overnight repurchase agreements | Cash-lender perspective: outflow on the adjusted start date, principal plus simple repo interest on the adjusted maturity date. `CollateralType::Special` may move the repo rate; the haircut sizes required collateral but does not change cashflows or base PV. Implements `Marginable` with an optional `RepoMarginSpec`; margin calls, substitutions and tri-party operations are caller-generated, not modeled here | no |
| `ir_future/` | STIR futures (SOFR, CORRA, Fed Funds, EURIBOR, historical ED/Short Sterling) | Quoted `100 − rate`; supports term rates, arithmetic overnight averages, and compounded overnight rates over explicit exchange reference periods. Historical observations are strict fixings; only unobserved days project from the forward curve. Contract specs and reference-period rules resolve from the convention registry (for example `CME:SR3`, `CME:SR1`, `CME:ZQ`, `MX:COA`, and `MX:CRA`) | no |
| `cms_swap/` | Constant-maturity swaps | Default `Black76` applies the first-order Hagan (2003) linear-swap-rate convexity adjustment at the ATM vol. Beyond roughly 10Y tenor or in high-vol regimes that understates the adjustment by ~5–10 bp; select `StaticReplication` for smile-aware pricing (requires positive forward swap rates) | no |
| `cms_option/` | Caps, floors and options on a CMS rate | Black-76 on the convexity-adjusted CMS forward, or static replication over a swaption portfolio (Hagan 2003; Brigo–Mercurio §13.7) | no |
| `cms_spread_option/` | Options on the spread between two CMS rates (steepener/flattener) | SABR marginals joined by a Gaussian copula, with CMS convexity from static replication. Default model is `StaticReplication` | no |
| `inflation_swap/` | Zero-coupon and year-on-year inflation swaps | Compounded fixed breakeven against the CPI ratio; 3-month index lag; CPI-U (USD), HICP ex-tobacco (EUR), RPI or CPI (GBP). Calibrates the real curve | no |
| `inflation_cap_floor/` | Caps and floors on YoY inflation | Black-76 or Bachelier on the forward YoY inflation rate derived from the inflation curve or index fixings | no |

Two non-instrument files sit beside the leaves: `forward_swap_rate.rs` (forward
swap rate, annuity and term-curve checks shared by the CMS leaves and
`ir_future`) and `cms_fixings.rs` (historical CMS fixing lookup, crate-private).
The Hull-White 1-factor Monte Carlo and LSMC engines that the swaption,
cap/floor, CMS, TARN, snowball, callable range accrual, MBS OAS and revolver
pricers drive live in `finstack_quant_models::rates::hw1f`; the κ/σ resolution
from pricing overrides or calibrated market scalars is
`finstack_quant_valuations_core::instruments::pricing::hw1f_params`, and the shared
`BermudanCallProvision` contract term is re-exported at
`finstack_quant_valuations::instruments`.

## Public surface

Import path: `finstack_quant_valuations_rates::<leaf>::<Type>` (or
`finstack_quant_valuations::instruments::rates::<leaf>::<Type>` through the
umbrella). All fifteen directories are `pub mod`; the crate root exports only the
`RATES` family static. The headline types are also flat at
`finstack_quant_valuations::instruments`:

`InterestRateSwap`, `BasisSwap`, `XccySwap`, `CapFloor`, `RateOptionType`,
`Swaption`, `BermudanSwaption`, `CmsSwap`, `CmsOption`, `CmsSpreadOption`,
`Deposit`, `ForwardRateAgreement`, `InterestRateFuture`,
`Repo`, `RepoType`, `CollateralSpec`, `CollateralType`,
`InflationSwap`, `YoYInflationSwap`, `InflationCapFloor`.

Reachable only under the family path — not exhaustive, but the types callers
actually reach for: the builders (`CapFloorBuilder`,
`SwaptionBuilder`, `RepoBuilder`, `InflationSwapBuilder`,
`YoYInflationSwapBuilder`, `InflationCapFloorBuilder`), `CapFloorVolType`,
`OvernightCouponConvention`, `OvernightSpreadCompounding`,
`swaption::{BermudanSchedule, BermudanType, CashSettlementMethod,
BermudanSwaptionPricer, BermudanSwaptionTreeValuator,
SimpleSwaptionBlackPricer, SimpleSwaptionNormalPricer}`, `xccy_swap::XccySwapLeg`,
`cms_swap::{FundingLeg, FundingLegSpec}` and `ir_future::FutureContractSpecs`.
`NotionalExchange` and `ResettingSide` are valuations-core types at
`finstack_quant_valuations::instruments`.
Reusable `HullWhiteParams` and Hull-White equations live at
`finstack_quant_models::rates::hull_white`; the HW1F Monte Carlo and LSMC
engines at `finstack_quant_models::rates::hw1f`.

Floating-leg compounding is the cashflows enum
`finstack_quant_cashflows::builder::FloatingLegCompounding`; there is no
valuations alias for it.

Swaptions use the shared `ExerciseStyle`, `SettlementType` and
`VolatilityModel` (`black` / `normal`) re-exported at
`finstack_quant_valuations::instruments`.

Inside a leaf, `metrics/`, `types.rs`/`types/`, `pricer.rs`/`pricing/` and the
model-specific pricers (`hw_pricer.rs`, `bermudan/`) are `pub(crate)` or private; supported
items surface through each leaf's `pub use`. Genuinely public submodules are
`swaption::lmm_pricer`, `cms_option::pricer` and `cms_option::replication_pricer`; `cms_common`,
`cms_fixings`, `cms_swap::pricer` and `forward_swap_rate` are `pub(crate)`.

## Family conventions

- **Multi-curve.** Every leg names its own `discount_curve_id` and, when
  floating, its own `forward_curve_id`. Never derive a forward from a discount
  curve implicitly. `market_dependencies()` must list all of them — the
  `curve_dependency_completeness` and `forward_curve_dependency_completeness`
  tests fail otherwise.
- **Clocks.** Year fractions for discounting come from the *curve's* day count
  measured from the *curve's* base date, not from the instrument's. Use
  core's date-based curve methods (`DiscountCurve::df_between_dates`,
  `ForwardCurve::curve_time`, `ForwardCurve::rate_between_dates`,
  `ForwardCurve::rate_period_between_dates`); do not call `disc.df(t)` with an
  instrument-derived `t`.
- **Leg pricing is shared.** `instruments::pricing::swap_legs` owns fixed-leg
  valuation and payment-delay handling. Floating coupons use the canonical
  cashflow builder; `instruments::pricing::overnight` supplies compounded-RFR
  projection (lookback, observation shift, cutoff, fixings) for IRS, cap/floor
  and their risk paths. New swap-like legs reuse these kernels.
- **Rates are `f64`, notionals are `Money`.** Spreads are basis points on the
  wire (`Bps`, `spread_bp`) and decimals internally. `Decimal`-typed wire fields
  convert through `common_impl::numeric::decimal_to_f64` so a bad value is a
  typed validation error.
- **Negative rates.** Anything that can see a non-positive forward must offer a
  normal/Bachelier or shifted-lognormal path. `cms_swap`'s `StaticReplication`
  and `cms_option`'s replication pricer require positive forwards and must not
  be the default in a negative-rate regime.
- **Conventions come from the registry.** Per-index, swaption, IR-future,
  cross-currency and inflation-swap conventions load from
  `data/conventions/*.json` through `crate::market::conventions`; see
  [`../../market/README.md`](../../market/README.md). `Deposit`,
  `ForwardRateAgreement` and `InterestRateSwap` expose `from_conventions`
  constructors that go through it.
- **Determinism.** LSMC, LMM and HW1F Monte Carlo take explicit seeds via
  `finstack_quant_models::rates::hw1f::Hw1fMcConfig` and must reproduce
  bit-identically.

## Registration

The general checklist is in [`../src/instruments/README.md`](../src/instruments/README.md#adding-an-instrument).
Every leaf registers in [`src/family.rs`](src/family.rs) (`RATES`), including
`xccy_swap`, the CMS products, the Bermudan swaption and the inflation
instruments:

| Leaf | Registered by |
|------|---------------|
| `irs`, `basis_swap`, `deposit`, `fra`, `repo`, `ir_future`, `ir_future_option`, `xccy_swap`, `cap_floor`, `swaption` (European: Black-76, Discounting, HullWhite1F) | `register_pricers`, mostly through `register_generic!` |
| `cms_swap`, `cms_option`, `cms_spread_option`, `swaption` (Bermudan: LSMC, tree, LMM) | concrete pricers |
| `inflation_swap`, `inflation_cap_floor` | `register_generic!` and `SimpleInflationCapFloorPricer` |

Other steps:

- `InstrumentType` variant in `../core/src/pricer/keys.rs`.
- One line in `with_instrument_json_registry!` in `../src/instruments/json_loader.rs` (umbrella), category
  `"rates"`. Current tags: `interest_rate_swap`, `basis_swap`, `xccy_swap`,
  `inflation_swap`, `yoy_inflation_swap`, `inflation_cap_floor`,
  `forward_rate_agreement`, `swaption`, `bermudan_swaption`,
  `interest_rate_future`, `cap_floor`, `cms_swap`, `cms_option`,
  `cms_spread_option`, `deposit`, `repo`. Note `swaption/`
  and `inflation_swap/` each own two tags.
- `register_<name>_metrics` in the leaf's `metrics/`, called from
  `register_metrics` in `family.rs`. Bermudan swaption greeks are
  registered there with the documented default Hull-White parameters
  (`DEFAULT_KAPPA`, `DEFAULT_SIGMA`); production risk should re-register on a
  cloned registry with parameters calibrated to co-terminal Europeans.
- `mise run rust-gen-schemas`, verified by `mise run rust-check-schemas`.

## Tests and benches

Integration tests that need only this crate, valuations-core and the
test-support crate live in `tests/<leaf>/`, compiled into the single `rates`
target (`mise run rust-test-crate -- finstack-quant-valuations-rates`): `irs`,
`basis_swap`, `cap_floor`, `fra`, `ir_future`, `cms_option`, `inflation_swap`,
`xccy_swap`, plus `inflation_cap_floor/pricing.rs` and `repo/margin.rs`. The
IRS builder they share is `tests/support/rates.rs`; the umbrella's test trees
and benches include that file by path. Tests that price through the standard
registries or reach another family stay in the umbrella's
`../tests/instruments/<leaf>/`: `deposit/`, `repo/` (bar `margin.rs`),
`swaption/` (its shared fixtures build a `Bond`), `future_option/`,
`inflation_cap_floor/metrics.rs` and the rates wire-key tests. `cms_swap` and
`cms_spread_option` are covered by colocated `#[cfg(test)]` modules;
`swaption/pricing/numeraire_tests.rs` holds the in-crate no-arbitrage
numéraire checks for the Bermudan engines.

```bash
# whole target
cargo nextest run -p finstack-quant-valuations --test valuations instruments

# one leaf (filter is a substring match on the test name)
cargo nextest run -p finstack-quant-valuations --test valuations instruments::swaption::
cargo nextest run -p finstack-quant-valuations --test valuations instruments::irs::

# colocated unit tests
cargo nextest run -p finstack-quant-valuations --lib rates::cms_swap

# the HW1F engines themselves
cargo nextest run -p finstack-quant-models --lib rates::hw1f

# whole workspace, what CI runs
mise run rust-test
```

Use `cargo nextest`, not `cargo test` — the latter also runs doc tests, which
this project keeps out of the normal loop. Lint with `mise run rust-lint`.

Criterion benches in `../benches/`: `swap_pricing` (IRS PV, DV01, par
rate), `swaption_pricing` (Black and SABR), `cms_pricing`, `xccy_pricing`,
`inflation_pricing`, and `linear_rates` (deposit, FRA, basis swap, cap/floor,
repo, futures).

```bash
cargo bench -p finstack-quant-valuations --bench swap_pricing
mise run rust-bench          # all workspace benches, short sampling
```

## Related

- [`../README.md`](../README.md) — `Instrument` trait, JSON contract, add-an-instrument checklist
- [`../common_impl/README.md`](../common_impl/README.md) — shared leg pricing, parameter types and the trait plumbing
- [`irs/README.md`](irs/README.md) — the reference leaf for this family
- [`../fixed_income/README.md`](../fixed_income/README.md) — bonds, loans, mortgages, structured credit
- [`../fx/README.md`](../fx/README.md) — FX instruments, including the `xccy_swap` pricer shard
- [`../../calibration/README.md`](../../calibration/README.md) — deposits, FRAs, futures and swaps as calibration instruments
- [`../../market/README.md`](../../market/README.md) — the convention registry these leaves resolve against
- [`../tests/instruments/README.md`](../tests/instruments/README.md) — test layout and generated fixtures
