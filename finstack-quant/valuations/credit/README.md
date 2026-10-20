# finstack-quant-valuations-credit

The credit-derivatives family of the valuations stack: single-name CDS, CDS
indices, index tranches and CDS options. It implements the framework in
[`finstack-quant-valuations-core`](../core/README.md) and is re-exported by
the umbrella crate [`finstack-quant-valuations`](../README.md) as
`finstack_quant_valuations::instruments::credit_derivatives`, so every public
path there is unchanged. Depends on `finstack-quant-core`,
`finstack-quant-cashflows`, `finstack-quant-models`, `finstack-quant-margin`
(the `Marginable` impls of `CreditDefaultSwap` and `CdsIndex`) and
valuations-core only; no other instrument family.

## Leaves

| Directory | Prices | Market convention / model |
|-----------|--------|---------------------------|
| `cds/` | Single-name credit default swaps | ISDA Standard Model conventions (IMM rolls, ACT/360 premium leg, accrual on default, step-in `T+1`), `CdsValuationConvention` for the Bloomberg CDSW clean/dirty presentation, upfront and quoted-spread quoting |
| `cds_index/` | CDX / iTraxx indices | Constituent-level or index-curve pricing, factor and IMM-roll handling, `IndexResult` constituent breakdown |
| `cds_tranche/` | Synthetic CDO tranches | Gaussian copula with base-correlation interpolation, expected-loss and tranche-spread metrics |
| `cds_option/` | Payer / receiver options on CDS and indices | Lognormal spread (Black) and price-strike forms, knock-out convention, Bloomberg CDSO quadrature reconciliation |
| `cs01.rs` | Not an instrument | The shared CS01 fast path: hazard-curve bump and reprice without rebuilding the schedule |
| `family.rs` | Not an instrument | `CREDIT`, the family's pricer and metric registrations |

```rust
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_credit::cds::CreditDefaultSwap;

let cds = CreditDefaultSwap::example().unwrap();
assert!(!cds.id().is_empty());
```

## Public surface

Import path: `finstack_quant_valuations_credit::<leaf>::<Type>` (or
`finstack_quant_valuations::instruments::credit_derivatives::<leaf>::<Type>`
through the umbrella); the crate root exports only the `CREDIT` family static.
`CreditDefaultSwap`, `CdsIndex`, `CdsTranche` and `CdsOption` are also flat at
`finstack_quant_valuations::instruments`. The quadrature
helpers in `cds_option::{bloomberg_quadrature, pricer}` and the CDS pricing
engine in `cds::pricing` are `#[doc(hidden)] pub`: the umbrella's Bloomberg
CDSO golden test reaches them because its market bootstrap needs
`finstack-quant-calibration`, which this crate cannot dev-depend on.

## Tests and benches

Integration tests that need only this crate, valuations-core and the
test-support crate live in `tests/`, compiled into the single `credit` target
(`mise run rust-test-crate -- finstack-quant-valuations-credit`). Most of the
family's integration tests stay in the umbrella's `../tests/instruments/
{cds,cds_index,cds_option,cds_tranche}/`: they bootstrap hazard curves and
build `PricingOptions` through `finstack-quant-calibration`
(`tests/support/credit.rs`). Benches stay in the umbrella.
