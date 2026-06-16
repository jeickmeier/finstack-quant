# FX Swap

## Features

- Near/far FX swap with explicit base/quote currencies, settlement dates, and base notional.
- Optional explicit near/far rates or forward points; otherwise derives spot/forward from FX matrix and discount curves.
- Uses domestic and foreign discount curves and integrates with FX matrix for conversions.

## Methodology & References

- Standard FX swap PV: discount near/far exchanges in each currency, convert foreign leg to domestic via spot/forward, and sum.
- **Spot convention**: The `model_spot` from the FX matrix represents the as_of date spot rate (value date T+2).
- **Forward parity**: `F = S × (DF_for(far)/DF_for(near)) / (DF_dom(far)/DF_dom(near))` when far rate not supplied.
- **Settlement handling**: Near leg included if `near_date >= as_of`; far leg included if `far_date >= as_of`.
- Deterministic discounting; no funding adjustments or cross-currency basis beyond curve inputs.

## Usage Example

```rust
use finstack_quant_valuations::instruments::fx::fx_swap::FxSwap;

let swap = FxSwap::example();
let pv = swap.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Assumes availability of FX matrix when near/far rates are absent.
- No explicit CSA/basis handling beyond the chosen curves; funding adjustments must be modeled externally.
- Does not model optional early termination or broken-date rollovers beyond supplied dates.

## Metrics

- PV plus forward points and par far rate implied from curves.
- DV01 on domestic/foreign curves and FX delta exposures via bump-and-revalue.
- Cashflow breakdown by near/far legs for attribution.
