# Quantitative Code Review Examples

## Day count convention errors

### Issue: Incorrect ACT/360 implementation

```rust
// BAD: Using calendar days incorrectly
fn accrual_factor_act360(start: NaiveDate, end: NaiveDate) -> f64 {
    let days = (end - start).num_days();
    days as f64 / 365.0  // WRONG: ACT/360 uses 360, not 365
}

// GOOD: Correct ACT/360
fn accrual_factor_act360(start: NaiveDate, end: NaiveDate) -> f64 {
    let days = (end - start).num_days();
    days as f64 / 360.0  // Correct divisor for ACT/360
}
```

**Review finding**: Blocker - Using wrong year basis. ACT/360 divides by 360, not 365. This will cause ~1.4% error in all interest calculations.

### Issue: 30/360 edge case handling

```python
# BAD: Missing month-end adjustment
def day_count_30_360(d1: date, d2: date) -> float:
    y1, m1, day1 = d1.year, d1.month, d1.day
    y2, m2, day2 = d2.year, d2.month, d2.day
    # Missing: if day1 == 31: day1 = 30
    # Missing: if day2 == 31 and day1 >= 30: day2 = 30
    return (360 * (y2 - y1) + 30 * (m2 - m1) + (day2 - day1)) / 360

# GOOD: Proper US 30/360 Bond Basis
def day_count_30_360(d1: date, d2: date) -> float:
    y1, m1, day1 = d1.year, d1.month, d1.day
    y2, m2, day2 = d2.year, d2.month, d2.day

    if day1 == 31:
        day1 = 30
    if day2 == 31 and day1 >= 30:
        day2 = 30

    return (360 * (y2 - y1) + 30 * (m2 - m1) + (day2 - day1)) / 360
```

**Review finding**: Major - Missing 30/360 adjustment rules causes incorrect accrual for month-end dates.

## Precision and numerical stability

### Issue: Catastrophic cancellation in forward rate

```rust
// BAD: Precision loss when discount factors are close
fn forward_rate(df1: f64, df2: f64, tau: f64) -> f64 {
    (df1 - df2) / (tau * df2)  // When df1 ≈ df2, numerator loses precision
}

// BETTER: Use log formulation for better numerical stability
fn forward_rate(df1: f64, df2: f64, tau: f64) -> f64 {
    // For continuous compounding: f = -ln(df2/df1) / tau
    // For simple compounding, reformulate to avoid subtraction
    (df1 / df2 - 1.0) / tau  // Division is more stable than subtraction
}

// BEST: Use log for continuous rates, handle edge cases
fn forward_rate(df1: f64, df2: f64, tau: f64) -> Result<f64, RateError> {
    if tau.abs() < 1e-10 {
        return Err(RateError::ZeroTenor);
    }
    if df2 <= 0.0 {
        return Err(RateError::InvalidDiscountFactor);
    }
    Ok((df1 / df2 - 1.0) / tau)
}
```

**Review finding**: Major - Subtraction of similar values causes precision loss for short tenors. Reformulate using division.

### Issue: Accumulation error in cashflow summation

```python
# BAD: Naive summation accumulates floating point errors
def calculate_pv(cashflows: list[tuple[date, float]], curve) -> float:
    pv = 0.0
    for pay_date, amount in cashflows:
        df = curve.discount_factor(pay_date)
        pv += amount * df  # Error accumulates with many cashflows
    return pv

# BETTER: Use math.fsum for accurate summation
import math

def calculate_pv(cashflows: list[tuple[date, float]], curve) -> float:
    discounted = [amount * curve.discount_factor(pay_date)
                  for pay_date, amount in cashflows]
    return math.fsum(discounted)  # Tracked summation, much more accurate
```

**Review finding**: Minor - For swaps with many cashflows (30Y quarterly = 120+ cashflows), naive summation can accumulate noticeable error. Use `math.fsum` or Kahan summation.

## Sign and direction errors

