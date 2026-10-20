//! Quanto option instrument definition.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_valuations_core::impl_instrument_base;
use finstack_quant_valuations_core::instruments::Attributes;
use finstack_quant_valuations_core::instruments::OptionType;

/// Quanto option instrument.
///
/// Quanto options have payoffs that depend on an underlying asset in one currency
/// but are settled in another currency, creating FX exposure.
#[derive(
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, try_from = "QuantoOptionUnchecked")]
pub struct QuantoOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying equity ticker symbol
    pub underlying_ticker: String,
    /// Per-unit strike of the equity underlying, in `base_currency` price units.
    pub strike: f64,
    /// Option type (call or put)
    pub option_type: OptionType,
    /// Option expiry date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Strike-equivalent domestic reference notional.
    ///
    /// When `quantity` and `payoff_fx_rate` are supplied, this must
    /// equal `quantity * payoff_fx_rate * strike`.
    pub notional: Money,
    /// Number of underlying units covered by the option payoff.
    ///
    /// Quanto pricing is performed per unit of the underlying in the base currency.
    /// This quantity converts the per-unit price into contract-level exposure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub quantity: Option<f64>,
    /// Fixed payoff FX conversion rate from base-currency payoff into quote currency.
    ///
    /// Example: for a JPY-underlying option settled in USD at a fixed 140 JPY/USD
    /// conversion, `payoff_fx_rate` is `1.0 / 140.0` USD per JPY.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub payoff_fx_rate: Option<f64>,
    /// Base currency (equity denomination)
    pub base_currency: Currency,
    /// Quote currency (payment/settlement currency)
    pub quote_currency: Currency,
    /// Correlation between equity price and FX rate
    pub correlation: f64, // Correlation between equity and FX
    /// Day count convention (defaults to ACT/365F).
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::day_count_act365f")]
    #[builder(default = finstack_quant_core::dates::DayCount::Act365F)]
    pub day_count: finstack_quant_core::dates::DayCount,
    /// Discount curve ID (domestic currency)
    pub domestic_discount_curve_id: CurveId,
    /// Discount curve ID (foreign currency)
    pub foreign_discount_curve_id: CurveId,
    /// Equity spot price identifier
    pub spot_id: PriceId,
    /// Equity volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional dividend-yield scalar ID
    pub div_yield_id: Option<PriceId>,
    /// Optional FX rate identifier.
    ///
    /// The referenced scalar must be quoted as **quote currency per unit of
    /// base currency** (e.g. for base JPY / quote USD, USD-per-JPY ≈ 0.0071,
    /// conventionally named `JPYUSD`). The pricer compounds it with
    /// `exp((r_quote − r_base)·t)` to obtain the ATM forward for the FX vol
    /// lookup, so an inverted quote (JPY-per-USD) silently mislocates the
    /// smile. `validate()` rejects ids whose embedded pair name contradicts
    /// this direction.
    pub fx_spot_id: Option<PriceId>,
    /// Optional FX volatility surface ID.
    ///
    /// Must be calibrated in the same **quote-per-base** direction as
    /// `fx_spot_id` so absolute-strike lookups at the CIRP forward land on
    /// the correct smile. `validate()` rejects ids whose embedded pair name
    /// contradicts this direction.
    pub fx_vol_surface_id: Option<CurveId>,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct QuantoOptionUnchecked {
    /// Unique instrument identifier.
    id: InstrumentId,
    /// Underlying equity ticker symbol.
    underlying_ticker: String,
    /// Per-unit strike of the equity underlying, in `base_currency` price units.
    strike: f64,
    /// Option type (call or put).
    option_type: OptionType,
    /// Option expiry date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    expiry: Date,
    /// Strike-equivalent domestic reference notional.
    notional: Money,
    /// Number of underlying units covered by the option payoff.
    #[serde(default)]
    quantity: Option<f64>,
    /// Fixed payoff FX conversion rate from base-currency payoff into quote currency.
    #[serde(default)]
    payoff_fx_rate: Option<f64>,
    /// Base currency (equity denomination).
    base_currency: Currency,
    /// Quote currency (payment/settlement currency).
    quote_currency: Currency,
    /// Correlation between equity price and FX rate.
    correlation: f64,
    /// Day count convention (defaults to ACT/365F).
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::day_count_act365f")]
    day_count: finstack_quant_core::dates::DayCount,
    /// Discount curve ID (domestic currency).
    domestic_discount_curve_id: CurveId,
    /// Discount curve ID (foreign currency).
    foreign_discount_curve_id: CurveId,
    /// Equity spot price identifier.
    spot_id: PriceId,
    /// Equity volatility surface ID.
    vol_surface_id: CurveId,
    /// Optional dividend-yield scalar ID.
    #[serde(default)]
    div_yield_id: Option<PriceId>,
    /// Optional FX rate identifier.
    #[serde(default)]
    fx_spot_id: Option<PriceId>,
    /// Optional FX volatility surface ID.
    #[serde(default)]
    fx_vol_surface_id: Option<CurveId>,
    /// Instrument-owned pricing inputs.
    #[serde(default)]
    instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    metric_pricing_overrides: finstack_quant_valuations_core::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping.
    attributes: Attributes,
}

