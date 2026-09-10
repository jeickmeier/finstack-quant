//! `CDSOption` instrument: European option to enter a forward CDS at a
//! typed strike — a forward spread, or a clean index price (CDX HY
//! convention).
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
//! `CDSOption::new` validates all inputs at construction time:
//! - Spread strikes must be positive and within the distressed-credit bound;
//!   clean-price strikes must be positive percentage points (values above
//!   100 are valid)
//! - Clean-price strikes require an index underlying, no-knockout terms, an
//!   explicit positive coupon, both index factors in (0, 1] with
//!   `f <= f0`, and realized loss bounded by the removed original notional
//! - Option expiry must precede underlying CDS maturity
//! - Recovery rate must be in (0, 1)
//! - Implied volatility override must be in (0, 5] when specified
//! - Only European exercise is supported; settlement may be cash or physical
//!
//! # Volatility convention
//!
//! Volatilities are lognormal (Black) forward-spread model volatilities in
//! decimal form (e.g. 0.30 for 30%) for both strike conventions. The
//! Bloomberg CDSO terminal expects the same.

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
/// Numerical tolerance for index factor/loss consistency checks. Factors and
/// realized losses are market data quoted to ~6 decimal places; the tolerance
/// absorbs the resulting rounding without admitting economically inconsistent
/// state.
pub(crate) const FACTOR_TOLERANCE: f64 = 1e-6;

/// Accrual-start convention for the synthetic underlying CDS used by CDSO.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProtectionStartConvention {
    /// Spot-protection CDS: standard prior CDS roll relative to valuation date.
    #[default]
    Spot,
    /// Forward-protection CDS: accrual starts at option expiry.
    Forward,
}

