//! `CDSOption` instrument: European option to enter a forward CDS at a fixed
//! strike spread.
//!
//! Pricing is performed by the Bloomberg CDSO numerical-quadrature model
//! ([`super::pricer`] / [`super::bloomberg_quadrature`]) per *Pricing Credit
//! Index Options* (Bloomberg L.P. Quantitative Analytics, DOCS 2055833). The
//! legacy closed-form Black-on-spreads pricer was removed when the Bloomberg
//! model became the default; see DOCS 2055833 §1.2 ("the Black model will be
//! decommissioned").
//!
//! # Validation
//!
//! `CDSOption::try_new` validates all inputs at construction time:
//! - Strike spread must be positive
//! - Option expiry must precede underlying CDS maturity
//! - Recovery rate must be in (0, 1)
//! - Index factor must be in (0, 1] when specified
//! - Implied volatility override must be in (0, 5] when specified
//! - Only European, cash-settled CDS options are supported
//!
//! # Volatility convention
//!
//! Volatilities are lognormal (Black) volatilities in decimal form (e.g. 0.30
//! for 30%). The Bloomberg CDSO terminal expects the same.

use crate::instruments::common_impl::parameters::CreditParams;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::{ExerciseStyle, OptionType, SettlementType};
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::{
    adjust, calendar_by_id, BusinessDayConvention, DateExt, HolidayCalendar,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use rust_decimal::Decimal;
use time::Month;

use super::parameters::CDSOptionParams;
use crate::impl_instrument_base;

/// Maximum valid recovery rate (exclusive upper bound).
pub(crate) const MAX_RECOVERY_RATE: f64 = 1.0;
/// Maximum valid implied volatility (inclusive upper bound).
/// 500% lognormal vol is extremely high but theoretically valid.
pub(crate) const MAX_IMPLIED_VOL: f64 = 5.0;

/// Accrual-start convention for the synthetic underlying CDS used by CDSO.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionStartConvention {
    /// Spot-protection CDS: standard prior CDS roll relative to valuation date.
    #[default]
    Spot,
    /// Forward-protection CDS: accrual starts at option expiry.
    Forward,
}

/// Credit option instrument (option on CDS spread)
///
/// Currently the public pricing surface supports only European, cash-settled
/// CDS options. Other exercise and settlement styles are rejected at pricing
/// time so deserialized instruments cannot silently fall through to the
/// Black-on-spreads engine.
#[derive(
    Debug,
    Clone,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[serde(deny_unknown_fields)]