impl TryFrom<QuantoOptionUnchecked> for QuantoOption {
    type Error = finstack_quant_core::Error;

    fn try_from(value: QuantoOptionUnchecked) -> std::result::Result<Self, Self::Error> {
        let quanto = Self {
            id: value.id,
            underlying_ticker: value.underlying_ticker,
            strike: value.strike,
            option_type: value.option_type,
            expiry: value.expiry,
            notional: value.notional,
            quantity: value.quantity,
            payoff_fx_rate: value.payoff_fx_rate,
            base_currency: value.base_currency,
            quote_currency: value.quote_currency,
            correlation: value.correlation,
            day_count: value.day_count,
            domestic_discount_curve_id: value.domestic_discount_curve_id,
            foreign_discount_curve_id: value.foreign_discount_curve_id,
            spot_id: value.spot_id,
            vol_surface_id: value.vol_surface_id,
            div_yield_id: value.div_yield_id,
            fx_spot_id: value.fx_spot_id,
            fx_vol_surface_id: value.fx_vol_surface_id,
            instrument_pricing_overrides: value.instrument_pricing_overrides,
            metric_pricing_overrides: value.metric_pricing_overrides,
            scenario_pricing_overrides: value.scenario_pricing_overrides,
            attributes: value.attributes,
        };
        quanto.validate()?;
        Ok(quanto)
    }
}