### Issue: Wrong pay/receive sign convention

```rust
// BAD: Inconsistent sign convention
impl InterestRateSwap {
    fn calculate_fixed_leg_pv(&self) -> f64 {
        // Assumes we receive fixed
        self.fixed_cashflows.iter()
            .map(|cf| cf.amount * self.curve.df(cf.date))
            .sum()
    }

    fn calculate_float_leg_pv(&self) -> f64 {
        // BUG: Also assumes we receive, but should be opposite!
        self.float_cashflows.iter()
            .map(|cf| cf.amount * self.curve.df(cf.date))
            .sum()
    }

    fn npv(&self) -> f64 {
        // This gives wrong sign - both legs have same sign!
        self.calculate_fixed_leg_pv() + self.calculate_float_leg_pv()
    }
}

// GOOD: Explicit direction with pay/receive
#[derive(Clone, Copy)]
pub enum Direction {
    Pay,
    Receive,
}

impl InterestRateSwap {
    fn leg_pv(&self, cashflows: &[Cashflow], direction: Direction) -> f64 {
        let sign = match direction {
            Direction::Receive => 1.0,
            Direction::Pay => -1.0,
        };
        sign * cashflows.iter()
            .map(|cf| cf.amount * self.curve.df(cf.date))
            .sum::<f64>()
    }

    fn npv(&self) -> f64 {
        self.leg_pv(&self.fixed_cashflows, self.fixed_direction)
            + self.leg_pv(&self.float_cashflows, self.float_direction)
    }
}
```

**Review finding**: Blocker - Sign convention error causes NPV to be wrong. Swap NPV should be receive leg minus pay leg. Make direction explicit.

## Greeks calculation issues

### Issue: Wrong bump size for rate delta

```python
# BAD: Bump size too large, causes truncation error
def calculate_dv01(pricer, curve):
    bump = 0.01  # 100 bps - WAY too large!
    pv_base = pricer.price(curve)
    curve_up = curve.shift(bump)
    pv_up = pricer.price(curve_up)
    return (pv_up - pv_base) / bump

# BAD: Bump size too small, causes numerical noise
def calculate_dv01(pricer, curve):
    bump = 1e-10  # Too small - floating point noise dominates
    # ...

# GOOD: Appropriate bump size with central difference
def calculate_dv01(pricer, curve) -> float:
    """Calculate DV01 (dollar value of 1 basis point).

    Uses central difference with 1bp bump for optimal accuracy.
    """
    bump = 0.0001  # 1 basis point
    curve_up = curve.shift(bump)
    curve_down = curve.shift(-bump)
    pv_up = pricer.price(curve_up)
    pv_down = pricer.price(curve_down)

    # Central difference: more accurate than forward difference
    # Result is per 1bp, so no division by bump needed
    return (pv_up - pv_down) / 2.0
```

**Review finding**: Major - 100bp bump causes significant truncation error in Greeks. Use 1bp bump with central difference for rate delta.

### Issue: Gamma calculation instability

```rust
// BAD: Using forward differences for gamma
fn calculate_gamma(pricer: &Pricer, spot: f64, bump: f64) -> f64 {
    let pv = pricer.price(spot);
    let pv_up = pricer.price(spot + bump);
    let pv_up2 = pricer.price(spot + 2.0 * bump);
    // Forward difference gamma - less accurate
    (pv_up2 - 2.0 * pv_up + pv) / (bump * bump)
}

// GOOD: Central difference gamma
fn calculate_gamma(pricer: &Pricer, spot: f64, bump: f64) -> f64 {
    let pv = pricer.price(spot);
    let pv_up = pricer.price(spot + bump);
    let pv_down = pricer.price(spot - bump);
    // Central difference: O(h²) error vs O(h) for forward difference
    (pv_up - 2.0 * pv + pv_down) / (bump * bump)
}
```

**Review finding**: Minor - Forward difference gamma has O(h) error; central difference has O(h²). Use central difference for better accuracy.

