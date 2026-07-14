//! Fluent `CashFlowBuilder` methods for coupon, fee, and payment-split legs.
//!
//! These `impl CashFlowBuilder` blocks accumulate coupon, fee, and
//! payment-program state on the builder. The build orchestration that
//! consumes that state — validation, compilation, date collection, and
//! projection — lives in [`super::orchestrator`].

use finstack_quant_core::dates::Date;
use rust_decimal::Decimal;

use super::compiler::{
    CouponProgramPiece, CouponSpec, PaymentProgramPiece, ProgramWindow, WindowBound,
};
use super::orchestrator::CashFlowBuilder;
use super::specs::{
    CouponType, FeeSpec, FixedCouponSpec, FixedWindow, FloatingCouponSpec, ScheduleParams,
    StepUpCouponSpec,
};

impl CashFlowBuilder {
    fn record_error(&mut self, error: finstack_quant_core::Error) {
        if self.pending_error.is_none() {
            self.pending_error = Some(error);
        }
    }

    fn push_coupon_program(
        &mut self,
        window: ProgramWindow,
        schedule: ScheduleParams,
        coupon: CouponSpec,
        split: CouponType,
    ) -> &mut Self {
        self.coupon_program.push(CouponProgramPiece {
            window,
            schedule,
            coupon,
        });
        self.payment_program
            .push(PaymentProgramPiece { window, split });
        self
    }

    fn push_full_horizon_coupon(
        &mut self,
        schedule: ScheduleParams,
        coupon: CouponSpec,
        split: CouponType,
    ) -> &mut Self {
        self.push_coupon_program(ProgramWindow::full_horizon(), schedule, coupon, split)
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
}

impl CashFlowBuilder {
    /// Adds a fixed coupon over the half-open window `[start, end)`.
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn add_fixed_window(&mut self, start: Date, end: Date, spec: FixedCouponSpec) -> &mut Self {
        self.push_coupon_program(
            ProgramWindow::explicit(start, end),
            spec.schedule,
            CouponSpec::Fixed { rate: spec.rate },
            spec.coupon_type,
        )
    }

