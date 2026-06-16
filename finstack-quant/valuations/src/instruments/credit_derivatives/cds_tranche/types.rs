//! CDS Tranche types, builder entrypoint, and pricing impl.

use crate::cashflow::builder::ScheduleParams;
use crate::cashflow::traits::CashflowProvider;
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use finstack_quant_core::dates::{
    is_cds_date, BusinessDayConvention, Date, DayCount, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

use super::parameters::CDSTrancheParams;
use super::pricer;
use crate::impl_instrument_base;

/// Buyer/seller perspective for CDS tranche premium/protection
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TrancheSide {
    /// Buy protection on the tranche (pay running, receive protection)
    BuyProtection,
    /// Sell protection on the tranche (receive running, pay protection)
    SellProtection,
}

impl std::fmt::Display for TrancheSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrancheSide::BuyProtection => write!(f, "buy_protection"),
            TrancheSide::SellProtection => write!(f, "sell_protection"),
        }
    }
}

impl std::str::FromStr for TrancheSide {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "buy_protection" | "buy" => Ok(TrancheSide::BuyProtection),
            "sell_protection" | "sell" => Ok(TrancheSide::SellProtection),
            other => Err(format!("Unknown tranche side: {}", other)),
        }
    }
}

/// CDS Tranche instrument definition (boilerplate)
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CDSTranche {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Index name (e.g., "CDX.NA.IG", "CDX.NA.HY", "iTraxx EUR")
    pub index_name: String,
    /// Series number (e.g., 37)
    pub series: u16,
    /// Attachment point in percent (e.g., 0.0 for equity)
    pub attach_pct: f64,
    /// Detachment point in percent (e.g., 3.0 for 0-3% tranche)
    pub detach_pct: f64,
    /// Notional amount of the tranche
    pub notional: Money,
    /// Maturity date of the tranche
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Running coupon in basis points (e.g., 100 = 1.00%)
    pub running_coupon_bp: f64,
    /// Payment frequency (typically quarterly)
    pub frequency: Tenor,
    /// Day count (typically Act/360)
    pub day_count: DayCount,
    /// Business day convention
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Optional holiday calendar id
    pub calendar_id: Option<String>,
    /// Discount curve identifier (by quote currency)
    pub discount_curve_id: CurveId,
    /// Credit index identifier for survival/loss modeling (placeholder)
    pub credit_index_id: CurveId,
    /// Tranche side (buy/sell protection)
    pub side: TrancheSide,
    /// Optional effective date for schedule anchoring (if None, uses as_of date)
    #[schemars(with = "Option<String>")]
    pub effective_date: Option<Date>,
    /// Accumulated realized loss as fraction of original portfolio notional
    pub accumulated_loss: f64,
    /// Whether to enforce standard IMM dates (20th of Mar, Jun, Sep, Dec)
    pub standard_imm_dates: bool,
    /// Optional upfront payment (date, amount). Positive means paid by protection buyer.
    #[serde(default)]
    #[schemars(with = "Option<(String, Money)>")]
    pub upfront: Option<(Date, Money)>,
    /// Attributes for tagging and selection
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl CDSTranche {
    pub(crate) fn contractual_effective_date(&self, as_of: Date) -> Option<Date> {
        if let Some(effective_date) = self.effective_date {
            return Some(effective_date);
        }

        if !self.standard_imm_dates {
            return None;
        }

        let mut current = as_of.min(self.maturity);
        while !is_cds_date(current) {
            current -= time::Duration::days(1);
        }
        Some(current)
    }

    /// Validate structural invariants of the tranche.
    ///
    /// Checks that attachment/detachment points are ordered and in range,
    /// and that notional is positive. Can be called after builder construction
    /// to catch configuration errors early.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.attach_pct >= self.detach_pct {
            return Err(finstack_quant_core::Error::Validation(format!(
                "attach_pct ({}) must be less than detach_pct ({})",
                self.attach_pct, self.detach_pct
            )));
        }
        if self.attach_pct < 0.0 || self.detach_pct > 100.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "attach_pct ({}) and detach_pct ({}) must be in [0, 100]",
                self.attach_pct, self.detach_pct
            )));
        }
        if self.detach_pct <= 1.0 && self.detach_pct > 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "detach_pct={} looks like a fraction; attach/detach are quoted in percent \
                 (e.g., 3.0 for 3%). Multiply fractional inputs by 100.",
                self.detach_pct
            )));
        }
        if self.notional.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Tranche notional must be positive, got {}",
                self.notional.amount()
            )));
        }
        Ok(())
    }

    /// Create a canonical example CDS tranche (CDX.NA.IG 0-3% equity tranche).
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use crate::cashflow::builder::ScheduleParams;
        use finstack_quant_core::currency::Currency;
        use time::macros::date;
        let params = super::parameters::CDSTrancheParams::equity_tranche(
            "CDX.NA.IG",
            42,
            Money::new(10_000_000.0, Currency::USD),
            date!(2029 - 12 - 20),
            100.0,
        );
        let sched = ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        };
        CDSTranche::new(
            InstrumentId::new("CDXIG-42-0X3"),
            &params,
            &sched,
            CurveId::new("USD-OIS"),
            CurveId::new("CDX.NA.IG.HAZARD"),
            TrancheSide::BuyProtection,
        )
        .expect("Valid tranche parameters")
    }
    /// Create a new CDS tranche using parameter structs.
    ///
    /// # Panics
    ///
    /// Returns an error if tranche parameters are invalid:
    /// - `attach_pct` must be less than `detach_pct`
    /// - Both must be in [0, 100] (percent, not fraction)
    /// - Notional must be positive
    pub fn new(
        id: impl Into<InstrumentId>,
        tranche_params: &CDSTrancheParams,
        schedule_params: &ScheduleParams,
        discount_curve_id: impl Into<CurveId>,
        credit_index_id: impl Into<CurveId>,
        side: TrancheSide,
    ) -> finstack_quant_core::Result<Self> {
        // Validate tranche parameters
        if tranche_params.attach_pct >= tranche_params.detach_pct {
            return Err(finstack_quant_core::Error::Validation(format!(
                "attach_pct ({}) must be less than detach_pct ({})",
                tranche_params.attach_pct, tranche_params.detach_pct
            )));
        }
        if tranche_params.attach_pct < 0.0 || tranche_params.detach_pct > 100.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "attach_pct ({}) and detach_pct ({}) must be in [0, 100] (percent, not fraction)",
                tranche_params.attach_pct, tranche_params.detach_pct
            )));
        }
        if tranche_params.notional.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Tranche notional must be positive, got {}",
                tranche_params.notional.amount()
            )));
        }

        // Reject values that look like fractions (very small non-zero
        // detachment). Tranche points are quoted in percent; a 0.03/0.07
        // "3-7%" tranche passed as fractions would silently price a 100×
        // thinner tranche.
        if tranche_params.detach_pct <= 1.0 && tranche_params.detach_pct > 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "detach_pct={} looks like a fraction; attach/detach are quoted in percent \
                 (e.g., 3.0 for 3%). Multiply fractional inputs by 100.",
                tranche_params.detach_pct
            )));
        }

        Ok(Self {
            id: id.into(),
            index_name: tranche_params.index_name.to_owned(),
            series: tranche_params.series,
            attach_pct: tranche_params.attach_pct,
            detach_pct: tranche_params.detach_pct,
            notional: tranche_params.notional,
            maturity: tranche_params.maturity,
            running_coupon_bp: tranche_params.running_coupon_bp,
            frequency: schedule_params.freq,
            day_count: schedule_params.dc,
            bdc: schedule_params.bdc,
            calendar_id: Some(schedule_params.calendar_id.clone()),
            discount_curve_id: discount_curve_id.into(),
            credit_index_id: credit_index_id.into(),
            side,
            effective_date: None,
            accumulated_loss: tranche_params.accumulated_loss,
            standard_imm_dates: false,
            upfront: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        })
    }

    /// Create a standard CDS tranche with IMM dates and market conventions
    #[allow(clippy::too_many_arguments)]
    pub fn standard(
        id: impl Into<InstrumentId>,
        tranche_params: &CDSTrancheParams,
        discount_curve_id: impl Into<CurveId>,
        credit_index_id: impl Into<CurveId>,
        side: TrancheSide,
    ) -> finstack_quant_core::Result<Self> {
        use crate::cashflow::builder::ScheduleParams;
        let sched = ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        };

        let mut tranche = Self::new(
            id,
            tranche_params,
            &sched,
            discount_curve_id,
            credit_index_id,
            side,
        )?;
        tranche.standard_imm_dates = true;
        tranche.calendar_id = None;
        Ok(tranche)
    }

    /// Calculate upfront amount for the tranche
    pub fn upfront(&self, curves: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.calculate_upfront(self, curves, as_of)
    }

    /// Calculate spread DV01 (sensitivity to 1bp change in running coupon)
    pub fn spread_dv01(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.calculate_spread_dv01(self, curves, as_of)
    }

    /// Calculate the par spread (running coupon in basis points).
    pub fn par_spread(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.calculate_par_spread(self, curves, as_of)
    }

    /// Calculate expected loss metric
    pub fn expected_loss(&self, curves: &MarketContext) -> finstack_quant_core::Result<f64> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.calculate_expected_loss(self, curves)
    }

    /// Calculate jump-to-default metric
    pub fn jump_to_default(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.calculate_jump_to_default(self, curves, as_of)
    }

    /// Calculate correlation delta (Correlation01): PV change per **1%
    /// (0.01 absolute)** base-correlation shift, matching `Recovery01` units.
    pub fn correlation_delta(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.calculate_correlation_delta(self, curves, as_of)
    }

    /// Calculate accrued premium on the tranche.
    ///
    /// Returns the premium accrued since the last payment date, calculated on
    /// the outstanding notional (after any realized losses).
    ///
    /// # Returns
    ///
    /// The accrued premium amount in the tranche currency. This represents:
    /// - For protection buyer: amount owed to seller
    /// - For protection seller: amount receivable from buyer
    ///
    /// # Use Cases
    ///
    /// - Dirty vs clean price calculation: `dirty_price = clean_price + accrued`
    /// - Settlement amount calculation
    /// - Mark-to-market accounting
    pub fn accrued_premium(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.calculate_accrued_premium(self, curves, as_of)
    }

    /// Calculate detailed jump-to-default metrics including min, max, and average.
    ///
    /// For heterogeneous portfolios, provides the full distribution of JTD impacts.
    pub fn jump_to_default_detail(
        &self,
        curves: &MarketContext,
    ) -> finstack_quant_core::Result<pricer::JumpToDefaultResult> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.calculate_jump_to_default_detail(self, curves)
    }

    /// Get the expected loss curve for diagnostic purposes.
    ///
    /// Returns (Date, EL_fraction) pairs showing cumulative expected loss over time.
    pub fn expected_loss_curve(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<(Date, f64)>> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.get_expected_loss_curve(self, curves, as_of)
    }

    // Builder now provided by derive
}

// Attributable is provided via blanket impl for all Instrument types

impl Instrument for CDSTranche {
    impl_instrument_base!(crate::pricer::InstrumentType::CDSTranche);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::HazardRate
    }

    // === Pricing Methods ===

    fn base_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let pricer = pricer::CDSTranchePricer::new();
        pricer.price_tranche(self, curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        self.effective_date
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl CashflowProvider for CDSTranche {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let pricer = pricer::CDSTranchePricer::new();
        let mut schedule = pricer.build_projected_schedule(self, curves, as_of)?;
        schedule.meta.representation = crate::cashflow::builder::CashflowRepresentation::Projected;
        Ok(schedule)
    }
}

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for CDSTranche {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .credit(self.credit_index_id.clone())
            .build()
    }
}