## Curve construction issues

### Issue: Extrapolation without bounds checking

```rust
// BAD: Silent extrapolation can give nonsensical rates
impl YieldCurve {
    fn zero_rate(&self, date: NaiveDate) -> f64 {
        self.interpolator.interpolate(date)  // What if date is year 2100?
    }
}

// GOOD: Explicit extrapolation policy
impl YieldCurve {
    fn zero_rate(&self, date: NaiveDate) -> Result<f64, CurveError> {
        if date < self.base_date {
            return Err(CurveError::DateBeforeBaseDate { date, base: self.base_date });
        }

        if date > self.max_date {
            // Explicit policy: flat extrapolation with warning
            log::warn!(
                "Extrapolating curve beyond max date {} to {}",
                self.max_date, date
            );
            return Ok(self.interpolator.interpolate(self.max_date));
        }

        Ok(self.interpolator.interpolate(date))
    }
}
```

**Review finding**: Major - Unbounded extrapolation can silently produce incorrect rates for dates far beyond curve range. Make extrapolation policy explicit.

### Issue: Negative forward rates not validated

```python
# BAD: No validation of forwards
class YieldCurve:
    def forward_rate(self, t1: float, t2: float) -> float:
        df1 = self.discount_factor(t1)
        df2 = self.discount_factor(t2)
        tau = t2 - t1
        return (df1 / df2 - 1.0) / tau  # Could be negative!

# GOOD: Validate and handle appropriately
class YieldCurve:
    def forward_rate(self, t1: float, t2: float, allow_negative: bool = False) -> float:
        """Calculate simple forward rate.

        Args:
            t1: Start time (years)
            t2: End time (years)
            allow_negative: If False, raises on negative forwards (default behavior
                           for most markets). Set True for markets with negative rates.

        Raises:
            ValueError: If t2 <= t1 or if forward is negative and not allowed
        """
        if t2 <= t1:
            raise ValueError(f"End time {t2} must be after start time {t1}")

        df1 = self.discount_factor(t1)
        df2 = self.discount_factor(t2)
        tau = t2 - t1
        forward = (df1 / df2 - 1.0) / tau

        if forward < 0 and not allow_negative:
            raise ValueError(
                f"Negative forward rate {forward:.4%} between {t1} and {t2}. "
                "This may indicate a curve construction error. "
                "Set allow_negative=True if negative rates are expected."
            )

        return forward
```

**Review finding**: Minor - Negative forward rates often indicate curve construction errors but could be valid in negative rate environments. Make this explicit rather than silent.

## Cashflow generation issues

### Issue: Missing notional exchange

```rust
// BAD: Cross-currency swap missing notional exchanges
fn generate_xccy_cashflows(swap: &XccySwap) -> Vec<Cashflow> {
    let mut cashflows = Vec::new();

    // Only generates interest cashflows
    for period in &swap.domestic_leg.periods {
        cashflows.push(period.interest_cashflow());
    }
    for period in &swap.foreign_leg.periods {
        cashflows.push(period.interest_cashflow());
    }

    cashflows  // MISSING: Initial and final notional exchanges!
}

// GOOD: Include all notional exchanges
fn generate_xccy_cashflows(swap: &XccySwap) -> Vec<Cashflow> {
    let mut cashflows = Vec::new();

    // Initial notional exchange (we receive domestic, pay foreign)
    cashflows.push(Cashflow {
        date: swap.effective_date,
        amount: swap.domestic_notional,
        currency: swap.domestic_ccy,
        cf_type: CashflowType::NotionalExchange,
    });
    cashflows.push(Cashflow {
        date: swap.effective_date,
        amount: -swap.foreign_notional,  // Pay
        currency: swap.foreign_ccy,
        cf_type: CashflowType::NotionalExchange,
    });

    // Interest cashflows
    for period in &swap.domestic_leg.periods {
        cashflows.push(period.interest_cashflow());
    }
    for period in &swap.foreign_leg.periods {
        cashflows.push(period.interest_cashflow());
    }

    // Final notional exchange (reverse of initial)
    cashflows.push(Cashflow {
        date: swap.maturity_date,
        amount: -swap.domestic_notional,  // Pay back
        currency: swap.domestic_ccy,
        cf_type: CashflowType::NotionalExchange,
    });
    cashflows.push(Cashflow {
        date: swap.maturity_date,
        amount: swap.foreign_notional,  // Receive back
        currency: swap.foreign_ccy,
        cf_type: CashflowType::NotionalExchange,
    });

    cashflows
}
```

