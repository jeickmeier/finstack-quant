//! Deterministic cashflow discounting pricer for term loans.
//!
//! This module provides the standard pricer for term loans using:
//! - Complete cashflow generation (DDTL draws, interest, amortization, PIK, fees)
//! - Discounting to present value using the instrument's discount curve
//! - PIK interest capitalization (excluded from PV, increases outstanding)
//!
//! # Pricing Methodology
//!
//! The pricer follows these steps:
//! 1. Generate full internal cashflow schedule via [`generate_cashflows`]
//! 2. Filter to cash flows only (exclude PIK capitalization)
//! 3. Discount flows using the discount curve anchored to `as_of` date
//! 4. Return present value in loan currency
//!
//! # PIK Treatment
//!
//! Payment-in-kind (PIK) interest is:
//! - Capitalized into outstanding principal (affects principal path)
//! - **Excluded from PV calculation** (not a cash flow to holder)
//! - Reflected in final redemption amount
//!
//! This follows institutional market practice where PIK increases debt balance
//! rather than generating cash flows.
//!
//! # Examples
//!
//! ```text
//! use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
//! use finstack_quant_valuations::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let loan = TermLoan::example().expect("TermLoan example is valid");
//! let market = MarketContext::new();
//! let as_of = Date::from_calendar_date(2025, Month::January, 15)?;
//!
//! // Price using deterministic discounting
//! // let pv = TermLoanDiscountingPricer::price(&loan, &market, as_of)?;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`generate_cashflows`] for cashflow generation details
//! - [`super::types::TermLoan`] for the instrument type

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::term_loan::types::RateSpec;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;

use crate::instruments::fixed_income::term_loan::cashflows::generate_cashflows;
use crate::instruments::fixed_income::term_loan::TermLoan;

/// Discount holder-view flows using the curve's date-based discount factors.
/// Cashflows on or before `as_of` are already settled and therefore excluded.
fn npv_by_date(
    disc: &DiscountCurve,
    as_of: finstack_quant_core::dates::Date,
    flows: &[(finstack_quant_core::dates::Date, Money)],
) -> finstack_quant_core::Result<Money> {
    if flows.is_empty() {
        return Err(finstack_quant_core::InputError::TooFewPoints.into());
    }

    let mut total = Money::new(0.0, flows[0].1.currency());
    for (date, amount) in flows {
        if *date <= as_of {
            continue;
        }
        total = total.checked_add(*amount * disc.df_between_dates(as_of, *date)?)?;
    }
    Ok(total)
}

/// Term loan pricer using deterministic cashflow discounting.
///
/// Prices term loans by generating complete cashflow schedules and discounting
/// to present value. Handles all term loan features including DDTL, PIK, covenants,
/// and amortization.
///
/// # Pricing Method
///
/// Uses full-fidelity cashflow generation with:
/// - Time-dependent outstanding principal (DDTL draws, amortization, PIK)
/// - Floating rate projection with floors/caps
/// - Covenant-driven margin adjustments
/// - Fee accruals (commitment, usage, upfront)
/// - PIK capitalization (excluded from PV)
///
/// # Thread Safety
///
/// This pricer is stateless and thread-safe (`Send + Sync`).
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer;
/// use finstack_quant_valuations::pricer::Pricer;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pricer = TermLoanDiscountingPricer;
/// let loan = TermLoan::example().expect("TermLoan example is valid");
/// let market = MarketContext::new();
/// let as_of = Date::from_calendar_date(2025, Month::January, 15)?;
///
/// // Price using the Pricer trait
/// // let result = pricer.price_dyn(&loan, &market, as_of)?;
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct TermLoanDiscountingPricer;

