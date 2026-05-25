# Quantitative Code Review Reference

## Authoritative sources

When verifying implementations, reference these standards:

### Industry documentation
- **ISDA Definitions**: 2006 ISDA Definitions for swaps, 2021 Definitions for fallbacks
- **ISDA SIMM**: Standard Initial Margin Model methodology
- **FpML**: Financial products Markup Language schemas
- **OpenGamma**: Open source analytics (Strata) as reference implementation

### Academic references
- **Hull**: "Options, Futures, and Other Derivatives" - vanilla derivatives
- **Brigo-Mercurio**: "Interest Rate Models" - curve construction, exotic rates
- **Glasserman**: "Monte Carlo Methods in Financial Engineering" - simulation
- **Gatheral**: "The Volatility Surface" - vol modeling

## Day count conventions

### Standard conventions

| Convention | Year basis | Accrual calculation | Common usage |
|------------|-----------|---------------------|--------------|
| ACT/360 | 360 | actual_days / 360 | USD/EUR money markets, Libor |
| ACT/365F | 365 | actual_days / 365 | GBP markets, some bonds |
| ACT/ACT ISDA | actual | day-by-day with leap year handling | Government bonds |
| ACT/ACT ICMA | actual | (days_in_period / period_length) × frequency | Corporate bonds |
| 30/360 | 360 | see adjustment rules below | US corporate bonds |
| 30E/360 | 360 | European variant | Euro bonds |

### 30/360 adjustment rules

```
D1, M1, Y1 = start date components
D2, M2, Y2 = end date components

# US 30/360 (Bond Basis)
if D1 == 31: D1 = 30
if D2 == 31 and D1 >= 30: D2 = 30

# 30E/360 (Eurobond)
if D1 == 31: D1 = 30
if D2 == 31: D2 = 30

days = 360 × (Y2 - Y1) + 30 × (M2 - M1) + (D2 - D1)
fraction = days / 360
```

### Implementation checklist
- [ ] Handle month-end dates correctly
- [ ] Leap year handling for ACT/ACT
- [ ] February edge cases (28th/29th)
- [ ] Negative accrual periods (if allowed)

## Business day conventions

### Adjustment rules

| Convention | Rule |
|------------|------|
| Following | Move to next business day |
| Modified Following | Next business day, unless crosses month boundary → Previous |
| Preceding | Move to previous business day |
| Modified Preceding | Previous business day, unless crosses month boundary → Following |
| No Adjustment | Keep date even if holiday |

### Roll conventions

| Convention | Rule |
|------------|------|
| EOM | If start is last business day of month, roll to last business day |
| IMM | Third Wednesday of March, June, September, December |
| SFE | Second Friday of the month |

## Curve construction

### Bootstrap ordering

Standard instrument ordering for yield curve:
1. Overnight rate (O/N)
2. T/N, S/N deposits
3. Short-term deposits (1W, 2W, 1M, 2M, 3M)
4. Futures or FRAs (3M strips out to 2-3Y)
5. Swaps (1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 15Y, 20Y, 30Y, 50Y)

### Interpolation methods

| Method | Pros | Cons | Use case |
|--------|------|------|----------|
| Linear on zero | Simple | Discontinuous forwards | Quick approximation |
| Log-linear on discount | Positive forwards | Can be unstable | Standard choice |
| Cubic spline on zero | Smooth | Can oscillate | General purpose |
| Monotone convex | Positive forwards, smooth | More complex | Production curves |
| Natural cubic spline | C² continuous | May not preserve positivity | Academic |

### Validation checks
- [ ] Discount factor at t=0 equals 1.0
- [ ] Discount factors monotonically decreasing
- [ ] No negative forward rates (unless intentional)
- [ ] Repricing error < 0.01 bp for all instruments
- [ ] Smooth forward curve (visual inspection)

## Greeks and sensitivities

### Standard bump sizes

| Risk type | Bump size | Notes |
|-----------|-----------|-------|
| Delta (rates) | 1 bp (0.0001) | Parallel shift |
| Gamma (rates) | 1 bp | Second derivative |
| Vega | 1% absolute (0.01) | Vol bump |
| Theta | 1 day | Time decay |
| Rho | 1 bp | Funding/repo rate |
| FX Delta | 1% relative | Spot move |

### Finite difference schemes

```
# First derivative (delta)
Central:  δP/δx ≈ (P(x+h) - P(x-h)) / (2h)
Forward:  δP/δx ≈ (P(x+h) - P(x)) / h
Backward: δP/δx ≈ (P(x) - P(x-h)) / h

# Second derivative (gamma)
Central:  δ²P/δx² ≈ (P(x+h) - 2P(x) + P(x-h)) / h²

# Cross derivative (cross-gamma)
δ²P/δxδy ≈ (P(x+h,y+k) - P(x+h,y-k) - P(x-h,y+k) + P(x-h,y-k)) / (4hk)
```

