# CDS Option

European payer/receiver options (credit swaptions) on a single-name CDS or a
CDS index. The Bloomberg CDSO numerical-quadrature model is the only pricing
engine: there is no closed-form Black-on-forward-spread alternative, and
`ModelKey::BloombergCdso` is the sole registration.

Strikes are quoted either on the forward spread (single-name, CDX IG, iTraxx) or
on a clean index price (the CDX HY convention), with explicit control over
settlement, protection-start convention, knockout, index factors, realized index
loss and the underlying CDS coupon.

## Public surface

Import path:
`finstack_quant_valuations::instruments::credit_derivatives::cds_option`
(`CDSOption` is also re-exported at `finstack_quant_valuations::instruments`).

| Item | Purpose |
|------|---------|
| `CDSOption` | The instrument. `CDSOption::new(id, &option_params, &credit_params, discount_curve_id, vol_surface_id)`; `CDSOption::example()`. |
| `CDSOptionParams` | Deal-level fields; `CDSOptionParams::call(..)` (payer) / `::put(..)` (receiver). |
| `CDSOptionStrike` | `Spread(Decimal)` or `CleanPricePct(Decimal)`. |
| `CDSOptionStrikeKind` | Discriminant for branching pricing and metric paths. |
| `ProtectionStartConvention` | `Spot` (default) or `Forward`. |

Useful methods on `CDSOption`: `with_implied_vol(vol)` (instrument-level vol
override, highest precedence), `effective_cash_settlement_date(as_of)`, and the
direct Greek entry points `delta`, `gamma`, `vega`, `theta` and
`implied_vol(curves, as_of, target_price, initial_guess)`. Each is the same
computation the corresponding metric calculator registers.

## Module layout

```
cds_option/
├── mod.rs                    # public re-exports and model overview
├── types.rs                  # CDSOption, ProtectionStartConvention, validation, example
├── parameters.rs             # CDSOptionParams (call/put constructors)
├── strike.rs                 # CDSOptionStrike, CDSOptionStrikeKind
├── bloomberg_quadrature.rs   # crate-private DOCS 2055833 quadrature kernel
├── pricer.rs                 # crate-private registry adapter and pricing primitives
└── metrics/                  # delta, gamma, vega, theta, dv01, spread_dv01, par_spread,
                              # implied_vol, recovery01
```

The quadrature kernel and synthetic-underlying builder are crate-private.
Callers enter through `Instrument::value`, `CDSOption` analytics, or registered
metrics so validation, volatility resolution, and lifecycle policy are applied.

Registered as `(InstrumentType::CdsOption, ModelKey::BloombergCdso)` →
`BloombergCdsoPricer` in [`src/pricer/credit.rs`](../../../pricer/credit.rs).

## Strike conventions

`CDSOptionStrike` is a typed enum; the pre-enum bare-decimal wire shape is
rejected with no compatibility fallback.

- **`Spread`** — decimal annual rate. `{"spread": "0.0325"}` means 325 bp.
  Single-name, CDX IG and iTraxx options quote this way.
- **`CleanPricePct`** — percentage-price points. `{"clean_price_pct": "107.0"}`
  means a clean-price fraction `K = 1.07`. CDX HY index options quote this way.
  A price strike requires an index underlying (`underlying_is_index`),
  no-knockout terms, an explicit `underlying_cds_coupon`, the current index
  factor `f` (`index_factor`), and the original strike factor `f0`
  (`strike_index_factor`). `f0` is never inferred from `f` after a default,
  because settled defaults reduce `f` below `f0`.

## Methodology

The pricer implements Bloomberg DOCS 2055833 Eq. 2.5:

```text
O = P(t_settle) · E_0[(ξ · V_te + H(K) + D)+]
```

- `V_te` is the random forward CDS value at option expiry. The state variable is
  the **lognormal forward CDS spread** for both strike conventions — a
  clean-price strike axis does not change the state variable.
- `H(K)` is the deterministic strike term, branched by strike kind:
  - spread strike: `H_spread = ξ · (c − K) · A(K)` (Eq. 2.4);
  - clean-price strike: `H_price = ξ · (K − 1) · f0 / f`, evaluated inside the
    outer current-factor scale `f`.
