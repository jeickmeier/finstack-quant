//! Period-aligned DataFrame exports for cashflow schedules.
//!
//! This module provides DataFrame-like representations of cashflow schedules aligned
//! to period boundaries. It computes all derived columns (discount factors, survival
//! probabilities, base rates, spreads, unfunded amounts) in Rust for consistency
//! across language bindings.
//!
//! ## Design
//!
//! - All computations happen in Rust to ensure deterministic results across Python/WASM bindings
//! - Historical cashflows (`date <= as_of/base`) are included for auditability but contribute zero PV
//! - Optional columns (survival_probs, base_rates, spreads, etc.) are conditionally computed
//! - Facility limits enable undrawn balance calculations for revolving credit facilities

use crate::builder::schedule::{amounts_approx_equal, CashFlowSchedule};
use crate::primitives::CFKind;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, DayCountContext, Period, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

// =============================================================================
// Helper functions for DataFrame construction (extracted for testability)
// =============================================================================

/// Initialize an optional column vector, reusing existing allocation if available.
///
/// This helper reduces repetitive initialization code for optional columns
/// in `PeriodDataFrame`. When `enabled` is true, it clears and reserves
/// capacity on the existing vector (or creates a new one). When false,
/// it sets the column to `None`.
fn init_optional_column<T>(enabled: bool, capacity: usize, existing: &mut Option<Vec<T>>) {
    *existing = if enabled {
        let mut vec = existing.take().unwrap_or_default();
        vec.clear();
        vec.reserve(capacity);
        Some(vec)
    } else {
        None
    };
}

/// Compute the signed year fraction from base date to cashflow date for discounting.
///
/// Returns:
/// - 0.0 if dates are equal
/// - Positive year fraction if cf_date > base
/// - Negative year fraction if cf_date < base (historical cashflow)
///
/// # References
///
/// - ISDA 2006 Definitions §4.16 (Day Count Fractions). Sign convention for
///   pre-base dates follows the standard "discount-back" treatment used by
///   `finstack_core::cashflow::npv`.
fn compute_discount_time(
    cf_date: Date,
    base: Date,
    dc: DayCount,
    dc_ctx: DayCountContext<'_>,
) -> finstack_core::Result<f64> {
    if cf_date == base {
        Ok(0.0)
    } else if cf_date > base {
        dc.year_fraction(base, cf_date, dc_ctx)
    } else {
        Ok(-dc.year_fraction(cf_date, base, dc_ctx)?)
    }
}

/// Compute notional columns (drawn and undrawn) for accruing cashflows.
///
/// Returns `(drawn_notional, undrawn_notional)` where:
/// - `drawn_notional` is `Some(outstanding)` for interest/fee-like flows, `None` otherwise
/// - `undrawn_notional` is `Some(limit - outstanding)` if facility_limit provided and currencies match
fn compute_notional_columns(
    cf: &finstack_core::cashflow::CashFlow,
    outstanding_pre: f64,
    facility_limit: Option<&Money>,
) -> (Option<f64>, Option<f64>) {
    use crate::primitives::CFKind;

    // Check if this is an accruing flow that should show notional
    let is_accruing = matches!(
        cf.kind,
        CFKind::Fixed
            | CFKind::Stub
            | CFKind::FloatReset
            | CFKind::CommitmentFee
            | CFKind::UsageFee
            | CFKind::FacilityFee
    ) || cf.accrual_factor > 0.0;

    if !is_accruing {
        return (None, None);
    }

    let drawn = Some(outstanding_pre);
    let undrawn = facility_limit.and_then(|limit| {
        if limit.currency() == cf.amount.currency() {
            Some((limit.amount() - outstanding_pre).max(0.0))
        } else {
            None
        }
    });

    (drawn, undrawn)
}

/// Compute floating rate decomposition (base rate and spread) for a cashflow.
///
/// Returns `(base_rate, spread)` where:
/// - `base_rate` is the forward rate at reset time
/// - `spread` is the difference between the all-in rate and base rate
///
/// Both are `None` if the cashflow is not a floating rate reset or if no forward curve is provided.
///
/// # References
///
/// - ISDA 2021 IBOR Fallbacks Supplement, §7. Spreads are quoted as simple
///   additive spreads over the index, consistent with the affine-rate model
///   used by `FloatingRateSpec`.
/// - Andersen & Piterbarg, *Interest Rate Modeling, Vol I* (2010), §6.3.
fn compute_floating_decomposition(
    cf: &finstack_core::cashflow::CashFlow,
    fwd: Option<&std::sync::Arc<finstack_core::market_data::term_structures::ForwardCurve>>,
    base: Date,
    period_start: Date,
    dc_ctx: DayCountContext<'_>,
) -> finstack_core::Result<(Option<f64>, Option<f64>)> {
    use crate::primitives::CFKind;

    // Only compute for floating rate resets with a forward curve
    if !matches!(cf.kind, CFKind::FloatReset) {
        return Ok((None, None));
    }

    let Some(fwd) = fwd else {
        return Ok((None, None));
    };

    // Compute reset time using forward curve's day count
    let reset_t = if let Some(reset_date) = cf.reset_date {
        compute_discount_time(reset_date, base, fwd.day_count(), dc_ctx)?
    } else {
        // Fallback to period start if no explicit reset date. Use the signed
        // helper so a historical period_start yields a negative time instead
        // of a day-count error or mis-signed year fraction.
        compute_discount_time(period_start, base, fwd.day_count(), dc_ctx)?
    };

    let base_rate = fwd.rate(reset_t);
    let spread = cf.rate.map(|rate| rate - base_rate);

    Ok((Some(base_rate), spread))
}

