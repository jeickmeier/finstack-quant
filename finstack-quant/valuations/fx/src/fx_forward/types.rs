//! FX Forward types and implementations.
//!
//! Defines the `FxForward` instrument for outright forward contracts on
//! currency pairs. Pricing uses covered interest rate parity (CIRP) with
//! optional contract rate override.

use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::Result;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::validation;
use finstack_quant_valuations_core::impl_instrument_base;
use time::macros::date;

/// FX forward (outright forward) instrument.
///
/// Represents a commitment to exchange one currency for another at a specified
/// future date at a predetermined rate. The position is long base currency
/// (foreign) and short quote currency (domestic).
///
/// # Pricing
///
/// Forward value is calculated using covered interest rate parity:
/// ```text
/// F_market = S × DF_foreign(T) / DF_domestic(T)
/// PV = notional × (F_market - F_contract) × DF_domestic(T)
/// ```
/// where:
/// - S = spot FX rate (from FxMatrix or quoted_spot)
/// - DF_foreign(T) = discount factor in base currency to maturity
/// - DF_domestic(T) = discount factor in quote currency to maturity
/// - F_contract = contract_rate (if provided, else F_market for at-market forward)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations_fx::fx_forward::FxForward;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let forward = FxForward::builder()
///     .id(InstrumentId::new("EURUSD-FWD-6M"))
///     .base_currency(Currency::EUR)
///     .quote_currency(Currency::USD)
///     .maturity(Date::from_calendar_date(2025, Month::June, 15).unwrap())
///     .notional(Money::from((1_000_000_i64, Currency::EUR)))
///     .domestic_discount_curve_id(CurveId::new("USD-OIS"))
///     .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
///     .build()
///     .expect("Valid forward");
/// ```
#[derive(
    Clone,
    Debug,
    PartialEq,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, remote = "Self")]
pub struct FxForward {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Base currency (foreign currency, numerator of the pair).
    pub base_currency: Currency,
    /// Quote currency (domestic currency, denominator of the pair, PV currency).
    pub quote_currency: Currency,
    /// Maturity/settlement date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Notional amount in base currency.
    pub notional: Money,
    /// Contract forward rate (quote per base). If None, valued at-market.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_rate: Option<f64>,
    /// Domestic (quote currency) discount curve ID.
    pub domestic_discount_curve_id: CurveId,
    /// Foreign (base currency) discount curve ID.
    pub foreign_discount_curve_id: CurveId,
    /// Optional spot rate override (quote per base). If None, source from FxMatrix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quoted_spot: Option<f64>,
    /// Optional base currency calendar for business day adjustment.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Optional quote currency calendar for business day adjustment.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

finstack_quant_valuations_core::validate_on_deserialize!(FxForward);

/// Inputs of [`FxForward::from_trade_date`].
///
/// Every date convention is explicit (record CORE-CONVENTIONS-011 #6, #7):
/// the spot lag, both calendars, the business-day convention and
/// business-EOM preservation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FxForwardTradeDateParams {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Base (foreign) currency: the numerator of the quote and the notional currency.
    pub base_currency: Currency,
    /// Quote (domestic) currency: the denominator of the quote and the PV currency.
    pub quote_currency: Currency,
    /// Trade date from which the spot date is rolled.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub trade_date: Date,
    /// Calendar tenor from spot to maturity, e.g. `3M`.
    pub tenor: Tenor,
    /// Notional in `base_currency`.
    pub notional: Money,
    /// Quote-currency (domestic) discount curve identifier.
    pub domestic_discount_curve_id: CurveId,
    /// Base-currency (foreign) discount curve identifier.
    pub foreign_discount_curve_id: CurveId,
    /// Base-currency holiday calendar id (may itself be a composite such as
    /// `"target2+usny"` when the convention needs a USD gate).
    pub base_calendar_id: String,
    /// Quote-currency holiday calendar id.
    pub quote_calendar_id: String,
    /// Spot lag: business days from trade date to spot on the joint
    /// base/quote calendar. Stated by the trade's convention; no pair table
    /// supplies it.
    pub settlement_days: u32,
    /// Business-day convention rolling the maturity once on the joint
    /// calendar.
    pub business_day_convention: finstack_quant_core::dates::BusinessDayConvention,
    /// Keep a business month-end spot on the business month-end of the
    /// maturity month; month and year tenors only.
    pub preserve_business_eom: bool,
}

