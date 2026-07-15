//! Core types for Repurchase Agreement (Repo) instruments.

use crate::cashflow::traits::CashflowProvider;
use crate::contract_specs::embedded_registry;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use finstack_quant_core::dates::{adjust, BusinessDayConvention, Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Bps, CalendarId, CurveId, InstrumentId};
use finstack_quant_core::{Error, Result};
use finstack_quant_margin::RepoMarginSpec;
use rust_decimal::Decimal;

use crate::impl_instrument_base;

const DEFAULT_REPO_SPECS_ID: &str = "repo.usd_general_collateral";

/// Type of repurchase agreement.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum RepoType {
    /// Term repo with fixed maturity date
    #[default]
    Term,
    /// Open repo that can be terminated with notice
    Open,
    /// Overnight repo maturing next business day
    Overnight,
}

impl std::fmt::Display for RepoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoType::Term => write!(f, "term"),
            RepoType::Open => write!(f, "open"),
            RepoType::Overnight => write!(f, "overnight"),
        }
    }
}

impl std::str::FromStr for RepoType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "term" => Ok(RepoType::Term),
            "open" => Ok(RepoType::Open),
            "overnight" => Ok(RepoType::Overnight),
            other => Err(format!("Unknown repo type: {}", other)),
        }
    }
}

/// Classification of collateral for repos.
#[derive(
    Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum CollateralType {
    /// General collateral (standard market rates)
    #[default]
    General,
    /// Special collateral (specific securities in high demand, may trade at lower rates)
    Special {
        /// Identifier of the specific security
        security_id: String,
        /// Optional special rate adjustment in basis points (negative = lower rate)
        rate_adjustment_bp: Option<f64>,
    },
}

impl std::str::FromStr for CollateralType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "general" | "gc" => Ok(Self::General),
            "special" => Err(
                "CollateralType::Special requires security_id; use CollateralSpec::special()"
                    .to_string(),
            ),
            other => Err(format!(
                "Unknown collateral type: '{}'. Valid: general, special",
                other
            )),
        }
    }
}

/// Specification of collateral backing a repo.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CollateralSpec {
    /// Type of collateral (general vs special)
    pub collateral_type: CollateralType,
    /// Identifier for the collateral instrument
    pub instrument_id: String,
    /// Quantity/face value of collateral
    pub quantity: f64,
    /// Market value identifier in MarketContext (e.g., "BOND_ABC_PRICE")
    pub market_value_id: String,
}

impl CollateralSpec {
    /// Create a new collateral specification.
    pub fn new(
        instrument_id: impl Into<String>,
        quantity: f64,
        market_value_id: impl Into<String>,
    ) -> Self {
        Self {
            collateral_type: CollateralType::default(),
            instrument_id: instrument_id.into(),
            quantity,
            market_value_id: market_value_id.into(),
        }
    }

    /// Create special collateral specification.
    pub fn special(
        security_id: impl Into<String>,
        instrument_id: impl Into<String>,
        quantity: f64,
        market_value_id: impl Into<String>,
        rate_adjustment_bp: Option<f64>,
    ) -> Self {
        Self {
            collateral_type: CollateralType::Special {
                security_id: security_id.into(),
                rate_adjustment_bp,
            },
            instrument_id: instrument_id.into(),
            quantity,
            market_value_id: market_value_id.into(),
        }
    }

    /// Create special collateral specification using a typed rate adjustment in bps.
    pub fn special_bps(
        security_id: impl Into<String>,
        instrument_id: impl Into<String>,
        quantity: f64,
        market_value_id: impl Into<String>,
        rate_adjustment_bp: Option<Bps>,
    ) -> Self {
        Self {
            collateral_type: CollateralType::Special {
                security_id: security_id.into(),
                rate_adjustment_bp: rate_adjustment_bp.map(|bps| bps.as_bps() as f64),
            },
            instrument_id: instrument_id.into(),
            quantity,
            market_value_id: market_value_id.into(),
        }
    }

    /// Calculate the market value of this collateral.
    pub fn market_value(&self, context: &MarketContext) -> Result<Money> {
        let price_scalar = context.get_price(&self.market_value_id)?;
        let unit_value = match price_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => money.amount(),
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
        };

        // Derive currency from price scalar; error on unitless to enforce currency safety
        let currency = match price_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => {
                money.currency()
            }
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(_) => {
                return Err(Error::Input(finstack_quant_core::InputError::Invalid));
            }
        };

        Ok(Money::new(unit_value * self.quantity, currency))
    }
}