**Review finding**: Blocker - Cross-currency swaps require initial and final notional exchanges. Missing these causes massive valuation error.

### Issue: Off-by-one in payment schedule

```python
# BAD: Fence-post error in schedule generation
def generate_payment_dates(start: date, end: date, frequency: int) -> list[date]:
    dates = []
    current = start
    while current < end:  # BUG: < instead of <= misses final payment
        dates.append(current)
        current = add_months(current, 12 // frequency)
    return dates

# GOOD: Include both endpoints, handle stub
def generate_payment_dates(
    start: date,
    end: date,
    frequency: int,
    stub: StubType = StubType.SHORT_FRONT
) -> list[date]:
    """Generate payment schedule from start to end.

    Args:
        start: Effective date (first accrual start)
        end: Maturity date (final payment)
        frequency: Payments per year (1, 2, 4, 12)
        stub: How to handle non-standard first/last period

    Returns:
        List of payment dates including maturity
    """
    if start >= end:
        raise ValueError(f"Start {start} must be before end {end}")

    dates = []
    period_months = 12 // frequency

    if stub == StubType.SHORT_FRONT:
        # Work backwards from maturity
        current = end
        while current > start:
            dates.append(current)
            current = add_months(current, -period_months)
        dates.reverse()
    else:
        # Work forwards from start
        current = start
        while current < end:
            next_date = add_months(current, period_months)
            dates.append(min(next_date, end))  # Don't overshoot maturity
            current = next_date

    return dates
```

**Review finding**: Major - Off-by-one error causes missing final payment. For a 10Y swap this could be ~2.5% of total cashflows.

## SIMM calculation issues

### Issue: Wrong risk weight application

```rust
// BAD: Not applying concentration risk factor
fn calculate_simm_ir_delta(sensitivities: &[BucketSensitivity]) -> f64 {
    let mut sum_sq = 0.0;
    for sens in sensitivities {
        let weighted = sens.value * RISK_WEIGHTS[sens.bucket];
        sum_sq += weighted * weighted;
    }
    sum_sq.sqrt()  // Missing: correlation terms and concentration
}

// GOOD: Full SIMM aggregation
fn calculate_simm_ir_delta(sensitivities: &[BucketSensitivity]) -> f64 {
    // Step 1: Apply risk weights and concentration
    let weighted: Vec<_> = sensitivities.iter().map(|s| {
        let cr = concentration_risk_factor(s);
        s.value * RISK_WEIGHTS[s.bucket] * cr
    }).collect();

    // Step 2: Aggregate with correlations
    let mut total = 0.0;
    for (i, ws_i) in weighted.iter().enumerate() {
        total += ws_i * ws_i;  // Diagonal terms

        for (j, ws_j) in weighted.iter().enumerate().skip(i + 1) {
            let rho = BUCKET_CORRELATIONS[sensitivities[i].bucket][sensitivities[j].bucket];
            let g = correlation_adjustment(&sensitivities[i], &sensitivities[j]);
            total += 2.0 * rho * ws_i * ws_j * g;  // Off-diagonal terms
        }
    }

    total.sqrt()
}
```

**Review finding**: Blocker - SIMM requires concentration risk factors and cross-bucket correlations. Missing these produces non-compliant margin numbers.
