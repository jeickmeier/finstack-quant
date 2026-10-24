//! Fluent `CashFlowBuilder` methods: principal and amortization, principal
//! events, coupon, fee, and payment-split legs.
//!
//! This `impl CashFlowBuilder` block accumulates that state on the builder. The build orchestration that
//! consumes that state — validation, compilation, date collection, and
//! projection — lives in [`super::orchestrator`].

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

use super::compiler::{
    CouponProgramPiece, CouponSpec, PaymentProgramPiece, ProgramWindow, WindowBound,
};
use super::orchestrator::CashFlowBuilder;
use super::specs::{
    AmortizationSpec, CouponType, FeeSpec, FixedCouponSpec, FloatingCouponSpec, PrincipalEventSpec,
    PrincipalExchange, StepUpCouponSpec,
};

impl CashFlowBuilder {
    fn record_error(&mut self, error: finstack_quant_core::Error) {
        if self.pending_error.is_none() {
            self.pending_error = Some(error);
        }
    }

    fn push_coupon_program(&mut self, window: ProgramWindow, coupon: CouponSpec) -> &mut Self {
        self.coupon_program
            .push(CouponProgramPiece { window, coupon });
        self
    }

    /// Windows `[issue, d1), [d1, d2), …, [dn, maturity)` for dated steps.
    ///
    /// Returns `None` when an earlier call already failed, or records an error
    /// and returns `None` when `dates` are not strictly increasing.
    fn step_windows(&mut self, method: &str, dates: &[Date]) -> Option<Vec<ProgramWindow>> {
        if self.pending_error.is_some() {
            return None;
        }
        if let Some(w) = dates.windows(2).find(|w| w[0] >= w[1]) {
            self.record_error(finstack_quant_core::Error::Validation(format!(
                "{method} dates must be strictly increasing; found {} followed by {}",
                w[0], w[1]
            )));
            return None;
        }
        let bounds: Vec<WindowBound> = std::iter::once(WindowBound::Issue)
            .chain(dates.iter().map(|&date| WindowBound::Date(date)))
            .chain(std::iter::once(WindowBound::Maturity))
            .collect();
        Some(
            bounds
                .windows(2)
                .map(|w| ProgramWindow {
                    start: w[0],
                    end: w[1],
                })
                .collect(),
        )
    }

    fn push_payment_program(&mut self, window: ProgramWindow, split: CouponType) -> &mut Self {
        self.payment_program
            .push(PaymentProgramPiece { window, split });
        self
    }

    fn floating_spec_with_margin(
        spec: &FloatingCouponSpec,
        spread_bp: Decimal,
    ) -> FloatingCouponSpec {
        let mut next = spec.clone();
        next.rate_spec.spread_bp = spread_bp;
        next
    }

    /// Adds a fixed coupon over the half-open window `[start, end)`.
    ///
    /// `start` and `end` are horizon cut points that regenerate the coupon
    /// dates on each side; see [window boundaries](CashFlowBuilder#window-boundaries).
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn add_fixed_window(&mut self, start: Date, end: Date, spec: FixedCouponSpec) -> &mut Self {
        self.push_coupon_program(ProgramWindow::explicit(start, end), CouponSpec::Fixed(spec))
    }

    /// Adds a floating coupon over the half-open window `[start, end)`.
    ///
    /// `start` and `end` are horizon cut points that regenerate the coupon
    /// dates on each side; see [window boundaries](CashFlowBuilder#window-boundaries).
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn add_floating_window(
        &mut self,
        start: Date,
        end: Date,
        spec: FloatingCouponSpec,
    ) -> &mut Self {
        self.push_coupon_program(ProgramWindow::explicit(start, end), CouponSpec::Float(spec))
    }

