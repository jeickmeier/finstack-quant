# Convertible Bond

## Features

- Hybrid fixed-income/equity instrument with conversion terms (`ConversionSpec`), call/put schedules, and optional fixed or floating coupons.
- Separate discount and credit curves for Tsiveriotis-Zhang style split between debt and equity components, with configurable recovery rate.
- Settlement lag support (`settlement_days`) for correct accrued interest and clean price calculation.
- Supports voluntary/mandatory/windowed conversion, anti-dilution policies, dividend adjustments, and underlying equity linkage.
- Mandatory variable delivery (PERCS/DECS/ACES) with upper/lower conversion price bounds.
- Call/put exercise periods (not just discrete dates) with optional make-whole call specifications.
- Soft-call triggers with observation window adjustment for multi-day barrier approximation.
- Event-triggered conversion via `PriceTrigger` (barrier approximation in tree).
- Accrued interest / clean price decomposition for market quote reconciliation.

## Methodology & References

- Tree-based Tsiveriotis-Zhang (1995) convertible framework with binomial or trinomial lattices (`ConvertibleTreeType`).
- **Full term structure discounting**: Per-step discount factors extracted from the risk-free and credit curves, capturing yield curve shape instead of using a single flat rate. Risky DFs are blended with recovery rate: `risky_adj = risky * (1-R) + rf * R`.
- Cashflow generation reused from the bond cashflow builder; conversion, call, and put events mapped onto tree steps.
- **Central finite differences** for all Greeks (delta, gamma, vega, rho) ensuring O(h^2) accuracy.
- Soft-call trigger uses a volatility-adjusted barrier to approximate the standard 20-of-30 day observation window (adapted from Broadie-Glasserman-Kou 1997 discrete monitoring correction).

## Usage Example

```rust
use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond;
use finstack_quant_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 2)?;
let cb = ConvertibleBond::example().unwrap();
let pv = cb.value(&market_context, as_of)?;
```

## Supported Conversion Policies

| Policy | Description |
|--------|-------------|
| `Voluntary` | Holder may convert at any time (American-style) |
| `MandatoryOn(date)` | Bond mandatorily converts on a specific date |
| `Window { start, end }` | Holder may convert within a date window (Bermudan-style) |
| `UponEvent(PriceTrigger { .. })` | Conversion when stock exceeds threshold (barrier approx.) |
| `UponEvent(QualifiedIpo)` | Conversion upon IPO (requires scenario analysis) |
| `UponEvent(ChangeOfControl)` | Conversion upon M&A (requires scenario analysis) |
| `MandatoryVariable { .. }` | PERCS/DECS/ACES with variable delivery ratio |

## Metrics

- PV plus tree-based Greeks (delta/gamma/vega/theta/rho) from full repricing with central differences.
- Parity, conversion premium, and conversion value analytics.
- Accrued interest and clean price for market quote reconciliation (settlement-date aware).
- Credit DV01/CS01 via credit-curve bumps; Dividend01; Conversion01.
- Bucketed DV01 via key-rate shifts.
- **OAS**: Option-adjusted spread via Brent solver (requires `quoted_clean_price`).
- **Bond floor**: Straight-bond value (PV of cashflows without conversion option).
- **Implied volatility**: Equity vol implied from market CB price via Brent solver.

## Limitations / Known Issues

- Tree model only; no Monte Carlo or PDE implementation.
- Single equity underlying with deterministic volatility; no stochastic credit/equity correlation or jump processes.
- **Drift-discount mismatch**: The CRR/trinomial tree uses a single flat risk-free rate for evolution (up/down factors and probabilities) to maintain the recombining property, while backward-induction uses per-step forward discount factors from the full term structure. This introduces a small bias when the yield curve is steep (~0.1-0.5% of notional for a 200bp slope over 5Y). A fully consistent implementation would require per-step evolution parameters and a non-recombining tree.
- `QualifiedIpo` and `ChangeOfControl` events cannot be modeled deterministically in a tree; require external scenario analysis.
- Make-whole call price computation is represented in the data model but not yet implemented in the tree pricer (uses `price_pct_of_par` fallback).
- No CoCo/AT1, exchangeable, reset, or cross-currency convertible support.
