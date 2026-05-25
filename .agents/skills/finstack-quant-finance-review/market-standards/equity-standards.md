# Equity Instrument Standards

## Equity Options

### Market conventions

| Market | Vol quote | Settlement | Premium |
|--------|-----------|------------|---------|
| US listed | Implied vol | T+1 | Upfront |
| European listed | Implied vol | T+1 | Upfront |
| OTC equity | Lognormal vol | Negotiated | Upfront |

### Black-Scholes-Merton formula (professional standard)

```
Call = S × e^(-q × T) × N(d1) - K × e^(-r × T) × N(d2)
Put  = K × e^(-r × T) × N(-d2) - S × e^(-q × T) × N(-d1)

d1 = [ln(S/K) + (r - q + σ²/2) × T] / (σ × √T)
d2 = d1 - σ × √T

where:
  S = spot price
  K = strike
  r = risk-free rate
  q = dividend yield (continuous)
  σ = volatility
  T = time to expiry
```

### Discrete dividend handling

```
For known dividends D_i at times t_i:

Method 1: Spot adjustment (QuantLib default)
S_adj = S - Σ D_i × e^(-r × t_i)  [for t_i < T]

Method 2: Forward adjustment
F = (S - PV(divs)) × e^(r × T)

Method 3: Escrowed dividend model (more accurate)
Use tree/MC with dividend drops on ex-dates
```

### Greeks (analytical)

```
Delta = e^(-q × T) × N(d1)                          [call]
Delta = e^(-q × T) × (N(d1) - 1)                    [put]
Gamma = e^(-q × T) × φ(d1) / (S × σ × √T)
Vega  = S × e^(-q × T) × φ(d1) × √T
Theta_call = -S × e^(-q × T) × φ(d1) × σ / (2√T)
            + q × S × e^(-q × T) × N(d1)
            - r × K × e^(-r × T) × N(d2)
Rho   = K × T × e^(-r × T) × N(d2)                  [call]

where φ(x) = standard normal PDF
```

### Audit checklist - Equity options

- [ ] BSM includes dividend yield (not vanilla Black)
- [ ] Discrete dividends handled appropriately
- [ ] American exercise uses binomial/finite diff (not BSM)
- [ ] Greeks consistent with pricing formula
- [ ] Vol surface interpolation for OTM strikes

---

## Variance Swaps

### Market conventions

| Component | Standard | Notes |
|-----------|----------|-------|
| Variance calculation | Realized variance (log returns) | Not price returns |
| Annualization | 252 trading days | US equity markets |
| Settlement | Cash, on expiry + 2 | T+2 from final fixing |

### Variance swap pricing (ISDA/professional standard)

```
Realized variance (annualized):
σ²_realized = (252 / N) × Σ [ln(S_i / S_{i-1})]²

P&L = Var_notional × (σ²_realized - K²_var)

where:
  N = number of observations
  K²_var = variance strike in the same decimal variance units as σ²_realized

Quote handling:
- If the market quotes strike vol in vol points, convert it to decimal variance before applying the payout formula
- Do not mix decimal realized variance with basis-point-squared or vol-point-squared quoting units

Var_notional relationship to vega_notional:
Var_notional = Vega_notional / (2 × K_vol)
```

### Log contract replication

```
Fair variance strike (continuous-strike, no-jump idealization):
K_var² = (2 e^(rT) / T) × [∫₀^F P(K)/K² dK + ∫_F^∞ C(K)/K² dK]

where:
  F = forward level for maturity T
  P(K), C(K) = OTM put and call prices

Desk implementation:
- Use OTM puts below the forward and OTM calls above the forward
- Apply discrete strike summation, truncation, and wing assumptions explicitly
- Do not present ad hoc alternative rearrangements as the generic street-standard formula
```

### Audit checklist - Variance swaps

- [ ] Log returns (not simple returns)
- [ ] 252 trading days annualization
- [ ] Variance notional vs vega notional conversion correct
- [ ] No weekends/holidays in return calculation
- [ ] Handle stock splits/dividends in return series

---

## Equity Total Return Swaps (TRS)

### Market conventions

| Component | Standard | Notes |
|-----------|----------|-------|
| Equity leg | Total return (price + dividends) | Gross or net of tax |
| Financing leg | Funding index of trade currency + spread | SOFR/SONIA/ESTR etc. |
| Reset | Daily or periodic | Mark-to-market |
| Dividend treatment | Pass-through | Paid when received |

### TRS pricing