/// Options for period-aligned DataFrame exports.
///
/// Controls which optional columns are computed and provides configuration
/// for market data lookups and discounting conventions.
#[derive(Debug, Clone, Default)]
pub struct PeriodDataFrameOptions<'a> {
    /// Optional credit curve ID for credit-adjusted discounting
    pub credit_curve_id: Option<&'a str>,
    /// Optional forward curve ID for floating rate decomposition
    pub forward_curve_id: Option<&'a str>,
    /// Valuation date (defaults to discount curve base date if not provided)
    pub as_of: Option<Date>,
    /// Day count convention for year fraction calculations
    pub day_count: Option<DayCount>,
    /// Optional override for discounting time calculation basis.
    ///
    /// When provided, the discounting time 't' will be computed using this
    /// day-count instead of `day_count`/schedule DC.
    pub discount_day_count: Option<DayCount>,
    /// Facility limit/commitment for undrawn balance calculations
    pub facility_limit: Option<Money>,
    /// Whether to include floating rate decomposition (base_rates, spreads)
    pub include_floating_decomposition: bool,
    /// Optional coupon frequency for day count context (required for Act/Act ISMA).
    ///
    /// When the day count convention is `ActActIsma`, this frequency is used to
    /// construct the proper `DayCountContext` for year fraction calculations.
    /// If not provided, defaults to `None` which may cause incorrect year fractions
    /// for Act/Act ISMA convention.
    pub frequency: Option<Tenor>,
    /// Optional calendar ID for day count context (required for Bus/252).
    ///
    /// When the day count convention is `Bus252`, this calendar ID is used to
    /// look up the holiday calendar for business day counting.
    pub calendar_id: Option<&'a str>,
}

/// Period-aligned DataFrame-like result.
///
/// Contains row-oriented vectors representing cashflows aligned to period boundaries.
/// All vectors have the same length corresponding to the number of cashflows that
/// fall within the provided periods.
///
/// # Field Groups
///
/// The 20 fields are organized into logical groups for clarity:
///
/// ## Core Date/Time (always present)
/// - `start_dates`, `end_dates`, `pay_dates` - Period and payment dates
/// - `reset_dates` - Floating rate fixing dates (may be `None` per row)
/// - `yr_fraqs`, `days` - Time metrics between dates
///
/// ## Cashflow Identity (always present)
/// - `cf_types` - Cashflow kind (Fixed, FloatReset, Amortization, etc.)
/// - `currencies` - Currency for each cashflow
/// - `amounts` - Cashflow amounts (coupons, principal, fees)
/// - `accrual_factors`, `rates` - Rate calculation inputs
///
/// ## Discounting (always computed)
/// - `discount_factors` - DF from base date to payment dates
/// - `pvs` - Present values (`amount * DF * survival_prob`)
/// - `survival_probs` - Optional survival probabilities (if hazard curve provided)
///
/// ## Notional Tracking
/// - `notionals` - Outstanding (drawn) balance for accruing flows
/// - `undrawn_notionals` - Unused commitment (if `facility_limit` provided)
/// - `unfunded_amounts`, `commitment_amounts` - Facility-related columns
///
/// ## Floating Rate Decomposition (if enabled)
/// - `base_rates` - Forward rates from index curve
/// - `spreads` - Margin over forward rate (`rate - base_rate`)
///
/// # Usage Notes
///
/// - Historical cashflows (`date <= as_of`) are included but contribute zero PV
/// - Optional columns are `None` when not requested via [`PeriodDataFrameOptions`]
/// - All computation happens in Rust for deterministic results across bindings
///
/// # Python Bindings
///
/// The flat field structure is intentional for Python/WASM binding compatibility.
/// Access fields directly (e.g., `frame.start_dates`, `frame.amounts`).
#[derive(Clone)]
pub struct PeriodDataFrame {
    /// Period start dates
    pub start_dates: Vec<Date>,
    /// Period end dates
    pub end_dates: Vec<Date>,
    /// Payment dates (potentially adjusted for business days)
    pub pay_dates: Vec<Date>,
    /// Reset dates for floating rate coupons (if applicable)
    pub reset_dates: Vec<Option<Date>>,
    /// Cashflow types (coupon, amortization, fee, etc.)
    pub cf_types: Vec<CFKind>,
    /// Currencies for each cashflow
    pub currencies: Vec<Currency>,
    /// Outstanding notional amounts
    pub notionals: Vec<Option<f64>>,
    /// Undrawn notional amounts (for committed facilities)
    pub undrawn_notionals: Option<Vec<Option<f64>>>,
    /// Year fractions for each period (time between dates in years)
    pub yr_fraqs: Vec<f64>,
    /// Accrual factors (day count convention applied)
    pub accrual_factors: Vec<f64>,
    /// Calendar days in each period
    pub days: Vec<i64>,
    /// Cashflow amounts (coupons, principal, fees)
    pub amounts: Vec<f64>,
    /// Interest rates for each period
    pub rates: Vec<f64>,
    /// Discount factors from base date to payment dates
    pub discount_factors: Vec<f64>,
    /// Survival probabilities (if credit risk modeled)
    pub survival_probs: Option<Vec<Option<f64>>>,
    /// Present values of cashflows
    pub pvs: Vec<f64>,
    /// Unfunded amounts (drawn commitment minus outstanding)
    pub unfunded_amounts: Option<Vec<Option<f64>>>,
    /// Total commitment amounts per period
    pub commitment_amounts: Option<Vec<Option<f64>>>,
    /// Base forward rates for floating coupons (if decomposed)
    pub base_rates: Option<Vec<Option<f64>>>,
    /// Spread over base rates for floating coupons (if decomposed)
    pub spreads: Option<Vec<Option<f64>>>,
}

