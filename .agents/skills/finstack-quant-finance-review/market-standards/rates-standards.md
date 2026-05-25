# Interest Rate Instrument Standards

## Benchmark Transition Baseline

As of April 2026, treat LIBOR as a legacy/fallback benchmark, not a default production benchmark for new pricing. FCA states all 35 LIBOR settings have permanently ceased, and ICE describes LIBOR benchmark cessation in September 2024. ISDA no longer supports the 2006 ISDA Definitions and provides the 2021 Interest Rate Derivatives Definitions as the current digital framework.

Audit implications:

- Flag any generic `LIBOR`/`USDLibor` default path unless it is explicitly scoped to legacy contracts, historical fixings, fallback modeling, or migration tests.
- Do not replace LIBOR with a single generic `SOFR` enum. Model overnight SOFR, SOFR Index, SOFR Averages, Term SOFR, simple in-arrears, compounded in-arrears, lookback, lockout, observation shift, and payment delay as distinct convention choices.
- Keep fallback spread adjustment, cessation/loss-of-representativeness trigger, contractual fallback waterfall, and historical fixing behavior explicit.
- Prefer instrument templates based on current CCP/venue/confirmation standards over hard-coded 2006-era defaults.

Reference URLs for live re-checks:

- FCA LIBOR transition: https://www.fca.org.uk/markets/libor-transition
- ICE LIBOR: https://www.ice.com/iba/libor
- New York Fed SOFR Averages and Index: https://www.newyorkfed.org/markets/reference-rates/sofr-averages-and-index
- ISDA 2021 Interest Rate Derivatives Definitions: https://www.isda.org/2021/05/01/2021-isda-interest-rate-derivatives-definitions/

## Term-Index / Legacy IRS

### Standard market conventions

| Currency | Float index | Day count (fixed) | Day count (float) | Frequency (fixed) | Frequency (float) | Bus day |
|----------|-------------|-------------------|-------------------|-------------------|-------------------|---------|
| USD | Term SOFR or legacy 3M LIBOR | 30/360 | ACT/360 | Semi-annual | Quarterly | NYC |
| EUR | 6M EURIBOR | 30/360 | ACT/360 | Annual | Semi-annual | TARGET |
| GBP | Legacy 6M GBP LIBOR | ACT/365F | ACT/365F | Semi-annual | Semi-annual | London |
| JPY | 6M TIBOR / JPY LIBOR legacy | ACT/365F | ACT/365F | Semi-annual | Semi-annual | Tokyo |
| CHF | Legacy CHF LIBOR or contract-specific term-rate fallback | 30/360 | ACT/360 | Annual | Semi-annual | Zurich |

This section is for term-index or legacy IRS templates, not the current generic G10 OIS market standard. For current benchmark swap conventions in USD/GBP/EUR risk systems, use the OIS section below unless the trade explicitly references a term index.

### Standard trade template

```
Vanilla IRS defaults:
- Effective date: T+2 (spot start)
- Roll convention: Modified Following
- End of month rule: True (if start is EOM, roll to EOM)
- Float leg: simple accrual off the underlying term index
- Stub: none for standard spot-start schedules; explicit broken-date rule if non-standard
- Notional: No exchange (single currency)
```

## Overnight Indexed Swaps (OIS)

### Standard market conventions

| Currency | Overnight index | Float day count | Fixed-leg convention | Typical payment lag |
|----------|-----------------|-----------------|---------------------|---------------------|
| USD | SOFR | ACT/360 | CCP / venue template; do not inherit vanilla IRS defaults | 2 business days common |
| EUR | ESTR | ACT/360 | CCP / venue template | 1-2 business days by template |
| GBP | SONIA | ACT/365F | CCP / venue template | Often 0 business days |
| JPY | TONAR | ACT/365F | CCP / venue template | Template-specific |
| CHF | SARON | ACT/360 | CCP / venue template | Template-specific |

### Coupon construction parameters

Treat these as separate trade-template fields, not interchangeable labels:

- **Lookback**: shift the observation window backward by N business days
- **Observation shift**: shift both rates and day weights consistently over the observation window
- **Lockout / rate cutoff**: repeat the last observed fixing for the final N days
- **Payment delay**: pay N business days after the accrual period ends