impl FxForward {
    /// Validate the FX forward parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `base_currency` equals `quote_currency` (must be different currencies)
    /// - `notional.currency()` does not match `base_currency`
    /// - `contract_rate` is provided but is not positive
    /// - `quoted_spot` is provided but is not positive
    pub fn validate(&self) -> Result<()> {
        validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FX forward",
        )?;

        // Notional must be in base currency
        validation::require_with(self.notional.currency() == self.base_currency, || {
            format!(
                "FX forward notional currency ({}) must match base_currency ({})",
                self.notional.currency(),
                self.base_currency
            )
        })?;
        validation::validate_money_gt(self.notional, 0.0, "FX forward notional")?;

        // Contract rate must be positive if provided
        if let Some(rate) = self.contract_rate {
            validation::require_with(rate > 0.0, || {
                format!("FX forward contract_rate must be positive, got {}", rate)
            })?;
            validation::require_with(rate.is_finite(), || {
                "FX forward contract_rate must be finite".to_string()
            })?;
        }

        // Spot rate override must be positive if provided
        if let Some(rate) = self.quoted_spot {
            validation::require_with(rate > 0.0, || {
                format!("FX forward quoted_spot must be positive, got {}", rate)
            })?;
            validation::require_with(rate.is_finite(), || {
                "FX forward quoted_spot must be finite".to_string()
            })?;
        }