impl TermLoanDiscountingPricer {
    /// Price a term loan using deterministic cashflows and discounting.
    ///
    /// Generates complete cashflow schedule including DDTL draws, interest, fees,
    /// amortization, and redemptions, then discounts cash flows to present value.
    ///
    /// # Arguments
    ///
    /// * `loan` - The term loan instrument to price
    /// * `market` - Market context with discount curves and forward rate data
    /// * `as_of` - Valuation date (cashflows before this date are excluded)
    ///
    /// # Returns
    ///
    /// Present value of the loan in the loan's currency. Represents the fair value
    /// to a holder on the valuation date.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Discount curve is not found in market context
    /// - Cashflow generation fails (invalid dates, schedule errors)
    /// - Currency mismatch in flows
    /// - Forward rate projection fails for floating rate loans
    ///
    /// # PIK Treatment
    ///
    /// PIK (payment-in-kind) interest is capitalized into outstanding principal and excluded
    /// from PV calculation. Only cash flows are discounted:
    /// - Coupons (Fixed, FloatReset, Stub)
    /// - Amortization
    /// - Redemptions (Notional)
    /// - Fees
    ///
    /// PIK capitalization affects the outstanding principal path and is reflected in the
    /// final redemption amount.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
    /// use finstack_quant_valuations::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::dates::Date;
    /// use time::Month;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let loan = TermLoan::example().expect("TermLoan example is valid");
    /// let market = MarketContext::new();
    /// let as_of = Date::from_calendar_date(2025, Month::January, 15)?;
    ///
    /// // let pv = TermLoanDiscountingPricer::price(&loan, &market, as_of)?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) fn price(
        loan: &TermLoan,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        if loan.settlement_date(as_of)? >= loan.maturity {
            return Ok(Money::new(0.0, loan.currency));
        }

        // Retrieve discount curve and discount the holder-view flows to the
        // settlement date using date-based DF mapping. This anchors valuation on
        // the settlement date rather than the trade date, consistent with
        // leveraged loan market conventions.
        let (settlement_date, flows) = Self::pricing_flows(loan, market, as_of)?;
        let disc = market.get_discount(loan.discount_curve_id.as_str())?;

        npv_by_date(disc.as_ref(), settlement_date, &flows)
    }

    /// Build the holder-view cashflows the discounting pricer values, anchored
    /// to the loan's settlement date.
    ///
    /// Returns `(settlement_date, flows)` where `flows` are the same `(date,
    /// amount)` pairs [`price`](Self::price) discounts: PIK capitalization and
    /// pre-settlement flows are excluded, and seasoned floating coupons reflect
    /// historical fixings where available. Sharing this builder keeps the
    /// z-spread CS01 fallback (which reprices these flows under a spread bump)
    /// consistent with the base PV.
    pub(crate) fn pricing_flows(
        loan: &TermLoan,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(
        finstack_quant_core::dates::Date,
        Vec<(finstack_quant_core::dates::Date, Money)>,
    )> {
        use finstack_quant_core::cashflow::CFKind;

        // Compute settlement date using business-day conventions when calendar is available.
        let settlement_date = loan.settlement_date(as_of)?;

        let schedule = Self::pricing_schedule(loan, market, as_of)?;

        // Filter flows: exclude PIK (capitalized interest) and past flows from PV.
        // PIK increases outstanding and is repaid via principal redemption.
        // Past flows (before settlement_date) have already settled and must not be discounted.
        let flows: Vec<(finstack_quant_core::dates::Date, Money)> = schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.kind != CFKind::PIK && cf.date >= settlement_date)
            .map(|cf| (cf.date, cf.amount))
            .collect();

        Ok((settlement_date, flows))
    }

    /// Canonical generated schedule with all economically known fixings applied.
    pub(crate) fn pricing_schedule(
        loan: &TermLoan,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::schedule::CashFlowSchedule> {
        let mut schedule = generate_cashflows(loan, market, as_of)?;
        Self::apply_fixings(loan, market, as_of, &mut schedule)?;
        Ok(schedule)
    }

    /// Replace forward-projected rates with historical fixings for seasoned
    /// floating-rate periods while retaining each period's effective margin.
    fn apply_fixings(
        loan: &TermLoan,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
        schedule: &mut crate::cashflow::builder::schedule::CashFlowSchedule,
    ) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::cashflow::CFKind;
        use rust_decimal::prelude::ToPrimitive;
        use std::collections::BTreeMap;

        let float_spec = match &loan.rate {
            RateSpec::Floating(spec) => spec,
            RateSpec::Fixed { .. } => return Ok(()),
        };

        if loan.issue_date >= as_of {
            return Ok(());
        }

        let has_past_reset = schedule.get_flows().iter().any(|flow| {
            flow.kind == CFKind::FloatReset && flow.reset_date.is_some_and(|date| date < as_of)
        });
        if !has_past_reset {
            return Ok(());
        }

        let fixing_series = finstack_quant_core::market_data::fixings::get_fixing_series(
            market,
            float_spec.index_id.as_str(),
        )?;

        // Margin events are effective from the start of the contractual accrual
        // period. Combine coincident covenant and override changes before lookup.
        let mut margin_deltas = BTreeMap::<finstack_quant_core::dates::Date, i32>::new();
        if let Some(covenants) = &loan.covenants {
            for step in &covenants.margin_stepups {
                *margin_deltas.entry(step.date).or_default() += step.delta_bp;
            }
        }
        if let Some(overrides) = &loan.instrument_pricing_overrides.term_loan {
            for (date, delta_bp) in &overrides.margin_add_bp_by_date {
                *margin_deltas.entry(*date).or_default() += *delta_bp;
            }
        }

        let base_params = crate::cashflow::builder::FloatingRateParams {
            spread_bp: float_spec.spread_bp.to_f64().unwrap_or_default(),
            gearing: float_spec.gearing.to_f64().unwrap_or(1.0),
            gearing_includes_spread: float_spec.gearing_includes_spread,
            index_floor_bp: float_spec.index_floor_bp.and_then(|value| value.to_f64()),
            index_cap_bp: float_spec.index_cap_bp.and_then(|value| value.to_f64()),
            all_in_floor_bp: float_spec.all_in_floor_bp.and_then(|value| value.to_f64()),
            all_in_cap_bp: float_spec.all_in_cap_bp.and_then(|value| value.to_f64()),
        };

        let outstanding_path = schedule.outstanding_by_date()?;
        let initial_notional = schedule.get_notional().initial.amount();
        let notional_before = |target: finstack_quant_core::dates::Date| -> f64 {
            let mut last = initial_notional;
            for (date, amount) in &outstanding_path {
                if *date < target {
                    last = amount.amount();
                } else {
                    break;
                }
            }
            last
        };

        schedule.try_update_flows(|flow| {
            if flow.kind != CFKind::FloatReset {
                return Ok(());
            }
            let reset_date = match flow.reset_date {
                Some(date) if date < as_of => date,
                _ => return Ok(()),
            };

            let raw_fixing = finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                Some(fixing_series),
                float_spec.index_id.as_str(),
                reset_date,
                as_of,
            )?;
            let accrual_start = flow.accrual.map_or(reset_date, |accrual| accrual.start);
            let active_delta_bp: i32 = margin_deltas
                .range(..=accrual_start)
                .map(|(_, delta_bp)| *delta_bp)
                .sum();
            let mut params = base_params.clone();
            params.spread_bp += f64::from(active_delta_bp);
            let all_in_rate = crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                raw_fixing, &params,
            );

            // Rescaling retains the builder's accrual notional and any
            // intra-period balance segmentation. The balance fallback handles
            // a projected coupon whose original rate was exactly zero.
            let new_amount = match flow.rate {
                Some(old_rate) if old_rate.is_finite() && old_rate.abs() > 1.0e-14 => {
                    flow.amount.amount() * all_in_rate / old_rate
                }
                _ => notional_before(flow.date) * all_in_rate * flow.accrual_factor,
            };
            flow.rate = Some(all_in_rate);
            flow.amount = Money::new(new_amount, flow.amount.currency());
            Ok(())
        })?;
        Ok(())
    }
}