impl QuantoOption {
    /// Create a canonical example quanto equity option (Nikkei in USD)
    /// expiring on the project-wide stable example epoch.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::dates::DayCount;
        QuantoOption::builder()
            .id(InstrumentId::new("QUANTO-NKY-USD-CALL"))
            .underlying_ticker("NKY".to_string())
            .strike(35_000.0)
            .option_type(finstack_quant_valuations_core::instruments::OptionType::Call)
            .expiry(finstack_quant_valuations_core::instruments::common_impl::example_constants::FAR_EXPIRY)
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .quantity_opt(Some(4_000.0))
            .payoff_fx_rate_opt(Some(1.0 / 140.0))
            .base_currency(Currency::JPY)
            .quote_currency(Currency::USD)
            .correlation(-0.2)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("JPY-OIS"))
            .spot_id("NKY-SPOT".into())
            .vol_surface_id(CurveId::new("NKY-VOL"))
            .div_yield_id_opt(Some(PriceId::new("NKY-DIV")))
            .fx_spot_id_opt(Some(PriceId::new("JPYUSD-SPOT")))
            .fx_vol_surface_id_opt(Some(CurveId::new("JPYUSD-VOL")))
            .attributes(Attributes::new())
            .build()
    }

    /// Validate quanto option economics at construction boundaries.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        finstack_quant_core::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "QuantoOption",
        )?;
        if !self.correlation.is_finite() || !(-1.0..=1.0).contains(&self.correlation) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "QuantoOption correlation must be a finite value in [-1, 1], got {}",
                self.correlation
            )));
        }
        if self.notional.currency() != self.quote_currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "QuantoOption notional currency ({}) must match quote_currency ({})",
                self.notional.currency(),
                self.quote_currency
            )));
        }
        if !self.strike.is_finite() || self.strike <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "QuantoOption strike must be positive and finite; got {}",
                self.strike
            )));
        }
        if !self.notional.amount().is_finite() || self.notional.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "QuantoOption notional must be positive and finite; got {}",
                self.notional.amount()
            )));
        }
        match (self.quantity, self.payoff_fx_rate) {
            (Some(quantity), Some(fx_rate)) => {
                if !quantity.is_finite() || quantity <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "QuantoOption quantity must be positive and finite; got {}",
                        quantity
                    )));
                }
                if !fx_rate.is_finite() || fx_rate <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "QuantoOption payoff_fx_rate must be positive and finite; got {}",
                        fx_rate
                    )));
                }
                let expected = quantity * fx_rate * self.strike;
                let tolerance = 1e-8 * expected.abs().max(self.notional.amount().abs()).max(1.0);
                if (self.notional.amount() - expected).abs() > tolerance {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "QuantoOption notional ({}) must match quantity * payoff_fx_rate * strike ({})",
                        self.notional.amount(),
                        expected
                    )));
                }
            }
            (None, None) => {}
            _ => {
                return Err(finstack_quant_core::Error::Validation(
                    "QuantoOption requires both quantity and payoff_fx_rate when either is supplied".to_string(),
                ));
            }
        }
        if let Some(fx_spot_id) = &self.fx_spot_id {
            self.validate_fx_id_direction(fx_spot_id, "fx_spot_id")?;
        }
        if let Some(fx_vol_surface_id) = &self.fx_vol_surface_id {
            self.validate_fx_id_direction(fx_vol_surface_id.as_str(), "fx_vol_surface_id")?;
        }
        Ok(())
    }

    /// Reject FX market-data ids whose embedded pair name contradicts the
    /// required **quote-per-base** direction.
    ///
    /// The pricer interprets the referenced rate/vol in quote-per-base terms
    /// (conventional pair name `{base}{quote}`, e.g. `JPYUSD` for base JPY /
    /// quote USD). An id embedding the reversed pair (`USDJPY`) is the classic
    /// silent-inversion bug, so it fails validation. Ids that embed neither
    /// ordering are accepted (no naming magic beyond the contradiction check).
    fn validate_fx_id_direction(
        &self,
        fx_id: &str,
        field: &str,
    ) -> finstack_quant_core::Result<()> {
        let base = self.base_currency.to_string().to_uppercase();
        let quote = self.quote_currency.to_string().to_uppercase();
        let id_upper = fx_id.to_uppercase();
        let correct = format!("{base}{quote}");
        let reversed = format!("{quote}{base}");
        if id_upper.contains(&reversed) && !id_upper.contains(&correct) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "QuantoOption {field} '{fx_id}' embeds the pair {reversed}, but the quanto \
                 pricer requires quote-per-base market data ({correct}: {quote} per unit \
                 {base}). Re-quote the rate/vol in the {correct} direction or rename the id.",
            )));
        }
        Ok(())
    }
}

// Option risk metric providers (metrics adapters)