pub struct CDSOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Strike spread as a decimal rate (e.g., 0.01 = 100bp)
    pub strike: Decimal,
    /// Option type (Call = right to buy protection, Put = right to sell protection)
    pub option_type: OptionType,
    /// Exercise style
    pub exercise_style: ExerciseStyle,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Underlying CDS maturity date
    #[schemars(with = "String")]
    pub cds_maturity: Date,
    /// Notional amount
    pub notional: Money,
    /// Settlement type
    pub settlement: SettlementType,
    /// Cash premium settlement date for Black time-to-expiry, when the screen
    /// quotes option time from premium settlement rather than valuation date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    #[builder(default)]
    pub cash_settlement_date: Option<Date>,
    /// Exercise settlement date for Black time-to-expiry, when distinct from
    /// the legal option expiration date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    #[builder(default)]
    pub exercise_settlement_date: Option<Date>,
    /// Underlying CDS accrual-effective date used for forward spread and risky
    /// annuity. Bloomberg CDSO can quote a standard CDS effective date before
    /// option expiry; in that case premium accrues from this date while
    /// protection starts at expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    #[builder(default)]
    pub underlying_effective_date: Option<Date>,
    /// Convention used to select the synthetic underlying CDS accrual start
    /// when `underlying_effective_date` is not explicitly supplied.
    #[serde(default)]
    #[builder(default)]
    pub protection_start_convention: ProtectionStartConvention,
    /// Whether the option knocks out if the underlying defaults before
    /// exercise. This is contract-specific; new instruments default to
    /// no-knockout and legacy single-name books can opt in explicitly.
    #[serde(default)]
    #[builder(default)]
    pub knockout: bool,
    /// Recovery rate assumption
    pub recovery_rate: f64,
    /// Discount curve identifier
    pub discount_curve_id: CurveId,
    /// Credit curve identifier
    pub credit_curve_id: CurveId,
    /// Volatility surface identifier
    pub vol_surface_id: CurveId,
    /// Convention used by the underlying CDS contract.
    ///
    /// This controls the CDS schedule, settlement lag, business day convention,
    /// and other market-standard mechanics used when deriving forward spread and
    /// risky annuity for the option's underlying.
    #[serde(default)]
    #[builder(default)]
    pub underlying_convention: crate::instruments::credit_derivatives::cds::CDSConvention,
    /// Instrument-owned pricing overrides (including implied volatility).
    #[builder(default)]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only valuation adjustments.
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Additional attributes
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
    /// If true, the underlying is a CDS index; else single-name CDS.
    ///
    /// The Bloomberg CDSO model treats the two cases differently in the
    /// no-knockout calibration `F_0 = E[V_te]` (DOCS 2055833 §1.2): index
    /// options trade no-knockout and the calibration target includes the
    /// `(1−R)·(1−q_te)` FEP-equivalent contribution; single-name options
    /// knock out on default and skip it.
    #[serde(default)]
    pub underlying_is_index: bool,
    /// Optional index factor scaling for the index underlying.
    pub index_factor: Option<f64>,
    /// Realized cumulative index loss from option inception to valuation
    /// date, expressed per unit of original index notional.
    ///
    /// Bloomberg CDSO treats index options as no-knockout. Settled losses
    /// after option inception are therefore deterministic payoff adjustments
    /// at exercise (DOCS 2055833 Eq. 2.5 and DOCS 2151513). Single-name
    /// options knock out instead and must leave this unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub realized_index_loss: Option<f64>,
    /// Contractual coupon `c` of the underlying CDS, expressed as a decimal
    /// rate (e.g., 0.01 for the 100 bp standard CDX coupon, 0.05 for the
    /// 500 bp standard CDX.HY coupon). When `None`, the synthetic underlying
    /// CDS uses `strike` as its running coupon — the appropriate single-name
    /// SNAC default where the trade is struck at the par spread. For CDS
    /// index options where the index has a fixed standard coupon different
    /// from the option strike, set this explicitly so the strike-adjustment
    /// term `H(K) = ξN(c − K)A(K)` (DOCS 2055833 Eq. 2.4) is populated.
    #[serde(default)]
    pub underlying_cds_coupon: Option<Decimal>,
}