Desk-standard guidance: model the exact combination from the confirmation, CCP template, or market-standard product definition for that currency.

### OIS compounding formula (ISDA standard)

```
Coupon factor = ∏(1 + r_i × α_i)

Compounded coupon rate over the accrual period:
R = (Coupon factor - 1) / A

where:
  r_i = overnight rate for day i
  α_i = day count fraction for observation i using the overnight index basis
  A = total compounded accrual fraction over the coupon period on the overnight index basis

Implementation note:
  - Lookback, observation shift, and lockout change different parts of the coupon construction
  - Payment delay affects cashflow timing, not which rates are compounded
```

### Audit checklist - OIS

- [ ] Day count matches market convention for currency
- [ ] Daily compounding uses the confirmed overnight coupon convention
- [ ] Lookback, observation shift, lockout, and payment lag are modeled separately
- [ ] Payment lag matches the clearinghouse or bilateral template
- [ ] Stub handling is explicit only for broken-date schedules
- [ ] Business day calendar correct for currency
- [ ] Roll convention respects EOM rule

---

## Forward Rate Agreements (FRA)

### Market conventions

| Currency | Day count | Fixing lag | Settlement | Rate convention |
|----------|-----------|------------|------------|-----------------|
| USD | ACT/360 | T-2 | Settlement date | Discounted |
| EUR | ACT/360 | T-2 | Settlement date | Discounted |
| GBP | ACT/365F | T-0 | Settlement date | Discounted |

### FRA pricing formula (ISDA/QuantLib standard)

```
Settlement amount = Notional × (r_fra - r_fix) × τ / (1 + r_fix × τ)

where:
  r_fra = contract FRA rate
  r_fix = fixing rate (reference index)
  τ = accrual period (day count fraction)

Note: Payment is at settlement date, discounted from maturity
```

### QuantLib implementation reference

```cpp
// QuantLib ForwardRateAgreement
FRA(
    valueDate,           // Settlement date
    maturityDate,        // End date of underlying period
    position,            // Long/Short
    strikeRate,          // Contract rate
    notional,
    index,               // e.g., USDLibor3M
    discountingCurve     // For PV calculation
)

// Key: FRA settles discounted at valueDate
```

### Audit checklist - FRA

- [ ] Settlement is discounted (not paid at maturity)
- [ ] Fixing date calculated correctly (spot lag from settlement)
- [ ] Day count matches underlying index
- [ ] FRA rate is simple rate (not compounded)
- [ ] Discount factor uses settlement date

---

## Basis Swaps

### Market conventions

| Swap type | Leg 1 | Leg 2 | Spread convention |
|-----------|-------|-------|-------------------|
| OIS-OIS | SOFR | FF | Spread on FF leg |
| Tenor basis | 6M EURIBOR | 3M EURIBOR | Quote convention, tenor-specific |
| XCCY basis | USD SOFR | EUR EURIBOR or €STR leg | Pair-specific market convention |

### QuantLib defaults

```
Basis swap conventions:
- Spread placement: Determined by the quoted market template, not a universal rule
- Notional: Single currency = no exchange; XCCY may be fixed-notional or MTM resettable, with resettable-notional structures common in major currency pairs
- Payment: Same dates or different based on tenor
- Day count: Each leg follows its index convention
```

### Audit checklist - Basis swap

- [ ] Spread applied to correct leg (market convention)
- [ ] Each leg uses its own day count convention
- [ ] XCCY notional exchange and reset mechanics match the product template
- [ ] FX reset rule and notionals are modeled correctly for MTM structures
- [ ] Payment frequencies can differ between legs

---

## Caps/Floors

### Market conventions

| Currency | Day count | Volatility quote | Premium settlement |
|----------|-----------|-----------------|--------------------|
| USD | ACT/360 | Lognormal (Black) | Per confirmation, often spot-style |
| EUR | ACT/360 | Normal (Bachelier) | Per confirmation |
| GBP | ACT/365F | Lognormal or normal by market surface | Per confirmation |

