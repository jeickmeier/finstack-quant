# FX Instrument Standards

## FX Spot

### Market conventions

| Pair | Spot days | Quote convention | Base currency |
|------|-----------|------------------|---------------|
| EUR/USD | T+2 | Direct (EUR = 1) | EUR |
| GBP/USD | T+2 | Direct (GBP = 1) | GBP |
| USD/JPY | T+2 | Indirect (USD = 1) | USD |
| USD/CHF | T+2 | Indirect (USD = 1) | USD |
| USD/CAD | T+1 | Indirect (USD = 1) | USD |
| AUD/USD | T+2 | Direct (AUD = 1) | AUD |
| NZD/USD | T+2 | Direct (NZD = 1) | NZD |

### Quote priority (professional standard)

```
Bloomberg/Reuters hierarchy for CCY1/CCY2:
1. EUR is always CCY1 (base)
2. GBP is CCY1 unless EUR is present
3. AUD, NZD are CCY1 unless EUR/GBP present
4. USD is CCY1 for most other pairs
5. Emerging markets: USD typically base

Example: EURGBP not GBPEUR, AUDUSD not USDAUD
```

### Audit checklist - FX Spot

- [ ] Quote convention follows market standard
- [ ] Settlement days correct (for example USD/CAD, USD/TRY, and USD/RUB are T+1; many G10 pairs are T+2)
- [ ] Business day calendar uses joint calendars (both currencies)
- [ ] Base/quote currency follows priority rules

---

## FX Forward

### Market conventions

| Tenor | Calculation | Quote convention |
|-------|-------------|------------------|
| O/N (overnight) | T to T+1 | Forward points |
| T/N (tom-next) | T+1 to T+2 | Forward points |
| S/N (spot-next) | T+2 to T+3 | Forward points |
| 1W - 1Y | Standard tenor from spot | Forward points |
| >1Y | Broken dates | Outright or points |

### Forward points formula (professional standard)

```
Forward rate = Spot × (1 + r_d × T) / (1 + r_f × T)      [Simple]
Forward rate = Spot × exp((r_d - r_f) × T)               [Continuous]
Forward rate = Spot × DF_f(T) / DF_d(T)                  [Discount factors]

Forward points = (Forward - Spot) × point_factor

where:
  r_d = domestic (quote currency) rate
  r_f = foreign (base currency) rate
  point_factor = 10000 for most pairs, 100 for JPY pairs
```

### Bloomberg/QuantLib implementation

```
QuantLib FxForward:
- Uses discount factor method (most accurate)
- Settles notional on both currencies
- Forward points come from the domestic and foreign discount curves, each with its own money-market basis

Bloomberg FXFA:
- Quotes in forward points (pips)
- Converts to outright internally
- Settlement: Both currencies on value date
```

### NDF (Non-Deliverable Forward) conventions

| Currency | Fixing source | Settlement |
|----------|---------------|------------|
| CNY | PBOC | USD net settlement T+2 |
| INR | RBI | USD net settlement T+2 |
| KRW | KFTC | USD net settlement T+1 |
| BRL | PTAX | USD net settlement T+2 |

### Audit checklist - FX Forward

- [ ] Forward calculated from discount factors (not simple rates)
- [ ] Points quoted per market convention (10000 or 100)
- [ ] Both currency notionals settle on value date
- [ ] Business days use joint calendar
- [ ] For NDF: correct fixing source and settlement convention

---

## FX Swap

### Market conventions

```
FX swap = Spot + Forward (opposite directions)

Near leg: Buy CCY1 / Sell CCY2 at spot
Far leg:  Sell CCY1 / Buy CCY2 at forward

Swap points = Far forward points - Near forward points
```

### QuantLib/Bloomberg standard

```
Pricing:
PV_near = Notional_1 × (FX_near - Strike_near) × DF_2(t_near)
PV_far = Notional_1 × (Strike_far - FX_far) × DF_2(t_far)
Total PV = PV_near + PV_far

where:
  Notional keeps same base currency amount
  FX quoted as units of CCY2 per CCY1
```

### Audit checklist - FX Swap

- [ ] Near and far legs have opposite directions
- [ ] Notional in base currency is same for both legs
- [ ] Swap points = far points - near points
- [ ] Each leg settles independently on value dates
- [ ] PV in single reporting currency

---

## FX Options

