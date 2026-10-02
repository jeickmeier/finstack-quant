//! Inflation cap/floor instrument and pricing logic.
//!
//! Prices YoY inflation caps/floors using Black-76 (lognormal) or
//! Bachelier (normal) on the forward YoY inflation rate.
//!
//! # Inflation Rate Convention
//!
//! This module computes **period inflation rates** based on the schedule's accrual periods:
//!
//! ```text
//! forward_rate = (CPI_end / CPI_start - 1) / accrual_fraction
//! ```
//!
//! For annual frequency, this equals the true Year-over-Year (YoY) rate. For other
//! frequencies (semi-annual, quarterly), the rate is annualized over the shorter period.
//!
//! **Important**: If you need true YoY rates regardless of payment frequency (i.e.,
//! `CPI(T) / CPI(T - 1 year) - 1`), ensure the schedule uses annual frequency or
//! adjust the CPI observation dates accordingly.
//!
//! # Volatility Convention
//!
//! The volatility surface must match the pricing model convention:
//! - **Black-76 (lognormal)**: Vol surface should contain lognormal vols (percentage of rate)
//! - **Bachelier (normal)**: Vol surface should contain normal vols (absolute rate terms)
//!
//! Both models take the CPI-curve ratio as their payment-measure forward input.
//! With a published denominator this is the matching linear CPI payoff forward.
//! With a future denominator it is an explicit deterministic-ratio approximation;
//! no joint stochastic CPI/rate convexity correction is inferred from option vols.
//! CPI forwards must be supplied consistently with the cash-payment measure;
//! this quote-based engine does not perform a stochastic change of measure.
//!
//! # Observation Lag
//!
//! Inflation indices typically have an observation lag (e.g., 3 months for US CPI).
//! The lag determines CPI reference months. Publication dates independently
//! determine when those values are known. The selected volatility-expiry convention
//! determines the ACT/365F clock on which annualized option volatility is quoted.

use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::common_impl::vol_resolution::{
    resolve_volatility, ResolvedVolatility, VolatilityRequest,
};
use crate::instruments::rates::cap_floor::pricing::payoff::CapletFloorletInputs;
use crate::instruments::rates::cap_floor::pricing::pricer::price_caplet_quote;
use crate::instruments::rates::cap_floor::RateOptionType;
use crate::pricer::ModelKey;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::volatility::VolatilityConvention;
use rust_decimal::Decimal;

/// Expiry convention of the annualized inflation-option volatility surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum InflationVolatilityExpiry {
    /// ACT/365F time to the lagged contractual end date. If CPI is still
    /// unpublished after this date, the quote clock is inconsistent and pricing
    /// fails instead of treating the unknown payoff as deterministic.
    #[default]
    ReferenceDate,
    /// ACT/365F time to publication of the final required CPI anchor. Requires
    /// explicit publication dates for every outstanding monthly anchor and
    /// a volatility surface quoted on this publication-expiry clock.
    PublicationDate,
}

/// YoY inflation cap/floor instrument.
#[derive(
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InflationCapFloor {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Cap/floor type (cap, floor, caplet, floorlet). Caplet and floorlet price a
    /// single period.
    pub rate_option_type: RateOptionType,
    /// Notional amount in quote currency.
    pub notional: Money,
    /// Strike (annualized, decimal).
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub strike: Decimal,
    /// Start date of the first inflation period.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start_date: Date,
    /// End date of the final inflation period.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Payment frequency (ignored for caplet/floorlet).
    pub frequency: Tenor,
    /// Day count convention for accrual. Option quote time always uses ACT/365F.
    pub day_count: DayCount,
    /// Schedule stub convention.
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Business day convention for schedule and payments.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional holiday calendar identifier.
    #[builder(optional)]
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Inflation index/curve identifier (e.g., US-CPI-U).
    pub inflation_index_id: CurveId,
    /// Discount curve identifier.
    pub discount_curve_id: CurveId,
    /// Volatility surface identifier.
    pub vol_surface_id: CurveId,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Contractual CPI observation lag; when `None` the index lag, then the
    /// curve's indexation lag, applies.
    #[builder(optional)]
    pub lag: Option<InflationLag>,
    /// Contractual monthly CPI interpolation; takes precedence over index metadata.
    /// Defaults to monthly step interpolation when neither source supplies it.
    #[builder(optional)]
    pub interpolation: Option<finstack_quant_core::market_data::scalars::InflationInterpolation>,
    /// Clock used by the volatility surface's annualized quotes. This is
    /// independent of CPI publication policy; changing it requires matching
    /// quotes, not merely relabelling the old surface.
    #[builder(default)]
    #[serde(default)]
    pub volatility_expiry: InflationVolatilityExpiry,

    /// Attributes for scenario selection and tagging.
    pub attributes: Attributes,
}