/// Repurchase Agreement instrument.
#[derive(
    Debug,
    Clone,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Repo {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Cash amount being lent/borrowed
    pub cash_amount: Money,
    /// Collateral specification
    pub collateral: CollateralSpec,
    /// Repo rate (annual, as decimal)
    pub repo_rate: Decimal,
    /// Start date of the repo
    #[schemars(with = "String")]
    pub start_date: Date,
    /// Maturity date of the repo
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Haircut percentage (as decimal, e.g., 0.02 = 2%)
    pub haircut: f64,
    /// Type of repo
    pub repo_type: RepoType,
    /// Whether this is a tri-party repo
    pub triparty: bool,
    /// Day count convention for interest calculations
    pub day_count: DayCount,
    /// Business day convention
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Optional calendar for business day adjustments
    pub calendar_id: Option<CalendarId>,
    /// Discount curve identifier for valuation
    pub discount_curve_id: CurveId,
    /// Optional margin specification for mark-to-market margining.
    ///
    /// When present, enables margin call generation, collateral valuation,
    /// and margin interest calculations. See [`RepoMarginSpec`] for details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<RepoMarginSpec>,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl Repo {
    /// Validate structural and economic repo invariants at the canonical boundary.
    pub fn validate(&self) -> Result<()> {
        if !self.cash_amount.amount().is_finite() || self.cash_amount.amount() <= 0.0 {
            return Err(Error::Validation(format!(
                "Repo '{}' cash_amount must be finite and positive, got {}",
                self.id,
                self.cash_amount.amount()
            )));
        }
        if !self.haircut.is_finite() || !(0.0..1.0).contains(&self.haircut) {
            return Err(Error::Validation(format!(
                "Repo '{}' haircut must be finite and in [0, 1), got {}",
                self.id, self.haircut
            )));
        }
        if !self.collateral.quantity.is_finite() || self.collateral.quantity <= 0.0 {
            return Err(Error::Validation(format!(
                "Repo '{}' collateral quantity must be finite and positive, got {}",
                self.id, self.collateral.quantity
            )));
        }
        if let CollateralType::Special {
            rate_adjustment_bp: Some(adjustment),
            ..
        } = &self.collateral.collateral_type
        {
            if !adjustment.is_finite() {
                return Err(Error::Validation(format!(
                    "Repo '{}' special-collateral rate adjustment must be finite, got {}",
                    self.id, adjustment
                )));
            }
        }
        if self.start_date >= self.maturity {
            return Err(Error::Validation(format!(
                "Repo '{}' start_date ({}) must precede maturity ({})",
                self.id, self.start_date, self.maturity
            )));
        }
        let (adjusted_start, adjusted_maturity) = self.adjusted_dates()?;
        if adjusted_start >= adjusted_maturity {
            return Err(Error::Validation(format!(
                "Repo '{}' adjusted start date ({adjusted_start}) must precede adjusted maturity ({adjusted_maturity})",
                self.id
            )));
        }
        let effective_rate = self.effective_rate()?;
        if !effective_rate.is_finite() {
            return Err(Error::Validation(format!(
                "Repo '{}' effective rate must be finite",
                self.id
            )));
        }
        Ok(())
    }

    /// Create a canonical example term repo for testing and documentation.
    ///
    /// Returns a 7-day general collateral USD repo.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        let collateral = CollateralSpec::new("UST-10Y", 10_000.0, "UST_10Y_PRICE");
        let start =
            Date::from_calendar_date(2024, time::Month::January, 2).expect("Valid example date");
        let maturity =
            Date::from_calendar_date(2024, time::Month::January, 9).expect("Valid example date");
        Self::term(
            "REPO-GC-7D",
            Money::new(10_000_000.0, finstack_quant_core::currency::Currency::USD),
            collateral,
            0.0525,
            start,
            maturity,
            "USD-OIS",
        )
        .expect("Example repo construction should not fail")
    }

    /// Create a new repo builder (provided by derive).
    /// Create a standard overnight repo.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique instrument identifier
    /// * `cash_amount` - Cash amount being lent/borrowed
    /// * `collateral` - Collateral specification
    /// * `repo_rate` - Repo rate (annual, as decimal)
    /// * `start_date` - Start date of the repo (will be adjusted if not a business day)
    /// * `calendar_id` - Calendar identifier for business day adjustments (e.g., "target2", "nyse")
    /// * `discount_curve_id` - Discount curve identifier for valuation
    ///
    /// # Market Standard
    ///
    /// The start date is business-day adjusted using the specified calendar, and
    /// maturity is calculated as the next business day after the adjusted start.
    pub fn overnight(
        id: impl Into<String>,
        cash_amount: Money,
        collateral: CollateralSpec,
        repo_rate: f64,
        start_date: Date,
        calendar_id: impl Into<String>,
        discount_curve_id: impl Into<CurveId>,
    ) -> Result<Self> {
        use finstack_quant_core::dates::calendar::calendar_by_id;

        let defaults = embedded_registry()?.repo_defaults(DEFAULT_REPO_SPECS_ID)?;
        let cal_id = calendar_id.into();
        let calendar = calendar_by_id(&cal_id).ok_or_else(|| {
            Error::Input(finstack_quant_core::InputError::NotFound {
                id: format!("calendar:{}", cal_id),
            })
        })?;

        // Adjust start date to next business day if needed
        let adj_start = adjust(start_date, BusinessDayConvention::Following, calendar)?;
        // Maturity is next business day after adjusted start
        let maturity = adj_start.add_business_days(1, calendar)?;

        let repo_rate = Decimal::try_from(repo_rate)
            .map_err(|_| finstack_quant_core::InputError::ConversionOverflow)?;

        Repo::builder()
            .id(id.into().into())
            .cash_amount(cash_amount)
            .collateral(collateral)
            .repo_rate(repo_rate)
            .start_date(adj_start)
            .maturity(maturity)
            .haircut(defaults.haircut)
            .repo_type(RepoType::Overnight)
            .triparty(defaults.triparty)
            .day_count(defaults.day_count)
            .bdc(defaults.business_day_convention)
            .calendar_id_opt(Some(cal_id.into()))
            .discount_curve_id(discount_curve_id.into())
            .margin_spec_opt(None)
            .attributes(Attributes::default())
            .build()
    }

    /// Create a term repo with specified maturity.
    pub fn term(
        id: impl Into<String>,
        cash_amount: Money,
        collateral: CollateralSpec,
        repo_rate: f64,
        start_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<CurveId>,
    ) -> Result<Self> {
        let defaults = embedded_registry()?.repo_defaults(DEFAULT_REPO_SPECS_ID)?;
        let repo_rate = Decimal::try_from(repo_rate)
            .map_err(|_| finstack_quant_core::InputError::ConversionOverflow)?;

        Repo::builder()
            .id(id.into().into())
            .cash_amount(cash_amount)
            .collateral(collateral)
            .repo_rate(repo_rate)
            .start_date(start_date)
            .maturity(maturity)
            .haircut(defaults.haircut)
            .repo_type(RepoType::Term)
            .triparty(defaults.triparty)
            .day_count(defaults.day_count)
            .bdc(defaults.business_day_convention)
            .calendar_id_opt(Some(defaults.calendar_id.into()))
            .discount_curve_id(discount_curve_id.into())
            .margin_spec_opt(None)
            .attributes(Attributes::default())
            .build()
    }

    /// Create an open repo with an initial maturity (can be rolled/terminated later).
    pub fn open(
        id: impl Into<String>,
        cash_amount: Money,
        collateral: CollateralSpec,
        repo_rate: f64,
        start_date: Date,
        initial_maturity: Date,
        discount_curve_id: impl Into<CurveId>,
    ) -> Result<Self> {
        let defaults = embedded_registry()?.repo_defaults(DEFAULT_REPO_SPECS_ID)?;
        let repo_rate = Decimal::try_from(repo_rate)
            .map_err(|_| finstack_quant_core::InputError::ConversionOverflow)?;

        Repo::builder()
            .id(id.into().into())
            .cash_amount(cash_amount)
            .collateral(collateral)
            .repo_rate(repo_rate)
            .start_date(start_date)
            .maturity(initial_maturity)
            .haircut(defaults.haircut)
            .repo_type(RepoType::Open)
            .triparty(defaults.triparty)
            .day_count(defaults.day_count)
            .bdc(defaults.business_day_convention)
            .calendar_id_opt(Some(defaults.calendar_id.into()))
            .discount_curve_id(discount_curve_id.into())
            .margin_spec_opt(None)
            .attributes(Attributes::default())
            .build()
    }

    /// Calculate the effective repo rate considering special collateral adjustments.
    ///
    /// # Errors
    ///
    /// Returns an error if `repo_rate` cannot be represented as `f64`.
    pub fn effective_rate(&self) -> Result<f64> {
        let base_rate = decimal_to_f64(self.repo_rate, "Repo repo_rate")?;
        Ok(match &self.collateral.collateral_type {
            CollateralType::General => base_rate,
            CollateralType::Special {
                rate_adjustment_bp, ..
            } => {
                if let Some(adjustment_bp) = rate_adjustment_bp {
                    const ONE_BP: f64 = 1e-4;
                    base_rate + (adjustment_bp * ONE_BP) // Convert bp to decimal
                } else {
                    base_rate
                }
            }
        })
    }

    /// Calculate required collateral value including haircut.
    ///
    /// Formula: `Cash_lent = Collateral_value * (1 - Haircut)`
    /// Therefore: `Collateral_value = Cash_lent / (1 - Haircut)`
    pub fn required_collateral_value(&self) -> Result<Money> {
        if self.haircut >= 1.0 {
            return Err(Error::Input(finstack_quant_core::InputError::Invalid));
        }
        let factor = 1.0 - self.haircut;
        if factor <= 0.0 {
            return Err(Error::Input(finstack_quant_core::InputError::Invalid));
        }
        Ok(self.cash_amount / factor)
    }

    /// Check if the repo is adequately collateralized.
    pub fn is_adequately_collateralized(&self, context: &MarketContext) -> Result<bool> {
        let collateral_value = self.collateral.market_value(context)?;
        let required_value = self.required_collateral_value()?;

        // Ensure same currency for comparison
        if collateral_value.currency() != required_value.currency() {
            return Err(Error::CurrencyMismatch {
                expected: required_value.currency(),
                actual: collateral_value.currency(),
            });
        }

        Ok(collateral_value.amount() >= required_value.amount())
    }

    /// Compute present value of the repo using curves in the market context.
    ///
    /// Returns the PV of future cashflows.
    ///
    /// If `as_of <= start_date`:
    ///   PV = PV(repayment) - PV(initial_outflow)
    ///
    /// If `start_date < as_of < maturity`:
    ///   PV = PV(repayment)
    ///
    /// If `as_of >= maturity`:
    ///   PV = 0 (assumes settled)
    ///
    /// # Market Standard
    ///
    /// Uses business-day adjusted dates for all comparisons and discount factor
    /// calculations to ensure correct accrual fractions and haircut coverage.
    pub fn pv(&self, context: &MarketContext, as_of: Date) -> Result<Money> {
        let (_, adj_maturity) = self.adjusted_dates()?;
        if as_of >= adj_maturity {
            return Ok(Money::new(0.0, self.cash_amount.currency()));
        }

        let disc_curve = context.get_discount(self.discount_curve_id.as_str())?;
        let flows = self.dated_cashflows(context, as_of)?;

        if flows.is_empty() {
            return Ok(Money::new(0.0, self.cash_amount.currency()));
        }

        flows.into_iter().try_fold(
            Money::new(0.0, self.cash_amount.currency()),
            |acc, (date, amount)| {
                let df = disc_curve.df_between_dates(as_of, date)?;
                acc.checked_add(amount * df)
            },
        )
    }

    /// Calculate repo interest amount.
    ///
    /// # Market Standard
    ///
    /// Uses business-day adjusted dates for the accrual period to ensure
    /// accurate interest calculations. Unadjusted weekends can misstate
    /// accrual fractions.
    pub fn interest_amount(&self) -> Result<Money> {
        // Apply business day adjustments to get correct accrual period
        let (adj_start, adj_maturity) = self.adjusted_dates()?;

        let year_fraction = self.day_count.year_fraction(
            adj_start,
            adj_maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;

        let effective_rate = self.effective_rate()?;
        let interest = self.cash_amount.amount() * effective_rate * year_fraction;

        Ok(Money::new(interest, self.cash_amount.currency()))
    }

    /// Calculate total repayment amount (principal + interest).
    pub fn total_repayment(&self) -> Result<Money> {
        let interest = self.interest_amount()?;
        self.cash_amount.checked_add(interest)
    }

    /// Returns business-day adjusted start and maturity dates.
    ///
    /// # Market Standard
    ///
    /// Repo start/end dates must be business-adjusted (typically T+1/T+2).
    /// Unadjusted weekends misstate accrual fractions and haircut coverage.
    /// A calendar is required for accurate schedule generation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `calendar_id` is not specified (required for repos)
    /// - `calendar_id` references an unknown calendar
    pub fn adjusted_dates(&self) -> Result<(Date, Date)> {
        use finstack_quant_core::dates::calendar::calendar_by_id;
        use finstack_quant_core::dates::CalendarRegistry;

        let cal_id = self.calendar_id.as_ref().ok_or_else(|| {
            Error::Validation(
                "Repo instruments require a calendar_id for accurate schedule generation. \
                 Specify a valid calendar ID (e.g., 'nyse', 'target2', 'usny') to ensure \
                 start and maturity dates are adjusted correctly for business days."
                    .to_string(),
            )
        })?;

        let calendar = calendar_by_id(cal_id).ok_or_else(|| {
            Error::Input(finstack_quant_core::InputError::NotFound {
                id: format!(
                    "calendar_id:{} (available: {})",
                    cal_id,
                    CalendarRegistry::global().available_ids().join(", ")
                ),
            })
        })?;

        let adj_start = adjust(self.start_date, self.bdc, calendar)?;
        let adj_maturity = adjust(self.maturity, self.bdc, calendar)?;
        Ok((adj_start, adj_maturity))
    }
}