    /// Adds a floating coupon over the half-open window `[start, end)`.
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn add_floating_window(
        &mut self,
        start: Date,
        end: Date,
        spec: FloatingCouponSpec,
    ) -> &mut Self {
        self.push_coupon_program(
            ProgramWindow::explicit(start, end),
            spec.schedule,
            CouponSpec::Float {
                rate_spec: spec.rate_spec,
            },
            spec.coupon_type,
        )
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
    /// [`build_with_curves`](Self::build_with_curves).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    /// let maturity = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
    ///     .fixed_cf(FixedCouponSpec {
    ///         coupon_type: CouponType::Cash,
    ///         rate: dec!(0.05),
    ///         schedule: ScheduleParams::semiannual_30360(),
    ///     })
    ///     .build_with_curves(None)
    ///     .expect("fixed schedule builds");
    ///
    /// assert!(!schedule.flows.is_empty());
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn fixed_cf(&mut self, spec: FixedCouponSpec) -> &mut Self {
        self.push_full_horizon_coupon(
            spec.schedule.clone(),
            CouponSpec::Fixed { rate: spec.rate },
            spec.coupon_type,
        )
    }

    /// Adds a floating coupon specification.
    ///
    /// The coupon leg spans the full principal horizon set by
    /// [`principal`](Self::principal). Floating-rate projection is deferred
    /// until [`build_with_curves`](Self::build_with_curves), where the forward
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
    /// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
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
    ///     .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
    ///     .floating_cf(FloatingCouponSpec {
    ///         coupon_type: CouponType::Cash,
    ///         rate_spec: FloatingRateSpec {
    ///             index_id: CurveId::new("USD-SOFR-3M"),
    ///             spread_bp: dec!(200),
    ///             gearing: dec!(1),
    ///             gearing_includes_spread: true,
    ///             index_floor_bp: Some(dec!(0)),
    ///             all_in_floor_bp: None,
    ///             all_in_cap_bp: None,
    ///             index_cap_bp: None,
    ///             overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
    ///             reset_freq: Tenor::quarterly(),
    ///             index_tenor: None,
    ///             reset_lag_days: 2,
    ///             fixing_calendar_id: None,
    ///             overnight_compounding: None,
    ///             overnight_basis: None,
    ///             fallback: FloatingRateFallback::SpreadOnly,
    ///         },
    ///         schedule: ScheduleParams::quarterly_act360(),
    ///     });
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn floating_cf(&mut self, spec: FloatingCouponSpec) -> &mut Self {
        self.push_full_horizon_coupon(
            spec.schedule.clone(),
            CouponSpec::Float {
                rate_spec: spec.rate_spec,
            },
            spec.coupon_type,
        )
    }
}

impl CashFlowBuilder {
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
    /// are returned by [`build_with_curves`](Self::build_with_curves).
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
    ///     .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
    ///     .fee(FeeSpec::PeriodicBps {
    ///         base: FeeBase::Drawn,
    ///         bps: dec!(25),
    ///         freq: Tenor::quarterly(),
    ///         dc: DayCount::Act360,
    ///         bdc: BusinessDayConvention::ModifiedFollowing,
    ///         calendar_id: "weekends_only".to_string(),
    ///         stub: StubKind::None,
    ///         accrual_basis: Default::default(),
    ///     });
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn fee(&mut self, spec: FeeSpec) -> &mut Self {
        self.fees.push(spec);
        self
    }
}

impl CashFlowBuilder {
    /// Adds (or overrides) a payment split over a single date window.
    ///
    /// The lower-level primitive behind [`payment_split_program`](Self::payment_split_program).
    /// Pushes a single payment-program piece covering `[start, end)` and uses
    /// `split` as the coupon settlement type within that window. Subsequent
    /// calls add additional pieces; later windows take precedence on overlap
    /// during compilation.
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
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn add_payment_window(&mut self, start: Date, end: Date, split: CouponType) -> &mut Self {
        self.payment_program.push(PaymentProgramPiece {
            window: ProgramWindow::explicit(start, end),
            split,
        });
        self
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
    ///     (Date::from_calendar_date(2027, Month::January, 1)?, CouponType::PIK),
    ///     (Date::from_calendar_date(2029, Month::January, 1)?, CouponType::Split {
    ///         cash_pct: dec!(0.5),
    ///         pik_pct: dec!(0.5)
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
    ///     .principal(Money::new(25_000_000.0, Currency::USD), issue, maturity)
    ///     .fixed_cf(fixed_spec)
    ///     .payment_split_program(&payment_steps)
    ///     .build_with_curves(None)?;
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
    /// - Periods not covered by steps default to `Cash`
    /// - Steps must be ordered by end date
    /// - Works with both fixed and floating coupons
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn payment_split_program(&mut self, steps: &[(Date, CouponType)]) -> &mut Self {
        if self.pending_error.is_some() {
            return self;
        }
        if let Some(w) = steps.windows(2).find(|w| w[0].0 >= w[1].0) {
            self.record_error(finstack_quant_core::Error::Validation(format!(
                "payment_split_program steps must be strictly increasing by date; found {} \
                 followed by {}",
                w[0].0, w[1].0
            )));
            return self;
        }
        let mut prev = WindowBound::Issue;
        for &(end, split) in steps {
            let _ = self.push_payment_program(
                ProgramWindow {
                    start: prev,
                    end: WindowBound::Date(end),
                },
                split,
            );
            prev = WindowBound::Date(end);
        }
        let _ = self.push_payment_program(
            ProgramWindow {
                start: prev,
                end: WindowBound::Maturity,
            },
            CouponType::Cash,
        );
        self
    }
}

impl CashFlowBuilder {
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
    /// failures are returned by [`build_with_curves`](Self::build_with_curves).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, ScheduleParams, StepUpCouponSpec};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
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
    ///     .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
    ///     .step_up_cf(StepUpCouponSpec {
    ///         coupon_type: CouponType::Cash,
    ///         initial_rate: dec!(0.04),
    ///         step_schedule: vec![(step, dec!(0.05))],
    ///         schedule: ScheduleParams::semiannual_30360(),
    ///     });
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn step_up_cf(&mut self, spec: StepUpCouponSpec) -> &mut Self {
        self.push_full_horizon_coupon(
            spec.schedule.clone(),
            CouponSpec::StepUp {
                initial_rate: spec.initial_rate,
                step_schedule: spec.step_schedule,
            },
            spec.coupon_type,
        )
    }

