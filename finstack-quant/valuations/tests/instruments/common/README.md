# Instrument test infrastructure

Tests for the cross-instrument machinery that has no single owning instrument
directory (the pricer registry, market conventions, the `Discounting`
contract, the rates-credit lattice baselines).

This directory is not a standalone test target. It is pulled into
[`tests/instruments.rs`](../../instruments.rs) as `mod common` via
`#[path = "instruments/common/mod.rs"]`. The fixtures, tolerance tiers and
parity helpers that used to live here are in the dev-only crate
[`finstack-quant-valuations-test-support`](../../../test-support/README.md).

## Layout

| Path | Contents |
|------|----------|
| `mod.rs` | Module wiring |
| `parameters/test_conventions.rs` | `BondConvention` lookup and `FromStr` behavior |
| `pricer/registry.rs` | `InstrumentType`, `ModelKey`, `PricerKey`, `PricingError`, and `PricerRegistry` lookup/batch coverage |
| `test_discountable.rs` | `finstack_quant_core::market_data::traits::Discounting` NPV contract against a mock flat curve |
| `test_callable_credit_baseline.rs` | Pinned PV/OAS baselines for callable bond and callable term loan on the rates-credit lattice |
| `test_callable_floating_resets.rs` | Node-dependent floating resets on the same lattice; FRN-invariance identity |

## Fixtures and helpers

`finstack_quant_valuations_test_support::test_helpers` is the single source for
instrument-test fixtures. The main entry points:

```rust
use finstack_quant_valuations_test_support::test_helpers::{
    flat_discount_curve,   // (rate, base_date, curve_id) -> DiscountCurve
    flat_forward_curve,    // (rate, base_date, curve_id) -> ForwardCurve
    flat_hazard_curve,     // (hazard_rate, recovery, base_date, curve_id) -> HazardCurve
    usd_swap_market,       // (base_date, rate) -> MarketContext
    credit_market,         // (base_date, disc_rate, hazard_rate, recovery) -> MarketContext
    usd, eur, gbp,         // f64 -> Money
    black_scholes_call,    // (spot, strike, rate, vol, time, div_yield) reference formula
    tolerances,            // tolerance tier constants (see below)
    scaled_tolerance,      // (base_tol, value, min_abs) -> f64
};
```

Fixed test dates live in `test_helpers::dates` (`TODAY`, `TODAY_WEEKDAY`,
`IMM_DATE`). Nothing in this tree reads the system clock.

## Tolerance tiers

Pick the tier that matches the calculation, not the tier that makes the test
pass. Values are defined in
`finstack_quant_valuations_test_support::tolerances`.

| Constant | Value | Use case |
|----------|-------|----------|
| `TIGHT` | 1e-10 | Identities and roundtrips that should be exact up to rounding |
| `ANALYTICAL` | 1e-6 | Closed-form solutions, put-call parity, zero-coupon YTM |
| `NUMERICAL` | 1e-4 | Newton-Raphson, tree pricing, other iterative methods |
| `CURVE_PRICING` | 5e-3 | Curve-based valuations with compounding-convention differences |
| `RELATIVE` | 1e-2 | Proportional comparisons, textbook benchmarks |
| `BUMP_VS_ANALYTICAL` | 1.5e-2 | Bump-and-reprice against an analytical approximation (DV01 vs duration) |
| `STATISTICAL` | 2e-2 | Monte Carlo, finite-difference greeks |

`scaled_tolerance(base_tol, value, min_abs)` gives a relative tolerance with an
absolute floor, for property tests where the magnitude varies.

## Parity comparisons

`finstack_quant_valuations_test_support::parity` compares a computed value
against an externally sourced reference using a relative tolerance with a
near-zero absolute fallback:

```rust
use finstack_quant_valuations_test_support::assert_parity;
use finstack_quant_valuations_test_support::parity::ParityConfig;

assert_parity!(computed, reference, ParityConfig::default(), "Bond PV");
assert_parity!(computed, reference, ParityConfig::tight(), "closed-form check");
assert_parity!(
    computed,
    reference,
    ParityConfig::with_relative_tolerance(0.05),
    "vendor uses semi-annual annuity; see fixture note",
);
```

| Config | Relative | Absolute (near zero) |
|--------|----------|----------------------|
| `ParityConfig::default()` | 0.0001 (1 bp) | 1e-8 |
| `ParityConfig::tight()` | 0.00001 (0.1 bp) | 1e-10 |
| `ParityConfig::with_relative_tolerance(x)` | caller supplied | 1e-8 |

A widened tolerance must come with a comment naming the convention difference
that justifies it. Full external-reference fixtures belong in
[`tests/golden/`](../../golden/README.md), not here.

## Running

```bash
# Everything in the instruments binary, including this module
cargo nextest run -p finstack-quant-valuations --test valuations instruments

# Just this module
cargo nextest run -p finstack-quant-valuations --test valuations instruments::common::

# One area
cargo nextest run -p finstack-quant-valuations --test valuations instruments::common::pricer::
cargo nextest run -p finstack-quant-valuations --test valuations instruments::common::test_callable_credit_baseline::
```

Nothing in this module is `#[ignore]`d; it all runs on the default path.
