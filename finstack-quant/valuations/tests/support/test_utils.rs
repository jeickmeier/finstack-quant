// Shared test helper fragment included by valuation, attribution, and statements tests.
use finstack_quant_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
// Shared helpers for unit tests to reduce boilerplate market setup.
use finstack_quant_core::{
    currency::Currency,
    dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor},
    market_data::context::MarketContext,
    market_data::{
        surfaces::VolSurface,
        term_structures::{DiscountCurve, ForwardCurve, PriceCurve},
    },
    money::Money,
    types::{CurveId, InstrumentId},
};
use rust_decimal::Decimal;
use time::Month;

use smallvec::SmallVec;
use finstack_quant_valuations::instruments::{EquityUnderlyingParams, FixedLegSpec, FloatLegSpec, FxUnderlyingParams};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::credit_derivatives::cds::{
    CDSConvention, CreditDefaultSwap, PayReceive, PremiumLegSpec,
    ProtectionLegSpec,
};
use finstack_quant_valuations::instruments::rates::irs::{FloatingLegCompounding, InterestRateSwap};
use finstack_quant_valuations::instruments::{
    Attributes, EquityOption, ExerciseStyle, FxOption, OptionType, InstrumentPricingOverrides,
    SettlementType,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
use std::sync::OnceLock;

/// Convenience date helper for tests.
pub fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
        .expect("valid date")
}

/// Create a USD IRS swap using the builder pattern.
pub fn usd_irs_swap(
    id: impl Into<InstrumentId>,
    notional: Money,
    fixed_rate: f64,
    start: Date,
    end: Date,
    side: PayReceive,
) -> finstack_quant_core::Result<InterestRateSwap> {
    let rate_decimal = Decimal::try_from(fixed_rate).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "Invalid fixed rate: {} cannot be converted to Decimal. \
             Check for NaN, infinity, or values exceeding Decimal range.",
            fixed_rate
        ))
    })?;

    let fixed = FixedLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        rate: rate_decimal,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Thirty360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".to_string()),
        stub: StubKind::None,
        start,
        end,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,    };

    let float = FloatLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: Decimal::ZERO,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".to_string()),
        stub: StubKind::None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        start,
        end,
        compounding: FloatingLegCompounding::Simple,
        payment_lag_days: 0,
        end_of_month: false,    };

    let swap = InterestRateSwap::builder()
        .id(id.into())
        .notional(notional)
        .side(side)
        .fixed(fixed)
        .float(float)
        .build()?;

    swap.validate()?;
    Ok(swap)
}

/// Lightweight instrument stub for attribution and metrics tests.
#[derive(Clone)]
pub struct TestInstrument {
    id: String,
    value: Money,
    discount_curves: SmallVec<[CurveId; 2]>,
}

impl TestInstrument {
    pub fn new(id: &str, value: Money) -> Self {
        Self {
            id: id.to_string(),
            value,
            discount_curves: SmallVec::<[CurveId; 2]>::new(),
        }
    }

    pub fn with_discount_curves(mut self, curves: &[&str]) -> Self {
        self.discount_curves = curves.iter().map(|id| CurveId::new(*id)).collect();
        self
    }
}

