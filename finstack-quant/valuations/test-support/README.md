# finstack-quant-valuations-test-support

Dev-only test support shared by the integration tests of
[`finstack-quant-valuations`](../README.md) and of every instrument-family
crate under `finstack-quant/valuations/`. It depends on
[`finstack-quant-core`](../../core/README.md) only, so a family crate can
dev-depend on it without pulling in any sibling family. Helpers that build an
instrument stay in the crate that owns the instrument.

Not `finstack-quant-test-utils`: that crate is a dev-dependency of `core`,
`models` and `calibration`, so giving it valuations dependencies would compile
valuations into every `core` test build.

## Contents

| Module | What it provides |
|---|---|
| `assertions` | `assert_positive`, `assert_non_negative`, `assert_finite` |
| `builders` | `TestMarketBuilder`, `simple_option_market` |
| `fixtures` | `base_date`, USD discount curves |
| `tolerances` | `TIGHT`, `ANALYTICAL`, `NUMERICAL`, `CURVE_PRICING`, `RELATIVE`, `BUMP_VS_ANALYTICAL`, `STATISTICAL` |
| `parity` | `ParityConfig`, `compare_values`, `assert_parity!` |
| `test_helpers` | standard dates; flat discount, forward and hazard curves; `usd_swap_market`, `credit_market`; `usd`, `eur` |
| `date`, `discount_forward_curves`, `volatility`, `commodity_curves` | one-line `date`, `flat_discount`, `flat_vol_surface`, `flat_price_curve` constructors |

## Example

```rust
use finstack_quant_valuations_test_support::parity::ParityConfig;
use finstack_quant_valuations_test_support::test_helpers::{flat_discount_curve, usd};
use finstack_quant_valuations_test_support::{assert_parity, date::date};

let as_of = date(2025, 1, 2);
let curve = flat_discount_curve(0.05, as_of, "USD-OIS");
assert!(curve.df(1.0) < 1.0);
assert_eq!(usd(100.0).amount(), 100.0);
assert_parity!(1.00005, 1.0, ParityConfig::default(), "flat curve parity");
```