    /// Adds a fixed coupon specification.
    ///
    /// The coupon leg spans the full principal horizon set by
    /// [`principal`](Self::principal). The builder emits fixed, stub, cash,
    /// split, or PIK coupon flows according to the supplied spec and the
    /// schedule conventions inside it.
    ///
    /// # Arguments
    ///
    /// * `spec` - Fixed-rate coupon quote, payment split, and schedule
    ///   conventions.
    ///
    /// # Returns
    ///
    /// Mutable builder reference for fluent chaining.
    ///
    /// # Errors
    ///
    /// This method may be called before principal dates are set. Schedule
    /// generation, day-count, calendar, and coupon-split errors are returned by
    /// [`build`](Self::build).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    /// let maturity = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
    ///     .fixed_cf(FixedCouponSpec {
    ///         coupon_type: CouponType::Cash,
    ///         rate: dec!(0.05),
    ///         schedule: ScheduleParams::semiannual_30360(),
    ///     })
    ///     .build(None)
    ///     .expect("fixed schedule builds");
    ///
    /// assert!(!schedule.get_flows().is_empty());
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn fixed_cf(&mut self, spec: FixedCouponSpec) -> &mut Self {
        self.push_coupon_program(ProgramWindow::full_horizon(), CouponSpec::Fixed(spec))
    }

    /// Adds a floating coupon specification.
    ///
    /// The coupon leg spans the full principal horizon set by
    /// [`principal`](Self::principal). Floating-rate projection is deferred
    /// until [`build`](Self::build), where the forward
    /// curve or fallback policy is applied.
    ///
    /// # Arguments
    ///
    /// * `spec` - Floating-rate index, spread, caps/floors, payment split, and
    ///   schedule conventions.
    ///
    /// # Returns
    ///
    /// Mutable builder reference for fluent chaining.
    ///
    /// # Errors
    ///
    /// This method may be called before principal dates are set. Floating spec
    /// validation, missing forward curves, calendar errors, and fallback-policy
    /// failures are returned by the terminal build or project step.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{
    ///     CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingRateFallback,
    ///     FloatingRateSpec, OvernightIndexConstraintApplication, ScheduleParams,
    /// };
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::CurveId;
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    /// let maturity = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
    /// let mut builder = CashFlowSchedule::builder();
    ///
    /// let _ = builder
    ///     .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
    ///     .floating_cf(FloatingCouponSpec {
    ///         coupon_type: CouponType::Cash,
    ///         rate_spec: FloatingRateSpec {
    ///             forward_curve_id: CurveId::new("USD-SOFR-3M"),
    ///             spread_bp: dec!(200),
    ///             gearing: dec!(1),
    ///             gearing_includes_spread: true,
    ///             index_floor_bp: Some(dec!(0)),
    ///             all_in_floor_bp: None,
    ///             all_in_cap_bp: None,
    ///             index_cap_bp: None,
    ///             overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
    ///             reset_frequency: Tenor::quarterly(),
    ///             index_tenor: None,
    ///             reset_lag_days: 2,
    ///             fixing_calendar_id: None,
    ///             compounding: None,
    ///             overnight_basis: None,
    ///             fallback: FloatingRateFallback::SpreadOnly,
    ///         },
    ///         schedule: ScheduleParams::quarterly_act360(),
    ///     });
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn floating_cf(&mut self, spec: FloatingCouponSpec) -> &mut Self {
        self.push_coupon_program(ProgramWindow::full_horizon(), CouponSpec::Float(spec))
    }