impl Pricer for TermLoanDiscountingPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::TermLoan, ModelKey::Discounting)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let loan = instrument
            .as_any()
            .downcast_ref::<TermLoan>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::TermLoan, instrument.key())
            })?;

        // Use the provided as_of date for valuation
        let pv = Self::price(loan, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(loan.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::CouponType;
    use crate::cashflow::builder::FloatingRateSpec;
    use crate::instruments::fixed_income::term_loan::spec::AmortizationSpec;
    use crate::instruments::pricing_overrides::InstrumentPricingOverrides;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use rust_decimal::Decimal;
    use time::Month;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("month"), d).expect("date")
    }

    #[test]
    fn pik_cashflows_are_excluded_from_pv() {
        let as_of = date(2025, 1, 15);
        let disc = DiscountCurve::builder(CurveId::new("USD-OIS"))
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc.clone());

        let mut loan = TermLoan::example().expect("TermLoan example is valid");
        loan.coupon_type = CouponType::PIK;
        loan.discount_curve_id = CurveId::new("USD-OIS");

        let schedule = generate_cashflows(&loan, &market, as_of).expect("cashflows");
        assert!(
            schedule.get_flows().iter().any(|cf| cf.kind == CFKind::PIK),
            "PIK loan should generate PIK cashflows"
        );

        let pv_excluding =
            TermLoanDiscountingPricer::price(&loan, &market, as_of).expect("pv excluding PIK");

        let flows_including: Vec<(Date, Money)> = schedule
            .get_flows()
            .iter()
            .map(|cf| (cf.date, cf.amount))
            .collect();
        let pv_including = npv_by_date(&disc, as_of, &flows_including).expect("pv including PIK");

        assert!(
            pv_including.amount() > pv_excluding.amount(),
            "Including PIK flows should increase PV (excluded by default)"
        );
    }

    // -----------------------------------------------------------------------
    // Fixing support tests
    // -----------------------------------------------------------------------

    /// Build a simple floating-rate term loan for fixing tests.
    ///
    /// Issue: 2024-01-01, Maturity: 2026-01-01 (2Y), Quarterly, Act/360.
    /// SOFR + 300 bps, 0% index floor, no amortization.
    fn floating_loan_for_fixings() -> TermLoan {
        let floating_rate = FloatingRateSpec {
            index_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::new(300, 0),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: Some(Decimal::ZERO),
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        };

        TermLoan::builder()
            .id(InstrumentId::new("TL-FIXING-TEST"))
            .currency(Currency::USD)
            .notional_limit(Money::new(10_000_000.0, Currency::USD))
            .issue_date(date(2024, 1, 1))
            .maturity(date(2026, 1, 1))
            .rate(RateSpec::Floating(floating_rate))
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .amortization(AmortizationSpec::None)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .oid_eir_opt(None)
            .call_schedule_opt(None)
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("floating loan for fixing tests")
    }

    /// Build market context with discount + forward curves (no fixings).
    fn market_without_fixings(base: Date) -> MarketContext {
        let disc = DiscountCurve::builder(CurveId::new("USD-OIS"))
            .base_date(base)
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .build()
            .expect("discount curve");

        // Base the (flat) forward curve at the loan issue date: seasoned resets
        // strictly before the curve base now error under the default fallback.
        let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(date(2024, 1, 1))
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.05), (0.25, 0.05), (5.0, 0.05)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("forward curve");

        MarketContext::new().insert(disc).insert(fwd)
    }

    /// Build market context with discount + forward curves + historical fixings.
    fn market_with_fixings(base: Date) -> MarketContext {
        let market = market_without_fixings(base);

        // Historical fixings: quarterly reset dates for a loan issued 2024-01-01.
        // Use a distinctly different rate (2%) vs the forward curve (5%) so the
        // test can clearly distinguish fixing-based vs forward-based amounts.
        let fixing_series = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![
                (date(2024, 1, 1), 0.02),  // Q1 2024 reset
                (date(2024, 4, 1), 0.02),  // Q2 2024 reset
                (date(2024, 7, 1), 0.02),  // Q3 2024 reset
                (date(2024, 10, 1), 0.02), // Q4 2024 reset
                (date(2025, 1, 1), 0.02),  // Q1 2025 reset
                (date(2025, 4, 1), 0.02),  // Q2 2025 reset
            ],
            None,
        )
        .expect("fixing series");

        market.insert_series(fixing_series)
    }

    #[test]
    fn fixing_replaces_forward_rate_for_past_periods() {
        // Valuation date mid-life: some resets are in the past.
        let as_of = date(2025, 4, 1);
        let loan = floating_loan_for_fixings();
        let market = market_with_fixings(as_of);

        // The shared builder resolves historical fixings while constructing the
        // canonical pricing schedule.
        let schedule = TermLoanDiscountingPricer::pricing_schedule(&loan, &market, as_of)
            .expect("complete fixing history");

        // Check that FloatReset flows with reset_date < as_of use the fixing rate.
        // Fixing rate = 0.02, spread = 300 bps = 0.03 => all_in = 0.05 (with gearing=1).
        let fixing_rate_expected = 0.02 + 0.03; // 5% all-in (but from 2% fixing + 300 bps)

        let past_float_flows: Vec<_> = schedule
            .get_flows()
            .iter()
            .filter(|cf| {
                cf.kind == CFKind::FloatReset && cf.reset_date.is_some_and(|rd| rd < as_of)
            })
            .collect();

        assert!(
            !past_float_flows.is_empty(),
            "should have past FloatReset flows"
        );

        for flow in &past_float_flows {
            let rate = flow.rate.expect("rate should be set");
            assert!(
                (rate - fixing_rate_expected).abs() < 1e-8,
                "past period rate should use fixing: got {rate}, expected {fixing_rate_expected}"
            );
        }
    }

    #[test]
    fn no_fixings_rejects_seasoned_floating_loan() {
        let as_of = date(2025, 4, 1);
        let loan = floating_loan_for_fixings();
        let market_no_fix = market_without_fixings(as_of);
        let err = TermLoanDiscountingPricer::price(&loan, &market_no_fix, as_of)
            .expect_err("seasoned floating loan must require historical fixings");
        assert!(err.to_string().contains("FIXING:USD-SOFR-3M"));
    }

    #[test]
    fn seasoned_fixing_preserves_active_margin_step_up() {
        let as_of = date(2025, 4, 15);
        let mut loan = floating_loan_for_fixings();
        loan.covenants = Some(
            crate::instruments::fixed_income::term_loan::TermLoanCovenantEvents {
                margin_stepups: vec![crate::instruments::fixed_income::term_loan::MarginStepUp {
                    date: date(2025, 1, 1),
                    delta_bp: 200,
                }],
                ..Default::default()
            },
        );
        let market = market_with_fixings(as_of);
        let schedule = TermLoanDiscountingPricer::pricing_schedule(&loan, &market, as_of)
            .expect("seasoned stepped schedule");

        let current_coupon = schedule
            .get_flows()
            .iter()
            .find(|flow| {
                flow.kind == CFKind::FloatReset && flow.reset_date == Some(date(2025, 4, 1))
            })
            .expect("current reset coupon");
        let rate = current_coupon.rate.expect("all-in rate");
        assert!(
            (rate - 0.07).abs() < 1e-12,
            "2% fixing + 300bp base margin + 200bp step-up must equal 7%, got {rate}"
        );
    }

    #[test]
    fn fixing_rate_differs_from_forward_changes_pv() {
        // Use as_of mid-period so that the current period's reset is in the past
        // but its payment date is in the future (included in PV).
        // For quarterly payments on a 2024-01-01 issue, reset dates fall on
        // quarter boundaries. as_of=2025-04-15 means the Q2-2025 reset on
        // 2025-04-01 is "known" but the payment on 2025-07-01 is still future.
        let as_of = date(2025, 4, 15);
        let loan = floating_loan_for_fixings();

        let market_base = market_without_fixings(as_of);

        // Fixings at 1% (distinctly different from the 5% forward rate).
        // All-in: 1% + 3% spread = 4%, vs forward all-in ~8% (5% index + 3% spread).
        let fixing_series_low = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![
                (date(2024, 1, 1), 0.01),
                (date(2024, 4, 1), 0.01),
                (date(2024, 7, 1), 0.01),
                (date(2024, 10, 1), 0.01),
                (date(2025, 1, 1), 0.01),
                (date(2025, 4, 1), 0.01),
            ],
            None,
        )
        .expect("fixing series");
        let market_low = market_base.clone().insert_series(fixing_series_low);

        // Fixings at 10% (much higher than the 5% forward rate).
        // All-in: 10% + 3% spread = 13%.
        let fixing_series_high = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![
                (date(2024, 1, 1), 0.10),
                (date(2024, 4, 1), 0.10),
                (date(2024, 7, 1), 0.10),
                (date(2024, 10, 1), 0.10),
                (date(2025, 1, 1), 0.10),
                (date(2025, 4, 1), 0.10),
            ],
            None,
        )
        .expect("fixing series");
        let market_high = market_base.insert_series(fixing_series_high);

        let pv_low = TermLoanDiscountingPricer::price(&loan, &market_low, as_of)
            .expect("price with low fixings");
        let pv_high = TermLoanDiscountingPricer::price(&loan, &market_high, as_of)
            .expect("price with high fixings");

        // Higher fixing rates => larger coupon amounts => higher PV for the holder.
        // The Q2-2025 coupon (payment date 2025-07-01) is affected by the fixing
        // since its reset_date (2025-04-01) is before as_of (2025-04-15).
        assert!(
            pv_high.amount() > pv_low.amount(),
            "higher fixing rates should produce higher PV: low={}, high={}",
            pv_low.amount(),
            pv_high.amount()
        );
    }

    #[test]
    fn fixed_rate_loan_ignores_fixings() {
        // Fixed-rate loans should be completely unaffected by fixing series.
        let as_of = date(2025, 4, 1);
        let loan = TermLoan::example().expect("fixed-rate example");
        let market_no_fix = market_without_fixings(as_of);
        let market_with_fix = market_with_fixings(as_of);

        let pv_no_fix = TermLoanDiscountingPricer::price(&loan, &market_no_fix, as_of)
            .expect("price without fixings");
        let pv_with_fix = TermLoanDiscountingPricer::price(&loan, &market_with_fix, as_of)
            .expect("price with fixings");

        assert!(
            (pv_no_fix.amount() - pv_with_fix.amount()).abs() < 1e-6,
            "fixed-rate loan should not be affected by fixings: no_fix={}, with_fix={}",
            pv_no_fix.amount(),
            pv_with_fix.amount()
        );
    }
}
