# CDS Option

CDS options are priced with the Bloomberg CDSO numerical-quadrature model, not
the legacy closed-form Black-on-forward-spread approximation. The instrument
supports European cash-settled payer/receiver options on single-name CDS and CDS
indices, with explicit controls for protection-start convention, knockout
behavior, index factor, realized index loss, and underlying CDS coupon.

## Methodology

The pricer implements Bloomberg DOCS 2055833 Eq. 2.5:

```text
O = P(t_e) * E_0[(xi * V_te + H(K) + D)+]
```

- `V_te` is the random forward CDS value at option expiry.
- `H(K)` is the deterministic strike adjustment for coupon/strike mismatch.
- `D` is deterministic realized index loss settlement.
- `m` is calibrated so the lognormal spread process reproduces the bootstrapped
  no-knockout forward value `F_0`.

Underlying CDS mechanics use Bloomberg CDSW-style conventions from DOCS 2057273
where relevant, including spot default-leg valuation and the CDSO-scoped
inclusive protection-end adjustment.

## Usage Example

```rust
use finstack_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 5)?;
let opt = CDSOption::example().unwrap();
let pv = opt.value(&market_context, as_of)?;
```

## Metrics

- PV, delta, gamma, vega, theta, CS01, DV01, Recovery01.
- `dv01` is the canonical CDSO interest-rate sensitivity: it bumps the
  calibrated swap-curve quotes and rebuilds the discount curve.
- `par_spread` reports the Bloomberg CDSO displayed ATM forward spread.
- `implied_vol` solves the Bloomberg quadrature price in log-vol space.

## Limitations

- European exercise and cash settlement only.
- Lognormal spread volatility; stochastic recovery and volatility are out of scope.
- Some Bloomberg CDSO internals remain proprietary; source-backed residuals are
  documented in the `cdx_ig_46` golden fixture rather than widened away.

## References

- Bloomberg L.P. Quantitative Analytics, *Pricing Credit Index Options*, DOCS
  2055833.
- Bloomberg L.P. Quantitative Analytics, *The Bloomberg CDS Model*, DOCS
  2057273.