impl Instrument for TestInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::Bond
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        static ATTRS: OnceLock<Attributes> = OnceLock::new();
        ATTRS.get_or_init(Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        unreachable!("TestInstrument::attributes_mut should not be called in tests")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies> {
        let mut deps =
            finstack_quant_valuations::instruments::MarketDependencies::new();
        for curve in &self.discount_curves {
            deps.add_discount_curve(curve.clone());
        }
        Ok(deps)
    }

    fn base_value(&self, _market: &MarketContext, _as_of: Date) -> finstack_quant_core::Result<Money> {
        Ok(self.value)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_core::Result<ValuationResult> {
        let value = self.value(market, as_of)?;
        Ok(ValuationResult::stamped(self.id(), as_of, value))
    }
}

/// Create an Equity European call option using the builder pattern.
pub fn equity_option_european_call(
    id: impl Into<String>,
    ticker: impl Into<String>,
    strike: f64,
    expiry: Date,
    contract_size: f64,
) -> finstack_quant_core::Result<EquityOption> {
    let ticker = ticker.into();
    let underlying = EquityUnderlyingParams::new(ticker, "EQUITY-SPOT", Currency::USD)
        .with_dividend_yield("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .notional(Money::new(contract_size, underlying.currency))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id(underlying.spot_id)
        .vol_surface_id(CurveId::new("EQUITY-VOL"))
        .div_yield_id_opt(underlying.div_yield_id)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}

/// Create an Equity European put option using the builder pattern.
pub fn equity_option_european_put(
    id: impl Into<String>,
    ticker: impl Into<String>,
    strike: f64,
    expiry: Date,
    contract_size: f64,
) -> finstack_quant_core::Result<EquityOption> {
    let ticker = ticker.into();
    let underlying = EquityUnderlyingParams::new(ticker, "EQUITY-SPOT", Currency::USD)
        .with_dividend_yield("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Put)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .notional(Money::new(contract_size, underlying.currency))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id(underlying.spot_id)
        .vol_surface_id(CurveId::new("EQUITY-VOL"))
        .div_yield_id_opt(underlying.div_yield_id)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}

/// Create an Equity American call option using the builder pattern.
pub fn equity_option_american_call(
    id: impl Into<String>,
    ticker: impl Into<String>,
    strike: f64,
    expiry: Date,
    contract_size: f64,
) -> finstack_quant_core::Result<EquityOption> {
    let ticker = ticker.into();
    let underlying = EquityUnderlyingParams::new(ticker, "EQUITY-SPOT", Currency::USD)
        .with_dividend_yield("EQUITY-DIVYIELD");

    EquityOption::builder()
        .id(InstrumentId::new(id.into()))
        .underlying_ticker(underlying.ticker)
        .strike(strike)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::American)
        .expiry(expiry)
        .notional(Money::new(contract_size, underlying.currency))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id(underlying.spot_id)
        .vol_surface_id(CurveId::new("EQUITY-VOL"))
        .div_yield_id_opt(underlying.div_yield_id)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}

/// Create an FX European call option using the builder pattern.
pub fn fx_option_european_call(
    id: impl Into<InstrumentId>,
    base_currency: Currency,
    quote_currency: Currency,
    strike: f64,
    expiry: Date,
    notional: Money,
    vol_surface_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<FxOption> {
    let fx_underlying = if quote_currency == Currency::USD && base_currency == Currency::EUR {
        FxUnderlyingParams::usd_eur()
    } else if quote_currency == Currency::USD && base_currency == Currency::GBP {
        FxUnderlyingParams::gbp_usd()
    } else {
        let domestic = CurveId::new(format!("{}-OIS", quote_currency));
        let foreign = CurveId::new(format!("{}-OIS", base_currency));
        FxUnderlyingParams::new(base_currency, quote_currency, domestic, foreign)
    };

    FxOption::builder()
        .id(id.into())
        .base_currency(fx_underlying.base_currency)
        .quote_currency(fx_underlying.quote_currency)
        .strike(strike)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .notional(notional)
        .settlement(SettlementType::Cash)
        .domestic_discount_curve_id(fx_underlying.domestic_discount_curve_id)
        .foreign_discount_curve_id(fx_underlying.foreign_discount_curve_id)
        .vol_surface_id(vol_surface_id.into())
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}

/// Create an FX European put option using the builder pattern.
pub fn fx_option_european_put(
    id: impl Into<InstrumentId>,
    base_currency: Currency,
    quote_currency: Currency,
    strike: f64,
    expiry: Date,
    notional: Money,
    vol_surface_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<FxOption> {
    let fx_underlying = if quote_currency == Currency::USD && base_currency == Currency::EUR {
        FxUnderlyingParams::usd_eur()
    } else if quote_currency == Currency::USD && base_currency == Currency::GBP {
        FxUnderlyingParams::gbp_usd()
    } else {
        let domestic = CurveId::new(format!("{}-OIS", quote_currency));
        let foreign = CurveId::new(format!("{}-OIS", base_currency));
        FxUnderlyingParams::new(base_currency, quote_currency, domestic, foreign)
    };

    FxOption::builder()
        .id(id.into())
        .base_currency(fx_underlying.base_currency)
        .quote_currency(fx_underlying.quote_currency)
        .strike(strike)
        .option_type(OptionType::Put)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .notional(notional)
        .settlement(SettlementType::Cash)
        .domestic_discount_curve_id(fx_underlying.domestic_discount_curve_id)
        .foreign_discount_curve_id(fx_underlying.foreign_discount_curve_id)
        .vol_surface_id(vol_surface_id.into())
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
}

/// Create a CDS buy protection position using the builder pattern.
#[allow(clippy::too_many_arguments)]
pub fn cds_buy_protection(
    id: impl Into<InstrumentId>,
    notional: Money,
    spread_bp: f64,
    start: Date,
    maturity: Date,
    discount_curve_id: impl Into<CurveId>,
    credit_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<CreditDefaultSwap> {
    let convention = CDSConvention::IsdaNa;
    let dc = convention.day_count();
    let freq = convention.frequency();
    let bdc = convention.business_day_convention();
    let stub = convention.stub_convention();

    let spread_bp_decimal = Decimal::try_from(spread_bp).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "spread_bp {} cannot be represented as Decimal: {}",
            spread_bp, e
        ))
    })?;

    let cds = CreditDefaultSwap::builder()
        .id(id.into())
        .notional(notional)
        .side(PayReceive::Pay)
        .convention(convention)
        .premium(PremiumLegSpec {
            start,
            end: maturity,
            frequency: freq,
            stub,
            bdc,
            calendar_id: Some(convention.default_calendar().to_string()),
            day_count: dc,
            spread_bp: spread_bp_decimal,
            discount_curve_id: discount_curve_id.into(),
        })
        .protection(ProtectionLegSpec {
            credit_curve_id: credit_id.into(),
            recovery_rate: STANDARD_RECOVERY_SENIOR,
            settlement_delay: convention.settlement_delay(),
        })
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()?;

    cds.validate()?;
    Ok(cds)
}

/// Create a CDS sell protection position using the builder pattern.
#[allow(clippy::too_many_arguments)]
pub fn cds_sell_protection(
    id: impl Into<InstrumentId>,
    notional: Money,
    spread_bp: f64,
    start: Date,
    maturity: Date,
    discount_curve_id: impl Into<CurveId>,
    credit_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<CreditDefaultSwap> {
    let convention = CDSConvention::IsdaNa;
    let dc = convention.day_count();
    let freq = convention.frequency();
    let bdc = convention.business_day_convention();
    let stub = convention.stub_convention();

    let spread_bp_decimal = Decimal::try_from(spread_bp).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "spread_bp {} cannot be represented as Decimal: {}",
            spread_bp, e
        ))
    })?;

    let cds = CreditDefaultSwap::builder()
        .id(id.into())
        .notional(notional)
        .side(PayReceive::Receive)
        .convention(convention)
        .premium(PremiumLegSpec {
            start,
            end: maturity,
            frequency: freq,
            stub,
            bdc,
            calendar_id: Some(convention.default_calendar().to_string()),
            day_count: dc,
            spread_bp: spread_bp_decimal,
            discount_curve_id: discount_curve_id.into(),
        })
        .protection(ProtectionLegSpec {
            credit_curve_id: credit_id.into(),
            recovery_rate: STANDARD_RECOVERY_SENIOR,
            settlement_delay: convention.settlement_delay(),
        })
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()?;

    cds.validate()?;
    Ok(cds)
}