impl Instrument for Repo {
    impl_instrument_base!(crate::pricer::InstrumentType::Repo);

    fn validate_invariants(&self) -> Result<()> {
        Repo::validate(self)
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_spot_id(&self.collateral.market_value_id);
        Ok(deps)
    }

    // === Pricing Methods ===

    fn base_value(&self, context: &MarketContext, as_of: Date) -> Result<Money> {
        // Use the instrument's own pricing method
        self.pv(context, as_of)
    }

    fn as_marginable(&self) -> Option<&dyn finstack_quant_margin::Marginable> {
        Some(self)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.start_date)
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

// Do not add explicit Instrument impl; provided by blanket impl.

// Attributable is provided via blanket impl for all Instrument types

impl finstack_quant_cashflows::CashflowScheduleSource for Repo {
    fn notional(&self) -> Option<Money> {
        Some(self.cash_amount)
    }

    fn raw_cashflow_schedule(
        &self,
        _context: &MarketContext,
        _as_of: Date,
    ) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        let (adj_start, adj_maturity) = self.adjusted_dates()?;

        let cash_outflow = Money::new(-self.cash_amount.amount(), self.cash_amount.currency());
        let total_repayment = self.total_repayment()?;
        let flows = vec![
            crate::cashflow::primitives::CashFlow::new(
                adj_start,
                None,
                cash_outflow,
                crate::cashflow::primitives::CFKind::Notional,
                0.0,
                None,
            ),
            crate::cashflow::primitives::CashFlow::new(
                adj_maturity,
                None,
                total_repayment,
                crate::cashflow::primitives::CFKind::Fixed,
                0.0,
                None,
            ),
        ];