impl CDSOption {
    pub(crate) fn validate_supported_configuration(&self) -> finstack_quant_core::Result<()> {
        if self.exercise_style != ExerciseStyle::European {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS options currently support only European exercise; got {:?}",
                self.exercise_style
            )));
        }

        if self.settlement != SettlementType::Cash {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS options currently support only cash settlement; got {:?}",
                self.settlement
            )));
        }

        Ok(())
    }

    /// Validate the CDSOption parameters.
    fn validate(&self) -> finstack_quant_core::Result<()> {
        use crate::instruments::common_impl::validation;

        super::parameters::validate_common_terms(
            self.strike,
            self.expiry,
            self.cds_maturity,
            self.index_factor,
        )?;
        validation::validate_money_finite(self.notional, "CDS option notional")?;
        validation::validate_money_gt(self.notional, 0.0, "CDS option notional")?;

        if let (Some(cash_settlement), Some(exercise_settlement)) =
            (self.cash_settlement_date, self.exercise_settlement_date)
        {
            if exercise_settlement <= cash_settlement {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "exercise_settlement_date ({}) must be after cash_settlement_date ({})",
                    exercise_settlement, cash_settlement
                )));
            }
        }
        if let Some(exercise_settlement) = self.exercise_settlement_date {
            if exercise_settlement >= self.cds_maturity {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "exercise_settlement_date ({}) must be before CDS maturity ({})",
                    exercise_settlement, self.cds_maturity
                )));
            }
        }
        if let Some(underlying_effective_date) = self.underlying_effective_date {
            if underlying_effective_date >= self.cds_maturity {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "underlying_effective_date ({}) must be before CDS maturity ({})",
                    underlying_effective_date, self.cds_maturity
                )));
            }
        }

        // Recovery rate validation
        if !self.recovery_rate.is_finite()
            || self.recovery_rate <= 0.0
            || self.recovery_rate >= MAX_RECOVERY_RATE
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "recovery_rate must be finite and in (0, 1), got {}",
                self.recovery_rate
            )));
        }

        // Realized index loss validation
        if let Some(loss) = self.realized_index_loss {
            if !(0.0..=1.0).contains(&loss) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "realized_index_loss must be in [0, 1], got {}",
                    loss
                )));
            }
            if loss > 0.0 && !self.underlying_is_index {
                return Err(finstack_quant_core::Error::Validation(
                    "realized_index_loss is only supported for CDS index options".to_string(),
                ));
            }
        }

        if self.underlying_is_index && self.underlying_cds_coupon.is_none() {
            return Err(finstack_quant_core::Error::Validation(
                "underlying_cds_coupon is required for CDS index options".to_string(),
            ));
        }

        // Implied volatility override validation
        if let Some(vol) = self
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility
        {
            if !vol.is_finite() || vol <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "implied_volatility must be finite and positive, got {}",
                    vol
                )));
            }
            if vol > MAX_IMPLIED_VOL {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "implied_volatility {} exceeds maximum {}",
                    vol, MAX_IMPLIED_VOL
                )));
            }
        }

        Ok(())
    }

    /// Create a canonical example CDS option (call on CDS spread).
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use time::macros::date;
        let option_params = CDSOptionParams::call(
            Decimal::new(1, 2), // 0.01 = 100bp
            date!(2025 - 06 - 20),
            date!(2030 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )?;
        let credit_params =
            crate::instruments::common_impl::parameters::CreditParams::corporate_standard(
                "CORP",
                "CORP-HAZARD",
            );
        CDSOption::new(
            InstrumentId::new("CDSOPT-CALL-CORP-5Y"),
            &option_params,
            &credit_params,
            "USD-OIS",
            "CDSOPT-VOL",
        )
    }

    /// Create a new credit option using parameter structs with validation.
    ///
    /// # Arguments
    ///
    /// - `id`: Unique instrument identifier
    /// - `option_params`: deal-level fields (strike as decimal rate, expiry, CDS maturity, notional, option type)
    /// - `credit_params`: reference entity, recovery rate, and the hazard `credit_id`
    /// - `discount_curve_id`: discount curve identifier for discounting cashflows
    /// - `vol_surface_id`: volatility surface identifier for the CDS option
    ///
    /// # Errors
    ///
    /// Returns an error if any validation fails. See [`CDSOptionParams`] for parameter constraints.
    pub fn new(
        id: impl Into<InstrumentId>,
        option_params: &CDSOptionParams,
        credit_params: &CreditParams,
        discount_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let option = Self {
            id: id.into(),
            strike: option_params.strike,
            option_type: option_params.option_type,
            exercise_style: ExerciseStyle::European,
            expiry: option_params.expiry,
            cds_maturity: option_params.cds_maturity,
            notional: option_params.notional,
            settlement: SettlementType::Cash,
            cash_settlement_date: None,
            exercise_settlement_date: None,
            underlying_effective_date: None,
            protection_start_convention: option_params.protection_start_convention,
            knockout: false,
            recovery_rate: credit_params.recovery_rate,
            discount_curve_id: discount_curve_id.into(),
            credit_curve_id: credit_params.credit_curve_id.to_owned(),
            vol_surface_id: vol_surface_id.into(),
            underlying_convention:
                crate::instruments::credit_derivatives::cds::CDSConvention::default(),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
            underlying_is_index: option_params.underlying_is_index,
            index_factor: option_params.index_factor,
            realized_index_loss: None,
            underlying_cds_coupon: option_params.underlying_cds_coupon,
        };
        option.validate()?;
        Ok(option)
    }

    /// Set implied volatility override with validation.
    ///
    /// # Arguments
    ///
    /// * `vol` - Lognormal (Black) volatility in decimal form (e.g., 0.30 for 30%)
    ///
    /// # Errors
    ///
    /// Returns an error if volatility is not positive.
    pub fn with_implied_vol(mut self, vol: f64) -> finstack_quant_core::Result<Self> {
        if vol <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "implied_volatility must be positive, got {}",
                vol
            )));
        }
        if vol > MAX_IMPLIED_VOL {
            return Err(finstack_quant_core::Error::Validation(format!(
                "implied_volatility {} exceeds maximum {}",
                vol, MAX_IMPLIED_VOL
            )));
        }
        self.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(vol);
        Ok(self)
    }

    /// Bloomberg CDSO Black time-to-expiry: calendar days across the option
    /// premium/exercise settlement window, divided by 365.
    ///
    /// Matches the convention published in *Pricing Credit Index Options*
    /// (DOCS 2055833) §2.1 — the lognormal spread process is parameterised
    /// in years and Bloomberg's reference implementation (and FinancePy's
    /// open-source port) hard-codes the 365-day denominator. The day-count
    /// rule that governs the underlying CDS premium-leg accrual (Act/360)
    /// does not apply to option-pricing time-to-expiry — they are separate
    /// quantities.
    pub(crate) fn time_to_expiry(
        &self,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let start = self.effective_cash_settlement_date(as_of)?;
        let end = self.exercise_settlement_date.unwrap_or(self.expiry);
        if end <= start {
            return Ok(0.0);
        }
        let days = (end - start).whole_days() as f64;
        Ok(days / 365.0)
    }

    /// Effective cash-settlement date for the option premium. Defaults to
    /// the underlying CDS convention's settlement lag from the next
    /// business day after `as_of`.
    #[doc(hidden)]
    pub fn effective_cash_settlement_date(&self, as_of: Date) -> finstack_quant_core::Result<Date> {
        if let Some(date) = self.cash_settlement_date {
            return Ok(date);
        }

        let calendar = self.standard_calendar()?;
        let trade_date = adjust(as_of, BusinessDayConvention::Following, calendar)?;
        trade_date.add_business_days(
            self.underlying_convention.settlement_delay().into(),
            calendar,
        )
    }

    /// Effective contractual coupon `c` of the synthetic underlying CDS,
    /// as a decimal rate. Returns the explicitly-set `underlying_cds_coupon`
    /// when present (e.g., the 100 bp standard CDX coupon), otherwise falls
    /// back to `strike` for single-name SNAC trades where the option is
    /// struck at the underlying CDS coupon.
    pub(crate) fn effective_underlying_cds_coupon(&self) -> Decimal {
        self.underlying_cds_coupon.unwrap_or(self.strike)
    }

    /// Effective accrual-start date for the synthetic underlying CDS. When
    /// the user specifies `underlying_effective_date` explicitly we honour
    /// it (e.g. Bloomberg CDSW screen value). Otherwise the typed protection
    /// convention selects either standard spot-protection accrual from the
    /// prior CDS roll relative to valuation date, or forward accrual from
    /// legal option expiry.
    pub(crate) fn effective_underlying_effective_date(&self, as_of: Date) -> Date {
        if let Some(date) = self.underlying_effective_date {
            return date;
        }
        match self.protection_start_convention {
            ProtectionStartConvention::Spot => prior_cds_roll_on_or_before(as_of)
                .saturating_add(time::Duration::days(1))
                .min(as_of),
            ProtectionStartConvention::Forward => self.expiry,
        }
    }

    fn standard_calendar(&self) -> finstack_quant_core::Result<&'static dyn HolidayCalendar> {
        let calendar_id = self.underlying_convention.default_calendar();
        calendar_by_id(calendar_id).ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "missing CDS option calendar '{calendar_id}' for {:?}",
                self.underlying_convention
            ))
        })
    }

    /// Bloomberg CDSO Δ — closed-form Black-76 N(d₁) on the displayed
    /// ATM forward spread (DOCS 2055833 §2.5). Returned as a unit-less
    /// ratio (multiply by 100 for the displayed percentage). Calls
    /// Δ ≥ 0, puts Δ ≤ 0.
    pub fn delta(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        super::metrics::delta::delta(self, curves, as_of)
    }

    /// Bloomberg CDSO Γ — central difference of the Black-76 N(d₁)
    /// delta across a ±5 bp move in the displayed ATM forward (DOCS
    /// 2055833 §2.5). Returned as a unit-less number.
    pub fn gamma(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        super::metrics::gamma::gamma(self, curves, as_of)
    }

    /// Bloomberg CDSO Vega(1%) — one-sided forward difference of the
    /// canonical Bloomberg quadrature NPV on a `+0.01` lognormal-vol
    /// bump (DOCS 2055833 §2.5).
    pub fn vega(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        super::metrics::vega::vega(self, curves, as_of)
    }

    /// Bloomberg CDSO θ: change in option premium for a one-calendar-day
    /// decrease in option maturity (DOCS 2055833 §2.5). Implemented by
    /// shortening the exercise time `t_e` by `1/365.25` and re-pricing
    /// with the same calibrated forward; the year denominator (365.25)
    /// is the Bloomberg convention.
    pub fn theta(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        super::pricer::theta(self, curves, as_of)
    }

    /// Solve for the Bloomberg CDSO implied volatility `σ` that reproduces
    /// the observed `target_price` under the same numerical-quadrature
    /// pricer used for valuation. Brent root finding in log-σ space.
    pub fn implied_vol(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        target_price: f64,
        initial_guess: Option<f64>,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        super::pricer::implied_vol(self, curves, as_of, target_price, initial_guess)
    }
}