```
Model TRS as contractual period cashflows, not a single terminal return expression.

For reset period i:
Equity_cashflow_i =
    Q_{i-1} × (S_i - S_{i-1})
  + dividends_i
  - withholding_tax_i
  + corporate_action_adjustment_i

Financing_cashflow_i =
    N_{i-1} × (L_i + spread) × τ_i

Present value:
PV = Σ DF(t_i) × [Equity_cashflow_i - Financing_cashflow_i]

where:
  Q_{i-1} = equity share quantity or equivalent units over the period
  N_{i-1} = resettable financing notional
  L_i = contractual funding index for the trade currency / CSA
  τ_i = accrual fraction for period i

Desk-standard guidance:
- Explicitly model reset dates, notional reset mechanics, and contract currency
- Handle manufactured dividends, withholding tax, and corporate actions explicitly
- Distinguish funded economics from collateral/CSA discounting
```

### Audit checklist - Equity TRS

- [ ] Total return = price return + dividend return
- [ ] Dividend withholding tax treatment (gross vs net)
- [ ] Financing spread matches funding
- [ ] Margin/collateral properly handled
- [ ] Corporate actions (splits, M&A) handled

---

## Equity Index Futures

### Market conventions

| Index | Point value | Settlement | Roll cycle |
|-------|-------------|------------|------------|
| S&P 500 (ES) | $50/point | Cash | Mar, Jun, Sep, Dec |
| E-mini NASDAQ (NQ) | $20/point | Cash | Mar, Jun, Sep, Dec |
| Euro Stoxx 50 | €10/point | Cash | Mar, Jun, Sep, Dec |
| FTSE 100 | £10/point | Cash | Mar, Jun, Sep, Dec |
| Nikkei 225 | ¥1000/point | Cash | Mar, Jun, Sep, Dec |

### Fair value formula

```
Fair value = Index × e^((r - q) × T)

or with discrete dividends:
Fair value = (Index - PV(divs)) × e^(r × T)

Basis = Futures - Spot
Fair basis = Fair_value - Spot
```

### Audit checklist - Index futures

- [ ] Point value matches exchange specification
- [ ] Fair value uses dividend yield/discrete dividends
- [ ] Roll date handling for continuous series
- [ ] Settlement price methodology (mark to close)

---

## Exotic equity options

### Barrier options

| Type | Barrier condition | Payoff |
|------|-------------------|--------|
| Down-and-out | Knockout if S < H | Vanilla if survives |
| Down-and-in | Knock in if S < H | Vanilla if triggered |
| Up-and-out | Knockout if S > H | Vanilla if survives |
| Up-and-in | Knock in if S > H | Vanilla if triggered |

### Barrier monitoring convention

```
Professional standard: Continuous monitoring
Implementation: Discrete (daily close) with adjustment

Broadie-Glasserman-Kou adjustment for discrete:
H_adj = H × exp(±0.5826 × σ × √(Δt))

where the barrier moves away from spot:
- up barrier: use +
- down barrier: use -
```

### Asian options

```
Arithmetic average (market standard):
A = (1/n) × Σ S(t_i)

Geometric average (for closed-form):
G = [Π S(t_i)]^(1/n)

Payoff: max(A - K, 0) for average price call
```

### Audit checklist - Exotic options

- [ ] Barrier monitoring convention explicit
- [ ] Discrete monitoring adjustment applied
- [ ] Asian averaging dates specified clearly
- [ ] Monte Carlo paths sufficient (>10k for pricing)
- [ ] Boundary conditions correct (at barrier, at expiry)

---

## Common implementation errors

### 1. Missing dividend yield in BSM

```rust
// WRONG: Vanilla Black (no dividends)
let d1 = ((s / k).ln() + (r + 0.5 * vol * vol) * t) / (vol * t.sqrt());

// CORRECT: BSM with dividend yield
let d1 = ((s / k).ln() + (r - q + 0.5 * vol * vol) * t) / (vol * t.sqrt());
```

### 2. Simple returns instead of log returns for variance

```rust
// WRONG: Simple returns
let ret = (price[i] - price[i-1]) / price[i-1];
let var = returns.iter().map(|r| r * r).sum::<f64>() / n;

// CORRECT: Log returns
let ret = (price[i] / price[i-1]).ln();
let var = returns.iter().map(|r| r * r).sum::<f64>() * 252.0 / n;
```

### 3. Wrong barrier adjustment for discrete monitoring

```rust
// WRONG: Using exact barrier level
let knocked_out = prices.iter().any(|&p| p < barrier);

// CORRECT: For a down barrier, move the barrier away from spot
let adjustment = (-0.5826 * vol * dt.sqrt()).exp();
let adjusted_barrier = barrier * adjustment;
let knocked_out = prices.iter().any(|&p| p < adjusted_barrier);
```

### 4. Variance notional vs vega notional confusion

```rust
// WRONG: Using vega notional directly
let pnl = vega_notional * (realized_var - strike_var);

// CORRECT: Convert to variance notional
let var_notional = vega_notional / (2.0 * strike_vol);
let pnl = var_notional * (realized_var - strike_var);
```