impl PeriodDataFrame {
    /// Create a DataFrame with preallocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            start_dates: Vec::with_capacity(capacity),
            end_dates: Vec::with_capacity(capacity),
            pay_dates: Vec::with_capacity(capacity),
            reset_dates: Vec::with_capacity(capacity),
            cf_types: Vec::with_capacity(capacity),
            currencies: Vec::with_capacity(capacity),
            notionals: Vec::with_capacity(capacity),
            undrawn_notionals: None,
            yr_fraqs: Vec::with_capacity(capacity),
            accrual_factors: Vec::with_capacity(capacity),
            days: Vec::with_capacity(capacity),
            amounts: Vec::with_capacity(capacity),
            rates: Vec::with_capacity(capacity),
            discount_factors: Vec::with_capacity(capacity),
            survival_probs: None,
            pvs: Vec::with_capacity(capacity),
            unfunded_amounts: None,
            commitment_amounts: None,
            base_rates: None,
            spreads: None,
        }
    }

    /// Clear all columns while preserving allocations.
    pub fn clear(&mut self) {
        self.start_dates.clear();
        self.end_dates.clear();
        self.pay_dates.clear();
        self.reset_dates.clear();
        self.cf_types.clear();
        self.currencies.clear();
        self.notionals.clear();
        if let Some(undrawn) = self.undrawn_notionals.as_mut() {
            undrawn.clear();
        }
        self.yr_fraqs.clear();
        self.accrual_factors.clear();
        self.days.clear();
        self.amounts.clear();
        self.rates.clear();
        self.discount_factors.clear();
        if let Some(survival) = self.survival_probs.as_mut() {
            survival.clear();
        }
        self.pvs.clear();
        if let Some(unfunded) = self.unfunded_amounts.as_mut() {
            unfunded.clear();
        }
        if let Some(commitment) = self.commitment_amounts.as_mut() {
            commitment.clear();
        }
        if let Some(base_rates) = self.base_rates.as_mut() {
            base_rates.clear();
        }
        if let Some(spreads) = self.spreads.as_mut() {
            spreads.clear();
        }
    }
}

impl CashFlowSchedule {
    /// Period-aligned DataFrame-like export with optional credit and floating decomposition.
    ///
    /// This computes all derived columns (discount factors, survival probabilities,
    /// base rate, spread, all-in rate, unfunded amounts) in Rust for consistency
    /// across language bindings. Bindings should only perform type conversion.
    ///
    /// Historical cashflows (`date <= as_of/base`) are included in the table for
    /// auditability but contribute zero PV by convention (matching
    /// `pv_by_period` and the credit-adjusted PV paths).
    ///
    /// Cashflows are bucketed into half-open period intervals
    /// `[period.start, period.end)`, matching
    /// [`crate::aggregation::aggregate_by_period`]: a flow dated exactly on a
    /// period boundary belongs to the next period. Flows outside all supplied
    /// periods are omitted from the table, but they still update the
    /// outstanding/undrawn balance replay so balances are correct under
    /// partial period coverage.
    ///
    /// # Arguments
    ///
    /// * `periods` - Period definitions with start/end boundaries
    /// * `market` - Market context containing discount and optional curves
    /// * `discount_curve_id` - ID of the discount curve to use
    /// * `options` - Additional configuration (hazard/forward IDs, overrides, facility limits)
    ///
    /// # Returns
    ///
    /// A `PeriodDataFrame` with all computed columns.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The discount curve is not found in the market context
    /// - Hazard curve is specified but not found
    /// - Currency mismatches occur in facility limit calculations
    pub fn to_period_dataframe(
        &self,
        periods: &[Period],
        market: &MarketContext,
        discount_curve_id: &str,
        options: PeriodDataFrameOptions<'_>,
    ) -> finstack_core::Result<PeriodDataFrame> {
        let mut out = PeriodDataFrame::with_capacity(self.flows.len());
        self.to_period_dataframe_into(periods, market, discount_curve_id, options, &mut out)?;
        Ok(out)
    }