/// Build a flat discount curve with two knots: (0, 1.0) and (1y, exp(-rate)).
pub fn flat_discount(id: &str, as_of: Date, rate: f64) -> DiscountCurve {
    flat_discount_with_tenor(id, as_of, rate, 1.0)
}

/// Build a flat discount curve with a configurable far-tenor knot.
pub fn flat_discount_with_tenor(
    id: &str,
    as_of: Date,
    rate: f64,
    tenor_years: f64,
) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(as_of)
        .knots([(0.0, 1.0), (tenor_years, (-rate * tenor_years).exp())])
        .build()
        .expect("discount curve should build in tests")
}

/// Build a flat forward curve with two knots and a constant rate.
pub fn flat_forward_with_tenor(id: &str, as_of: Date, rate: f64, tenor_years: f64) -> ForwardCurve {
    ForwardCurve::builder(id, tenor_years)
        .base_date(as_of)
        .knots([(0.0, rate), (tenor_years, rate)])
        .build()
        .expect("forward curve should build in tests")
}

/// Build a flat price curve with a constant price level (for commodity forward prices).
pub fn flat_price_curve(id: &str, as_of: Date, price: f64, tenor_years: f64) -> PriceCurve {
    PriceCurve::builder(id)
        .base_date(as_of)
        .spot_price(price)
        .knots([(0.0, price), (tenor_years, price)])
        .build()
        .expect("price curve should build in tests")
}