pub(crate) fn prior_cds_roll_on_or_before(date: Date) -> Date {
    const CDS_ROLL_MONTHS: [Month; 4] =
        [Month::March, Month::June, Month::September, Month::December];

    for month in CDS_ROLL_MONTHS.iter().rev().copied() {
        if let Ok(candidate) = Date::from_calendar_date(date.year(), month, 20) {
            if candidate <= date {
                return candidate;
            }
        }
    }

    Date::from_calendar_date(date.year().saturating_sub(1), Month::December, 20).unwrap_or(date)
}

impl crate::instruments::common_impl::traits::Instrument for CDSOption {
    impl_instrument_base!(crate::pricer::InstrumentType::CDSOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()?;
        self.validate_supported_configuration()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        use crate::instruments::common_impl::dependencies::VolatilityDependency;
        use rust_decimal::prelude::ToPrimitive;

        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_credit_curve(self.credit_curve_id.clone());
        deps.add_volatility_dependency(VolatilityDependency::new(
            self.vol_surface_id.clone(),
            None,
            self.strike.to_f64(),
        ));
        Ok(deps)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::BloombergCdso
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        super::pricer::npv(self, curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
    }

    crate::impl_focused_pricing_overrides!();
}

// Declare canonical market dependencies for the DV01 calculator.
crate::impl_empty_cashflow_provider!(
    CDSOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    #[test]
    fn cash_settlement_date_defaults_to_t_plus_settle_lag() {
        let option_params = CDSOptionParams::call(
            Decimal::from_str_exact("0.0058395400").expect("valid strike"),
            date!(2026 - 06 - 26),
            date!(2031 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )
        .expect("valid option params");
        let credit_params = CreditParams::corporate_standard("IBM", "IBM-USD-SENIOR");
        let option = CDSOption::new(
            "IBM-USD-CDSO-PAYER-ATM-3M-20260502",
            &option_params,
            &credit_params,
            "USD-S531-SWAP",
            "IBM-CDSO-VOL",
        )
        .expect("valid option");

        // T+3 BD from 2026-05-02 (Sat) is 2026-05-07 (Thu) under the
        // ISDA-NA weekend calendar.
        let as_of = date!(2026 - 05 - 02);
        assert_eq!(
            option
                .effective_cash_settlement_date(as_of)
                .expect("cash settlement date"),
            date!(2026 - 05 - 07)
        );

        // No explicit underlying_effective_date → default Spot convention uses
        // the standard prior CDS roll relative to valuation date.
        assert_eq!(
            option.effective_underlying_effective_date(as_of),
            date!(2026 - 03 - 21)
        );
    }

    #[test]
    fn focused_overrides_preserve_legacy_wire_shape() {
        let option_params = CDSOptionParams::call(
            Decimal::from_str_exact("0.0058395400").expect("valid strike"),
            date!(2026 - 06 - 26),
            date!(2031 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )
        .expect("valid option params");
        let credit_params = CreditParams::corporate_standard("IBM", "IBM-USD-SENIOR");
        let mut option = CDSOption::new(
            "IBM-USD-CDSO-WIRE",
            &option_params,
            &credit_params,
            "USD-S531-SWAP",
            "IBM-CDSO-VOL",
        )
        .expect("valid option");
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.31);
        option.metric_pricing_overrides.mc_seed_scenario = Some("vega_down".to_string());
        option.scenario_pricing_overrides.scenario_spread_shock_bp = Some(8.0);

        let value = serde_json::to_value(&option).expect("serialize focused overrides");
        assert!(value.get("instrument_pricing_overrides").is_none());
        assert!(value.get("metric_pricing_overrides").is_none());
        assert!(value.get("scenario_pricing_overrides").is_none());
        let wire = value
            .get("pricing_overrides")
            .and_then(serde_json::Value::as_object)
            .expect("legacy pricing_overrides object");
        assert_eq!(
            wire.get("implied_volatility"),
            Some(&serde_json::json!(0.31))
        );
        assert_eq!(
            wire.get("mc_seed_scenario"),
            Some(&serde_json::json!("vega_down"))
        );
        assert_eq!(
            wire.get("scenario_spread_shock_bp"),
            Some(&serde_json::json!(8.0))
        );

        let roundtrip: CDSOption = serde_json::from_value(value).expect("deserialize legacy wire");
        assert_eq!(
            roundtrip
                .instrument_pricing_overrides
                .market_quotes
                .implied_volatility,
            Some(0.31)
        );
        assert_eq!(
            roundtrip
                .metric_pricing_overrides
                .mc_seed_scenario
                .as_deref(),
            Some("vega_down")
        );
        assert_eq!(
            roundtrip
                .scenario_pricing_overrides
                .scenario_spread_shock_bp,
            Some(8.0)
        );
    }

    #[test]
    fn index_option_requires_underlying_cds_coupon() {
        let option_params = CDSOptionParams::call(
            Decimal::from_str_exact("0.005").expect("valid strike"),
            date!(2026 - 06 - 26),
            date!(2031 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )
        .expect("valid option params")
        .as_index(1.0)
        .expect("valid index factor");
        let credit_params = CreditParams::corporate_standard("CDX", "CDX-IG");

        let err = CDSOption::new(
            "CDX-CDSO-MISSING-COUPON",
            &option_params,
            &credit_params,
            "USD-S531-SWAP",
            "CDX-CDSO-VOL",
        )
        .expect_err("index option without contractual coupon should fail");

        assert!(
            err.to_string().contains("underlying_cds_coupon"),
            "error should point to missing underlying_cds_coupon: {err}"
        );
    }

    #[test]
    fn pricing_boundary_rejects_non_finite_and_unsupported_terms() {
        let mut option = CDSOption::example().expect("example");
        option.recovery_rate = f64::NAN;
        assert!(option
            .validate_for_pricing()
            .expect_err("NaN recovery must fail")
            .to_string()
            .contains("finite"));

        option.recovery_rate = 0.4;
        option.exercise_style = ExerciseStyle::American;
        assert!(option
            .validate_for_pricing()
            .expect_err("American CDS option must fail")
            .to_string()
            .contains("European"));
    }
}