    /// Convenience: floating margin step-up program with Decimal margins.
    ///
    /// Creates consecutive floating coupon windows whose margin over the
    /// floating index changes at the supplied boundary dates.
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn float_margin_stepup_decimal(
        &mut self,
        steps: &[(Date, Decimal)],
        base_spec: FloatingCouponSpec,
    ) -> &mut Self {
        let mut prev = WindowBound::Issue;
        for &(end, margin_decimal) in steps {
            let window_spec = Self::floating_spec_with_margin(&base_spec, margin_decimal);
            let _ = self.push_coupon_program(
                ProgramWindow {
                    start: prev,
                    end: WindowBound::Date(end),
                },
                window_spec.schedule.clone(),
                CouponSpec::Float {
                    rate_spec: window_spec.rate_spec,
                },
                window_spec.coupon_type,
            );
            prev = WindowBound::Date(end);
        }
        let margin = steps
            .last()
            .map(|(_, margin)| *margin)
            .unwrap_or(base_spec.rate_spec.spread_bp);
        let final_spec = Self::floating_spec_with_margin(&base_spec, margin);
        let _ = self.push_coupon_program(
            ProgramWindow {
                start: prev,
                end: WindowBound::Maturity,
            },
            final_spec.schedule.clone(),
            CouponSpec::Float {
                rate_spec: final_spec.rate_spec,
            },
            final_spec.coupon_type,
        );
        self
    }

    /// Convenience: fixed-to-float switch at `switch` date.
    ///
    /// Creates a hybrid instrument that pays fixed coupons until a switch date,
    /// then converts to floating coupons. Common for convertible/callable bonds
    /// and structured products with changing payment profiles.
    ///
    /// # Arguments
    ///
    /// * `switch` - Date when coupon switches from fixed to floating
    /// * `fixed_win` - Fixed rate and schedule for pre-switch period
    /// * `float_spec` - Canonical floating coupon spec for the post-switch period
    /// * `fixed_split` - Payment type for the fixed pre-switch period
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, Tenor, DayCount, BusinessDayConvention, StubKind};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::CurveId;
    /// use finstack_quant_cashflows::builder::{
    ///     CashFlowSchedule, CouponType, FixedWindow, FloatingCouponSpec, FloatingRateSpec,
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
    /// let fixed_win = FixedWindow {
    ///     rate: dec!(0.05),
    ///     schedule: ScheduleParams::semiannual_30360(),
    /// };
    ///
    /// let float_spec = FloatingCouponSpec {
    ///     coupon_type: CouponType::Cash,
    ///     rate_spec: FloatingRateSpec {
    ///         index_id: CurveId::new("USD-SOFR"),
    ///         spread_bp: dec!(250),
    ///         gearing: dec!(1),
    ///         gearing_includes_spread: true,
    ///         index_floor_bp: None,
    ///         all_in_cap_bp: None,
    ///         all_in_floor_bp: None,
    ///         index_cap_bp: None,
    ///         overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
    ///         reset_freq: Tenor::quarterly(),
    ///         index_tenor: None,
    ///         reset_lag_days: 2,
    ///         fixing_calendar_id: None,
    ///         overnight_compounding: None,
    ///         overnight_basis: None,
    ///         fallback: Default::default(),
    ///     },
    ///     schedule: ScheduleParams::quarterly_act360(),
    /// };
    ///
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(Money::new(10_000_000.0, Currency::USD), issue, maturity)
    ///     .fixed_to_float(switch, fixed_win, float_spec, CouponType::Cash)
    ///     .build_with_curves(None)?;
    ///
    /// assert!(schedule.flows.len() > 0);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "builder methods should be chained or terminated with .build_with_curves(...)"]
    pub fn fixed_to_float(
        &mut self,
        switch: Date,
        fixed_win: FixedWindow,
        float_spec: FloatingCouponSpec,
        fixed_split: CouponType,
    ) -> &mut Self {
        let _ = self.push_coupon_program(
            ProgramWindow {
                start: WindowBound::Issue,
                end: WindowBound::Date(switch),
            },
            fixed_win.schedule,
            CouponSpec::Fixed {
                rate: fixed_win.rate,
            },
            fixed_split,
        );
        let _ = self.push_coupon_program(
            ProgramWindow {
                start: WindowBound::Date(switch),
                end: WindowBound::Maturity,
            },
            float_spec.schedule.clone(),
            CouponSpec::Float {
                rate_spec: float_spec.rate_spec,
            },
            float_spec.coupon_type,
        );
        self
    }
}