/// Build a contango price curve (forward prices increase with time).
pub fn contango_price_curve(
    id: &str,
    as_of: Date,
    spot: f64,
    carry_rate: f64,
    tenor_years: f64,
) -> PriceCurve {
    // F(T) = S * exp(r * T)
    let far_price = spot * (carry_rate * tenor_years).exp();
    PriceCurve::builder(id)
        .base_date(as_of)
        .spot_price(spot)
        .knots([(0.0, spot), (tenor_years, far_price)])
        .build()
        .expect("price curve should build in tests")
}

/// Build a constant vol surface using provided expiries/strikes grid.
pub fn flat_vol_surface(id: &str, expiries: &[f64], strikes: &[f64], vol: f64) -> VolSurface {
    let mut builder = VolSurface::builder(id).expiries(expiries).strikes(strikes);
    for _ in expiries {
        builder = builder.row(&vec![vol; strikes.len()]);
    }
    builder.build().expect("vol surface should build in tests")
}

/// Calibration-specific helpers for integration tests.
pub mod calibration {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::{
        CurveState, MarketContext, MarketContextState,
    };
    use finstack_quant_core::Result;
    use finstack_quant_valuations::calibration::api::engine;
    use finstack_quant_valuations::calibration::api::market_datum::{
        CollateralEntry, DividendScheduleDatum, FxSpotDatum, MarketDatum, PriceDatum,
    };
    use finstack_quant_valuations::calibration::api::prior_market::PriorMarketObject;
    use finstack_quant_valuations::calibration::api::schema::{
        CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams, CALIBRATION_SCHEMA,
    };
    use finstack_quant_valuations::calibration::{CalibrationConfig, CalibrationReport};
    use finstack_quant_valuations::market::quotes::ids::QuoteId;
    use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;

    /// Extract the `QuoteId` of each [`MarketQuote`] in the slice.
    ///
    /// Used to build v3 `quote_sets: HashMap<String, Vec<QuoteId>>` from the
    /// legacy `Vec<MarketQuote>` form. Pair with [`extend_market_data`] to
    /// also flatten the quotes onto `market_data`.
    pub fn quote_set_ids(quotes: &[MarketQuote]) -> Vec<QuoteId> {
        quotes
            .iter()
            .map(|q| match q {
                MarketQuote::Rates(q) => q.id().clone(),
                MarketQuote::Cds(q) => q.id().clone(),
                MarketQuote::CDSTranche(q) => q.id().clone(),
                MarketQuote::Fx(q) => q.id().clone(),
                MarketQuote::Inflation(q) => q.id().clone(),
                MarketQuote::Vol(q) => q.id().clone(),
                MarketQuote::Xccy(q) => q.id().clone(),
                MarketQuote::Bond(q) => q.id().clone(),
            })
            .collect()
    }

    /// Append the `MarketDatum::from(quote)` projection of every quote in
    /// `quotes` to `market_data`. Companion to [`quote_set_ids`].
    pub fn extend_market_data(market_data: &mut Vec<MarketDatum>, quotes: &[MarketQuote]) {
        market_data.extend(quotes.iter().cloned().map(MarketDatum::from));
    }

