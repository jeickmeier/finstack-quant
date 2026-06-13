//! Bond constructors, factory methods, and schedule building.

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::PricingOverrides;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DateExt, DayCount};
use finstack_core::money::Money;
use finstack_core::types::{Bps, CurveId, InstrumentId, Rate};
use finstack_core::Result;
use time::macros::date;

use super::definitions::{Bond, BondSettlementConvention, CallPut, CallPutSchedule};
use super::CashflowSpec;

impl Bond {
    /// Create a canonical example bond for testing and documentation.
    ///
    /// Returns a 10-year USD Treasury-style bond with realistic parameters.
    pub fn example() -> finstack_core::Result<Self> {
        // SAFETY: All inputs are compile-time validated constants
        Self::with_convention(
            "US912828XG33",
            Money::new(1_000_000.0, Currency::USD),
            Rate::from_decimal(0.0425),
            date!(2024 - 01 - 15),
            date!(2034 - 01 - 15),
            crate::instruments::common_impl::parameters::BondConvention::USTreasury,
            "USD-TREASURY",
        )
    }

    /// Create a standard fixed-rate bond (most common use case).
    ///
    /// Creates a bond with semi-annual frequency, 30/360 day count, and T+2
    /// settlement. Uses simplified schedule conventions (Following BDC,
    /// weekends-only calendar). For production-grade market conventions
    /// including proper holiday calendars and Modified Following BDC, use
    /// `::with_convention(BondConvention::Corporate, ...)` instead.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the bond
    /// * `notional` - Principal amount of the bond
    /// * `coupon_rate` - Annual coupon rate as a typed `Rate`
    /// * `issue` - Issue date of the bond
    /// * `maturity` - Maturity date of the bond
    /// * `discount_curve_id` - Discount curve identifier for pricing
    ///
    /// # Returns
    ///
    /// A `Bond` instance with semi-annual 30/360 conventions.
    ///
    /// # Panics
    ///
    /// Panics if bond construction fails (should not occur with valid inputs).
    ///
    /// # Regional Bond Conventions
    ///
    /// ## US Corporate (use `BondConvention::Corporate` for full conventions)
    /// - **Day Count:** 30/360 (US Bond Basis)
    /// - **Tenor:** Semi-annual
    /// - **Settlement:** T+2
    /// - **BDC:** Modified Following
    /// - **Calendar:** US (NYSE holidays)
    ///
    /// ## US Treasury (use `BondConvention::USTreasury`)
    /// - **Day Count:** ACT/ACT ICMA
    /// - **Tenor:** Semi-annual
    /// - **Settlement:** T+1
    /// - **Calendar:** US (Federal Reserve holidays)
    ///
    /// ## US Agency (use `BondConvention::USAgency`)
    /// - **Day Count:** 30/360 (US Bond Basis)
    /// - **Tenor:** Semi-annual
    /// - **Settlement:** T+1
    /// - **Calendar:** US (NYSE holidays)
    ///
    /// ## United Kingdom (use `BondConvention::UKGilt`)
    /// - **Day Count:** ACT/ACT ICMA
    /// - **Tenor:** Semi-annual
    /// - **Settlement:** T+1
    /// - **Ex-coupon:** 7 business days
    /// - **Calendar:** UK (London holidays)
    ///
    /// ## Europe (use `BondConvention::GermanBund` or `FrenchOAT`)
    /// - **Day Count:** ACT/ACT ICMA
    /// - **Tenor:** Annual
    /// - **Settlement:** T+2
    /// - **Calendar:** TARGET2
    ///
    /// ## Japan (use `BondConvention::JGB`)
    /// - **Day Count:** ACT/365 Fixed
    /// - **Tenor:** Semi-annual
    /// - **Settlement:** T+2 (cross-border; domestic is T+1 since May 2018)
    /// - **Calendar:** Japan holidays
    ///
    /// # Example
    /// ```ignore
    /// use finstack_cashflows::CashflowProvider;
    /// use finstack_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_core::money::Money;
    /// use finstack_core::currency::Currency;
    /// use time::macros::date;
    ///
    /// // US Corporate bond (default)
    /// let notional = Money::new(1_000_000.0, Currency::USD);
    /// let issue = date!(2025-01-15);
    /// let maturity = date!(2030-01-15);
    /// let corp_bond = Bond::fixed(
    ///     "CORP-001",
    ///     notional,
    ///     finstack_core::types::Rate::from_percent(5.0),
    ///     issue,
    ///     maturity,
    ///     "USD-OIS",
    /// )
    /// .unwrap();
    /// # let _ = corp_bond;
    ///
    /// // For US Treasury, use with_convention:
    /// // let treasury = Bond::with_convention("UST-001", notional,
    /// //     finstack_core::types::Rate::from_percent(4.0), issue, maturity,
    /// //                                       BondConvention::USTreasury, "USD-TREASURY").unwrap();
    /// ```
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    pub fn fixed(
        id: impl Into<InstrumentId>,
        notional: Money,
        coupon_rate: impl Into<Rate>,
        issue: Date,
        maturity: Date,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_core::Result<Self> {
        let coupon_rate = coupon_rate.into();
        let bond = Self::builder()
            .id(id.into())
            .notional(notional)
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(CashflowSpec::fixed_rate(
                coupon_rate,
                finstack_core::dates::Tenor::semi_annual(),
                DayCount::Thirty360,
            )?)
            .discount_curve_id(discount_curve_id.into())
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()?;

        // Validate all parameters before returning
        bond.validate()?;
        Ok(bond)
    }

    /// Create a bond with standard market conventions.
    ///
    /// Prefer typed rates for clarity and unit safety.
    ///
    /// Applies region-specific conventions for day count, frequency, and
    /// calendar adjustments. For full customization, use `::builder()`.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the bond
    /// * `notional` - Principal amount of the bond
    /// * `coupon_rate` - Annual coupon rate as a typed `Rate`
    /// * `issue` - Issue date of the bond
    /// * `maturity` - Maturity date of the bond
    /// * `convention` - Regional bond convention (e.g., `BondConvention::USTreasury`)
    /// * `discount_curve_id` - Discount curve identifier for pricing
    ///
    /// # Returns
    ///
    /// A `Bond` instance with conventions matching the specified regional standard.
    ///
    /// # Panics
    ///
    /// Panics if bond construction fails (should not occur with valid inputs).
    ///
    /// # Example
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_valuations::instruments::BondConvention;
    /// use finstack_core::money::Money;
    /// use finstack_core::currency::Currency;
    /// use time::macros::date;
    ///
    /// let notional = Money::new(1_000_000.0, Currency::USD);
    /// let issue = date!(2025-01-15);
    /// let maturity = date!(2030-01-15);
    /// let treasury = Bond::with_convention(
    ///     "UST-5Y",
    ///     notional,
    ///     finstack_core::types::Rate::from_decimal(0.03),
    ///     issue,
    ///     maturity,
    ///     BondConvention::USTreasury,
    ///     "USD-TREASURY"
    /// );
    /// # let _ = treasury;
    /// ```
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    pub fn with_convention(
        id: impl Into<InstrumentId>,
        notional: Money,
        coupon_rate: impl Into<Rate>,
        issue: Date,
        maturity: Date,
        convention: crate::instruments::common_impl::parameters::BondConvention,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_core::Result<Self> {
        let coupon_rate = coupon_rate.into();
        let mut cashflow_spec =
            CashflowSpec::fixed_rate(coupon_rate, convention.frequency(), convention.day_count())?;
        if let CashflowSpec::Fixed(spec) = &mut cashflow_spec {
            spec.bdc = convention.business_day_convention();
            spec.calendar_id = convention
                .calendar_id()
                .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID)
                .to_string();
            spec.end_of_month =
                issue.end_of_month() == issue && maturity.end_of_month() == maturity;
        }
        let bond = Self::builder()
            .id(id.into())
            .notional(notional)
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(cashflow_spec)
            .discount_curve_id(discount_curve_id.into())
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: convention.settlement_days(),
                ex_coupon_days: convention.ex_coupon_days().unwrap_or(0),
                ex_coupon_calendar_id: convention.calendar_id().map(|id| id.to_string()),
            }))
            .build()?;

        // Validate all parameters before returning
        bond.validate()?;
        Ok(bond)
    }

    /// Create a floating-rate bond (FRN).
    ///
    /// Creates a bond with floating-rate coupons linked to a forward index
    /// (e.g., SOFR, EURIBOR) plus a margin.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the bond
    /// * `notional` - Principal amount of the bond
    /// * `index_id` - Forward curve identifier (e.g., "USD-SOFR-3M")
    /// * `margin_bp` - Spread over index in typed basis points (e.g., `Bps::new(200)`)
    /// * `issue` - Issue date of the bond
    /// * `maturity` - Maturity date of the bond
    /// * `freq` - Payment frequency (e.g., `Tenor::quarterly()`)
    /// * `dc` - Day count convention (e.g., `DayCount::Act360`)
    /// * `discount_curve_id` - Discount curve identifier for pricing
    ///
    /// # Returns
    ///
    /// A `Bond` instance configured as a floating-rate note.
    ///
    /// # Panics
    ///
    /// Panics if bond construction fails (should not occur with valid inputs).
    ///
    /// # Example
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_core::dates::{Tenor, DayCount};
    /// use finstack_core::money::Money;
    /// use finstack_core::currency::Currency;
    /// use time::macros::date;
    ///
    /// // 3M SOFR + 200bps, quarterly payments
    /// let notional = Money::new(1_000_000.0, Currency::USD);
    /// let issue = date!(2025-01-15);
    /// let maturity = date!(2030-01-15);
    /// let frn = Bond::floating(
    ///     "FRN-001",
    ///     notional,
    ///     "USD-SOFR-3M",
    ///     finstack_core::types::Bps::new(200),
    ///     issue,
    ///     maturity,
    ///     Tenor::quarterly(),
    ///     DayCount::Act360,
    ///     "USD-OIS"
    /// );
    /// # let _ = frn;
    /// ```
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    #[allow(clippy::too_many_arguments)]
    pub fn floating(
        id: impl Into<InstrumentId>,
        notional: Money,
        index_id: impl Into<CurveId>,
        margin_bp: impl Into<Bps>,
        issue: Date,
        maturity: Date,
        freq: finstack_core::dates::Tenor,
        dc: DayCount,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_core::Result<Self> {
        let margin_bp = margin_bp.into();
        let bond = Self::builder()
            .id(id.into())
            .notional(notional)
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(CashflowSpec::floating_bps(
                index_id.into(),
                margin_bp,
                freq,
                dc,
            ))
            .discount_curve_id(discount_curve_id.into())
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .settlement_convention_opt(None)
            .build()?;

        // Validate all parameters before returning
        bond.validate()?;
        Ok(bond)
    }

    /// Create a zero-coupon bond.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the bond
    /// * `notional` - Face value of the bond (paid at maturity)
    /// * `issue` - Issue date of the bond
    /// * `maturity` - Maturity date of the bond
    /// * `discount_curve_id` - Discount curve identifier for pricing
    ///
    /// # Returns
    ///
    /// A zero-coupon `Bond` with no settlement convention. Use the builder
    /// directly or `::with_convention()` for market-specific settlement rules.
    ///
    /// # Example
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_core::money::Money;
    /// use finstack_core::currency::Currency;
    /// use time::macros::date;
    ///
    /// let zcb = Bond::zero_coupon(
    ///     "ZCB-001",
    ///     Money::new(1_000_000.0, Currency::USD),
    ///     date!(2025-01-15),
    ///     date!(2027-01-15),
    ///     "USD-OIS",
    /// ).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the builder fails validation (e.g., maturity <= issue).
    pub fn zero_coupon(
        id: impl Into<InstrumentId>,
        notional: Money,
        issue: Date,
        maturity: Date,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_core::Result<Self> {
        let bond = Self::builder()
            .id(id.into())
            .notional(notional)
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(CashflowSpec::fixed(
                0.0,
                finstack_core::dates::Tenor::annual(),
                DayCount::Act365F,
            )?)
            .discount_curve_id(discount_curve_id.into())
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .settlement_convention_opt(None)
            .build()?;

        bond.validate()?;
        Ok(bond)
    }

    /// Create a bond from a pre-built cashflow schedule.
    ///
    /// This extracts key bond parameters from the cashflow schedule and creates
    /// a bond that will use these custom cashflows for all calculations.
    /// Use this for complex structures like PIK bonds, fixed-to-floating, or
    /// custom amortization schedules.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the bond
    /// * `schedule` - Pre-built cashflow schedule
    /// * `discount_curve_id` - Discount curve identifier for pricing
    /// * `quoted_clean` - Optional quoted clean price as percentage of par
    ///
    /// # Returns
    ///
    /// A `Bond` instance configured to use the provided custom cashflow schedule.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Schedule has fewer than 2 dates
    /// - Bond construction fails
    ///
    /// # Example
    /// ```ignore
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
    /// use finstack_core::money::Money;
    /// use finstack_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
    /// use finstack_valuations::instruments::Bond;
    /// use rust_decimal_macros::dec;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_core::Result<()> {
    /// // Build a custom schedule (PIK, step-ups, amortization, etc.) with `CashFlowSchedule::builder()`.
    /// let issue = date!(2025-01-15);
    /// let maturity = date!(2027-01-15);
    /// let fixed_spec = FixedCouponSpec {
    ///     coupon_type: CouponType::Cash,
    ///     rate: dec!(0.06),
    ///     freq: Tenor::semi_annual(),
    ///     dc: DayCount::Act365F,
    ///     bdc: BusinessDayConvention::Following,
    ///     calendar_id: "weekends_only".to_string(),
    ///     stub: StubKind::None,
    ///     end_of_month: false,
    ///     payment_lag_days: 0,
    /// };
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
    ///     .fixed_cf(fixed_spec)
    ///     .build_with_curves(None)?;
    ///
    /// let bond = Bond::from_cashflows("PIK-001", schedule, "USD-HY", Some(95.0))?;
    /// # let _ = bond;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_cashflows(
        id: impl Into<InstrumentId>,
        schedule: CashFlowSchedule,
        discount_curve_id: impl Into<CurveId>,
        quoted_clean: Option<f64>,
    ) -> finstack_core::Result<Self> {
        // Extract parameters from the schedule
        let notional = schedule.notional.initial;

        // Find issue and maturity from the cashflow dates
        let dates = schedule.dates();
        if dates.len() < 2 {
            return Err(finstack_core::InputError::TooFewPoints.into());
        }
        let issue = dates[0];
        let maturity = dates
            .last()
            .copied()
            .ok_or(finstack_core::InputError::TooFewPoints)?;

        // Infer a representative coupon frequency from the schedule's coupon dates.
        //
        // This is used only for yield/duration conventions; the actual cashflows
        // always come from `custom_cashflows`.
        //
        // We compute the mode (most frequent) interval across all consecutive
        // coupon dates, ignoring potential stubs at the front or back. This is
        // more robust than using just the first interval, which may be a stub.
        use crate::cashflow::primitives::CFKind;
        use finstack_core::dates::Tenor;
        use std::collections::HashMap;

        let mut coupon_dates: Vec<Date> = schedule
            .flows
            .iter()
            .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub | CFKind::FloatReset))
            .map(|cf| cf.date)
            .collect();
        coupon_dates.sort();
        coupon_dates.dedup();

        let inferred_freq = if coupon_dates.len() < 2 {
            // Fallback to annual if we cannot infer a pattern
            Tenor::annual()
        } else {
            // Compute all interval lengths between consecutive coupon dates
            let mut interval_counts: HashMap<i64, usize> = HashMap::new();
            for window in coupon_dates.windows(2) {
                let d0 = window[0];
                let d1 = window[1];
                let days = (d1 - d0).whole_days().abs();
                // Bucket into standard frequency ranges for robust mode detection.
                // Ranges are widened to account for business-day convention shifts
                // (±3-5 days) that can occur with Modified Following, month-end
                // rolls, and holiday-heavy calendars.
                let bucket = match days {
                    355..=375 => 365, // Annual
                    175..=192 => 182, // Semi-annual
                    85..=98 => 91,    // Quarterly
                    55..=68 => 60,    // Bimonthly
                    25..=37 => 30,    // Monthly
                    5..=9 => 7,       // Weekly
                    _ => days,        // Non-standard interval
                };
                *interval_counts.entry(bucket).or_insert(0) += 1;
            }

            // Find the mode (most frequent interval)
            let (mode_days, _mode_count) = interval_counts
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(days, count)| (*days, *count))
                .unwrap_or((365, 1));

            // Map bucketed mode to standard Tenor
            match mode_days {
                365 => Tenor::annual(),
                182 => Tenor::semi_annual(),
                91 => Tenor::quarterly(),
                60 => Tenor::new(2, finstack_core::dates::TenorUnit::Months),
                30 => Tenor::monthly(),
                7 => Tenor::weekly(),
                _ => finstack_core::dates::Tenor::new(
                    mode_days as u32,
                    finstack_core::dates::TenorUnit::Days,
                ),
            }
        };

        // Use schedule day count and inferred frequency in the spec so that
        // YTM/YTW/duration/convexity use correct conventions for custom bonds.
        let cashflow_spec = CashflowSpec::fixed(0.0, inferred_freq, schedule.day_count)?;

        let pricing_overrides = if let Some(price) = quoted_clean {
            PricingOverrides::default().with_quoted_clean_price(price)
        } else {
            PricingOverrides::default()
        };

        Self::builder()
            .id(id.into())
            .notional(notional)
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(cashflow_spec)
            .discount_curve_id(discount_curve_id.into())
            .credit_curve_id_opt(None)
            .pricing_overrides(pricing_overrides)
            .custom_cashflows_opt(Some(schedule))
            .attributes(Attributes::new())
            .settlement_convention_opt(None)
            .build()
    }

    /// Set custom cashflows for this bond.
    ///
    /// When custom cashflows are set, they will be used instead of generating
    /// cashflows from the bond's coupon and amortization specifications.
    ///
    /// # Arguments
    ///
    /// * `schedule` - Custom cashflow schedule to use for pricing and metrics
    ///
    /// # Returns
    ///
    /// The bond instance with custom cashflows set.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_cashflows::builder::CashFlowSchedule;
    /// use finstack_core::money::Money;
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::Date;
    ///
    /// # let bond = Bond::example().unwrap();
    /// # let schedule = CashFlowSchedule::builder()
    /// #     .principal(Money::new(1_000_000.0, Currency::USD), Date::from_calendar_date(2024, time::Month::January, 1).unwrap(), Date::from_calendar_date(2034, time::Month::January, 1).unwrap())
    /// #     .build_with_curves(None).unwrap();
    /// let bond_with_custom = bond.with_cashflows(schedule);
    /// ```
    pub fn with_cashflows(mut self, schedule: CashFlowSchedule) -> Self {
        self.custom_cashflows = Some(schedule);
        self
    }

    /// Number of settlement days (e.g., 2 for T+2), or `None` if no convention is set.
    pub fn settlement_days(&self) -> Option<u32> {
        self.settlement_convention
            .as_ref()
            .map(|c| c.settlement_days)
    }

    /// Number of ex-coupon days before coupon date, or `None` if zero/unset.
    pub fn ex_coupon_days(&self) -> Option<u32> {
        self.settlement_convention
            .as_ref()
            .and_then(|c| (c.ex_coupon_days > 0).then_some(c.ex_coupon_days))
    }

    /// Calendar identifier for ex-coupon day counting, or `None` if unset.
    pub fn ex_coupon_calendar_id(&self) -> Option<&str> {
        self.settlement_convention
            .as_ref()
            .and_then(|c| c.ex_coupon_calendar_id.as_deref())
    }

    /// Build accrual configuration from bond's accrual method and ex-coupon convention.
    ///
    /// Creates the generic `AccrualConfig` needed by the cashflow accrual engine,
    /// incorporating both the accrual method and ex-coupon rules.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::bond::Bond;
    ///
    /// # let bond = Bond::example().unwrap();
    /// let accrual_config = bond.accrual_config();
    /// ```
    pub fn accrual_config(&self) -> crate::cashflow::accrual::AccrualConfig {
        crate::cashflow::accrual::AccrualConfig {
            method: self.accrual_method.clone(),
            ex_coupon: self
                .ex_coupon_days()
                .map(|d| crate::cashflow::accrual::ExCouponRule {
                    days_before_coupon: d,
                    calendar_id: self
                        .settlement_convention
                        .as_ref()
                        .and_then(|c| c.ex_coupon_calendar_id.clone()),
                }),
            include_pik: true,
            frequency: Some(self.cashflow_spec.frequency()),
        }
    }

    /// Build the bond's full internal cashflow schedule with kinds.
    ///
    /// This returns the complete `CashFlowSchedule` including all cashflow types
    /// (Fixed, Float, PIK, Amortization, Notional, etc.) and metadata in the
    /// builder's native convention (typically issuer view).
    ///
    /// For floating rate bonds, requires market curves to properly compute floating
    /// coupon amounts (forward rate + discount margin).
    ///
    /// Note: Amortization amounts are stored as POSITIVE values in the schedule.
    ///
    /// # Arguments
    ///
    /// * `curves` - Market context containing discount and forward curves
    ///
    /// # Returns
    ///
    /// The complete `CashFlowSchedule` with all cashflow types and metadata.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Market curves are missing (for floating rate bonds)
    /// - Cashflow schedule building fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_core::market_data::context::MarketContext;
    ///
    /// let bond = Bond::example().unwrap();
    /// let curves = MarketContext::new();
    /// let schedule = bond.full_cashflow_schedule(&curves)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub(crate) fn full_cashflow_schedule(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
    ) -> Result<CashFlowSchedule> {
        use crate::cashflow::builder::CashFlowSchedule;

        // If custom cashflows are set, return them directly
        if let Some(ref custom) = self.custom_cashflows {
            return Ok(custom.clone());
        }

        // Build the schedule using the cashflow builder and cashflow_spec
        let mut b = CashFlowSchedule::builder();
        let _ = b.principal(self.notional, self.issue_date, self.maturity);

        // Match on the cashflow spec variant
        match &self.cashflow_spec {
            CashflowSpec::Fixed(spec) => {
                let _ = b.fixed_cf(spec.clone());
            }
            CashflowSpec::Floating(spec) => {
                let _ = b.floating_cf(spec.clone());
            }
            CashflowSpec::StepUp(spec) => {
                let _ = b.step_up_cf(spec.clone());
            }
            CashflowSpec::Amortizing { base, schedule } => {
                let _ = b.amortization(schedule.clone());
                match &**base {
                    CashflowSpec::Fixed(spec) => {
                        let _ = b.fixed_cf(spec.clone());
                    }
                    CashflowSpec::Floating(spec) => {
                        let _ = b.floating_cf(spec.clone());
                    }
                    CashflowSpec::StepUp(spec) => {
                        let _ = b.step_up_cf(spec.clone());
                    }
                    CashflowSpec::Amortizing { .. } => {
                        return Err(finstack_core::InputError::Invalid.into());
                    }
                }
            }
        }

        // Build the schedule with market curves for floating rate computation
        b.build_with_curves(Some(curves))
    }

    /// Create an example floating-rate note (FRN) for testing and documentation.
    ///
    /// Returns a 5-year USD SOFR-linked FRN with:
    /// - $1M notional
    /// - SOFR + 150bps spread
    /// - Quarterly payments, Act/360
    /// - 0% index floor
    /// - T-2 reset lag
    pub fn example_floating() -> finstack_core::Result<Self> {
        use crate::cashflow::builder::specs::{CouponType, FloatingCouponSpec, FloatingRateSpec};
        use finstack_core::dates::{BusinessDayConvention, StubKind, Tenor};
        use rust_decimal::Decimal;

        let cashflow_spec = CashflowSpec::Floating(FloatingCouponSpec {
            rate_spec: FloatingRateSpec {
                index_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: Decimal::new(150, 0),
                gearing: Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: Some(Decimal::ZERO),
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                dc: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                payment_lag_days: 0,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            },
            coupon_type: CouponType::Cash,
            freq: Tenor::quarterly(),
            stub: StubKind::ShortFront,
        });

        let bond = Self::builder()
            .id(InstrumentId::new("FRN-USD-SOFR-5Y"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2029 - 01 - 15))
            .cashflow_spec(cashflow_spec)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()?;

        bond.validate()?;
        Ok(bond)
    }

    /// Create an example callable fixed-rate bond for testing and documentation.
    ///
    /// Returns a 10-year USD corporate bond with:
    /// - $1M notional, 5% semi-annual coupon, 30/360
    /// - Call schedule at years 3, 5, and 7 with declining premiums (103, 101, 100)
    pub fn example_callable() -> finstack_core::Result<Self> {
        let cashflow_spec = CashflowSpec::fixed(
            0.05,
            finstack_core::dates::Tenor::semi_annual(),
            DayCount::Thirty360,
        )?;

        let call_put = CallPutSchedule {
            calls: vec![
                CallPut {
                    start_date: date!(2027 - 01 - 15),
                    end_date: date!(2027 - 01 - 15),
                    price_pct_of_par: 103.0,
                    make_whole: None,
                },
                CallPut {
                    start_date: date!(2029 - 01 - 15),
                    end_date: date!(2029 - 01 - 15),
                    price_pct_of_par: 101.0,
                    make_whole: None,
                },
                CallPut {
                    start_date: date!(2031 - 01 - 15),
                    end_date: date!(2031 - 01 - 15),
                    price_pct_of_par: 100.0,
                    make_whole: None,
                },
            ],
            puts: vec![],
        };

        let bond = Self::builder()
            .id(InstrumentId::new("CALLABLE-USD-10Y"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2034 - 01 - 15))
            .cashflow_spec(cashflow_spec)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .call_put_opt(Some(call_put))
            .attributes(Attributes::new())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()?;

        bond.validate()?;
        Ok(bond)
    }

    /// Create an example amortizing fixed-rate bond for testing and documentation.
    ///
    /// Returns a 5-year USD amortizing bond with:
    /// - $1M notional, 4% semi-annual coupon, 30/360
    /// - Linear amortization to $200K final notional
    pub fn example_amortizing() -> finstack_core::Result<Self> {
        use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};
        use crate::cashflow::builder::AmortizationSpec;
        use finstack_core::dates::{BusinessDayConvention, StubKind, Tenor};
        use rust_decimal::Decimal;

        let base = CashflowSpec::Fixed(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::new(4, 2),
            freq: Tenor::semi_annual(),
            dc: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
        });

        let cashflow_spec = CashflowSpec::Amortizing {
            base: Box::new(base),
            schedule: AmortizationSpec::LinearTo {
                final_notional: Money::new(200_000.0, Currency::USD),
            },
        };

        let bond = Self::builder()
            .id(InstrumentId::new("AMORT-USD-5Y"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2029 - 01 - 15))
            .cashflow_spec(cashflow_spec)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()?;

        bond.validate()?;
        Ok(bond)
    }
}
