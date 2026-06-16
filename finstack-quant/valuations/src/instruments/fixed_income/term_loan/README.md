# Term Loan

Institutional term loans with DDTL, PIK, covenant-driven events, amortization, and borrower calls.

## Features

- Fixed or floating rates with floors, caps, and gearing
- Delayed-draw term loans (DDTL) with commitment and usage fees
- PIK toggles and split cash/PIK coupons
- Amortization: bullet, linear, percent-per-period, or custom schedules
- Covenant events: margin step-ups, cash sweeps, PIK toggles, draw restrictions
- OID handling and step-down call schedules
- Metrics: DV01, CS01, YTM, YTC, YTW, discount margin, all-in rate

## Usage Example

```rust
use finstack_quant_valuations::instruments::fixed_income::term_loan::{TermLoan, TermLoanSpec};

let spec = TermLoanSpec::example()?; // or build from fields
let loan: TermLoan = spec.try_into()?;
let pv = loan.value(&market_context, as_of)?;
```

See `mod.rs` for the full specification surface (`TermLoanSpec`, `RateSpec`, DDTL and covenant types) and cashflow/pricing modules for methodology.

## Limitations

- Deterministic cashflow projection; no stochastic prepayment or default
- Covenant evaluation uses supplied metric inputs rather than live financial statements
- Custom fee schedules beyond standard commitment/usage/upfront fees require extension

## Metrics

Yield metrics (YTM, YTC, YTW, discount margin), rate sensitivities (DV01, bucketed DV01), credit sensitivities (CS01), and theta via the shared metrics registry.