    fn split_market_context_state(
        state: MarketContextState,
    ) -> Result<(Vec<PriorMarketObject>, Vec<MarketDatum>)> {
        let mut prior = state
            .curves
            .into_iter()
            .map(|curve| match curve {
                CurveState::Discount(c) => PriorMarketObject::DiscountCurve(c),
                CurveState::Forward(c) => PriorMarketObject::ForwardCurve(c),
                CurveState::Hazard(c) => PriorMarketObject::HazardCurve(c),
                CurveState::Inflation(c) => PriorMarketObject::InflationCurve(c),
                CurveState::BaseCorrelation(c) => PriorMarketObject::BaseCorrelationCurve(c),
                CurveState::BasisSpread(c) => PriorMarketObject::BasisSpreadCurve(c),
                CurveState::Parametric(c) => PriorMarketObject::ParametricCurve(c),
                CurveState::Price(c) => PriorMarketObject::PriceCurve(c),
                CurveState::VolIndex(c) => PriorMarketObject::VolatilityIndexCurve(c),
            })
            .collect::<Vec<_>>();
        prior.extend(
            state
                .surfaces
                .into_iter()
                .map(PriorMarketObject::VolSurface),
        );

        let mut data = Vec::new();
        if let Some(fx) = state.fx {
            data.extend(fx.quotes.into_iter().map(|(from, to, rate)| {
                MarketDatum::FxSpot(FxSpotDatum {
                    id: format!("{from}/{to}"),
                    from,
                    to,
                    rate,
                })
            }));
        }
        data.extend(
            state
                .prices
                .into_iter()
                .map(|(id, scalar)| MarketDatum::Price(PriceDatum { id, scalar })),
        );
        data.extend(state.series.into_iter().map(MarketDatum::FixingSeries));
        data.extend(
            state
                .inflation_indices
                .into_iter()
                .map(MarketDatum::InflationFixings),
        );
        data.extend(state.dividends.into_iter().map(|schedule| {
            MarketDatum::DividendSchedule(DividendScheduleDatum { schedule })
        }));
        data.extend(
            state
                .credit_indices
                .into_iter()
                .map(MarketDatum::CreditIndex),
        );
        data.extend(
            state
                .fx_delta_vol_surfaces
                .into_iter()
                .map(MarketDatum::FxVolSurface),
        );
        data.extend(state.vol_cubes.into_iter().map(MarketDatum::VolCube));
        for (currency, csa_currency) in state.collateral {
            data.push(MarketDatum::Collateral(CollateralEntry {
                id: parse_snapshot_currency(&currency, "collateral currency")?,
                csa_currency: parse_snapshot_currency(&csa_currency, "CSA currency")?,
            }));
        }
        Ok((prior, data))
    }

    fn parse_snapshot_currency(value: &str, field: &str) -> Result<Currency> {
        value
            .parse()
            .map_err(|err| finstack_quant_core::Error::Calibration {
                message: format!("Invalid {field} in market context snapshot: '{value}' ({err})"),
                category: "market_context_split".to_string(),
            })
    }

    /// Split a [`MarketContext`] into the v3 envelope's `(prior_market,
    /// market_data)` pair. Pure-Rust convenience for tests that historically
    /// built `initial_market: Some((&ctx).into())`.
    pub fn split_initial_market(
        ctx: &MarketContext,
    ) -> (Vec<PriorMarketObject>, Vec<MarketDatum>) {
        split_market_context_state(MarketContextState::from(ctx))
            .expect("valid market context snapshot")
    }

    /// Execute a single calibration step for tests/benchmarks without engaging the full plan engine.
    pub fn execute_step(
        params: &StepParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, CalibrationReport)> {
        // Split the legacy market context snapshot into the v3 envelope
        // inputs: pre-built calibrated objects (`prior`) and flat market data
        // (`data`).
        let (prior, mut data) = split_market_context_state(MarketContextState::from(context))?;

        // Build the `default` quote set and flatten its quotes onto market data.
        let ids = quote_set_ids(quotes);
        extend_market_data(&mut data, quotes);

        let mut quote_sets = finstack_quant_core::HashMap::default();
        quote_sets.insert("default".to_string(), ids);

        let plan = CalibrationPlan {
            id: "test-plan".to_string(),
            description: None,
            quote_sets,
            steps: vec![CalibrationStep {
                id: "step-0".to_string(),
                quote_set: "default".to_string(),
                params: params.clone(),
            }],
            settings: global_config.clone(),
        };

        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: CALIBRATION_SCHEMA.to_string(),
            plan,
            market_data: data,
            prior_market: prior,
        };

        let result = engine::execute(&envelope)?;
        let market = MarketContext::try_from(result.result.final_market)?;
        Ok((market, result.result.report))
    }

    #[test]
    fn market_context_split_rejects_malformed_collateral_currency() {
        let mut state = MarketContextState::from(&MarketContext::new());
        state
            .collateral
            .insert("NOT_A_CURRENCY".to_string(), "USD".to_string());

        let err = split_market_context_state(state).expect_err("invalid currency should error");
        assert!(err.to_string().contains("Invalid collateral currency"));
        assert!(err.to_string().contains("NOT_A_CURRENCY"));
    }
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    TestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);
