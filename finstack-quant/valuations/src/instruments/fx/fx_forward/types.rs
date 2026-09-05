//! FX Forward types and implementations.
//!
//! Defines the `FxForward` instrument for outright forward contracts on
//! currency pairs. Pricing uses covered interest rate parity (CIRP) with
//! optional contract rate override.

use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::primitives::CFKind;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;
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
/// - S = spot FX rate (from FxMatrix or spot_rate_override)
/// - DF_foreign(T) = discount factor in base currency to maturity
/// - DF_domestic(T) = discount factor in quote currency to maturity
/// - F_contract = contract_rate (if provided, else F_market for at-market forward)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
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
#[serde(deny_unknown_fields, try_from = "FxForwardUnchecked")]
pub struct FxForward {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Base currency (foreign currency, numerator of the pair).
    pub base_currency: Currency,
    /// Quote currency (domestic currency, denominator of the pair, PV currency).
    pub quote_currency: Currency,
    /// Maturity/settlement date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Notional amount in base currency.
    pub notional: Money,
    /// Contract forward rate (quote per base). If None, valued at-market.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_rate: Option<f64>,
    /// Domestic (quote currency) discount curve ID.
    pub domestic_discount_curve_id: CurveId,
    /// Foreign (base currency) discount curve ID.
    pub foreign_discount_curve_id: CurveId,
    /// Optional spot rate override (quote per base). If None, source from FxMatrix.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_rate_override: Option<f64>,
    /// Optional base currency calendar for business day adjustment.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_calendar_id: Option<String>,
    /// Optional quote currency calendar for business day adjustment.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_calendar_id: Option<String>,
    /// Attributes for tagging and selection.
    #[builder(default)]
    /// Instrument-owned pricing inputs.
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
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct FxForwardUnchecked {
    /// Unique instrument identifier.
    id: InstrumentId,
    /// Base currency (foreign currency, numerator of the pair).
    base_currency: Currency,
    /// Quote currency (domestic currency, denominator of the pair, PV currency).
    quote_currency: Currency,
    /// Maturity/settlement date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    maturity: Date,
    /// Notional amount in base currency.
    notional: Money,
    /// Contract forward rate (quote per base). If None, valued at-market.
    #[serde(default)]
    contract_rate: Option<f64>,
    /// Domestic (quote currency) discount curve ID.
    domestic_discount_curve_id: CurveId,
    /// Foreign (base currency) discount curve ID.
    foreign_discount_curve_id: CurveId,
    /// Optional spot rate override (quote per base). If None, source from FxMatrix.
    #[serde(default)]
    spot_rate_override: Option<f64>,
    /// Optional base currency calendar for business day adjustment.
    #[serde(default)]
    base_calendar_id: Option<String>,
    /// Optional quote currency calendar for business day adjustment.
    #[serde(default)]
    quote_calendar_id: Option<String>,
    /// Per-instrument pricing/sensitivity override knobs.
    #[serde(default)]
    instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging.
    attributes: Attributes,
}

impl TryFrom<FxForwardUnchecked> for FxForward {
    type Error = finstack_quant_core::Error;