    /// Adds a fee specification.
    ///
    /// Fixed fees emit a one-time `Fee` cashflow on their configured date.
    /// Periodic basis-point fees generate a schedule over the principal horizon
    /// and accrue against the configured [`crate::builder::FeeBase`].
    ///
    /// # Arguments
    ///
    /// * `spec` - Fixed or periodic fee specification to add to the schedule.
    ///
    /// # Returns
    ///
    /// Mutable builder reference for fluent chaining.
    ///
    /// # Errors
    ///
    /// This method does not return errors directly. Missing principal dates,
    /// invalid fee schedules, calendar lookup failures, and currency mismatches
    /// are returned by [`build`](Self::build).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, FeeBase, FeeSpec};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    /// let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
    /// let mut builder = CashFlowSchedule::builder();
    ///
    /// let _ = builder
    ///     .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
    ///     .fee(FeeSpec::PeriodicBp {
    ///         base: FeeBase::Drawn,
    ///         bp: dec!(25),
    ///         frequency: Tenor::quarterly(),
    ///         day_count: DayCount::Act360,
    ///         business_day_convention: BusinessDayConvention::ModifiedFollowing,
    ///         calendar_id: "weekends_only".into(),
    ///         stub: StubKind::None,
    ///         accrual_basis: Default::default(),
    ///     });
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn fee(&mut self, spec: FeeSpec) -> &mut Self {
        self.fees.push(spec);
        self
    }

    /// Adds a payment split over a single date window.
    ///
    /// The lower-level primitive behind [`payment_split_program`](Self::payment_split_program).
    /// Pushes a single payment-program piece covering `[start, end)` and uses
    /// `split` as the coupon settlement type within that window. Subsequent
    /// calls add additional pieces. Windows may be disjoint or nested: where
    /// they nest, the innermost (narrowest) window's split applies whatever the
    /// call order, and for identical windows the last call wins. Partially
    /// overlapping windows are rejected by `build` with a validation error.
    ///
    /// `start` and `end` also cut the coupon horizon, so an off-cycle date
    /// moves the coupon dates around it; see
    /// [window boundaries](CashFlowBuilder#window-boundaries).
    ///
    /// Prefer [`payment_split_program`](Self::payment_split_program) for
    /// PIK-toggle scheduling, which sequences windows from a single
    /// boundary-step list. Use this method only when you need to wire up
    /// non-contiguous or hand-crafted payment windows.
    ///
    /// # Arguments
    ///
    /// * `start` - Inclusive start of the payment window.
    /// * `end` - Exclusive end of the payment window.
    /// * `split` - Coupon settlement type (Cash / PIK / Split) for the window.
    ///
    /// # Returns
    ///
    /// Mutable builder reference for fluent chaining.
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn add_payment_window(&mut self, start: Date, end: Date, split: CouponType) -> &mut Self {
        self.push_payment_program(ProgramWindow::explicit(start, end), split)
    }