### Numerical stability considerations
- Use central differences for better accuracy
- Bump size trade-off: too small → numerical noise, too large → truncation error
- Typical optimal bump: 1-10 bp for rates, 0.1-1% for vol
- Check gamma sign consistency with delta direction
- Validate: analytical Greeks (if available) vs numerical

## Pricing formulas

### Present value

```
PV = Σ CF_i × DF(t_i)

where:
  CF_i = cashflow at time t_i
  DF(t_i) = discount factor to time t_i
```

### Forward rate

```
# Simple forward rate
F(t1, t2) = (1/τ) × (DF(t1)/DF(t2) - 1)

# Continuous forward rate
f(t) = -d/dt ln(DF(t))

where τ = day count fraction between t1 and t2
```

### Swap rate (par rate)

```
S = (DF(t_0) - DF(t_n)) / Σ τ_i × DF(t_i)

where:
  t_0 = effective date
  t_n = maturity date
  τ_i = day count fraction for period i
```

### Black-76 (caps/floors/swaptions)

```
Call = DF × (F × N(d1) - K × N(d2))
Put  = DF × (K × N(-d2) - F × N(-d1))

d1 = (ln(F/K) + 0.5 × σ² × T) / (σ × √T)
d2 = d1 - σ × √T

where:
  F = forward rate
  K = strike
  σ = volatility (lognormal)
  T = time to expiry
  DF = discount factor to payment date
```

## ISDA SIMM overview

### Risk classes
1. Interest Rate (IR)
2. Credit (Qualifying and Non-Qualifying)
3. Equity
4. Commodity
5. FX

### IR delta bucketing

| Bucket | Tenors |
|--------|--------|
| 1 | 2W |
| 2 | 1M |
| 3 | 3M |
| 4 | 6M |
| 5 | 1Y |
| 6 | 2Y |
| 7 | 3Y |
| 8 | 5Y |
| 9 | 10Y |
| 10 | 15Y |
| 11 | 20Y |
| 12 | 30Y |

### Risk weight formula

```
WS_k = RW_k × S_k × CR_k

where:
  WS_k = weighted sensitivity
  RW_k = risk weight for bucket k
  S_k = net sensitivity
  CR_k = concentration risk factor
```

### Aggregation formula

```
K = √(Σ_k WS_k² + Σ_k Σ_l≠k ρ_kl × WS_k × WS_l × g_kl)

where:
  ρ_kl = correlation between buckets
  g_kl = concentration adjustment
```

## Common numerical issues

### Catastrophic cancellation

```python
# BAD: loses precision when a ≈ b
result = a - b

# BETTER: reformulate if possible
# Example: (1 - cos(x)) for small x
# BAD:  1 - cos(x)  # loses precision
# GOOD: 2 * sin(x/2)**2  # numerically stable
```

### Division by near-zero

```python
# BAD: may divide by zero or tiny number
rate = (pv_up - pv_down) / (2 * bump)

# BETTER: check for degeneracy
if abs(bump) < 1e-12:
    return 0.0  # or raise, depending on context
rate = (pv_up - pv_down) / (2 * bump)
```

### Accumulation errors

```python
# BAD: accumulating many small values
total = 0.0
for cf in cashflows:
    total += cf  # error accumulates

# BETTER: use Kahan summation for many terms
# Or: sort by magnitude, sum smallest first
# Or: use higher precision intermediate
```

### Interpolation at boundaries

```python
# BAD: extrapolation without validation
rate = interpolate(curve, date)  # date might be outside curve range

# BETTER: explicit boundary handling
if date < curve.min_date or date > curve.max_date:
    # explicit policy: flat extrapolation, error, or specific model
    return extrapolate_flat(curve, date)
rate = interpolate(curve, date)
```

## Testing requirements for quant code

### Unit tests
- Known analytical solutions (Black-Scholes, simple bonds)
- Boundary conditions (at expiry, zero vol, zero rate)
- Symmetry properties (put-call parity, cap-floor parity)
- Monotonicity (longer maturity → higher option value for vanillas)

### Integration tests
- Reprice market instruments to input quotes (< 0.01 bp error)
- Cross-validate against independent implementation
- Regression tests with golden values

### Property-based tests
- No-arbitrage conditions
- Greeks consistency (delta, gamma, theta relationship)
- Curve construction round-trips

### Stress tests
- Extreme rates (negative, very high)
- Near-zero time to expiry
- Deep in/out of the money
- Steep/inverted curves