    fn try_from(value: FxForwardUnchecked) -> std::result::Result<Self, Self::Error> {
        let forward = Self {
            id: value.id,
            base_currency: value.base_currency,
            quote_currency: value.quote_currency,
            maturity: value.maturity,
            notional: value.notional,
            contract_rate: value.contract_rate,
            domestic_discount_curve_id: value.domestic_discount_curve_id,
            foreign_discount_curve_id: value.foreign_discount_curve_id,
            spot_rate_override: value.spot_rate_override,
            base_calendar_id: value.base_calendar_id,
            quote_calendar_id: value.quote_calendar_id,
            instrument_pricing_overrides: value.instrument_pricing_overrides,
            metric_pricing_overrides: value.metric_pricing_overrides,
            scenario_pricing_overrides: value.scenario_pricing_overrides,
            attributes: value.attributes,
        };
        forward.validate()?;
        Ok(forward)
    }
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
    /// - `spot_rate_override` is provided but is not positive
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
        validation::validate_money_finite(self.notional, "FX forward notional")?;
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
        if let Some(rate) = self.spot_rate_override {
            validation::require_with(rate > 0.0, || {
                format!(
                    "FX forward spot_rate_override must be positive, got {}",
                    rate
                )
            })?;
            validation::require_with(rate.is_finite(), || {
                "FX forward spot_rate_override must be finite".to_string()
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

    /// Returns the standard spot settlement days for a currency pair.
    ///
    /// # Market Conventions
    ///
    /// | Pair | Settlement | Notes |
    /// |------|------------|-------|
    /// | USD/CAD | T+1 | North American same-day zone |
    /// | USD/MXN | T+2 | Standard emerging market convention |
    /// | USD/TRY | T+1 | Istanbul market convention |
    /// | Other | T+2 | Standard settlement |
    ///
    /// # Arguments
    ///
    /// * `base` - Base currency (foreign)
    /// * `quote` - Quote currency (domestic)
    ///
    /// # Returns
    ///
    /// Number of business days for spot settlement (1 or 2).
    pub fn standard_spot_days(base: Currency, quote: Currency) -> u32 {
        finstack_quant_core::dates::fx::fx_standard_spot_lag_days(base, quote)
    }

    /// Construct an FX forward from trade date and tenor using joint calendar spot roll.
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier
    /// * `base_currency` - Foreign currency (numerator)
    /// * `quote_currency` - Domestic currency (denominator)
    /// * `trade_date` - Trade date
    /// * `tenor` - Calendar tenor from spot to maturity (for example, 3M).
    /// * `notional` - Notional in base currency
    /// * `domestic_discount_curve_id` - Quote currency discount curve
    /// * `foreign_discount_curve_id` - Base currency discount curve
    /// * `base_calendar_id` - Optional base currency calendar
    /// * `quote_calendar_id` - Optional quote currency calendar
    /// * `spot_lag_days` - Spot lag (typically 2, or 1 for USD/CAD). Use
    ///   [`standard_spot_days`](Self::standard_spot_days) to determine automatically.
    /// * `business_day_convention` - Business day convention
    /// * `end_of_month` - Preserve month-end when the spot date is month-end.
    #[allow(clippy::too_many_arguments)]
    pub fn from_trade_date(
        id: impl Into<InstrumentId>,
        base_currency: Currency,
        quote_currency: Currency,
        trade_date: Date,
        tenor: Tenor,
        notional: Money,
        domestic_discount_curve_id: impl Into<CurveId>,
        foreign_discount_curve_id: impl Into<CurveId>,
        base_calendar_id: Option<String>,
        quote_calendar_id: Option<String>,
        spot_lag_days: i32,
        business_day_convention: finstack_quant_core::dates::BusinessDayConvention,
        end_of_month: bool,
    ) -> finstack_quant_core::Result<Self> {
        use crate::instruments::common_impl::fx_dates::{
            add_fx_standard_tenor, fx_spot_date_for_pair,
        };

        // CLS-consistent spot roll: a US holiday on an intermediate day does not
        // delay a USD pair's spot date (FX spot convention
        // finding).
        let spot_date = fx_spot_date_for_pair(
            trade_date,
            spot_lag_days,
            base_currency,
            quote_currency,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;
        let maturity = add_fx_standard_tenor(
            spot_date,
            tenor,
            business_day_convention,
            end_of_month,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;

        Self::builder()
            .id(id.into())
            .base_currency(base_currency)
            .quote_currency(quote_currency)
            .maturity(maturity)
            .notional(notional)
            .domestic_discount_curve_id(domestic_discount_curve_id.into())
            .foreign_discount_curve_id(foreign_discount_curve_id.into())
            .base_calendar_id_opt(base_calendar_id)
            .quote_calendar_id_opt(quote_calendar_id)
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
    /// # use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
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
    /// Converts pips to decimal points with
    /// [`fx_pip_size`](finstack_quant_core::money::fx::fx_pip_size) for this
    /// instrument's stored `base_currency` / `quote_currency`, then delegates
    /// to [`with_forward_points`](Self::with_forward_points). Pip size is
    /// `0.01` when either side is JPY, KRW, or HUF, otherwise `0.0001`.
    ///
    /// # Arguments
    ///
    /// * `spot_rate` - Current spot rate in quote-per-base units. Must be
    ///   finite and strictly positive.
    /// * `pips` - Forward points in pip units (for example `50.0` for "+50"
    ///   on EURUSD). May be negative for a discount; must be finite.
    ///
    /// # Errors
    ///
    /// Returns an error when `spot_rate` is non-finite or not strictly
    /// positive, `pips` is non-finite, or `spot_rate + pips * pip_size` is
    /// not a finite strictly positive contract rate.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::dates::Date;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::types::{CurveId, InstrumentId};
    /// # use time::Month;
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
    ///     .with_forward_pips(1.1000, 50.0)
    ///     .unwrap();
    ///
    /// // Contract rate = 1.1000 + 50 × 0.0001 = 1.1050
    /// assert!((forward.contract_rate.unwrap() - 1.1050).abs() < 1e-10);
    /// ```
    pub fn with_forward_pips(self, spot_rate: f64, pips: f64) -> finstack_quant_core::Result<Self> {
        if !pips.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX forward pips must be finite, got {}",
                pips
            )));
        }
        let pip_size =
            finstack_quant_core::money::fx::fx_pip_size(self.base_currency, self.quote_currency);
        self.with_forward_points(spot_rate, pips * pip_size)
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
        crate::instruments::fx::shared::covered_interest_parity_forward(
            crate::instruments::fx::shared::FxForwardRateRequest {
                market,
                as_of,
                maturity: self.maturity,
                base_currency: self.base_currency,
                quote_currency: self.quote_currency,
                domestic_discount_curve_id: &self.domestic_discount_curve_id,
                foreign_discount_curve_id: &self.foreign_discount_curve_id,
                spot_rate_override: self.spot_rate_override,
                context: "FxForward",
            },
        )
    }

    fn contractual_forward_rate(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        self.contract_rate
            .map(Ok)
            .unwrap_or_else(|| self.market_forward_rate(market, as_of))
    }

    fn single_leg_schedule(
        &self,
        as_of: Date,
        amount: Money,
    ) -> finstack_quant_core::Result<(Date, Money)> {
        let _ = as_of;
        Ok((self.maturity, amount))
    }
}

impl crate::instruments::common_impl::traits::Instrument for FxForward {
    impl_instrument_base!(crate::pricer::InstrumentType::FxForward);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
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
        self.validate()?;

        // End-of-day policy: the settlement legs remain live on maturity.
        if crate::instruments::fx::shared::event_has_occurred(self.maturity, as_of) {
            return Ok(finstack_quant_core::money::Money::from((
                0_i64,
                self.quote_currency,
            )));
        }

        let inputs = crate::instruments::fx::shared::collect_fx_forward_inputs(
            crate::instruments::fx::shared::FxForwardRateRequest {
                market,
                as_of,
                maturity: self.maturity,
                base_currency: self.base_currency,
                quote_currency: self.quote_currency,
                domestic_discount_curve_id: &self.domestic_discount_curve_id,
                foreign_discount_curve_id: &self.foreign_discount_curve_id,
                spot_rate_override: self.spot_rate_override,
                context: "FxForward",
            },
        )?;
        let pv = self.contract_rate.map_or(0.0, |contract_rate| {
            self.notional.amount()
                * (inputs.spot * inputs.df_foreign - contract_rate * inputs.df_domestic)
        });

        finstack_quant_core::money::Money::new(pv, self.quote_currency)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
    }

    fn valuation_details(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> Option<crate::results::ValuationDetails> {
        // Project invariant: FX-policy visibility per layer. Record whether
        // the spot was a direct quote or triangulated through the matrix
        // pivot. `spot_rate_override` short-circuits the matrix lookup, so
        // there is no triangulation to report in that case.
        use finstack_quant_core::money::fx::FxQuery;
        let fx_triangulated = if self.spot_rate_override.is_some() {
            None
        } else {
            market
                .fx()
                .and_then(|fx| {
                    fx.rate(FxQuery::new(self.base_currency, self.quote_currency, as_of))
                        .ok()
                })
                .map(|q| q.triangulated)
        };
        Some(crate::results::ValuationDetails::Fx(
            crate::results::FxValuationDetails { fx_triangulated },
        ))
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for FxForward {
    fn raw_cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.validate()?;
        if crate::instruments::fx::shared::event_has_occurred(self.maturity, as_of) {
            return Ok(crate::cashflow::traits::schedule_from_classified_flows(
                Vec::new(),
                finstack_quant_core::dates::DayCount::Act365F,
                crate::cashflow::traits::ScheduleBuildOpts {
                    notional_hint: Some(Money::from((0_i64, self.base_currency))),
                    meta: crate::cashflow::builder::CashFlowMeta {
                        representation:
                            crate::cashflow::builder::CashflowRepresentation::NoResidual,
                        ..Default::default()
                    },
                },
            ));
        }
        let contract_rate = self.contractual_forward_rate(market, as_of)?;
        let base_amount = Money::new(self.notional.amount(), self.base_currency)?;
        let quote_amount =
            Money::new(-self.notional.amount() * contract_rate, self.quote_currency)?;

        let base_flow = self.single_leg_schedule(as_of, base_amount)?;
        let quote_schedule = self.single_leg_schedule(as_of, quote_amount)?;
        let representation = if self.contract_rate.is_some() {
            crate::cashflow::builder::CashflowRepresentation::Contractual
        } else {
            crate::cashflow::builder::CashflowRepresentation::Projected
        };
        let schedule = crate::cashflow::traits::schedule_from_dated_flows(
            vec![base_flow, quote_schedule],
            CFKind::Notional,
            finstack_quant_core::dates::DayCount::Act365F,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(Money::from((0_i64, self.base_currency))),
                meta: crate::cashflow::builder::CashFlowMeta {
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
    use crate::cashflow::CashflowProvider;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
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
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        let fx_matrix = FxMatrix::new(fx_provider);

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
            spot_rate_override: None,
            base_calendar_id: None,
            quote_calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let result = forward.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must differ from quote_currency"));
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
            spot_rate_override: None,
            base_calendar_id: None,
            quote_calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let result = forward.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must match base_currency"));
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
            spot_rate_override: None,
            base_calendar_id: None,
            quote_calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let result = forward.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("contract_rate must be positive"));
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
            spot_rate_override: Some(-1.10), // Negative rate - invalid
            base_calendar_id: None,
            quote_calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let result = forward.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("spot_rate_override must be positive"));
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
        assert_eq!(forward.spot_rate_override, None);

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
            .set_quote(Currency::EUR, Currency::USD, 1.20)
            .expect("valid rate");
        let moved_market = MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_fx(FxMatrix::new(fx_provider));
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

    #[test]
    fn standard_spot_days_matches_core_helper() {
        assert_eq!(
            FxForward::standard_spot_days(Currency::USD, Currency::CAD),
            1
        );
        assert_eq!(
            FxForward::standard_spot_days(Currency::USD, Currency::TRY),
            1
        );
        assert_eq!(
            FxForward::standard_spot_days(Currency::EUR, Currency::USD),
            2
        );
        assert_eq!(
            FxForward::standard_spot_days(Currency::USD, Currency::MXN),
            2
        );
    }
}