    /// Period-aligned DataFrame-like export into an existing buffer.
    ///
    /// This reuses allocations in `out` when possible and preserves the
    /// input ordering of cashflows.
    pub(crate) fn to_period_dataframe_into(
        &self,
        periods: &[Period],
        market: &MarketContext,
        discount_curve_id: &str,
        options: PeriodDataFrameOptions<'_>,
        out: &mut PeriodDataFrame,
    ) -> finstack_core::Result<()> {
        use finstack_core::dates::calendar::calendar_by_id;

        let dc = options.day_count.unwrap_or(self.day_count);

        // Resolve calendar for day count context (required for Bus/252 convention)
        let resolved_calendar = options.calendar_id.and_then(calendar_by_id);

        let disc_arc = market.get_discount(discount_curve_id)?;
        let base = options.as_of.unwrap_or_else(|| disc_arc.base_date());

        let has_hazard = options.credit_curve_id.is_some();
        let hazard_arc_opt = if let Some(hz) = options.credit_curve_id {
            Some(market.get_hazard(hz)?)
        } else {
            None
        };
        // Propagate forward-curve lookup failures: silently disabling the
        // requested floating decomposition would emit None columns without
        // any signal. `None` is only valid when no forward_curve_id was given.
        let forward_arc_opt = if options.include_floating_decomposition {
            match options.forward_curve_id {
                Some(fid) => Some(market.get_forward(fid)?),
                None => None,
            }
        } else {
            None
        };

        // Prefer explicit facility_limit; fallback to schedule meta (e.g., RCF commitment)
        let facility_limit = options.facility_limit.or(self.meta.facility_limit);
        let capacity = self.flows.len();
        let include_floating = options.include_floating_decomposition;
        let has_facility = facility_limit.is_some();

        out.clear();
        out.start_dates.reserve(capacity);
        out.end_dates.reserve(capacity);
        out.pay_dates.reserve(capacity);
        out.reset_dates.reserve(capacity);
        out.cf_types.reserve(capacity);
        out.currencies.reserve(capacity);
        out.notionals.reserve(capacity);
        out.yr_fraqs.reserve(capacity);
        out.accrual_factors.reserve(capacity);
        out.days.reserve(capacity);
        out.amounts.reserve(capacity);
        out.rates.reserve(capacity);
        out.discount_factors.reserve(capacity);
        out.pvs.reserve(capacity);

        // Initialize optional columns using helper to reduce repetition
        init_optional_column(has_facility, capacity, &mut out.undrawn_notionals);
        init_optional_column(has_hazard, capacity, &mut out.survival_probs);
        init_optional_column(has_facility, capacity, &mut out.unfunded_amounts);
        init_optional_column(has_facility, capacity, &mut out.commitment_amounts);
        init_optional_column(include_floating, capacity, &mut out.base_rates);
        init_optional_column(include_floating, capacity, &mut out.spreads);

        // Track outstanding drawn balance for Notional column
        let mut outstanding = self.notional.initial;

        // Anchor initial-funding detection on the schedule's issue date when
        // available (populated by `finalize_flows`); a pre-issue principal
        // event would otherwise make the real issue-date funding flow read as
        // a draw (~2x notional). Fall back to the first flow's date only when
        // no issue date is recorded.
        let funding_anchor = self
            .meta
            .issue_date
            .or_else(|| self.flows.first().map(|cf| cf.date));

        // Sort periods by start date for cursor advance
        let mut sorted_period_indices: Vec<usize> = (0..periods.len()).collect();
        sorted_period_indices.sort_by_key(|&i| periods[i].start);

        let mut period_cursor = 0;
        let dc_ctx = DayCountContext {
            calendar: resolved_calendar,
            frequency: options.frequency,
            bus_basis: None,
            coupon_period: None,
        };
        let disc_dc_ctx = DayCountContext {
            calendar: resolved_calendar,
            frequency: options.frequency,
            bus_basis: None,
            coupon_period: None,
        };
        let fwd_dc_ctx = DayCountContext {
            calendar: resolved_calendar,
            frequency: options.frequency,
            bus_basis: None,
            coupon_period: None,
        };

        for cf in &self.flows {
            // Outstanding before this cashflow
            let outstanding_pre = outstanding;

            // Detect initial funding notional flow (negative, equal to
            // -notional.initial on the issue date). It is already accounted
            // for in notional.initial, so we skip it to avoid double-counting.
            let is_initial_funding = cf.kind == CFKind::Notional
                && funding_anchor == Some(cf.date)
                && cf.amount.amount() < 0.0
                && amounts_approx_equal(cf.amount.amount().abs(), self.notional.initial.amount());

            // Balance replay must see every flow, including those outside the
            // supplied reporting periods, so the update happens BEFORE the
            // period-membership check below.
            match cf.kind {
                CFKind::Amortization => {
                    outstanding = outstanding.checked_sub(cf.amount)?;
                }
                CFKind::PIK => {
                    outstanding = outstanding.checked_add(cf.amount)?;
                }
                CFKind::Notional if !is_initial_funding => {
                    // Draws are negative, repays are positive from lender perspective
                    outstanding = outstanding.checked_sub(cf.amount)?;
                }
                _ => {}
            }

            // Advance cursor past periods that end on or before this cashflow.
            // Bucketing is half-open `[start, end)`, matching
            // `aggregation::aggregate_by_period`: a flow dated exactly on a
            // period boundary belongs to the NEXT period.
            while period_cursor < sorted_period_indices.len()
                && cf.date >= periods[sorted_period_indices[period_cursor]].end
            {
                period_cursor += 1;
            }
            // Check if current cashflow falls in the current period
            let period = if period_cursor < sorted_period_indices.len() {
                let pi = sorted_period_indices[period_cursor];
                let p = &periods[pi];
                if cf.date >= p.start && cf.date < p.end {
                    p
                } else {
                    continue;
                }
            } else {
                continue;
            };

            // Basic columns
            out.start_dates.push(period.start);
            out.end_dates.push(period.end);
            out.pay_dates.push(cf.date);
            out.reset_dates.push(cf.reset_date);
            out.cf_types.push(cf.kind);
            out.currencies.push(cf.amount.currency());
            out.amounts.push(cf.amount.amount());
            out.accrual_factors.push(cf.accrual_factor);
            out.rates.push(cf.rate.unwrap_or(0.0));

            // Notional balances for interest/fee-like rows
            let (notional_drawn, notional_undrawn) =
                compute_notional_columns(cf, outstanding_pre.amount(), facility_limit.as_ref());
            out.notionals.push(notional_drawn);
            if let Some(ref mut undrawn) = out.undrawn_notionals {
                undrawn.push(notional_undrawn);
            }

            // YrFraq and Days - use proper DayCountContext with frequency/calendar from options
            let yr_fraq = dc.year_fraction(period.start, cf.date, dc_ctx)?;
            out.yr_fraqs.push(yr_fraq);
            out.days.push((cf.date - period.start).whole_days());

            // Discount factor using configured discounting basis
            let dc_for_discounting = options.discount_day_count.unwrap_or(dc);
            let t = compute_discount_time(cf.date, base, dc_for_discounting, disc_dc_ctx)?;
            let df = disc_arc.df(t);
            // Fail loudly on non-finite curve output instead of silently
            // pushing NaN PVs (consistent with `npv`'s validation).
            if !df.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "discount curve '{discount_curve_id}' returned non-finite df ({df}) at t={t} (date {})",
                    cf.date
                )));
            }
            out.discount_factors.push(df);

            // Survival probability
            if let (Some(h), Some(spv)) = (hazard_arc_opt.as_ref(), out.survival_probs.as_mut()) {
                let sp = h.sp(t);
                if !sp.is_finite() {
                    return Err(finstack_core::Error::Validation(format!(
                        "hazard curve returned non-finite sp ({sp}) at t={t} (date {})",
                        cf.date
                    )));
                }
                spv.push(Some(sp));
            }

            // PV — PIK rows are notional accruals, not cash flows, so they
            // contribute zero PV.  Their economic value is already captured
            // in the inflated notional redemption at maturity.
            let is_non_cash = cf.kind == CFKind::PIK;
            let sp_mult = if let Some(ref spv) = out.survival_probs {
                spv.last().copied().flatten().unwrap_or(1.0)
            } else {
                1.0
            };
            let pv_amt = if is_non_cash || cf.date <= base {
                0.0
            } else {
                cf.amount.amount() * df * sp_mult
            };
            out.pvs.push(pv_amt);

            // Unfunded and commitment amounts
            if let Some(limit) = facility_limit.as_ref() {
                if let Some(ref mut unfunded_vec) = out.unfunded_amounts {
                    if limit.currency() == cf.amount.currency() {
                        let val = (limit.amount() - outstanding_pre.amount()).max(0.0);
                        unfunded_vec.push(Some(val));
                    } else {
                        unfunded_vec.push(None);
                    }
                }
                if let Some(ref mut commit_vec) = out.commitment_amounts {
                    if limit.currency() == cf.amount.currency() {
                        commit_vec.push(Some(limit.amount()));
                    } else {
                        commit_vec.push(None);
                    }
                }
            }

            // Floating decomposition (base rate and spread)
            let (base_rate_opt, spread_opt) = if options.include_floating_decomposition {
                compute_floating_decomposition(
                    cf,
                    forward_arc_opt.as_ref(),
                    base,
                    period.start,
                    fwd_dc_ctx,
                )?
            } else {
                (None, None)
            };
            if let Some(ref mut br) = out.base_rates {
                br.push(base_rate_opt);
            }
            if let Some(ref mut sp) = out.spreads {
                sp.push(spread_opt);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::schedule::{CashFlowMeta, CashFlowSchedule};
    use crate::builder::Notional;
    use finstack_core::cashflow::CashFlow;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{DayCount, Period, PeriodId};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("Valid month (1-12)"), day)
            .expect("Valid test date")
    }

    fn quarters_2025() -> Vec<Period> {
        vec![
            Period {
                id: PeriodId::quarter(2025, 1),
                start: d(2025, 1, 1),
                end: d(2025, 4, 1),
                is_actual: true,
            },
            Period {
                id: PeriodId::quarter(2025, 2),
                start: d(2025, 4, 1),
                end: d(2025, 7, 1),
                is_actual: false,
            },
        ]
    }

    #[test]
    fn dataframe_sets_zero_pv_for_historical_rows() {
        // Build a simple schedule with one historical and one future cashflow
        let base = d(2025, 4, 1);
        let flows = vec![
            CashFlow {
                date: d(2025, 3, 15), // historical
                reset_date: None,
                amount: Money::new(100.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: None,
            },
            CashFlow {
                date: d(2025, 5, 15), // future
                reset_date: None,
                amount: Money::new(200.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: None,
            },
        ];
        let schedule = CashFlowSchedule {
            flows,
            notional: Notional::par(1_000.0, Currency::USD),
            day_count: DayCount::Act365F,
            meta: CashFlowMeta::default(),
        };

        // Market context with flat discount curve (df = 1.0)
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (30.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed with valid test data");
        let market = MarketContext::new().insert(curve);

        let periods = quarters_2025();
        let options = PeriodDataFrameOptions {
            credit_curve_id: None,
            forward_curve_id: None,
            as_of: Some(base),
            day_count: Some(DayCount::Act365F),
            discount_day_count: None,
            facility_limit: None,
            include_floating_decomposition: false,
            frequency: None,
            calendar_id: None,
        };

        let df = schedule
            .to_period_dataframe(&periods, &market, "USD-OIS", options)
            .expect("PeriodDataFrame creation should succeed in test");
        // Find PVs aligned with input cashflows
        // Historical row should be 0.0 PV; future row should be amount * DF
        assert_eq!(df.pvs.len(), 2);
        assert!((df.pvs[0] - 0.0).abs() < 1e-12);
        assert!((df.pvs[1] - 200.0 * df.discount_factors[1]).abs() < 1e-12);
    }

    #[test]
    fn dataframe_does_not_double_count_initial_funding() {
        // Build a schedule that includes the initial funding notional flow
        // (like what CashFlowBuilder produces).
        // The initial funding is a NEGATIVE Notional flow on the first date.
        let issue = d(2025, 1, 15);
        let initial_amount = 1_000_000.0;
        let flows = vec![
            // Initial funding (negative from lender perspective - money out)
            CashFlow {
                date: issue,
                reset_date: None,
                amount: Money::new(-initial_amount, Currency::USD),
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
            },
            // First coupon
            CashFlow {
                date: d(2025, 4, 15),
                reset_date: None,
                amount: Money::new(12_500.0, Currency::USD), // 5% quarterly
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: Some(0.05),
            },
        ];
        let schedule = CashFlowSchedule {
            flows,
            notional: Notional::par(initial_amount, Currency::USD),
            day_count: DayCount::Act365F,
            meta: CashFlowMeta::default(),
        };

        // Market context
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed");
        let market = MarketContext::new().insert(curve);

        let periods = vec![
            Period {
                id: PeriodId::quarter(2025, 1),
                start: d(2025, 1, 1),
                end: d(2025, 4, 1),
                is_actual: true,
            },
            Period {
                id: PeriodId::quarter(2025, 2),
                start: d(2025, 4, 1),
                end: d(2025, 7, 1),
                is_actual: false,
            },
        ];
        let options = PeriodDataFrameOptions {
            as_of: Some(issue),
            day_count: Some(DayCount::Act365F),
            ..Default::default()
        };

        let df = schedule
            .to_period_dataframe(&periods, &market, "USD-OIS", options)
            .expect("PeriodDataFrame creation should succeed");

        // The coupon flow's notional should be the original notional (1M),
        // NOT double-counted (2M) due to the initial funding flow.
        // The coupon is the second row (index 1).
        assert_eq!(df.cf_types.len(), 2);
        assert_eq!(df.cf_types[0], CFKind::Notional);
        assert_eq!(df.cf_types[1], CFKind::Fixed);

        // The notional for the coupon row should be 1M, not 2M
        let coupon_notional = df.notionals[1].expect("Coupon should have notional");
        assert!(
            (coupon_notional - initial_amount).abs() < 1e-6,
            "Expected notional {} but got {} (double-counting bug if ~2M)",
            initial_amount,
            coupon_notional
        );
    }

    #[test]
    fn dataframe_omits_undrawn_notionals_without_facility_limit() {
        let base = d(2025, 4, 1);
        let flows = vec![CashFlow {
            date: d(2025, 5, 15),
            reset_date: None,
            amount: Money::new(200.0, Currency::USD),
            kind: CFKind::Fixed,
            accrual_factor: 0.25,
            rate: None,
        }];
        let schedule = CashFlowSchedule {
            flows,
            notional: Notional::par(1_000.0, Currency::USD),
            day_count: DayCount::Act365F,
            meta: CashFlowMeta::default(),
        };

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (30.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed with valid test data");
        let market = MarketContext::new().insert(curve);

        let options = PeriodDataFrameOptions {
            as_of: Some(base),
            day_count: Some(DayCount::Act365F),
            ..Default::default()
        };

        let df = schedule
            .to_period_dataframe(&quarters_2025(), &market, "USD-OIS", options)
            .expect("PeriodDataFrame creation should succeed in test");

        assert!(df.undrawn_notionals.is_none());
    }

    #[test]
    fn dataframe_buckets_boundary_flow_half_open_matching_aggregation() {
        // A flow exactly on a period boundary must bucket identically in
        // `to_period_dataframe` and `aggregate_by_period`: half-open
        // [start, end), so the boundary flow belongs to the NEXT period.
        use crate::aggregation::aggregate_by_period;
        use finstack_core::dates::PeriodId;

        let base = d(2025, 1, 1);
        let boundary = d(2025, 4, 1); // == Q1 end == Q2 start
        let flows = vec![CashFlow {
            date: boundary,
            reset_date: None,
            amount: Money::new(100.0, Currency::USD),
            kind: CFKind::Fixed,
            accrual_factor: 0.25,
            rate: None,
        }];
        let schedule = CashFlowSchedule {
            flows: flows.clone(),
            notional: Notional::par(1_000.0, Currency::USD),
            day_count: DayCount::Act365F,
            meta: CashFlowMeta::default(),
        };

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (30.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed");
        let market = MarketContext::new().insert(curve);
        let periods = quarters_2025();

        let df = schedule
            .to_period_dataframe(
                &periods,
                &market,
                "USD-OIS",
                PeriodDataFrameOptions {
                    as_of: Some(base),
                    day_count: Some(DayCount::Act365F),
                    ..Default::default()
                },
            )
            .expect("PeriodDataFrame creation should succeed");

        assert_eq!(df.pay_dates.len(), 1);
        // DataFrame assigns the boundary flow to Q2 [Apr 1, Jul 1).
        assert_eq!(df.start_dates[0], d(2025, 4, 1));
        assert_eq!(df.end_dates[0], d(2025, 7, 1));

        // aggregate_by_period agrees: the flow lands in Q2, not Q1.
        let dated: Vec<crate::DatedFlow> = flows.iter().map(|cf| (cf.date, cf.amount)).collect();
        let agg = aggregate_by_period(&dated, &periods).expect("aggregation succeeds");
        assert!(!agg.contains_key(&PeriodId::quarter(2025, 1)));
        assert!(agg.contains_key(&PeriodId::quarter(2025, 2)));
    }

    #[test]
    fn dataframe_balances_correct_under_partial_period_coverage() {
        // Flows outside the supplied reporting periods must still update the
        // outstanding balance replay: an early amortization excluded from the
        // periods reduces the notional seen by a later in-period coupon.
        let base = d(2025, 1, 1);
        let initial = 1_000_000.0;
        let flows = vec![
            CashFlow {
                date: d(2025, 2, 15), // outside the (Q2-only) reporting window
                reset_date: None,
                amount: Money::new(200_000.0, Currency::USD),
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            },
            CashFlow {
                date: d(2025, 5, 15), // inside Q2
                reset_date: None,
                amount: Money::new(10_000.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: Some(0.05),
            },
        ];
        let schedule = CashFlowSchedule {
            flows,
            notional: Notional::par(initial, Currency::USD),
            day_count: DayCount::Act365F,
            meta: CashFlowMeta::default(),
        };

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (30.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed");
        let market = MarketContext::new().insert(curve);

        // Reporting window covers Q2 only; the amortization flow is excluded.
        let periods = vec![Period {
            id: PeriodId::quarter(2025, 2),
            start: d(2025, 4, 1),
            end: d(2025, 7, 1),
            is_actual: false,
        }];

        let df = schedule
            .to_period_dataframe(
                &periods,
                &market,
                "USD-OIS",
                PeriodDataFrameOptions {
                    as_of: Some(base),
                    day_count: Some(DayCount::Act365F),
                    ..Default::default()
                },
            )
            .expect("PeriodDataFrame creation should succeed");

        // Only the coupon row is emitted, but its notional reflects the
        // out-of-window amortization (1M - 200k = 800k).
        assert_eq!(df.cf_types.len(), 1);
        assert_eq!(df.cf_types[0], CFKind::Fixed);
        let coupon_notional = df.notionals[0].expect("coupon should have notional");
        assert!(
            (coupon_notional - 800_000.0).abs() < 1e-6,
            "expected 800000 (balance replays out-of-window amortization), got {coupon_notional}"
        );
    }

    #[test]
    fn dataframe_pre_issue_event_does_not_double_notional() {
        // A pre-issue principal event must not make the real issue-date
        // funding flow read as a draw (which would ~double the notional
        // column). Initial-funding detection anchors on meta.issue_date.
        let issue = d(2025, 1, 15);
        let initial = 1_000_000.0;
        let flows = vec![
            // Pre-issue principal draw (delayed-funding structure)
            CashFlow {
                date: d(2025, 1, 5),
                reset_date: None,
                amount: Money::new(-100_000.0, Currency::USD),
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
            },
            // Real initial funding flow on the issue date
            CashFlow {
                date: issue,
                reset_date: None,
                amount: Money::new(-initial, Currency::USD),
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
            },
            // First coupon
            CashFlow {
                date: d(2025, 4, 15),
                reset_date: None,
                amount: Money::new(12_500.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: Some(0.05),
            },
        ];
        let meta = CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        };
        let schedule = CashFlowSchedule {
            flows,
            notional: Notional::par(initial, Currency::USD),
            day_count: DayCount::Act365F,
            meta,
        };

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed");
        let market = MarketContext::new().insert(curve);

        let periods = vec![
            Period {
                id: PeriodId::quarter(2025, 1),
                start: d(2025, 1, 1),
                end: d(2025, 4, 1),
                is_actual: true,
            },
            Period {
                id: PeriodId::quarter(2025, 2),
                start: d(2025, 4, 1),
                end: d(2025, 7, 1),
                is_actual: false,
            },
        ];

        let df = schedule
            .to_period_dataframe(
                &periods,
                &market,
                "USD-OIS",
                PeriodDataFrameOptions {
                    as_of: Some(issue),
                    day_count: Some(DayCount::Act365F),
                    ..Default::default()
                },
            )
            .expect("PeriodDataFrame creation should succeed");

        assert_eq!(df.cf_types.len(), 3);
        assert_eq!(df.cf_types[2], CFKind::Fixed);
        // The issue-date funding flow is recognized via meta.issue_date and
        // skipped, so the coupon notional is initial + pre-issue draw (1.1M),
        // NOT ~2.1M (which would indicate the funding flow was replayed as a
        // draw).
        let coupon_notional = df.notionals[2].expect("coupon should have notional");
        assert!(
            (coupon_notional - 1_100_000.0).abs() < 1e-6,
            "expected 1100000 but got {coupon_notional} (notional doubled if ~2.1M)"
        );
    }

    #[test]
    fn dataframe_errors_on_missing_forward_curve() {
        // A requested-but-missing forward curve must error instead of
        // silently disabling floating decomposition.
        let base = d(2025, 1, 1);
        let flows = vec![CashFlow {
            date: d(2025, 7, 1),
            reset_date: Some(d(2025, 4, 1)),
            amount: Money::new(100.0, Currency::USD),
            kind: CFKind::FloatReset,
            accrual_factor: 0.25,
            rate: Some(0.05),
        }];
        let schedule = CashFlowSchedule {
            flows,
            notional: Notional::par(1_000.0, Currency::USD),
            day_count: DayCount::Act365F,
            meta: CashFlowMeta::default(),
        };

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (30.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed");
        let market = MarketContext::new().insert(curve);

        let result = schedule.to_period_dataframe(
            &quarters_2025(),
            &market,
            "USD-OIS",
            PeriodDataFrameOptions {
                as_of: Some(base),
                day_count: Some(DayCount::Act365F),
                include_floating_decomposition: true,
                forward_curve_id: Some("MISSING-FWD"),
                ..Default::default()
            },
        );
        assert!(
            result.is_err(),
            "missing forward curve must not silently disable floating decomposition"
        );
    }

    #[test]
    fn dataframe_and_pv_by_period_agree_on_historical_flow_convention() {
        // Parity: a flow dated on/before the valuation base gets zero PV in
        // both the DataFrame export and pv_by_period.
        use crate::aggregation::DateContext;
        use crate::builder::schedule::PvDiscountSource;
        use finstack_core::dates::PeriodId;

        let base = d(2025, 4, 1);
        let flows = vec![
            CashFlow {
                date: d(2025, 2, 15), // historical
                reset_date: None,
                amount: Money::new(100.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: None,
            },
            CashFlow {
                date: d(2025, 5, 15), // future
                reset_date: None,
                amount: Money::new(200.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: None,
            },
        ];
        let schedule = CashFlowSchedule {
            flows,
            notional: Notional::par(1_000.0, Currency::USD),
            day_count: DayCount::Act365F,
            meta: CashFlowMeta::default(),
        };

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (30.0, 1.0)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed");
        let market = MarketContext::new().insert(curve.clone());
        let periods = vec![Period {
            id: PeriodId::annual(2025),
            start: d(2025, 1, 1),
            end: d(2026, 1, 1),
            is_actual: false,
        }];

        let df = schedule
            .to_period_dataframe(
                &periods,
                &market,
                "USD-OIS",
                PeriodDataFrameOptions {
                    as_of: Some(base),
                    day_count: Some(DayCount::Act365F),
                    ..Default::default()
                },
            )
            .expect("PeriodDataFrame creation should succeed");
        let df_pv_total: f64 = df.pvs.iter().sum();

        let pv_map = schedule
            .pv_by_period(
                &periods,
                PvDiscountSource::Discount {
                    disc: &curve,
                    credit: None,
                },
                DateContext::new(
                    base,
                    DayCount::Act365F,
                    finstack_core::dates::DayCountContext::default(),
                ),
            )
            .expect("pv_by_period succeeds");
        let agg_pv = pv_map[&PeriodId::annual(2025)][&Currency::USD].amount();

        // Both conventions zero the historical flow; with df = 1 the total
        // PV is just the future flow's amount.
        assert!((df_pv_total - 200.0).abs() < 1e-9, "df pv {df_pv_total}");
        assert!(
            (df_pv_total - agg_pv).abs() < 1e-9,
            "DataFrame PV total ({df_pv_total}) must match pv_by_period ({agg_pv})"
        );
    }

    #[test]
    fn dataframe_propagates_day_count_errors() {
        let base = d(2025, 1, 1);
        // Dated strictly inside Q2 [Apr 1, Jul 1): under half-open bucketing a
        // flow on the final period end would fall outside all periods and skip
        // the day-count path entirely.
        let flows = vec![CashFlow {
            date: d(2025, 5, 15),
            reset_date: None,
            amount: Money::new(100.0, Currency::USD),
            kind: CFKind::Fixed,
            accrual_factor: 0.5,
            rate: Some(0.04),
        }];
        let schedule = CashFlowSchedule {
            flows,
            notional: Notional::par(1_000.0, Currency::USD),
            day_count: DayCount::ActActIsma,
            meta: CashFlowMeta::default(),
        };

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed with valid test data");
        let market = MarketContext::new().insert(curve);

        let result = schedule.to_period_dataframe(
            &quarters_2025(),
            &market,
            "USD-OIS",
            PeriodDataFrameOptions {
                as_of: Some(base),
                day_count: Some(DayCount::ActActIsma),
                ..Default::default()
            },
        );

        assert!(
            result.is_err(),
            "missing ISMA frequency should not be silently zeroed"
        );
    }
}