    /// Convenience: payment split program with boundary dates (PIK toggle windows).
    ///
    /// Creates a payment profile where the coupon payment type (Cash, PIK, or Split)
    /// changes over time. Common for PIK toggle bonds and mezzanine loans where
    /// the borrower can elect to capitalize interest during specific periods.
    ///
    /// # Arguments
    ///
    /// * `steps` - Boundary dates and payment splits: `&[(end_date, split)]`
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, Tenor, DayCount, BusinessDayConvention, StubKind};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_cashflows::builder::{
    ///     CashFlowSchedule, FixedCouponSpec, CouponType, ScheduleParams
    /// };
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let issue = Date::from_calendar_date(2025, Month::January, 1)?;
    /// let maturity = Date::from_calendar_date(2030, Month::January, 1)?;
    ///
    /// // PIK toggle: 100% PIK for first 2 years, 50/50 split for next 2 years, then all cash
    /// let payment_steps = [
    ///     (Date::from_calendar_date(2027, Month::January, 1)?, CouponType::Pik),
    ///     (Date::from_calendar_date(2029, Month::January, 1)?, CouponType::Split {
    ///         cash_fraction: dec!(0.5),
    ///         pik_fraction: dec!(0.5)
    ///     }),
    ///     (maturity, CouponType::Cash),
    /// ];
    ///
    /// let fixed_spec = FixedCouponSpec {
    ///     coupon_type: CouponType::Cash,  // Will be overridden by payment program
    ///     rate: dec!(0.10),  // 10% PIK toggle
    ///     schedule: ScheduleParams::semiannual_30360(),
    /// };
    ///
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(Money::from((25_000_000_i64, Currency::USD)), issue, maturity)
    ///     .fixed_cf(fixed_spec)
    ///     .payment_split_program(&payment_steps)
    ///     .build(None)?;
    ///
    /// // Check that PIK flows increase outstanding balance
    /// let outstanding_path = schedule.outstanding_by_date()?;
    /// assert!(outstanding_path.len() > 0);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notes
    ///
    /// - Each split applies up to (not including) its date; periods after the
    ///   last step default to `Cash`
    /// - Steps must be ordered by end date
    /// - Works with both fixed and floating coupons
    /// - Step dates cut the coupon horizon, so an off-cycle date moves the
    ///   coupon dates around it; see
    ///   [window boundaries](CashFlowBuilder#window-boundaries)
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn payment_split_program(&mut self, steps: &[(Date, CouponType)]) -> &mut Self {
        let dates: Vec<Date> = steps.iter().map(|&(date, _)| date).collect();
        let Some(windows) = self.step_windows("payment_split_program", &dates) else {
            return self;
        };
        // Each step's split runs up to its date; Cash covers the rest.
        let splits = steps
            .iter()
            .map(|&(_, split)| split)
            .chain(std::iter::once(CouponType::Cash));
        for (window, split) in windows.into_iter().zip(splits) {
            let _ = self.push_payment_program(window, split);
        }
        self
    }

    /// Adds a step-up coupon specification.
    ///
    /// A step-up coupon starts at an initial rate and steps to different rates
    /// on specified dates. The compiler translates this into per-period fixed
    /// coupon schedules with the appropriate rate for each period.
    ///
    /// # Arguments
    ///
    /// * `spec` - Step-up coupon definition containing the initial rate, step
    ///   schedule, payment split, and schedule conventions.
    ///
    /// # Returns
    ///
    /// Mutable builder reference for fluent chaining.
    ///
    /// # Errors
    ///
    /// This method may be called before principal dates are set. Date
    /// generation, calendar lookup, coupon split validation, and day-count
    /// failures are returned by [`build`](Self::build).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, ScheduleParams, StepUpCouponSpec};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    /// let step = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
    /// let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");
    /// let mut builder = CashFlowSchedule::builder();
    ///
    /// let _ = builder
    ///     .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
    ///     .step_up_cf(StepUpCouponSpec {
    ///         coupon_type: CouponType::Cash,
    ///         initial_rate: dec!(0.04),
    ///         step_schedule: vec![(step, dec!(0.05))],
    ///         schedule: ScheduleParams::semiannual_30360(),
    ///     });
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn step_up_cf(&mut self, spec: StepUpCouponSpec) -> &mut Self {
        self.push_coupon_program(ProgramWindow::full_horizon(), CouponSpec::StepUp(spec))
    }

    /// Floating coupons whose margin changes on dated, effective-from steps.
    ///
    /// `base_spec.rate_spec.spread_bp` applies from issue until the first
    /// step; each step's spread applies from its date until the next step, and
    /// the last one runs to maturity. Each date starts a coupon window, so an
    /// off-cycle step date regenerates the coupon dates on both sides of it
    /// rather than splitting only the enclosing accrual period; see
    /// [window boundaries](CashFlowBuilder#window-boundaries). A step dated
    /// on the issue date replaces the base spread from issue; one dated on
    /// maturity never applies.
    ///
    /// # Arguments
    ///
    /// * `steps` - `(effective-from date, spread_bp)` pairs in strictly
    ///   increasing date order. `spread_bp` is the margin over the index in
    ///   basis points (`250` = 2.50%) and replaces the base spread; it is not
    ///   an increment.
    /// * `base_spec` - Floating coupon specification (index, gearing,
    ///   floors/caps, schedule and settlement split) shared by every window;
    ///   its `spread_bp` is the margin before the first step.
    ///
    /// # Errors
    ///
    /// Records a validation error, returned by `build`, when step dates are
    /// not strictly increasing. Dates outside `[issue, maturity]` fail at
    /// `build` as out-of-horizon coupon windows.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_cashflows::builder::{
    ///     CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingRateSpec, ScheduleParams,
    /// };
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::CurveId;
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    /// let step = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
    /// let maturity = Date::from_calendar_date(2027, Month::January, 15).expect("valid date");
    /// let base = FloatingCouponSpec {
    ///     coupon_type: CouponType::Cash,
    ///     rate_spec: FloatingRateSpec {
    ///         forward_curve_id: CurveId::new("USD-SOFR"),
    ///         spread_bp: dec!(100),
    ///         gearing: dec!(1),
    ///         gearing_includes_spread: true,
    ///         index_floor_bp: None,
    ///         all_in_cap_bp: None,
    ///         all_in_floor_bp: None,
    ///         index_cap_bp: None,
    ///         overnight_index_constraints: Default::default(),
    ///         reset_frequency: Tenor::quarterly(),
    ///         index_tenor: None,
    ///         reset_lag_days: 2,
    ///         fixing_calendar_id: None,
    ///         compounding: None,
    ///         overnight_basis: None,
    ///         fallback: Default::default(),
    ///     },
    ///     schedule: ScheduleParams::quarterly_act360(),
    /// };
    /// let mut builder = CashFlowSchedule::builder();
    /// // SOFR + 100 bp in year one, SOFR + 250 bp from 2026-01-15.
    /// let _ = builder
    ///     .principal(Money::new(1_000_000.0, Currency::USD).expect("valid money"), issue, maturity)
    ///     .float_margin_steps(&[(step, dec!(250))], base);
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn float_margin_steps(
        &mut self,
        steps: &[(Date, Decimal)],
        base_spec: FloatingCouponSpec,
    ) -> &mut Self {
        let dates: Vec<Date> = steps.iter().map(|&(date, _)| date).collect();
        let Some(windows) = self.step_windows("float_margin_steps", &dates) else {
            return self;
        };
        // The base spread runs until the first step; each step's spread runs from its date.
        let spreads = std::iter::once(base_spec.rate_spec.spread_bp)
            .chain(steps.iter().map(|&(_, spread_bp)| spread_bp));
        for (window, spread_bp) in windows.into_iter().zip(spreads) {
            let _ = self.push_coupon_program(
                window,
                CouponSpec::Float(Self::floating_spec_with_margin(&base_spec, spread_bp)),
            );
        }
        self
    }

    /// Convenience: fixed-to-float switch at `switch` date.
    ///
    /// Creates a hybrid instrument that pays fixed coupons until a switch date,
    /// then converts to floating coupons. Common for convertible/callable bonds
    /// and structured products with changing payment profiles. `switch` is a
    /// horizon cut point: each leg gets its own schedule, so the fixed leg does
    /// not continue into the floating roll; see
    /// [window boundaries](CashFlowBuilder#window-boundaries).
    ///
    /// # Arguments
    ///
    /// * `switch` - Date when coupon switches from fixed to floating
    /// * `fixed` - Annual decimal rate, settlement type, and schedule for the pre-switch period
    /// * `floating` - Canonical floating coupon spec for the post-switch period
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, Tenor};
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::ForwardCurve;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::CurveId;
    /// use finstack_quant_cashflows::builder::{
    ///     CashFlowSchedule, CouponType, FixedCouponSpec, FloatingCouponSpec, FloatingRateSpec,
    ///     OvernightIndexConstraintApplication, ScheduleParams,
    /// };
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let issue = Date::from_calendar_date(2025, Month::January, 1)?;
    /// let switch = Date::from_calendar_date(2027, Month::January, 1)?;
    /// let maturity = Date::from_calendar_date(2030, Month::January, 1)?;
    ///
    /// // Pay 5% fixed for 2 years, then SOFR + 250bps floating
    /// let fixed = FixedCouponSpec {
    ///     coupon_type: CouponType::Cash,
    ///     rate: dec!(0.05),
    ///     schedule: ScheduleParams::semiannual_30360(),
    /// };
    ///
    /// let floating = FloatingCouponSpec {
    ///     coupon_type: CouponType::Cash,
    ///     rate_spec: FloatingRateSpec {
    ///         forward_curve_id: CurveId::new("USD-SOFR"),
    ///         spread_bp: dec!(250),
    ///         gearing: dec!(1),
    ///         gearing_includes_spread: true,
    ///         index_floor_bp: None,
    ///         all_in_cap_bp: None,
    ///         all_in_floor_bp: None,
    ///         index_cap_bp: None,
    ///         overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
    ///         reset_frequency: Tenor::quarterly(),
    ///         index_tenor: None,
    ///         reset_lag_days: 2,
    ///         fixing_calendar_id: None,
    ///         compounding: None,
    ///         overnight_basis: None,
    ///         fallback: Default::default(),
    ///     },
    ///     schedule: ScheduleParams::quarterly_act360(),
    /// };
    ///
    /// // Projecting the floating leg needs a forward curve for the index.
    /// let market = MarketContext::new().insert(
    ///     ForwardCurve::builder("USD-SOFR", 0.25)
    ///         .base_date(issue)
    ///         .knots([(0.0, 0.04), (10.0, 0.04)])
    ///         .build()?,
    /// );
    ///
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(Money::from((10_000_000_i64, Currency::USD)), issue, maturity)
    ///     .fixed_to_float(switch, fixed, floating)
    ///     .build(Some(&market))?;
    ///
    /// assert!(!schedule.get_flows().is_empty());
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn fixed_to_float(
        &mut self,
        switch: Date,
        fixed: FixedCouponSpec,
        floating: FloatingCouponSpec,
    ) -> &mut Self {
        let _ = self.push_coupon_program(
            ProgramWindow {
                start: WindowBound::Issue,
                end: WindowBound::Date(switch),
            },
            CouponSpec::Fixed(fixed),
        );
        let _ = self.push_coupon_program(
            ProgramWindow {
                start: WindowBound::Date(switch),
                end: WindowBound::Maturity,
            },
            CouponSpec::Float(floating),
        );
        self
    }

    /// Sets principal details and instrument horizon.
    ///
    /// Full-horizon coupons and amortization may be configured before or after
    /// this method. Their horizon is resolved when the schedule is built.
    /// Calling this method does not clear a previously recorded builder error.
    ///
    /// # Arguments
    ///
    /// * `initial` - Initial outstanding principal and currency.
    /// * `issue_date` - Start date for the instrument.
    /// * `maturity` - Final maturity date.
    ///
    /// # Returns
    ///
    /// Mutable builder reference for fluent chaining.
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn principal(&mut self, initial: Money, issue_date: Date, maturity: Date) -> &mut Self {
        self.initial_principal = Some(initial);
        self.issue = Some(issue_date);
        self.maturity = Some(maturity);
        self
    }

    /// Selects whether issue funding and maturity redemption notionals are emitted.
    ///
    /// Outstanding still starts at the `principal` initial amount for coupon
    /// math. Scheduled amortization and explicit principal events still emit.
    /// The default is [`PrincipalExchange::InitialAndFinal`].
    ///
    /// # Arguments
    ///
    /// * `exchange` - `InitialAndFinal` emits the issue draw and the
    ///   redemption balloon on the lagged payment date. `None` tracks
    ///   outstanding only (vanilla IRS / basis-swap convention).
    ///
    /// # Returns
    ///
    /// Mutable builder reference for fluent chaining.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{
    ///     CashFlowSchedule, CouponType, FixedCouponSpec, PrincipalExchange, ScheduleParams,
    /// };
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let issue = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let maturity = Date::from_calendar_date(2026, Month::January, 15)?;
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
    ///     .principal_exchange(PrincipalExchange::None)
    ///     .fixed_cf(FixedCouponSpec {
    ///         coupon_type: CouponType::Cash,
    ///         rate: dec!(0.05),
    ///         schedule: ScheduleParams::semiannual_30360(),
    ///     })
    ///     .build(None)?;
    /// assert!(schedule.get_flows().iter().all(|cf| {
    ///     cf.kind != finstack_quant_core::cashflow::CFKind::Notional
    /// }));
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn principal_exchange(&mut self, exchange: PrincipalExchange) -> &mut Self {
        self.principal_exchange = exchange;
        self
    }

    /// Configures amortization for the instrument notional.
    ///
    /// This may be called before or after [`principal`](Self::principal).
    ///
    /// # Arguments
    ///
    /// * `spec` - Contractual repayment rule, with monetary targets in the principal currency.
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn amortization(&mut self, spec: AmortizationSpec) -> &mut Self {
        self.amortization = spec;
        self
    }

    /// Adds a single principal event.
    ///
    /// `delta` controls the outstanding balance change. The emitted cashflow
    /// sign is derived from `kind`: `CFKind::Amortization` emits `cash` as a
    /// positive repayment, while all other kinds emit `-cash` as a borrower
    /// draw/notional cashflow.
    ///
    /// # Arguments
    ///
    /// * `date` - Economic date on which the principal delta changes interest accrual.
    /// * `payment_date` - Cash settlement date; scheduled repayments may pay before
    ///   or after their contractual balance-effective date.
    /// * `delta` - Signed principal change in currency units; positive funds a draw.
    /// * `cash` - Optional settlement amount in the same currency. When absent,
    ///   repayments use `-delta` and draws use `delta` before cashflow sign handling.
    /// * `kind` - Principal classification controlling settlement sign and validation.
    ///
    /// # Sign conventions
    ///
    /// * `CFKind::Amortization` (repayment): `delta` must be `<= 0` (the
    ///   outstanding balance decreases). When `cash` is `None`, it defaults to
    ///   `-delta` so the emitted flow is a positive cash repayment.
    /// * `CFKind::Notional` (draw): `delta` must be `>= 0` (the outstanding
    ///   balance increases). When `cash` is `None`, it defaults to `delta`,
    ///   emitting a negative funding outflow.
    /// * Other kinds: no sign requirement; `cash` defaults to `delta`.
    ///
    /// # Errors
    ///
    /// Records a pending error — returned by `build(...)` — when:
    /// * `cash` is provided with a different currency than `delta`, or
    /// * `delta` violates the sign convention for `kind` (including NaN
    ///   amounts, which never satisfy either sign requirement).
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn add_principal_event(
        &mut self,
        date: Date,
        payment_date: Date,
        delta: Money,
        cash: Option<Money>,
        kind: CFKind,
    ) -> &mut Self {
        if self.pending_error.is_some() {
            return self;
        }
        match kind {
            CFKind::Amortization if delta.amount() > 0.0 || delta.amount().is_nan() => {
                self.record_error(finstack_quant_core::Error::Validation(format!(
                    "add_principal_event: CFKind::Amortization requires delta <= 0 \
                     (repayments reduce outstanding), got {} on {date}",
                    delta.amount()
                )));
                return self;
            }
            CFKind::Notional if delta.amount() < 0.0 || delta.amount().is_nan() => {
                self.record_error(finstack_quant_core::Error::Validation(format!(
                    "add_principal_event: CFKind::Notional requires delta >= 0 \
                     (draws increase outstanding), got {} on {date}",
                    delta.amount()
                )));
                return self;
            }
            _ => {}
        }
        let spec = PrincipalEventSpec {
            date,
            payment_date,
            delta,
            cash,
            kind,
        };
        let cash_leg = spec.resolved_cash();
        if cash_leg.currency() != delta.currency() {
            self.record_error(finstack_quant_core::Error::CurrencyMismatch {
                expected: delta.currency(),
                actual: cash_leg.currency(),
            });
            return self;
        }
        self.principal_events.push(spec);
        self
    }
}