        Ok(())
    }

    /// Create a canonical example FX forward for testing and documentation.
    ///
    /// Returns a 6-month EUR/USD forward with realistic parameters.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("EURUSD-FWD-6M"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .maturity(date!(2025 - 06 - 15))
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .contract_rate_opt(Some(1.12))
            .attributes(Attributes::new().with_tag("fx").with_meta("pair", "EURUSD"))
            .build()
    }

    /// Construct an FX forward from trade date and tenor using joint calendar spot roll.
    ///
    /// Spot is `trade_date` rolled forward by the T+N spot lag on the joint
    /// base/quote calendar (CLS-consistent: a USD holiday on an intermediate
    /// day does not delay a USD pair's spot); maturity is spot plus `tenor`
    /// under the FX end-of-month rule. The forward has no contract rate; chain
    /// [`Self::with_forward_points`] or [`Self::with_forward_pips`] to fix one.
    ///
    /// # Arguments
    ///
    /// * `params` - Identifier, currency pair, trade date, tenor, notional,
    ///   curves and the optional spot-roll conventions; see
    ///   [`FxForwardTradeDateParams`] for each field's default.
    ///
    /// # Errors
    ///
    /// Returns an error if a calendar id is unknown, the dates cannot be
    /// rolled, or the forward fails validation (same currencies, notional not
    /// in `base_currency`).
    pub fn from_trade_date(params: FxForwardTradeDateParams) -> finstack_quant_core::Result<Self> {
        let FxForwardTradeDateParams {
            id,
            base_currency,
            quote_currency,
            trade_date,
            tenor,
            notional,
            domestic_discount_curve_id,
            foreign_discount_curve_id,
            base_calendar_id,
            quote_calendar_id,
            settlement_days,
            business_day_convention,
            preserve_business_eom,
        } = params;
        let calendar = finstack_quant_core::dates::resolve_joint_calendar(
            Some(&base_calendar_id),
            Some(&quote_calendar_id),
        )?;
        let spot_date = crate::shared::spot_date_from_lag(trade_date, settlement_days, calendar)?;
        let maturity = finstack_quant_core::dates::fx::fx_forward_date(
            spot_date,
            tenor,
            business_day_convention,
            preserve_business_eom,
            calendar,
        )?;

        Self::builder()
            .id(id)
            .base_currency(base_currency)
            .quote_currency(quote_currency)
            .maturity(maturity)
            .notional(notional)
            .domestic_discount_curve_id(domestic_discount_curve_id)
            .foreign_discount_curve_id(foreign_discount_curve_id)
            .base_calendar_id_opt(Some(base_calendar_id.into()))
            .quote_calendar_id_opt(Some(quote_calendar_id.into()))
            .attributes(Attributes::new())
            .build()
    }

    /// Create an FX forward with forward points instead of outright rate.
    ///
    /// Forward points represent the interest rate differential between the two
    /// currencies and are added to the spot rate to obtain the forward rate.
    ///
    /// # Market Convention
    ///
    /// In the FX market, forward points are typically quoted in "pips" (1/10000
    /// for most pairs). For example, for EUR/USD:
    /// - Market quote: "50 pips" or "+50"
    /// - Decimal value: 0.0050 (50 × 0.0001)
    ///
    /// This method expects forward points in **decimal form**, not pip form.
    /// To convert from pips: `forward_points = pips × pip_size` where
    /// `pip_size = 0.0001` for most pairs (0.01 for JPY pairs).
    ///
    /// # Arguments
    ///
    /// * `spot_rate` - Current spot rate (quote per base)
    /// * `forward_points` - Forward points in decimal form (e.g., 0.0050 for 50 pips
    ///   on a standard pair, or 0.50 for 50 pips on a JPY pair)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_valuations_fx::fx_forward::FxForward;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::dates::Date;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::types::{CurveId, InstrumentId};
    /// # use time::Month;
    /// // EUR/USD spot at 1.1000, forward points quoted as "50" (pips)
    /// let spot = 1.1000;
    /// let pips = 50.0;
    /// let pip_size = 0.0001; // Standard pip size for EUR/USD
    /// let forward_points = pips * pip_size; // = 0.0050
    ///
    /// let forward = FxForward::builder()
    ///     .id(InstrumentId::new("EURUSD-FWD"))
    ///     .base_currency(Currency::EUR)
    ///     .quote_currency(Currency::USD)
    ///     .maturity(Date::from_calendar_date(2025, Month::June, 15).unwrap())
    ///     .notional(Money::from((1_000_000_i64, Currency::EUR)))
    ///     .domestic_discount_curve_id(CurveId::new("USD-OIS"))
    ///     .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
    ///     .build()
    ///     .unwrap()
    ///     .with_forward_points(spot, forward_points)
    ///     .unwrap();
    ///
    /// // Contract rate = 1.1000 + 0.0050 = 1.1050
    /// assert!((forward.contract_rate.unwrap() - 1.1050).abs() < 1e-10);
    /// ```
    pub fn with_forward_points(
        mut self,
        spot_rate: f64,
        forward_points: f64,
    ) -> finstack_quant_core::Result<Self> {
        let contract_rate = spot_rate + forward_points;
        if !spot_rate.is_finite() || spot_rate <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX forward spot_rate must be positive and finite, got {}",
                spot_rate
            )));
        }
        if !forward_points.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX forward forward_points must be finite, got {}",
                forward_points
            )));
        }
        if !contract_rate.is_finite() || contract_rate <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX forward contract_rate from spot_rate + forward_points must be positive and finite, got {}",
                contract_rate
            )));
        }
        self.contract_rate = Some(contract_rate);
        Ok(self)
    }

    /// Create an FX forward from spot plus forward points quoted in pips.
    ///
    /// Converts pips to rate points with the caller's explicit quote
    /// convention, which must name this instrument's stored
    /// `base_currency`/`quote_currency` order, then delegates to
    /// [`with_forward_points`](Self::with_forward_points). No pip size is
    /// inferred (record CORE-CONVENTIONS-011 #3).
    ///
    /// # Arguments
    ///
    /// * `spot_rate` - Current spot rate in quote-per-base units. Must be
    ///   finite and strictly positive.
    /// * `pips` - Forward points in pip units (for example `50.0` for "+50"
    ///   on EURUSD). May be negative for a discount; must be finite.
    /// * `convention` - Quote metadata of this instrument's ordered pair,
    ///   carrying the pip size.
    ///
    /// # Errors
    ///
    /// Returns an error when `convention` names another pair or order,
    /// `spot_rate` is non-finite or not strictly positive, `pips` is
    /// non-finite, or `spot_rate + pips * pip_size` is not a finite strictly
    /// positive contract rate.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_valuations_fx::fx_forward::FxForward;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::dates::Date;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::money::fx::FxPairConvention;
    /// # use finstack_quant_core::types::{CurveId, InstrumentId};
    /// # use time::Month;
    /// let eurusd = FxPairConvention::new(Currency::EUR, Currency::USD, 0.0001).unwrap();
    /// let forward = FxForward::builder()
    ///     .id(InstrumentId::new("EURUSD-FWD"))
    ///     .base_currency(Currency::EUR)
    ///     .quote_currency(Currency::USD)
    ///     .maturity(Date::from_calendar_date(2025, Month::June, 15).unwrap())
    ///     .notional(Money::from((1_000_000_i64, Currency::EUR)))
    ///     .domestic_discount_curve_id(CurveId::new("USD-OIS"))
    ///     .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
    ///     .build()
    ///     .unwrap()
    ///     .with_forward_pips(1.1000, 50.0, &eurusd)
    ///     .unwrap();
    ///
    /// // Contract rate = 1.1000 + 50 × 0.0001 = 1.1050
    /// assert!((forward.contract_rate.unwrap() - 1.1050).abs() < 1e-10);
    /// ```
    pub fn with_forward_pips(
        self,
        spot_rate: f64,
        pips: f64,
        convention: &finstack_quant_core::money::fx::FxPairConvention,
    ) -> finstack_quant_core::Result<Self> {
        if (convention.get_base(), convention.get_quote())
            != (self.base_currency, self.quote_currency)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX pip convention {}/{} does not match the forward's {}/{}",
                convention.get_base(),
                convention.get_quote(),
                self.base_currency,
                self.quote_currency
            )));
        }
        let points = convention.rate_points(pips)?;
        self.with_forward_points(spot_rate, points)
    }

    /// Compute the market forward rate via covered interest rate parity.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The maturity date is before the valuation date
    /// - Required discount curves are not found
    /// - FX rate is not available and no spot override is set
    pub fn market_forward_rate(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        crate::shared::covered_interest_parity_forward(crate::shared::FxForwardRateRequest {
            market,
            as_of,
            maturity: self.maturity,
            base_currency: self.base_currency,
            quote_currency: self.quote_currency,
            domestic_discount_curve_id: &self.domestic_discount_curve_id,
            foreign_discount_curve_id: &self.foreign_discount_curve_id,
            quoted_spot: self.quoted_spot,
            context: "FxForward",
        })
    }

    fn contractual_forward_rate(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        self.contract_rate
            .map(Ok)
            .unwrap_or_else(|| self.market_forward_rate(market, as_of))
    }
}