impl finstack_quant_valuations_core::instruments::OptionGreeksProvider for QuantoOption {
    fn option_greeks(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        request: &finstack_quant_valuations_core::instruments::OptionGreeksRequest,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::OptionGreeks>
    {
        let mut greeks = finstack_quant_valuations_core::instruments::OptionGreeks::default();
        match request.greek {
            finstack_quant_valuations_core::instruments::OptionGreekKind::Delta => {
                greeks.delta = self.option_delta(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Gamma => {
                greeks.gamma = self.option_gamma(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Vega => {
                greeks.vega = self.option_vega(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Theta => {
                greeks.theta = self.option_theta(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Rho => {
                greeks.rho_bp = self.option_rho_bp(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::ForeignRho => {
                greeks.foreign_rho_bp = self.option_foreign_rho_bp(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Vanna => {
                greeks.vanna = self.option_vanna(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Volga => {
                greeks.volga =
                    self.option_volga(market, as_of, request.require_base_pv()?, request.bumps)?
            }
        }
        Ok(greeks)
    }
}
impl QuantoOption {
    fn option_delta(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }
        Ok(Some(
            finstack_quant_valuations_core::metrics::central_diff_scalar_relative(
                self,
                market,
                as_of,
                &self.spot_id,
                bumps.spot_bump_decimal,
            )?,
        ))
    }
    fn option_gamma(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use finstack_quant_valuations_core::instruments::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();

        let spot_scalar = market.get_price(&self.spot_id)?;
        let current_spot =
            finstack_quant_valuations_core::metrics::scalar_numeric_value(spot_scalar);
        let bump_size = current_spot * bumps.spot_bump_decimal;
        if bump_size <= 0.0 {
            return Ok(Some(0.0));
        }

        let up = finstack_quant_valuations_core::metrics::bump_scalar_price(
            market,
            &self.spot_id,
            bumps.spot_bump_decimal,
        )?;
        let pv_up = self.value(&up, as_of)?.amount();
        let dn = finstack_quant_valuations_core::metrics::bump_scalar_price(
            market,
            &self.spot_id,
            -bumps.spot_bump_decimal,
        )?;
        let pv_dn = self.value(&dn, as_of)?.amount();

        Ok(Some(
            (pv_up - 2.0 * base_pv + pv_dn) / (bump_size * bump_size),
        ))
    }
    fn option_vega(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use finstack_quant_valuations_core::instruments::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        // Central difference on the equity vol source pricing reads
        // (implied-vol override, else the equity surface), per vol point.
        let (vol_bump, (up, market_up), (down, market_down)) = crate::shared::active_vol_scenarios(
            self,
            &self.instrument_pricing_overrides,
            market,
            self.vol_surface_id.as_str(),
            bumps.vol_bump_decimal,
        )?;
        let pv_up = up.value(&market_up, as_of)?.amount();
        let pv_down = down.value(&market_down, as_of)?.amount();
        let width =
            2.0 * vol_bump * finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((pv_up - pv_down) / width))
    }
    fn option_theta(
        &self,
        _market: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: finstack_quant_core::dates::Date,
        _bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Err(finstack_quant_core::Error::Validation(format!(
            "QuantoOption {}: Theta is not supported by the analytical quanto \
             pricer. Supported: Delta, Gamma, Vega, Rho, ForeignRho, Vanna, Volga.",
            self.id
        )))
    }
    fn option_rho_bp(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use finstack_quant_valuations_core::instruments::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let bump_bp = bumps.rate_bump_bp;
        let bumped = finstack_quant_valuations_core::metrics::bump_discount_curve_parallel(
            market,
            &self.domestic_discount_curve_id,
            bump_bp,
        )?;
        let pv_bumped = self.value(&bumped, as_of)?.amount();
        Ok(Some((pv_bumped - base_pv) / bump_bp))
    }
    fn option_foreign_rho_bp(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use finstack_quant_valuations_core::instruments::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let bump_bp = bumps.rate_bump_bp;
        let bumped = finstack_quant_valuations_core::metrics::bump_discount_curve_parallel(
            market,
            &self.foreign_discount_curve_id,
            bump_bp,
        )?;
        let pv_bumped = self.value(&bumped, as_of)?.amount();
        Ok(Some((pv_bumped - base_pv) / bump_bp))
    }
    fn option_vanna(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use finstack_quant_valuations_core::instruments::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let spot_scalar = market.get_price(&self.spot_id)?;
        let current_spot =
            finstack_quant_valuations_core::metrics::scalar_numeric_value(spot_scalar);
        let spot_bump = current_spot * bumps.spot_bump_decimal;
        if spot_bump <= 0.0 {
            return Ok(Some(0.0));
        }
        // Spot delta in each equity-vol scenario (central diff in spot).
        let (vol_bump, (inst_vol_up, market_vol_up), (inst_vol_dn, market_vol_dn)) =
            crate::shared::active_vol_scenarios(
                self,
                &self.instrument_pricing_overrides,
                market,
                self.vol_surface_id.as_str(),
                bumps.vol_bump_decimal,
            )?;
        let delta_at = |inst: &Self,
                        vol_market: &finstack_quant_core::market_data::context::MarketContext|
         -> finstack_quant_core::Result<f64> {
            let up = finstack_quant_valuations_core::metrics::bump_scalar_price(
                vol_market,
                &self.spot_id,
                bumps.spot_bump_decimal,
            )?;
            let dn = finstack_quant_valuations_core::metrics::bump_scalar_price(
                vol_market,
                &self.spot_id,
                -bumps.spot_bump_decimal,
            )?;
            let pv_up = inst.value(&up, as_of)?.amount();
            let pv_dn = inst.value(&dn, as_of)?.amount();
            Ok((pv_up - pv_dn) / (2.0 * spot_bump))
        };
        let delta_up = delta_at(&inst_vol_up, &market_vol_up)?;
        let delta_dn = delta_at(&inst_vol_dn, &market_vol_dn)?;

        // Report vanna per **vol point** on the σ axis (consistent with vega
        // and `MetricId::Vanna`): normalize by the bump width expressed in
        // vol points.
        let width =
            2.0 * vol_bump * finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((delta_up - delta_dn) / width))
    }
    fn option_volga(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        base_pv: f64,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use finstack_quant_valuations_core::instruments::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let (vol_bump, (up, market_up), (dn, market_dn)) = crate::shared::active_vol_scenarios(
            self,
            &self.instrument_pricing_overrides,
            market,
            self.vol_surface_id.as_str(),
            bumps.vol_bump_decimal,
        )?;
        let pv_up = up.value(&market_up, as_of)?.amount();
        let pv_dn = dn.value(&market_dn, as_of)?.amount();
        // Report volga per **vol point squared** (consistent with vega and
        // `MetricId::Volga`): normalize by the bump expressed in vol points.
        let width = vol_bump * finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((pv_up - 2.0 * base_pv + pv_dn) / (width * width)))
    }
}

impl finstack_quant_valuations_core::instruments::Instrument for QuantoOption {
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::QuantoOption,
        crate::FX
    );

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> finstack_quant_valuations_core::pricer::ModelKey {
        finstack_quant_valuations_core::pricer::ModelKey::QuantoBS
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.domestic_discount_curve_id.clone());
        deps.add_discount_curve(self.foreign_discount_curve_id.clone());
        deps.add_market_scalar_id(self.spot_id.as_str());
        deps.add_volatility_dependency(
            finstack_quant_valuations_core::instruments::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                Some(self.spot_id.clone()),
                Some(self.strike),
            ),
        );
        if let Some(dividend_yield) = &self.div_yield_id {
            deps.add_market_scalar_id(dividend_yield.as_str());
        }
        let fx_underlying_id = self.fx_spot_id.as_deref().map(|id| {
            deps.add_market_scalar_id(id);
            finstack_quant_core::types::PriceId::new(id)
        });
        if let Some(fx_volatility) = &self.fx_vol_surface_id {
            deps.add_volatility_dependency(
                finstack_quant_valuations_core::instruments::VolatilityDependency::new(
                    fx_volatility.clone(),
                    fx_underlying_id,
                    None,
                ),
            );
        }
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        use crate::quanto_option::pricer::QuantoOptionAnalyticalPricer;
        use finstack_quant_valuations_core::pricer::Pricer;

        self.validate()?;

        let pricer = QuantoOptionAnalyticalPricer;
        let result = pricer
            .price_dyn(self, market, as_of)
            .map_err(finstack_quant_core::Error::from)?;
        Ok(result.value)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.expiry)
    }

    fn valuation_details(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> Option<finstack_quant_valuations_core::results::ValuationDetails> {
        // Quanto FX policy stamp. `fx_spot_id` short-circuits the matrix,
        // so triangulation is unknown / not applicable in that case.
        let fx_triangulated = if self.fx_spot_id.is_some() {
            None
        } else {
            use finstack_quant_core::money::fx::FxQuery;
            market
                .fx()
                .and_then(|fx| {
                    fx.rate(FxQuery::new(self.base_currency, self.quote_currency, as_of))
                        .ok()
                })
                .map(|q| q.triangulated)
        };
        Some(
            finstack_quant_valuations_core::results::ValuationDetails::Fx(
                finstack_quant_valuations_core::results::FxValuationDetails { fx_triangulated },
            ),
        )
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

finstack_quant_valuations_core::impl_empty_cashflow_provider!(
    QuantoOption,
    finstack_quant_cashflows::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations_core::instruments::Instrument;

    #[test]
    fn test_quanto_option_example_creation() {
        let option = QuantoOption::example().expect("example");
        assert_eq!(option.id.as_str(), "QUANTO-NKY-USD-CALL");
        assert_eq!(option.quote_currency, Currency::USD);
        assert_eq!(option.base_currency, Currency::JPY);
        assert_eq!(option.quantity, Some(4_000.0));
        assert!(option.payoff_fx_rate.is_some());
        assert!(option.correlation < 0.0); // Negative correlation in example
    }

    #[test]
    fn test_quanto_option_market_dependencies() {
        let option = QuantoOption::example().expect("example");
        let deps = option
            .market_dependencies()
            .expect("market_dependencies")
            .curves;

        // Should include both domestic and foreign discount curves
        assert_eq!(deps.discount_curves.len(), 2);
        assert!(deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"));
        assert!(deps.discount_curves.iter().any(|c| c.as_str() == "JPY-OIS"));
    }

    #[test]
    fn canonical_dependencies_keep_equity_and_fx_volatility_contexts() {
        let option = QuantoOption::example().expect("example");
        let deps =
            finstack_quant_valuations_core::instruments::Instrument::market_dependencies(&option)
                .expect("dependencies");

        assert_eq!(
            deps.curves.discount_curves.as_slice(),
            &[
                option.domestic_discount_curve_id.clone(),
                option.foreign_discount_curve_id.clone(),
            ]
        );
        let mut expected_spots = vec![option.spot_id.as_str().to_string()];
        expected_spots.extend(option.div_yield_id.iter().map(|id| id.as_str().to_string()));
        expected_spots.push(option.fx_spot_id.clone().expect("FX spot id").to_string());
        assert_eq!(deps.market_scalar_ids, expected_spots);
        assert!(deps.series_ids.is_empty());
        assert_eq!(deps.volatility_dependencies.len(), 2);
        assert_eq!(
            deps.volatility_dependencies[0].spot_id.as_ref(),
            Some(&option.spot_id)
        );
        assert_eq!(
            deps.volatility_dependencies[0].reference_strike,
            Some(option.strike)
        );
        assert_eq!(
            deps.volatility_dependencies[1]
                .spot_id
                .as_ref()
                .map(|id| id.as_str()),
            option.fx_spot_id.as_deref()
        );
        assert_eq!(deps.fx_pairs[0].base, option.base_currency);
        assert_eq!(deps.fx_pairs[0].quote, option.quote_currency);
    }

    #[test]
    fn test_quanto_option_serde_rejects_inconsistent_notional() {
        let option = QuantoOption::example().expect("example");
        let mut json = serde_json::to_value(&option).expect("serialize");
        json["notional"]["amount"] = serde_json::json!("2000000");

        let err = serde_json::from_value::<QuantoOption>(json)
            .expect_err("inconsistent notional should fail during deserialization");
        assert!(
            err.to_string().contains("notional"),
            "error should mention notional consistency: {}",
            err
        );
    }
}