/// Credit option instrument (option on CDS spread or clean index price)
///
/// The public pricing surface supports European options with cash or
/// physical settlement. Before expiry, cash- and physical-settled options
/// carry the same cash-equivalent model NPV and route through the same
/// quadrature; the clean payoff excludes accrued because the same underlying
/// accrued appears on both sides before exercise and cancels. A physical
/// exercise cashflow at settlement is dirty (includes accrued at exercise
/// settlement), and this pricer does not create or deliver a live underlying
/// CDS position — valuation at or after a physical exercise boundary fails
/// explicitly. Non-European exercise is rejected at pricing time so
/// deserialized instruments cannot silently fall through to an unsupported
/// engine.
#[derive(
    PartialEq,
    Debug,
    Clone,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CDSOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Typed option strike: a decimal forward spread (`{"spread": "0.0325"}`)
    /// or a clean price in percentage points (`{"clean_price_pct": "107.0"}`).
    pub strike: super::strike::CDSOptionStrike,
    /// Option type (Call = right to buy protection, Put = right to sell protection)
    pub option_type: OptionType,
    /// Exercise style
    pub exercise_style: ExerciseStyle,
    /// Option expiry date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Underlying CDS maturity date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub cds_maturity: Date,
    /// Notional amount
    pub notional: Money,
    /// Settlement type
    pub settlement: SettlementType,
    /// Payment date of the option premium, returned by the settlement-date
    /// accessor. This does not change variance time or front-end protection;
    /// the option value excludes the separately agreed trade premium.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    #[builder(default)]
    pub cash_settlement_date: Option<Date>,
    /// Payment date of the exercise proceeds, defaulting to legal expiry.
    /// Must be on or after expiry and before CDS maturity. Discounting uses
    /// this date; spread variance ends at legal expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    #[builder(default)]
    pub exercise_settlement_date: Option<Date>,
    /// Underlying CDS accrual-effective date used for forward spread and risky
    /// annuity. Bloomberg CDSO can quote a standard CDS effective date before
    /// option expiry; in that case premium accrues from this date while
    /// protection starts at expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
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
    pub underlying_convention: crate::instruments::credit_derivatives::cds::CdsConvention,
    /// Instrument-owned pricing overrides (including implied volatility).
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only valuation adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
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
    ///
    /// This is the **current** index factor `f` at valuation. See
    /// [`Self::strike_index_factor`] for the original factor `f0` attached
    /// to a clean-price strike.
    pub index_factor: Option<f64>,
    /// Original index factor `f0` attached to the option strike.
    ///
    /// Distinct from [`Self::index_factor`], which is the current factor
    /// `f` at valuation: defaults settled between option strike and
    /// valuation reduce `f` below `f0`. A clean-price strike quotes the
    /// price on `f0` notional, so its deterministic payoff term scales by
    /// `f0 / f`; the strike factor is therefore required for clean-price
    /// strikes (`CDSOptionStrike::CleanPricePct`) and must not be inferred
    /// from the current factor after a default. Rejected for spread
    /// strikes, whose payoff does not reference it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub strike_index_factor: Option<f64>,
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
    #[serde(with = "finstack_quant_core::wire::optional_decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DecimalWire>")
    )]
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

        // Cash and physical settlement carry the same pre-expiry
        // cash-equivalent model value; both route through the same
        // quadrature. Post-expiry physical exercise lifecycle is rejected in
        // the pricer's valuation-date guard.
        Ok(())
    }

    /// Validate the CDSOption parameters.
    fn validate(&self) -> finstack_quant_core::Result<()> {
        use crate::instruments::common_impl::validation;

        super::parameters::validate_common_terms(
            &self.strike,
            self.expiry,
            self.cds_maturity,
            self.index_factor,
        )?;
        self.validate_strike_state()?;
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
            if exercise_settlement < self.expiry {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "exercise_settlement_date ({exercise_settlement}) must be on or after legal expiry ({})", self.expiry,
                )));
            }
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
        if let Some(coupon) = self.underlying_cds_coupon {
            if coupon <= Decimal::ZERO {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "underlying_cds_coupon must be positive when set, got {coupon}"
                )));
            }
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

    /// Validate index-state requirements that depend on the strike kind.
    ///
    /// Clean-price strikes require the full index state — index underlying,
    /// no-knockout, both factors — because the deterministic payoff term
    /// `ξ (K − 1) f0 / f` and the realized-loss settlement reference it
    /// directly. Spread strikes reject `strike_index_factor` as an inert
    /// input.
    fn validate_strike_state(&self) -> finstack_quant_core::Result<()> {
        use super::strike::CDSOptionStrike;

        match &self.strike {
            CDSOptionStrike::Spread(_) => {
                if self.strike_index_factor.is_some() {
                    return Err(finstack_quant_core::Error::Validation(
                        "strike_index_factor is only meaningful for clean-price strikes; \
                         remove it from this spread-struck option"
                            .to_string(),
                    ));
                }
                Ok(())
            }
            CDSOptionStrike::CleanPricePct(_) => {
                if !self.underlying_is_index {
                    return Err(finstack_quant_core::Error::Validation(
                        "clean-price strikes are an index-option convention; \
                         set underlying_is_index = true"
                            .to_string(),
                    ));
                }
                if self.knockout {
                    return Err(finstack_quant_core::Error::Validation(
                        "clean-price-struck index options must be no-knockout".to_string(),
                    ));
                }
                let Some(f0) = self.strike_index_factor else {
                    return Err(finstack_quant_core::Error::Validation(
                        "strike_index_factor (original factor f0) is required for \
                         clean-price strikes and must not be inferred from the \
                         current index_factor"
                            .to_string(),
                    ));
                };
                if !f0.is_finite() || f0 <= 0.0 || f0 > 1.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "strike_index_factor must be finite and in (0, 1], got {f0}"
                    )));
                }
                let Some(f) = self.index_factor else {
                    return Err(finstack_quant_core::Error::Validation(
                        "index_factor (current factor f) is required for \
                         clean-price strikes"
                            .to_string(),
                    ));
                };
                if f > f0 + FACTOR_TOLERANCE {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "current index_factor {f} exceeds strike_index_factor {f0}; \
                         the factor cannot rise after defaults"
                    )));
                }
                let loss = self.realized_index_loss.unwrap_or(0.0);
                let removed_notional = (f0 - f).max(0.0);
                if loss > removed_notional + FACTOR_TOLERANCE {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "realized_index_loss {loss} exceeds removed original notional \
                         {removed_notional} (f0 − f = {f0} − {f}); loss cannot exceed \
                         the defaulted weight"
                    )));
                }
                Ok(())
            }
        }
    }

    /// Create a canonical example CDS option (call on CDS spread).
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use time::macros::date;
        let option_params = CDSOptionParams::call(
            super::strike::CDSOptionStrike::Spread(Decimal::new(1, 2)), // 0.01 = 100bp
            date!(2025 - 06 - 20),
            date!(2030 - 06 - 20),
            Money::from((10_000_000_i64, Currency::USD)),
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
            settlement: option_params.settlement,
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
                crate::instruments::credit_derivatives::cds::CdsConvention::default(),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
            underlying_is_index: option_params.underlying_is_index,
            index_factor: option_params.index_factor,
            strike_index_factor: option_params.strike_index_factor,
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

    /// Actual/365F time from current valuation date to legal option expiry.
    /// Premium and exercise settlement dates govern cash payments, not the
    /// interval over which spread variance accumulates.
    pub(crate) fn time_to_expiry(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        Ok(((self.expiry - as_of).whole_days() as f64 / 365.0).max(0.0))
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
    /// back to the strike spread for single-name SNAC trades where the
    /// option is struck at the underlying CDS coupon.
    ///
    /// # Errors
    ///
    /// A clean-price strike has no spread representation, so it can never
    /// serve as the running coupon: the coupon must be explicit for
    /// price-struck options (validation enforces this for index options,
    /// which price strikes always are).
    pub(crate) fn effective_underlying_cds_coupon(&self) -> finstack_quant_core::Result<Decimal> {
        if let Some(coupon) = self.underlying_cds_coupon {
            return Ok(coupon);
        }
        match &self.strike {
            super::strike::CDSOptionStrike::Spread(s) => Ok(*s),
            super::strike::CDSOptionStrike::CleanPricePct(p) => {
                Err(finstack_quant_core::Error::Validation(format!(
                    "CDS option '{}' has clean-price strike {p} and no explicit \
                     underlying_cds_coupon; a price strike cannot serve as the \
                     running coupon",
                    self.id
                )))
            }
        }
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

    /// CDS option Δ, branched by strike kind: closed-form Black-76 N(d₁)
    /// on the displayed ATM forward spread for spread strikes (DOCS
    /// 2055833 §2.5), and the curve-reprice hedge ratio
    /// `option_CS01 / underlying_spread_DV01` for clean-price strikes.
    /// Returned as a unit-less ratio (multiply by 100 for the displayed
    /// percentage). Calls Δ ≥ 0, puts Δ ≤ 0.
    pub fn delta(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        super::metrics::delta::delta(self, curves, as_of)
    }

    /// CDS option Γ, branched by strike kind: central difference of the
    /// Black-76 N(d₁) delta across a ±5 bp move in the displayed ATM
    /// forward for spread strikes (DOCS 2055833 §2.5), and the change in
    /// the curve-reprice hedge-ratio delta under the same ±5 bp par-quote
    /// spread bump for clean-price strikes. Returned as a unit-less
    /// number.
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
    impl_instrument_base!(crate::pricer::InstrumentType::CdsOption);

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

        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_credit_curve(self.credit_curve_id.clone());
        deps.add_volatility_dependency(VolatilityDependency::new(
            self.vol_surface_id.clone(),
            None,
            Some(self.strike.native_surface_coordinate()?),
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
    use crate::instruments::credit_derivatives::cds_option::CDSOptionStrike;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    #[test]
    fn cash_settlement_date_defaults_to_t_plus_settle_lag() {
        let option_params = CDSOptionParams::call(
            CDSOptionStrike::Spread(Decimal::from_str_exact("0.0058395400").expect("valid strike")),
            date!(2026 - 06 - 26),
            date!(2031 - 06 - 20),
            Money::from((10_000_000_i64, Currency::USD)),
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
    fn index_option_requires_underlying_cds_coupon() {
        let option_params = CDSOptionParams::call(
            CDSOptionStrike::Spread(Decimal::from_str_exact("0.005").expect("valid strike")),
            date!(2026 - 06 - 26),
            date!(2031 - 06 - 20),
            Money::from((10_000_000_i64, Currency::USD)),
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

    #[test]
    fn production_cds_option_audit_exercise_payment_not_before_expiry() {
        let mut option = CDSOption::example().expect("example");
        option.exercise_settlement_date = Some(option.expiry - time::Duration::days(1));
        let error = option.validate().expect_err("cannot pay before exercise");
        assert!(error.to_string().contains("on or after legal expiry"));
    }
}
