# Production Quant Audit Playbook

Use this when asked for a broad quant-production audit, when the repo has many finance crates, or when the initial scope is unclear. Prefer concrete findings over checklist output once code is available.

## First-pass searches

```bash
cargo metadata --no-deps --format-version 1
rg -n "LIBOR|libor|IBOR|ibor|SOFR|SONIA|TONA|ESTR|SARON|fallback|lookback|lockout|observation" .
rg -n "single.*curve|discount_curve|forward_curve|ois|collateral|par_rate|swap_rate|annuity|discount_factor|forward_rate" .
rg -n "365\\.0|360\\.0|252\\.0|/ 365|/365|/ 360|/360|/ 252|/252" .
rg -n "weekday|Saturday|Sunday|business_day|holiday|calendar|modified_following|following|preceding|eom" .
rg -n "black|scholes|bachelier|normal|lognormal|implied|vega|delta|gamma|rho|theta" .
rg -n "bond|coupon|yield|ytm|clean|dirty|accrued|duration|convexity|zspread|oas" .
rg -n "swap|fixed_leg|float_leg|reset|fixing|stub|payment_lag" .
rg -n "bootstrap|curve|interpolation|extrapolation|pillar|node|zero|forward|df" .
rg -n "var|value_at_risk|expected_shortfall|cvar|percentile|quantile|tail|scenario|shock|bump|stress|dv01|pv01|cs01|vega" .
rg -n "unwrap\\(|expect\\(|panic!|todo!|unimplemented!|unreachable!|NaN|nan|partial_cmp" .
```

## Release-blocker defect classes

- Missing explicit valuation/as-of date; pricing uses wall-clock time.
- Missing day-count convention, business-day calendar, settlement lag, or EOM/stub handling.
- LIBOR modeled as a default modern benchmark instead of legacy/fallback-specific behavior.
- Single-curve pricing for collateralized modern rates products.
- Wrong discounting sign, compounding basis, or discount/projection curve role.
- Floating coupons without fixing schedules, publication lag, lookback, lockout, observation shift, or payment delay.
- Clean/dirty price confusion, wrong accrued interest basis, or settlement/trade-date mixup.
- Raw `f64` units for rates, basis points, vol points, currency amounts, and notionals without validation.
- `unwrap`, `expect`, `panic!`, silent `NaN`, or unchecked `partial_cmp` in valuation/risk paths.

## Asset-class checks

### Rates and curves

- Distinguish overnight rates, compounded-in-arrears coupons, term rates, indexes, and averages.
- Resolve curve roles explicitly: discount, projection per index, collateral, historical fixings, calendars, and settlement conventions.
- Bootstrap to instrument pillar dates derived from conventions, not `today + tenor`.
- Reprice every bootstrapped input quote within tolerance.
- Interpolate in a representation that preserves intended discount-factor/forward behavior; document extrapolation.

### Options and Greeks

- Use the model matching the underlying and quote convention: Black-Scholes for spot equity-style options, Black-76 for forwards/futures/rates optionlets, Bachelier/normal where market convention requires it.
- Enforce no-arbitrage price bounds before implied-vol solving.
- Use bracketed solvers or robust fallback when Newton fails near intrinsic, short expiry, or low vega.
- Document Greek units: rate risk per bp, vega per 1.00 vol or per vol point, spot delta basis, and currency.

### Fixed income

- Keep clean price, dirty price, accrued interest, settlement date, quote basis, and ex-coupon rules separate.
- Test odd first/last coupons, stubs, leap years, February EOM, modified following across month boundaries, and missing historical fixings.
- For z-spread/OAS, apply spread in the cashflow discounting model being claimed; do not relabel yield-spread logic.

### Risk and scenarios

- Define VaR/ES on losses or signed P&L and keep the sign convention explicit.
- Align return series before covariance, correlation, portfolio return, and VaR calculations.
- Use typed shocks: rate bp, credit-spread bp, vol points, relative spot shocks, FX shocks, and curve/key-rate bumps.
- Full-revalue dependent curves, vols, FX, and instrument state for scenario P&L unless the code explicitly documents an approximation.

### Statements and fundamentals

- Flag zero/negative denominators, mixed annual/quarterly periods, mixed currencies, latest-vs-original filing ambiguity, and basic-vs-diluted share confusion.
- Preserve metric formula, input period, currency, and warnings in any user-facing ratio result.

## Minimum evidence before calling an audit complete

- Findings cite exact file paths and lines when the code is available.
- Each material finding includes financial impact, convention/model issue, proposed fix, and regression test.
- Verification includes focused tests for the changed/reviewed surface, plus binding/parity checks when exposed through Python or WASM.
- Residual risk states what was not reviewed or what broader commands were blocked by unrelated failures.