- `D` settles already-realized index losses: `D = ξ · L / f`.
  Expected future front-end protection enters the calibrated loss-adjusted
  forward; it is never also added to `D`.
- The lognormal mean `m` is calibrated so the process reproduces the
  forward CDS value plus `(1−R)·(1−q_te)` for a non-knockout index.
  Delivered-CDS protection starts at expiry and front-end protection stops
  there. Both annuities use the same coupon schedule and accrued-premium
  subtraction. The curve anchor uses observed survival; spread states use
  the model's flat hazard.

The native ATM-forward clean-price coordinate follows from payer/receiver parity
under the same payoff:

```text
K_ATM = 1 − (f·F0 + L) / f0
```

exposed in percentage points (`100 · K_ATM`) for moneyness and surface
selection. In the limit `f = f0 = 1`, `L = 0`, `FEP = 0` it reduces to
`K_ATM = 1 − F0`.

Underlying CDS mechanics follow Bloomberg CDSW conventions from DOCS 2057273
where relevant, including the inclusive protection-end adjustment. The
option's delivered CDS excludes pre-expiry defaults.

Spread variance uses Actual/365F time from the current valuation date to
legal expiry. Exercise proceeds are discounted to `exercise_settlement_date`
(default: expiry). `cash_settlement_date` describes the separately agreed
trade-premium payment and does not freeze variance or protection windows.

## Settlement

Cash- and physical-settled European options carry the same cash-equivalent model
NPV before expiry and route through the same quadrature. The clean payoff
excludes accrued, because the same underlying accrued appears on both sides
before exercise and cancels; a physical exercise cashflow at settlement is dirty
and includes accrued at exercise settlement.

This pricer values the **pre-expiry option only**. It does not create or deliver
a live underlying CDS position, and valuation at or after a physical exercise
lifecycle boundary fails explicitly rather than returning a misleading cash
number. Manual exercise state, partial exercise and post-expiry settlement
lifecycle are out of scope. Non-European exercise is rejected at pricing time,
so a deserialized instrument cannot silently fall through to an unsupported
engine.

## Volatility

The quadrature consumes a **lognormal forward-spread model volatility** for both
strike conventions. Resolution is strict, with no clamped fallback:

1. An instrument implied-vol override has highest precedence and needs no
   surface.
2. Otherwise the `VolSurface` under `vol_surface_id` is evaluated with
   `models::volatility::get_surface_vol(t_expiry, native_strike_coordinate)` — the decimal spread
   (`0.0325`) for spread strikes, the percentage clean price (`107.0`) for price
   strikes. Expiry or strike extrapolation is an error.
3. The surface must carry `VolSurfaceAxis::Strike` and
   `VolQuoteType::BlackLognormal`. Stored values are model **spread** vols even
   on a price-quoted strike axis; provider premiums or provider-specific "price
   vols" must be inverted to model vol before a surface is materialized.

## Usage

```rust
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

let option = CDSOption::example()?;
let as_of = date!(2024 - 01 - 05);
let pv = option.value(&market, as_of)?;
```

Building one explicitly:

```rust
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::{
    CDSOption, CDSOptionParams, CDSOptionStrike,
};
use finstack_quant_valuations::instruments::CreditParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;
use time::macros::date;

// Payer (call on spread) struck at 100 bp.
let params = CDSOptionParams::call(
    CDSOptionStrike::Spread(Decimal::new(1, 2)),
    date!(2025 - 06 - 20),   // expiry
    date!(2030 - 06 - 20),   // underlying CDS maturity
    Money::new(10_000_000.0, Currency::USD)?,
)?;

let credit = CreditParams::corporate_standard("CORP", "CORP-HAZARD");

let option = CDSOption::new(
    "CDSOPT-CALL-CORP-5Y",
    &params,
    &credit,
    "USD-OIS",
    "CDSOPT-VOL",
)?;
```

## Metrics

Registered for `InstrumentType::CdsOption` in `metrics/mod.rs`:

| `MetricId` | Definition |
|-----------|-----------|
| `Delta` | Closed-form Black-76 `N(d₁)` on the displayed ATM forward spread (matches the Bloomberg CDSO screen). Not valid for a clean-price strike — price-struck delta uses curve-reprice hedge-ratio semantics instead. |
| `Gamma` | Central difference of delta across a ±5 bp move in the displayed ATM forward. Same spread/price branch as delta. |
| `Vega` | One-sided forward difference of the canonical quadrature NPV on a `+0.01` lognormal-vol bump. |
| `Theta` | DOCS 2055833 §2.5 verbatim: shorten `t_e` by `1/365.25` and re-price. |
| `Cs01`, `BucketedCs01` | Credit par-quote curve bumps (re-bootstrap and re-price). Falls back to a parallel hazard shift when the hazard curve has no CDS quote points. |
| `SpreadDv01` | Underlying spread sensitivity. |
| `Dv01`, `BucketedDv01` | The canonical CDSO interest-rate sensitivity: bump the calibrated swap-curve quotes and rebuild the discount curve. |
| `ParSpread` | Bloomberg CDSO displayed ATM forward spread. |
| `ImpliedVol` | Solves the Bloomberg quadrature price in log-vol space. |
| `Recovery01` | Recovery-rate sensitivity. |

## Bindings

Reachable from Python and WASM through the JSON envelope
(`InstrumentJson::CdsOption` inside `finstack_quant.instrument/1`):

- **Python**: `finstack_quant.valuations.instruments.price_instrument(...)`.
- **WASM**: `valuations.instruments.priceInstrument`.

There is no typed `CDSOption` class in either binding; the typed credit surface
covers `CreditDefaultSwap`, `CDSIndex` and `CDSTranche`.

## Limitations

- European exercise only, pre-expiry valuation only.
- Lognormal spread volatility; stochastic recovery and stochastic volatility are
  out of scope.
- Distressed forward spreads beyond the Bloomberg CDSO calibration guard are
  rejected rather than extrapolated.
- Some Bloomberg CDSO internals remain proprietary; source-backed residuals are
  documented in the `cdx_ig_46` golden fixture rather than widened away.

## Verification

```bash
# CDS-option pricing, Greeks, implied vol, moneyness, knockout and golden regressions
cargo nextest run -p finstack-quant-valuations --test instruments cds_option::

# Whole workspace (never `cargo test` — it runs doctests)
mise run rust-test

# Lints
mise run rust-lint
```

Golden data and screenshots live in
[`tests/golden/data/pricing/bloomberg/cds_option/`](../../../../tests/golden/data/pricing/bloomberg/cds_option/);
the regression suite is
[`tests/instruments/cds_option/`](../../../../tests/instruments/cds_option/),
including `test_bloomberg_cdsw_parity.rs`, `test_cdx_hy_price_strike.rs` and
`test_cdx_ig_46_cdso_regressions.rs`.

## References

- Bloomberg L.P. Quantitative Analytics, *Pricing Credit Index Options*,
  DOCS 2055833 —
  [`docs/REFERENCES.md#bloomberg-cdso`](../../../../../../docs/REFERENCES.md#bloomberg-cdso)
- Bloomberg L.P. Quantitative Analytics, *The Bloomberg CDS Model*,
  DOCS 2057273 —
  [`docs/REFERENCES.md#bloomberg-cds-model`](../../../../../../docs/REFERENCES.md#bloomberg-cds-model)
- S&P Dow Jones Indices, *CDS Indices Primer* — clean-price strike factor/loss
  adjustment (the `107.0 → 107.9874` fixture) —
  [`docs/REFERENCES.md#sp-cds-indices-primer`](../../../../../../docs/REFERENCES.md#sp-cds-indices-primer)
- O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*,
  ch. 11 —
  [`docs/REFERENCES.md#o-kane-2008`](../../../../../../docs/REFERENCES.md#o-kane-2008)

## See also

- [`../cds/`](../cds/) — the underlying single-name CDS
- [`../cds_index/`](../cds_index/) — CDS index
- [`../../README.md`](../../README.md) — instrument module map and how to add one
- [`INVARIANTS.md`](../../../../../../INVARIANTS.md) — Decimal/f64, determinism and serde invariants
