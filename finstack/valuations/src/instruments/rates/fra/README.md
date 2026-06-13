# Forward Rate Agreement (FRA)

## Features

- Single-period FRA with explicit fixing/start/end dates, fixed rate, pay/receive flag, and reset lag.
- Uses separate forward and discount curves plus instrument accrual day-count for payoff scaling.
- Provides deterministic cashflow schedule via `CashflowProvider` for downstream metrics.

## Methodology & References

- Standard FRA PV: `(Forward - Fixed) × Tau × Notional / (1 + Forward × Tau)` discounted from payment date.
- Forward projection uses the forward curve’s own day-count; discounting uses the chosen discount curve.
- Aligns with market FRA conventions (Act/360, pay-fixed vs receive-fixed).

## Usage Example

```rust
use finstack_valuations::instruments::rates::fra::ForwardRateAgreement;

let fra = ForwardRateAgreement::example().unwrap();
let pv = fra.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- No convexity adjustment for futures-style settlement; deterministic forwards only.
- Single-currency instrument; cross-currency FRAs require explicit FX handling outside this module.
- Does not model compounding, multi-period averaging, or optionality.

## Metrics

- PV, par FRA rate (solve K s.t. PV=0), and DV01 on discount/forward curves via bump calculators.
- Accrual factor and payoff breakdown for reporting.
- Simple sensitivity to fixing/reset lag via schedule recomputation.