### Market conventions

| Market | Vol quote | Delta convention | Premium currency |
|--------|-----------|------------------|------------------|
| G10 | Pair-specific ATM / RR / BF template | Pair-specific | Often CCY2 (quote ccy), but confirm pair and venue |
| EM | Pair-specific ATM / RR / BF template | Pair-specific, often premium-adjusted | Often USD, but confirm pair and venue |

### Volatility quote styles

```
ATM Definition:
- DNS (Delta-Neutral Straddle): Strike where call delta = -put delta
- ATMF (At-the-Money Forward): Strike = Forward
- ATM Spot: Strike = Spot (rarely used professionally)

Desk-standard guidance: ATM definition is pair- and venue-specific. Do not assume DNS globally.

Risk Reversal (RR):
- 25Δ RR = σ(25Δ call) - σ(25Δ put)
- Positive RR = calls more expensive (upside skew)

Butterfly (BF):
- 25Δ BF = [σ(25Δ call) + σ(25Δ put)] / 2 - σ(ATM)
- Measures smile convexity
```

### Garman-Kohlhagen formula (FX Black-Scholes)

```
Call = S × e^(-r_f × T) × N(d1) - K × e^(-r_d × T) × N(d2)
Put  = K × e^(-r_d × T) × N(-d2) - S × e^(-r_f × T) × N(-d1)

d1 = [ln(S/K) + (r_d - r_f + σ²/2) × T] / (σ × √T)
d2 = d1 - σ × √T

where:
  S = spot rate
  K = strike
  r_d = domestic (CCY2) rate
  r_f = foreign (CCY1) rate
  σ = volatility
  T = time to expiry
```

### Delta conventions

```
Spot delta (common as a screen/display convention, not the default interbank quote):
δ_spot = e^(-r_f × T) × N(d1)

Forward delta (common interbank default for many, not all, G10 pairs):
δ_fwd = N(d1)

Premium-adjusted delta (common for some EM pairs):
depends on:
- premium currency
- whether the market uses premium-adjusted spot or forward delta
- the exact quoting convention for the pair / venue

Desk-standard guidance:
- Do not hardcode a single generic premium-adjusted delta formula
- Use the market convention for the pair and quoting venue when inverting strikes or building a vol surface
```

### Audit checklist - FX Options

- [ ] Garman-Kohlhagen used (not vanilla Black-Scholes)
- [ ] Foreign rate discounts the spot, domestic rate discounts the strike
- [ ] Delta convention matches the specific pair and quoting venue
- [ ] ATM convention matches the surface construction rule for that pair
- [ ] Premium currency follows the market template and confirmation
- [ ] Vol surface interpolation respects smile

---

## Common implementation errors

### 1. Wrong rate assignment in FX forward

```rust
// WRONG: Rates swapped
let forward = spot * (1.0 + foreign_rate * t) / (1.0 + domestic_rate * t);

// CORRECT: Domestic rate in numerator
let forward = spot * (1.0 + domestic_rate * t) / (1.0 + foreign_rate * t);

// BEST: Use discount factors
let forward = spot * df_foreign / df_domestic;
```

### 2. Using Black-Scholes instead of Garman-Kohlhagen

```rust
// WRONG: Ignoring foreign rate
let d1 = (spot.ln() - strike.ln() + (rate + 0.5 * vol * vol) * t) / (vol * t.sqrt());

// CORRECT: Include both rates
let d1 = ((spot / strike).ln() + (r_dom - r_for + 0.5 * vol * vol) * t)
         / (vol * t.sqrt());
```

### 3. Forward delta vs spot delta

```rust
// Spot delta (needs adjustment for foreign rate)
let delta_spot = (-r_for * t).exp() * normal_cdf(d1);

// Forward delta (what traders quote)
let delta_fwd = normal_cdf(d1);

// Using wrong convention causes hedging errors
```

### 4. Wrong settlement for USD/CAD

```rust
// WRONG: Assuming T+2 for all pairs
let settlement = add_business_days(trade_date, 2);

// CORRECT: USD/CAD is T+1
let spot_days = match (ccy1, ccy2) {
    ("USD", "CAD") | ("CAD", "USD") => 1,
    ("USD", "TRY") | ("TRY", "USD") => 1,
    _ => 2,
};
let settlement = add_business_days(trade_date, spot_days);
```