Caps and floors are premium-bearing cash-settled optionlets/floorlets. Do not model them as physically delivered instruments.

### Pricing models

| Model | When to use | Formula reference |
|-------|-------------|-------------------|
| Black-76 | Lognormal vol quotes | Hull Ch. 29 |
| Bachelier | Normal vol quotes (EUR post-2015) | Hull Ch. 29 |
| Shifted Black | Low/negative rate environment | Brigo-Mercurio |
| SABR | Vol smile calibration | Hagan et al. 2002 |

### Black-76 formula (QuantLib/Bloomberg standard)

```
Caplet_price = DF(T_pay) × τ × [F × N(d1) - K × N(d2)]
Floorlet_price = DF(T_pay) × τ × [K × N(-d2) - F × N(-d1)]

d1 = [ln(F/K) + 0.5 × σ² × T] / (σ × √T)
d2 = d1 - σ × √T

where:
  F = forward rate
  K = strike
  σ = lognormal volatility
  T = time to expiry (fixing date)
  τ = accrual period
  DF(T_pay) = discount to payment date
```

### Audit checklist - Caps/Floors

- [ ] Volatility type matches market quote (lognormal vs normal)
- [ ] Time to expiry is to fixing date, not payment date
- [ ] Payment date discounting (not fixing date)
- [ ] Accrual factor uses correct day count
- [ ] For SOFR caps: use compounded forward rate
- [ ] Premium settlement convention (upfront vs running)

---

## Swaptions

### Market conventions

| Currency | Vol quote | Exercise | Settlement |
|----------|-----------|----------|------------|
| USD | Normal common; lognormal also seen in some markets | European | Physical or cash |
| EUR | Normal (Bachelier) | European | Cash (collateralized cash price) |
| GBP | Normal common; lognormal legacy also seen | European | Physical |

### Cash settlement conventions

```
Market-standard cash settlement depends on currency and market template.

EUR market standard:
- collateralized cash price
- discounting consistent with collateralized swap cashflows

Legacy par-yield / ISDA annuity settlement:
- historical convention for some markets and legacy trades
- do not treat as the default EUR desk standard
```

### QuantLib implementation

```cpp
// European swaption
Swaption(
    swap,                    // Underlying swap
    exercise,                // ExerciseDate
    settlementType,          // Physical or Cash
    settlementMethod         // ParYieldCurve (ISDA) or PhysicalOTC
)

// Pricing engines
BlackSwaptionEngine        // Lognormal vol
BachelierSwaptionEngine    // Normal vol
```

### Audit checklist - Swaptions

- [ ] Vol type matches the quoted market surface and calibration convention
- [ ] Cash settlement method matches the market standard for the currency and trade template
- [ ] Expiry date is exercise date (not settlement)
- [ ] Underlying swap terms match market standard
- [ ] Physical delivery creates actual swap position

---

## Common implementation errors

### 1. OIS compounding without explicit coupon convention

```rust
// WRONG: Simple average
let rate = daily_rates.iter().sum::<f64>() / daily_rates.len() as f64;

// WRONG: Compounding without observation shift
let compound = daily_rates.iter()
    .fold(1.0, |acc, &r| acc * (1.0 + r * day_frac));

// CORRECT: Compounding with explicit coupon convention parameters
let compound = (0..accrual_days).fold(1.0, |acc, i| {
    let obs_date = apply_ois_observation_convention(
        accrual_start + i,
        lookback_days,
        observation_shift,
        lockout_days,
    );
    let rate = get_fixing(obs_date);
    acc * (1.0 + rate * 1.0/360.0)
});
```

### 2. FRA settling at maturity

```rust
// WRONG: Pay undiscounted at maturity
let payment = notional * (fra_rate - fixing) * tau;

// CORRECT: Pay discounted at settlement
let payment = notional * (fra_rate - fixing) * tau / (1.0 + fixing * tau);
```

### 3. Wrong cap/floor time to expiry

```rust
// WRONG: Time to payment
let time = year_fraction(today, payment_date);

// CORRECT: Time to fixing
let time = year_fraction(today, fixing_date);
```