impl InflationCapFloor {
    /// Create a canonical example USD 5Y inflation cap (US-CPI, 3% strike, $1M notional).
    ///
    /// Returns a 5-year YoY inflation cap with annual frequency, 3-month CPI lag,
    /// and lognormal vol convention.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;

        InflationCapFloor::builder()
            .id(InstrumentId::new("INFLCAP-USD-5Y"))
            .rate_option_type(RateOptionType::Cap)
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .strike(Decimal::try_from(0.03).map_err(|e: rust_decimal::Error| {
                finstack_quant_core::Error::Validation(e.to_string())
            })?)
            .start_date(time::macros::date!(2024 - 01 - 15))
            .maturity(time::macros::date!(2029 - 01 - 15))
            .frequency(Tenor::annual())
            .day_count(DayCount::Act365F)
            .stub(StubKind::ShortFront)
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("USD-INFL-VOL"))
            .lag(InflationLag::Months(3))
            .attributes(Attributes::new())
            .build()
    }

    /// Validate structural invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        validation::require_or(
            self.start_date < self.maturity,
            finstack_quant_core::InputError::InvalidDateRange,
        )?;
        validation::require_or(
            self.notional.amount() > 0.0,
            finstack_quant_core::InputError::NonPositiveValue,
        )?;
        validation::require_or(
            self.frequency.count() != 0,
            finstack_quant_core::InputError::Invalid,
        )?;
        Ok(())
    }

    pub(crate) fn strike_f64(&self) -> finstack_quant_core::Result<f64> {
        decimal_to_f64(self.strike, "InflationCapFloor strike")
    }

    /// Minimum reasonable CPI value for developed market indices.
    /// Used to catch data errors that could cause numerical instability.
    const MIN_REASONABLE_CPI: f64 = 50.0;

    fn effective_lag(&self, curves: &MarketContext) -> InflationLag {
        crate::instruments::common_impl::helpers::resolve_inflation_lag(
            self.lag,
            self.inflation_index_id.as_str(),
            curves,
        )
    }

    fn lagged_fixing_date(
        &self,
        curves: &MarketContext,
        date: Date,
    ) -> finstack_quant_core::Result<Date> {
        crate::instruments::common_impl::helpers::apply_inflation_lag(
            date,
            self.effective_lag(curves),
        )
    }

    fn cpi_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
        date: Date,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::helpers::InflationReferenceValue,
    > {
        let interpolation = self
            .interpolation
            .or_else(|| {
                curves
                    .get_inflation_index(self.inflation_index_id.as_str())
                    .ok()
                    .map(|index| index.interpolation())
            })
            .unwrap_or_default();
        let resolved = crate::instruments::common_impl::helpers::resolve_reference_inflation(
            curves,
            self.inflation_index_id.as_str(),
            date,
            as_of,
            self.effective_lag(curves),
            interpolation,
        )?;
        Self::validate_cpi_value(resolved.value, date)?;
        Ok(resolved)
    }

    /// Validate that a CPI value is reasonable and won't cause numerical issues.
    fn validate_cpi_value(value: f64, date: Date) -> finstack_quant_core::Result<f64> {
        if value <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CPI value must be positive; got {:.4} on {}",
                value, date
            )));
        }
        if value < Self::MIN_REASONABLE_CPI {
            tracing::warn!(
                cpi = value,
                date = %date,
                min_reasonable = Self::MIN_REASONABLE_CPI,
                "CPI value is below minimum reasonable threshold for developed markets; \
                 this may indicate a data error"
            );
        }
        Ok(value)
    }

    fn schedule(&self) -> finstack_quant_core::Result<Vec<(Date, Date, Date)>> {
        if matches!(
            self.rate_option_type,
            RateOptionType::Caplet | RateOptionType::Floorlet
        ) {
            let pay = crate::cashflow::builder::calendar::adjust_date(
                self.maturity,
                self.business_day_convention,
                self.calendar_id
                    .as_deref()
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
            )?;
            return Ok(vec![(self.start_date, self.maturity, pay)]);
        }

        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: self.start_date,
                end: self.maturity,
                frequency: self.frequency,
                stub: self.stub,
                business_day_convention: self.business_day_convention,
                calendar_id: self
                    .calendar_id
                    .as_deref()
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
                end_of_month: false,
                day_count: self.day_count,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        )?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        Ok(periods
            .into_iter()
            .map(|period| {
                (
                    period.accrual_start,
                    period.accrual_end,
                    period.payment_date,
                )
            })
            .collect())
    }

    /// Price using an explicit model key (Black-76 or Normal).
    ///
    /// # Model Selection
    ///
    /// - **Black-76**: Standard for positive inflation expectations. Requires `forward > 0` and `strike > 0`.
    /// - **Normal (Bachelier)**: Use when deflation is possible or strike is at/below zero.
    ///
    /// # Arguments
    ///
    /// * `market` - CPI forward curve, observed index history, discount curve,
    ///   and option quotes matching `model` and `volatility_expiry`.
    /// * `as_of` - Valuation date and inclusive observation-availability cutoff.
    /// * `model` - `Black76` for lognormal rate quotes or `Normal` for absolute
    ///   rate quotes. Other model keys are rejected.
    pub fn npv_with_model(
        &self,
        market: &MarketContext,
        as_of: Date,
        model: ModelKey,
    ) -> finstack_quant_core::Result<Money> {
        let pv = self.npv_raw_with_model(market, as_of, model)?;
        Money::new(pv, self.notional.currency())
    }

    /// Raw (unrounded `f64`) present value, used by finite-difference metrics
    /// (gamma) where Money quantization noise would be amplified by tiny bump
    /// sizes.
    ///
    /// # Arguments
    ///
    /// * `market` - CPI forward curve and history, discount curve, and volatility
    ///   quotes with the selected model and expiry convention.
    /// * `as_of` - Valuation date; cashflows on or before it have settled.
    /// * `model` - `Black76` or `Normal`, selecting the quote and payoff convention.
    pub fn npv_raw_with_model(
        &self,
        market: &MarketContext,
        as_of: Date,
        model: ModelKey,
    ) -> finstack_quant_core::Result<f64> {
        let convention = match model {
            ModelKey::Normal => VolatilityConvention::Normal,
            ModelKey::Black76 => VolatilityConvention::Lognormal,
            _ => {
                return Err(finstack_quant_core::Error::Validation(
                    "inflation cap/floor requires Normal or Black76 pricing".to_owned(),
                ))
            }
        };
        let strike = self.strike_f64()?;
        let disc = market.get_discount(self.discount_curve_id.as_str())?;

        let mut total_pv = 0.0_f64;

        for (start, end, pay) in self.schedule()? {
            if pay <= as_of {
                continue;
            }

            let accrual = self
                .day_count
                .year_fraction(start, end, DayCountContext::default())?;
            validation::validate_f64_positive(accrual, "YoY inflation accrual year fraction")?;

            // CPI values are validated inside cpi_value()
            let cpi_start = self.cpi_value(market, as_of, start)?;
            let cpi_end = self.cpi_value(market, as_of, end)?;

            // Deterministic forward YoY ratio from the CPI curve. The YoY
            // *rate* over the period is `ratio − 1`, and the rate the option is
            // written on (annualized) is `(ratio − 1) / accrual`.
            let deterministic_ratio = cpi_end.value / cpi_start.value;
            validation::validate_f64_positive(deterministic_ratio, "YoY forward CPI ratio")?;
            let deterministic_rate = (deterministic_ratio - 1.0) / accrual;

            let all_known = cpi_start.is_known && cpi_end.is_known;
            let t_fix = if all_known {
                0.0
            } else {
                let expiry = match self.volatility_expiry {
                    InflationVolatilityExpiry::ReferenceDate => {
                        self.lagged_fixing_date(market, end)?
                    }
                    InflationVolatilityExpiry::PublicationDate => {
                        let mut last_publication = as_of;
                        for reference in [&cpi_start, &cpi_end] {
                            if !reference.is_known {
                                let publication = reference.publication_date.ok_or_else(|| {
                                    finstack_quant_core::Error::Validation(
                                        "publication-date inflation volatility requires explicit publication dates for every unpublished CPI anchor".into(),
                                    )
                                })?;
                                last_publication = last_publication.max(publication);
                            }
                        }
                        last_publication
                    }
                };
                let time = DayCount::Act365F.signed_year_fraction(
                    as_of,
                    expiry,
                    DayCountContext::default(),
                )?;
                if time <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(
                        "CPI is still unpublished after the configured volatility expiry; supply a publication-expiry quote surface and select publication_date".into(),
                    ));
                }
                time
            };

            // Date-based DF from as_of to payment: correct when the curve base
            // date differs from as_of.
            let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
                disc.as_ref(),
                as_of,
                pay,
            )?;

            // Volatility at the option strike (smile) prices the Black-76 /
            // Bachelier payoff.
            let resolve = |strike| {
                resolve_volatility(
                    &self.instrument_pricing_overrides.market_quotes,
                    market,
                    self.vol_surface_id.as_str(),
                    VolatilityRequest {
                        expiry: t_fix,
                        tenor: 0.0,
                        strike,
                        convention: Some(convention),
                        clamp: true,
                    },
                )
            };
            let quote = if t_fix > 0.0 {
                resolve(strike)?
            } else {
                ResolvedVolatility {
                    sigma: 0.0,
                    convention: VolatilityConvention::Normal,
                }
            };

            let inputs = CapletFloorletInputs {
                is_cap: self.rate_option_type.is_cap(),
                notional: self.notional.amount(),
                strike,
                // A known denominator gives the matching linear CPI forward.
                // Future denominators use the documented ratio approximation;
                // option quote volatility never changes the forward itself.
                forward: deterministic_rate,
                discount_factor: df,
                sigma: quote.sigma,
                time_to_fixing: t_fix,
                accrual_year_fraction: accrual,
                currency: self.notional.currency(),
            };
            let leg_pv = price_caplet_quote(inputs, quote)?;

            total_pv += leg_pv.amount();
        }

        Ok(total_pv)
    }
}

impl InflationCapFloorBuilder {}

impl crate::instruments::common_impl::traits::Instrument for InflationCapFloor {
    impl_instrument_base!(crate::pricer::InstrumentType::InflationCapFloor);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_inflation_curve(self.inflation_index_id.clone());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                None,
                Some(self.strike_f64()?),
            ),
        );
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.npv_with_model(curves, as_of, crate::pricer::ModelKey::Black76)
    }

    fn base_value_raw(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw_with_model(curves, as_of, crate::pricer::ModelKey::Black76)
    }

    fn base_value_raw_with_currency(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        Ok((
            self.npv_raw_with_model(curves, as_of, crate::pricer::ModelKey::Black76)?,
            self.notional.currency(),
        ))
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.start_date)
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    InflationCapFloor,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);
