# Interest rate swap (IRS)

Plain-vanilla and OIS-style interest rate swaps under ISDA-style leg conventions. Basis swaps are `BasisSwap` in `rates/basis_swap/`.

## Module layout

```
irs/
├── types.rs        # InterestRateSwap, PayReceive, leg specs
├── pricer.rs       # Leg PV and NPV
├── cashflow.rs     # Schedules
├── compounding.rs  # Simple vs compounded-in-arrears (RFR)
└── metrics/        # Annuity, par rate, leg PVs; generic DV01/theta
```

## Pricing

```text
Pay fixed:   PV = PV_fixed − PV_float
Receive fixed: PV = PV_float − PV_fixed
```

Fixed leg: `N × K × Σ τᵢ DF(Tᵢ)`.

Floating (term): forwards from the projection curve, discounted on the discount curve.

OIS (`FloatingLegCompounding::CompoundedInArrears`): compounded overnight-style coupon; unseasoned single-curve case can use `DF(start)/DF(end)` when conventions align.

PV and annuity paths use Kahan summation. Par rate divides float PV by fixed-leg annuity with a near-zero annuity guard.

## Construction

```rust
use finstack_quant_valuations::instruments::rates::irs::{
    FloatingLegCompounding, InterestRateSwap, PayReceive,
};
use finstack_quant_valuations::instruments::{FixedLegSpec, FloatLegSpec};
// … build with InterestRateSwap::builder(), then:
swap.validate()?;
```

RFR presets: `FloatingLegCompounding::sofr()`, `sonia()`, `estr()`, `tona()`.

## Metrics

| Metric | Notes |
|--------|--------|
| `Annuity`, `ParRate`, `PvFixed`, `PvFloat` | IRS-specific calculators |
| `Dv01`, `BucketedDv01`, `Theta` | Generic registry metrics |

Request via `price_with_metrics` with the corresponding `MetricId` values.

## Conventions (typical)

| Currency | Fixed | Float | Index |
|----------|-------|-------|-------|
| USD | Semi, 30/360 | Quarterly, ACT/360 | SOFR |
| EUR | Annual, 30/360 | Semi, ACT/360 | €STR / EURIBOR |
| GBP / JPY | Semi, ACT/365 | Semi, ACT/365 | SONIA / TONA |

Accrual day-count may differ from the discount curve day-count; that is intentional for USD swap markets.

Seasoned OIS inside an accrual period needs fixings in a `ScalarTimeSeries` with id `FIXING:{index_id}` for dates before `as_of`; remaining days project from the forward or discount curve.

## OIS fast path

Unseasoned, no lookback/shift, `as_of <= accrual_start`, single discount curve: coupon from `DF(start)/DF(end)`. Otherwise daily compounding with lookback/observation shift from `compounding.rs`.

## Margin

`InterestRateSwap` can carry `OtcMarginSpec` (`finstack-quant-margin`) for CSA/SIMM-style workflows. See `finstack-quant-margin` docs for SIMM sensitivities and VM/IM metrics.

## Limits

- Deterministic curves only in the default pricer; no embedded stochastic short-rate model here.
- CMS, callable, and cross-currency structures live in other modules.
- No embedded FVA/CVA/DVA; funding is reflected via curve choice.

## Tests

`tests/instruments/irs/` — construction, cashflows, pricing, metrics, validation.

## References

ISDA 2006 Definitions (and 2021 RFR supplement for overnight swaps). Internal: `docs/REFERENCES.md` where cited from rustdoc.