impl finstack_quant_valuations_core::instruments::Instrument for FxForward {
    fn with_bumped_fx_quote(
        &self,
        relative: f64,
    ) -> Option<Box<dyn finstack_quant_valuations_core::instruments::Instrument>> {
        let mut bumped = self.clone();
        *bumped.quoted_spot.as_mut()? *= 1.0 + relative;
        Some(Box::new(bumped))
    }

    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::FxForward,
        crate::FX
    );

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.domestic_discount_curve_id.clone());
        deps.add_discount_curve(self.foreign_discount_curve_id.clone());
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    /// Surface the FX pair so attribution can route the `Fx01 × Δspot` P&L
    /// to `fx_pnl` instead of leaving it in residual. Without this override
    /// `attribute_pnl_metrics_based`'s `instrument.fx_exposure()` lookup
    /// returns `None`, the FX01 metric this instrument *does* publish is
    /// ignored, and the entire spot-driven move falls into the residual
    /// bucket — a real attribution gap found by the QuantLib parity test.
    fn fx_exposure(
        &self,
    ) -> Option<(
        finstack_quant_core::currency::Currency,
        finstack_quant_core::currency::Currency,
    )> {
        Some((self.base_currency, self.quote_currency))
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        // End-of-day policy: the settlement legs remain live on maturity.
        if crate::shared::event_has_occurred(self.maturity, as_of) {
            return Ok(finstack_quant_core::money::Money::from((
                0_i64,
                self.quote_currency,
            )));
        }

        let inputs =
            crate::shared::collect_fx_forward_inputs(crate::shared::FxForwardRateRequest {
                market,
                as_of,
                maturity: self.maturity,
                base_currency: self.base_currency,
                quote_currency: self.quote_currency,
                domestic_discount_curve_id: &self.domestic_discount_curve_id,
                foreign_discount_curve_id: &self.foreign_discount_curve_id,
                quoted_spot: self.quoted_spot,
                context: "FxForward",
            })?;
        let pv = self.contract_rate.map_or(0.0, |contract_rate| {
            self.notional.amount()
                * (inputs.spot * inputs.df_foreign - contract_rate * inputs.df_domestic)
        });

        finstack_quant_core::money::Money::from_model_value(pv, self.quote_currency)
    }

    fn valuation_details(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> Option<finstack_quant_valuations_core::results::ValuationDetails> {
        // FX-policy visibility per layer; `quoted_spot` short-circuits the
        // matrix lookup, so triangulation does not apply then.
        Some(crate::shared::fx_valuation_details(
            market,
            self.base_currency,
            self.quote_currency,
            as_of,
            self.quoted_spot.is_some(),
        ))
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for FxForward {
    fn raw_cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.validate()?;
        if crate::shared::event_has_occurred(self.maturity, as_of) {
            return crate::shared::settled_schedule(Some(Money::from((0_i64, self.base_currency))));
        }
        let contract_rate = self.contractual_forward_rate(market, as_of)?;
        let base_amount = Money::new(self.notional.amount(), self.base_currency)?;
        let quote_amount =
            Money::from_model_value(-self.notional.amount() * contract_rate, self.quote_currency)?;

        let base_flow = (self.maturity, base_amount);
        let quote_schedule = (self.maturity, quote_amount);
        let representation = if self.contract_rate.is_some() {
            finstack_quant_cashflows::builder::CashflowRepresentation::Contractual
        } else {
            finstack_quant_cashflows::builder::CashflowRepresentation::Projected
        };
        let schedule = finstack_quant_cashflows::schedule_from_classified_flows(
            vec![base_flow, quote_schedule]
                .into_iter()
                .map(
                    |(date, cash): (
                        finstack_quant_core::dates::Date,
                        finstack_quant_core::money::Money,
                    )| {
                        // The exchanged amount is the principal movement (record
                        // CORE-CONVENTIONS-015 #4).
                        finstack_quant_core::cashflow::CashFlowParams::new(
                            date,
                            None,
                            cash,
                            CFKind::Notional,
                            0.0,
                            None,
                        )
                        .with_principal_delta(cash.checked_neg())
                        .with_principal_date(date)
                        .build()
                    },
                )
                .collect::<finstack_quant_core::Result<Vec<_>>>()?,
            finstack_quant_core::dates::DayCount::Act365F,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(Money::from((0_i64, self.base_currency))),
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    representation,
                    ..Default::default()
                },
            },
        );
        Ok(schedule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_cashflows::CashflowProvider;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_valuations_core::instruments::Instrument;
    use std::sync::Arc;
    use time::Month;

    fn test_market(as_of: Date) -> MarketContext {
        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
            .build()
            .expect("should build");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.5, 0.9851), (1.0, 0.9704)])
            .build()
            .expect("should build");

        let fx_provider = Arc::new(SimpleFxProvider::new());
        fx_provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
            .expect("valid rate");
        let fx_matrix = FxMatrix::new(fx_provider, None);

        MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_fx(fx_matrix)
    }

    #[test]
    fn test_fx_forward_serde_rejects_invalid_contract_rate() {
        let forward = FxForward::example().unwrap();
        let mut json = serde_json::to_value(&forward).expect("serialize");
        json["contract_rate"] = serde_json::json!(-1.0);

        let err = serde_json::from_value::<FxForward>(json)
            .expect_err("invalid contract rate should fail during deserialization");
        assert!(
            err.to_string().contains("contract_rate"),
            "error should mention contract_rate: {}",
            err
        );
    }

    #[test]
    fn test_validation_same_currency_fails() {
        let forward = FxForward {
            id: InstrumentId::new("TEST"),
            base_currency: Currency::EUR,
            quote_currency: Currency::EUR, // Same as base - invalid
            maturity: Date::from_calendar_date(2025, Month::June, 15).expect("valid date"),
            notional: Money::from((1_000_000_i64, Currency::EUR)),
            contract_rate: None,
            domestic_discount_curve_id: CurveId::new("EUR-OIS"),
            foreign_discount_curve_id: CurveId::new("EUR-OIS"),
            quoted_spot: None,
            base_calendar_id: None,
            quote_calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let result = forward.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must differ from quote_currency")
        );
    }

    #[test]
    fn test_validation_notional_currency_mismatch_fails() {
        let forward = FxForward {
            id: InstrumentId::new("TEST"),
            base_currency: Currency::EUR,
            quote_currency: Currency::USD,
            maturity: Date::from_calendar_date(2025, Month::June, 15).expect("valid date"),
            notional: Money::from((1_000_000_i64, Currency::USD)), // Wrong currency
            contract_rate: None,
            domestic_discount_curve_id: CurveId::new("USD-OIS"),
            foreign_discount_curve_id: CurveId::new("EUR-OIS"),
            quoted_spot: None,
            base_calendar_id: None,
            quote_calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let result = forward.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must match base_currency")
        );
    }

    #[test]
    fn test_validation_negative_contract_rate_fails() {
        let forward = FxForward {
            id: InstrumentId::new("TEST"),
            base_currency: Currency::EUR,
            quote_currency: Currency::USD,
            maturity: Date::from_calendar_date(2025, Month::June, 15).expect("valid date"),
            notional: Money::from((1_000_000_i64, Currency::EUR)),
            contract_rate: Some(-1.10), // Negative rate - invalid
            domestic_discount_curve_id: CurveId::new("USD-OIS"),
            foreign_discount_curve_id: CurveId::new("EUR-OIS"),
            quoted_spot: None,
            base_calendar_id: None,
            quote_calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let result = forward.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("contract_rate must be positive")
        );
    }

    #[test]
    fn test_validation_negative_spot_override_fails() {
        let forward = FxForward {
            id: InstrumentId::new("TEST"),
            base_currency: Currency::EUR,
            quote_currency: Currency::USD,
            maturity: Date::from_calendar_date(2025, Month::June, 15).expect("valid date"),
            notional: Money::from((1_000_000_i64, Currency::EUR)),
            contract_rate: Some(1.10),
            domestic_discount_curve_id: CurveId::new("USD-OIS"),
            foreign_discount_curve_id: CurveId::new("EUR-OIS"),
            quoted_spot: Some(-1.10), // Negative rate - invalid
            base_calendar_id: None,
            quote_calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let result = forward.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("quoted_spot must be positive")
        );
    }

    #[test]
    fn test_validation_valid_forward_passes() {
        let forward = FxForward::example().unwrap();
        assert!(forward.validate().is_ok());
    }
    #[test]
    fn forward_points_set_contract_rate_without_pinning_market_spot() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
        let forward = FxForward::builder()
            .id(InstrumentId::new("EURUSD-POINTS"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .maturity(Date::from_calendar_date(2024, Month::July, 15).expect("valid date"))
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("should build")
            .with_forward_points(1.10, 0.005)
            .expect("valid forward points");

        assert_eq!(forward.contract_rate, Some(1.105));
        assert_eq!(forward.quoted_spot, None);

        let pv_at_trade_spot = forward
            .base_value(&test_market(as_of), as_of)
            .expect("price at trade spot")
            .amount();

        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
            .build()
            .expect("should build");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.5, 0.9851), (1.0, 0.9704)])
            .build()
            .expect("should build");
        let fx_provider = Arc::new(SimpleFxProvider::new());
        fx_provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.20)])
            .expect("valid rate");
        let moved_market = MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_fx(FxMatrix::new(fx_provider, None));
        let pv_after_spot_move = forward
            .base_value(&moved_market, as_of)
            .expect("price after spot move")
            .amount();

        assert!(
            pv_after_spot_move > pv_at_trade_spot + 90_000.0,
            "long-base forward must retain live spot delta"
        );
    }

    #[test]
    fn test_cashflow_provider_emits_two_currency_legs() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
        let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
        let market = test_market(as_of);

        let forward = FxForward::builder()
            .id(InstrumentId::new("EURUSD-CF"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .maturity(maturity)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .contract_rate_opt(Some(1.12))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        let flows = forward
            .dated_cashflows(&market, as_of)
            .expect("contractual schedule should build");

        assert_eq!(
            flows.len(),
            2,
            "fx forward should emit both settlement legs"
        );
        assert!(flows.iter().all(|(date, _)| *date == maturity));
        let currencies: Vec<_> = flows.iter().map(|(_, m)| m.currency()).collect();
        assert!(currencies.contains(&Currency::EUR), "should have EUR leg");
        assert!(currencies.contains(&Currency::USD), "should have USD leg");

        let eur_flow = flows
            .iter()
            .find(|(_, m)| m.currency() == Currency::EUR)
            .unwrap();
        let usd_flow = flows
            .iter()
            .find(|(_, m)| m.currency() == Currency::USD)
            .unwrap();
        assert!((eur_flow.1.amount() - 1_000_000.0).abs() < 1e-10);
        assert!((usd_flow.1.amount() + 1_120_000.0).abs() < 1e-10);
    }
}