        let schedule = crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                ..Default::default()
            },
        );
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Contractual))
    }
}

// Declare canonical market dependencies for the DV01 calculator.
#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use time::Month;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("Valid month (1-12)"), d)
            .expect("Valid test date")
    }

    #[test]
    fn required_collateral_includes_haircut() {
        let collateral = CollateralSpec::new("BOND", 100.0, "BOND_PX");
        let repo = Repo::term(
            "R",
            Money::new(1_000_000.0, Currency::USD),
            collateral,
            0.05,
            date(2025, 1, 1),
            date(2025, 2, 1),
            "USD-OIS",
        )
        .expect("test repo construction");
        let req = repo
            .required_collateral_value()
            .expect("Required collateral value calculation should succeed in test");
        // 1_000_000 / (1 - 0.02) = 1_020_408.16
        assert!((req.amount() - 1_020_408.16).abs() < 1e-2);
    }

    #[test]
    fn interest_amount_respects_daycount_and_rate() {
        let collateral = CollateralSpec::new("BOND", 100.0, "BOND_PX");
        // Use business days that don't need adjustment
        // Jan 6, 2025 = Monday (business day)
        // Apr 7, 2025 = Monday (business day)
        // This gives ~91 days = 91/360 ≈ 0.2528 years
        let repo = Repo::term(
            "R",
            Money::new(1_000_000.0, Currency::USD),
            collateral,
            0.12, // 12%
            date(2025, 1, 6),
            date(2025, 4, 7),
            "USD-OIS",
        )
        .expect("test repo construction");
        let interest = repo
            .interest_amount()
            .expect("Interest amount calculation should succeed in test");
        // 1_000_000 * 0.12 * (91/360) ≈ 30_333.33
        assert!(
            (interest.amount() - 30_333.33).abs() < 100.0,
            "Interest was {}, expected ~30,333",
            interest.amount()
        );
    }

    #[test]
    fn collateral_value_requires_currency_price() {
        let collateral = CollateralSpec::new("BOND", 100.0, "BOND_PX");
        let repo = Repo::term(
            "R",
            Money::new(1_000_000.0, Currency::USD),
            collateral,
            0.05,
            date(2025, 1, 1),
            date(2025, 2, 1),
            "USD-OIS",
        )
        .expect("test repo construction");
        let ctx = MarketContext::new().insert_price(
            "BOND_PX",
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(1.0),
        );
        // Should error due to currency safety enforcement
        assert!(repo.collateral.market_value(&ctx).is_err());
    }

    /// Test that term repo over a weekend with TARGET calendar adjusts dates correctly.
    ///
    /// Market standard: repo start/end dates must be business-adjusted.
    /// This test verifies:
    /// - Weekend dates are shifted to the following business day
    /// - Adjusted dates match the calendar
    /// - PV is deterministic and stable
    #[test]
    fn term_repo_weekend_bdc_adjustment_target2() {
        use crate::cashflow::traits::CashflowProvider;
        use finstack_quant_core::dates::calendar::TARGET2;
        use finstack_quant_core::dates::HolidayCalendar;

        // Saturday Jan 4, 2025 -> should adjust to Monday Jan 6
        let start_saturday = date(2025, 1, 4);
        // Saturday Jan 11, 2025 -> should adjust to Monday Jan 13
        let maturity_saturday = date(2025, 1, 11);

        let collateral = CollateralSpec::new("BOND", 100.0, "BOND_PX");

        // Build repo with TARGET2 calendar and Following BDC
        let repo = Repo::builder()
            .id(InstrumentId::from("REPO-WEEKEND-TEST"))
            .cash_amount(Money::new(1_000_000.0, Currency::USD))
            .collateral(collateral)
            .repo_rate(rust_decimal::Decimal::try_from(0.05).expect("valid decimal"))
            .start_date(start_saturday)
            .maturity(maturity_saturday)
            .haircut(0.02)
            .repo_type(RepoType::Term)
            .triparty(false)
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::Following)
            .calendar_id_opt(Some("target2".into()))
            .discount_curve_id(CurveId::from("USD-OIS"))
            .margin_spec_opt(None)
            .attributes(Attributes::default())
            .build()
            .expect("Builder should succeed");

        // Verify adjusted dates
        let (adj_start, adj_maturity) = repo.adjusted_dates().expect("Adjustment should succeed");

        // Saturday Jan 4 -> Monday Jan 6 (Following)
        assert!(
            TARGET2.is_business_day(adj_start),
            "Adjusted start should be a business day"
        );
        assert_eq!(
            adj_start,
            date(2025, 1, 6),
            "Saturday should roll to Monday"
        );

        // Saturday Jan 11 -> Monday Jan 13 (Following)
        assert!(
            TARGET2.is_business_day(adj_maturity),
            "Adjusted maturity should be a business day"
        );
        assert_eq!(
            adj_maturity,
            date(2025, 1, 13),
            "Saturday should roll to Monday"
        );

        // Verify cashflows use adjusted dates
        let ctx = MarketContext::new();
        let flows = repo
            .dated_cashflows(&ctx, date(2025, 1, 1))
            .expect("Schedule should build");

        assert_eq!(flows.len(), 2, "Repo should have 2 cashflows");
        assert_eq!(
            flows[0].0, adj_start,
            "First flow should be on adjusted start"
        );
        assert_eq!(
            flows[1].0, adj_maturity,
            "Second flow should be on adjusted maturity"
        );

        // Verify determinism: run multiple times and check consistency
        for _ in 0..3 {
            let (s, m) = repo.adjusted_dates().expect("Should succeed");
            assert_eq!(s, adj_start, "Adjusted start should be deterministic");
            assert_eq!(m, adj_maturity, "Adjusted maturity should be deterministic");
        }
    }

    /// Test that repo without calendar_id errors on adjusted_dates().
    ///
    /// Market standard: repos require a calendar for accurate business day adjustments.
    /// Missing calendar should fail fast with a clear error rather than silently
    /// returning raw dates that may fall on weekends/holidays.
    #[test]
    fn repo_no_calendar_errors() {
        let collateral = CollateralSpec::new("BOND", 100.0, "BOND_PX");

        // Build repo without calendar
        let repo = Repo::builder()
            .id(InstrumentId::from("REPO-NO-CAL"))
            .cash_amount(Money::new(1_000_000.0, Currency::USD))
            .collateral(collateral)
            .repo_rate(rust_decimal::Decimal::try_from(0.05).expect("valid decimal"))
            .start_date(date(2025, 1, 4)) // Saturday
            .maturity(date(2025, 1, 11)) // Saturday
            .haircut(0.02)
            .repo_type(RepoType::Term)
            .triparty(false)
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::Following)
            .calendar_id_opt(None) // No calendar - should cause error
            .discount_curve_id(CurveId::from("USD-OIS"))
            .margin_spec_opt(None)
            .attributes(Attributes::default())
            .build()
            .expect("Builder should succeed");

        // adjusted_dates() should error because calendar_id is required
        let result = repo.adjusted_dates();
        assert!(result.is_err(), "Missing calendar_id should error");

        let err = result
            .expect_err("Missing calendar_id should error")
            .to_string();
        assert!(
            err.contains("calendar_id") || err.contains("calendar"),
            "Error should mention calendar requirement: {}",
            err
        );
    }

    /// Test that unknown calendar_id returns an error.
    #[test]
    fn repo_unknown_calendar_errors() {
        let collateral = CollateralSpec::new("BOND", 100.0, "BOND_PX");

        let repo = Repo::builder()
            .id(InstrumentId::from("REPO-BAD-CAL"))
            .cash_amount(Money::new(1_000_000.0, Currency::USD))
            .collateral(collateral)
            .repo_rate(rust_decimal::Decimal::try_from(0.05).expect("valid decimal"))
            .start_date(date(2025, 1, 4))
            .maturity(date(2025, 1, 11))
            .haircut(0.02)
            .repo_type(RepoType::Term)
            .triparty(false)
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::Following)
            .calendar_id_opt(Some("NONEXISTENT_CALENDAR".into()))
            .discount_curve_id(CurveId::from("USD-OIS"))
            .margin_spec_opt(None)
            .attributes(Attributes::default())
            .build()
            .expect("Builder should succeed");

        let result = repo.adjusted_dates();
        assert!(result.is_err(), "Unknown calendar should error");
    }

    /// Test PV stability: weekend-dated repo vs pre-adjusted repo should produce identical PVs.
    ///
    /// Market standard: repo start/end dates must be business-adjusted (often T+1/T+2).
    /// This test verifies:
    /// - Weekend dates are shifted to the following business day
    /// - PV difference vs manual (pre-adjusted) calculation is 0 within tolerance 1e-6
    /// - Interest calculations are deterministic and correct
    #[test]
    fn term_repo_weekend_pv_stability_target2() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        // Saturday Jan 4, 2025 -> Monday Jan 6
        let start_saturday = date(2025, 1, 4);
        // Saturday Jan 11, 2025 -> Monday Jan 13
        let maturity_saturday = date(2025, 1, 11);

        // Pre-adjusted (Monday) dates
        let start_monday = date(2025, 1, 6);
        let maturity_monday = date(2025, 1, 13);

        let cash_amount = Money::new(1_000_000.0, Currency::USD);
        let repo_rate = rust_decimal::Decimal::try_from(0.05).expect("valid decimal");

        let collateral1 = CollateralSpec::new("BOND", 100.0, "BOND_PX");
        let collateral2 = CollateralSpec::new("BOND", 100.0, "BOND_PX");

        // Repo with weekend dates (will be adjusted internally)
        let repo_weekend = Repo::builder()
            .id(InstrumentId::from("REPO-WEEKEND"))
            .cash_amount(cash_amount)
            .collateral(collateral1)
            .repo_rate(repo_rate)
            .start_date(start_saturday)
            .maturity(maturity_saturday)
            .haircut(0.02)
            .repo_type(RepoType::Term)
            .triparty(false)
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::Following)
            .calendar_id_opt(Some("target2".into()))
            .discount_curve_id(CurveId::from("USD-OIS"))
            .margin_spec_opt(None)
            .attributes(Attributes::default())
            .build()
            .expect("Weekend repo should build");

        // Repo with pre-adjusted dates (no adjustment needed)
        let repo_adjusted = Repo::builder()
            .id(InstrumentId::from("REPO-ADJUSTED"))
            .cash_amount(cash_amount)
            .collateral(collateral2)
            .repo_rate(repo_rate)
            .start_date(start_monday)
            .maturity(maturity_monday)
            .haircut(0.02)
            .repo_type(RepoType::Term)
            .triparty(false)
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::Following)
            .calendar_id_opt(Some("target2".into()))
            .discount_curve_id(CurveId::from("USD-OIS"))
            .margin_spec_opt(None)
            .attributes(Attributes::default())
            .build()
            .expect("Adjusted repo should build");

        // Verify adjusted dates match
        let (adj_start_w, adj_mat_w) = repo_weekend
            .adjusted_dates()
            .expect("Adjustment should succeed");
        let (adj_start_a, adj_mat_a) = repo_adjusted
            .adjusted_dates()
            .expect("Adjustment should succeed");

        assert_eq!(
            adj_start_w, adj_start_a,
            "Adjusted start dates should match"
        );
        assert_eq!(adj_mat_w, adj_mat_a, "Adjusted maturity dates should match");
        assert_eq!(
            adj_start_w, start_monday,
            "Weekend start should adjust to Monday"
        );
        assert_eq!(
            adj_mat_w, maturity_monday,
            "Weekend maturity should adjust to Monday"
        );

        // Verify interest amounts match (uses adjusted dates)
        let interest_weekend = repo_weekend
            .interest_amount()
            .expect("Interest should compute");
        let interest_adjusted = repo_adjusted
            .interest_amount()
            .expect("Interest should compute");

        let interest_diff = (interest_weekend.amount() - interest_adjusted.amount()).abs();
        assert!(
            interest_diff < 1e-6,
            "Interest amounts should match within 1e-6, got diff: {}",
            interest_diff
        );

        // Create market context with a flat discount curve
        let as_of = date(2025, 1, 2); // Thursday before the repo starts
        let flat_rate: f64 = 0.04;
        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, (-flat_rate).exp()),
                (5.0, (-flat_rate * 5.0).exp()),
            ])
            .build()
            .expect("DiscountCurve builder should succeed");
        let ctx = MarketContext::new().insert(disc_curve);

        // Verify PV stability within 1e-6 tolerance
        let pv_weekend = repo_weekend.pv(&ctx, as_of).expect("PV should compute");
        let pv_adjusted = repo_adjusted.pv(&ctx, as_of).expect("PV should compute");

        let pv_diff = (pv_weekend.amount() - pv_adjusted.amount()).abs();
        assert!(
            pv_diff < 1e-6,
            "PV should match within 1e-6, got diff: {}",
            pv_diff
        );

        // Verify determinism: multiple runs produce identical results
        for _ in 0..3 {
            let pv1 = repo_weekend.pv(&ctx, as_of).expect("PV should compute");
            let pv2 = repo_adjusted.pv(&ctx, as_of).expect("PV should compute");
            assert_eq!(
                pv1.amount(),
                pv_weekend.amount(),
                "PV should be deterministic"
            );
            assert_eq!(
                pv2.amount(),
                pv_adjusted.amount(),
                "PV should be deterministic"
            );
        }
    }

    /// Test overnight repo with calendar properly adjusts start date.
    #[test]
    fn overnight_repo_adjusts_start_date() {
        use finstack_quant_core::dates::calendar::TARGET2;
        use finstack_quant_core::dates::HolidayCalendar;

        // Saturday Jan 4, 2025 -> should adjust to Monday Jan 6
        let start_saturday = date(2025, 1, 4);
        let collateral = CollateralSpec::new("BOND", 100.0, "BOND_PX");

        let repo = Repo::overnight(
            "OVERNIGHT-TEST",
            Money::new(1_000_000.0, Currency::USD),
            collateral,
            0.05,
            start_saturday,
            "target2",
            "USD-OIS",
        )
        .expect("Overnight repo should build");

        // Verify the stored start_date is already adjusted (Monday Jan 6)
        assert_eq!(
            repo.start_date,
            date(2025, 1, 6),
            "Start date should be pre-adjusted to Monday"
        );
        assert!(
            TARGET2.is_business_day(repo.start_date),
            "Start date should be a business day"
        );

        // Maturity should be Tuesday Jan 7 (next business day after Monday)
        assert_eq!(
            repo.maturity,
            date(2025, 1, 7),
            "Maturity should be next business day (Tuesday)"
        );
        assert!(
            TARGET2.is_business_day(repo.maturity),
            "Maturity should be a business day"
        );

        // Adjusted dates should match stored dates (since they're already adjusted)
        let (adj_start, adj_maturity) = repo.adjusted_dates().expect("Adjustment should succeed");
        assert_eq!(
            adj_start, repo.start_date,
            "Adjusted start should match stored"
        );
        assert_eq!(
            adj_maturity, repo.maturity,
            "Adjusted maturity should match stored"
        );
    }

    /// Test overnight repo with unknown calendar returns error.
    #[test]
    fn overnight_repo_unknown_calendar_errors() {
        let collateral = CollateralSpec::new("BOND", 100.0, "BOND_PX");

        let result = Repo::overnight(
            "OVERNIGHT-BAD-CAL",
            Money::new(1_000_000.0, Currency::USD),
            collateral,
            0.05,
            date(2025, 1, 6),
            "NONEXISTENT_CALENDAR",
            "USD-OIS",
        );

        assert!(result.is_err(), "Unknown calendar should error");
    }
}
