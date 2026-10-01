//! Gaussian-copula pricer for CMS spread options.
//!
//! Each CMS rate has a lognormal payment-measure marginal with its mean set by
//! the shared first-order CMS convexity approximation. The two rates are coupled
//! by a Gaussian copula. Only Black volatility surfaces constant across strike
//! at the requested expiry are supported; tenor-axis ATM surfaces are also valid.
//! Smile cubes and nonflat smiles are rejected because a smile-to-payment-measure
//! distribution is not implemented. This is a flat-volatility approximation,
//! not SABR static replication. Conditional Black integration of the short rate
//! leaves one 20-node Gaussian quadrature over the long rate.

use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_spread_option::CmsSpreadOption;
use crate::instruments::OptionType;
use crate::market::resolve_vol_source;
use crate::metrics::MetricId;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurfaceAxis;
use finstack_quant_core::math::GaussHermiteQuadrature;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::volatility::{black_call, black_put};
use finstack_quant_models::volatility::VolSource;

const DEFAULT_QUADRATURE_ORDER: usize = 20;
const MIN_POSITIVE_RATE: f64 = 1.0e-8;
const MIN_VOL: f64 = 1.0e-8;
const FLAT_VOL_TOLERANCE: f64 = 1.0e-12;

#[derive(Debug, Clone, Copy)]
struct CmsSpreadLeg {
    forward_rate: f64,
    adjusted_forward_rate: f64,
    convexity_adjustment: f64,
    atm_volatility: f64,
    time_to_expiry: f64,
}

#[derive(Debug, Clone, Copy)]
struct CmsSpreadPricingData {
    long_leg: CmsSpreadLeg,
    short_leg: CmsSpreadLeg,
    discount_factor: f64,
    expected_payoff: f64,
}

/// CMS spread option pricer using Gaussian copula and flat-volatility lognormal marginals.
///
/// The payment-measure means use the shared first-order CMS convexity
/// approximation. Nonflat strike smiles and SABR cubes return a validation error;
/// this engine does not transform smile distributions between measures.
#[derive(Debug, Clone)]
pub struct CmsSpreadOptionPricer {
    quadrature_order: usize,
}

impl CmsSpreadOptionPricer {
    /// Create a pricer with the default quadrature order.
    pub fn new() -> Self {
        Self {
            quadrature_order: DEFAULT_QUADRATURE_ORDER,
        }
    }

    fn price_data(
        &self,
        inst: &CmsSpreadOption,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<CmsSpreadPricingData> {
        inst.validate()?;
        if inst.payment_date <= as_of {
            return Ok(CmsSpreadPricingData {
                long_leg: CmsSpreadLeg::zero(),
                short_leg: CmsSpreadLeg::zero(),
                discount_factor: 0.0,
                expected_payoff: 0.0,
            });
        }

        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
        let discount_factor =
            relative_df_discount_curve(discount_curve.as_ref(), as_of, inst.payment_date)?;

        // Seasoned options (expiry already past) have zero time to expiry;
        // the year fraction is only defined for `as_of <= expiry`.
        let time_to_expiry = if inst.expiry <= as_of {
            0.0
        } else {
            // Option expiry is calendar time: ACT/365F, not the accrual day count.
            DayCount::Act365F
                .year_fraction(as_of, inst.expiry, DayCountContext::default())?
                .max(0.0)
        };

        let long_vol = resolve_vol_source(market, inst.long_vol_surface_id.as_ref())?;
        let short_vol = resolve_vol_source(market, inst.short_vol_surface_id.as_ref())?;
        let long_leg = self.resolve_leg(
            inst,
            market,
            as_of,
            inst.long_cms_tenor,
            time_to_expiry,
            &long_vol,
        )?;
        let short_leg = self.resolve_leg(
            inst,
            market,
            as_of,
            inst.short_cms_tenor,
            time_to_expiry,
            &short_vol,
        )?;

        let expected_payoff = if time_to_expiry <= 0.0 {
            cms_spread_payoff(
                long_leg.forward_rate,
                short_leg.forward_rate,
                inst.strike_rate()?,
                inst.option_type,
            )
        } else {
            self.expected_payoff(inst, &long_leg, &short_leg)?
        };

        Ok(CmsSpreadPricingData {
            long_leg,
            short_leg,
            discount_factor,
            expected_payoff,
        })
    }

    fn resolve_leg(
        &self,
        inst: &CmsSpreadOption,
        market: &MarketContext,
        as_of: Date,
        tenor: Tenor,
        time_to_expiry: f64,
        vol_provider: &VolSource,
    ) -> Result<CmsSpreadLeg> {
        let tenor_years = tenor.to_years();

        // Seasoned option: both CMS rates fixed at the (past) expiry. Resolve
        // the leg from the recorded fixing (mirroring the cap/floor pricer) —
        // never re-project from the live curve, which books phantom P&L. The
        // rate is known, so there is no convexity adjustment and the payoff
        // collapses to intrinsic on the observed rates.
        if inst.expiry < as_of {
            let observed = crate::instruments::rates::hw1f::fixings::historical_cms_fixing(
                market,
                &inst.forward_curve_id,
                tenor_years,
                inst.expiry,
            )?;
            return Ok(CmsSpreadLeg {
                forward_rate: observed,
                adjusted_forward_rate: observed,
                convexity_adjustment: 0.0,
                atm_volatility: 0.0,
                time_to_expiry: 0.0,
            });
        }

        let tenor_months = tenor.months().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "CmsSpreadOption tenor {} must be month- or year-based",
                tenor
            ))
        })?;
        let reference_swap = inst.reference_swap();
        let swap_start = reference_swap.reference_swap_start(inst.expiry)?;
        let swap_end = swap_start.add_months(i32::try_from(tenor_months).map_err(|_| {
            finstack_quant_core::Error::Validation("CMS tenor months exceed supported range".into())
        })?)?;
        // Project the CMS forward swap rate on the instrument's resolved swap
        // conventions (explicit fields override the required index identifier).
        let (forward_rate, _) =
            reference_swap.forward_rate_and_annuity(market, as_of, swap_start, swap_end)?;
        if forward_rate <= 0.0 || !forward_rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CmsSpreadOption forward CMS rate must be positive and finite, got {}",
                forward_rate
            )));
        }

        let atm_volatility =
            clean_volatility(vol_provider, time_to_expiry, tenor_years, forward_rate)?;
        if time_to_expiry > 0.0 {
            validate_flat_marginal(vol_provider, time_to_expiry, tenor_years, atm_volatility)?;
        }
        let payment_delay = (inst.payment_date - swap_start).whole_days() as f64 / 365.0;
        let convexity = if time_to_expiry > 0.0 {
            crate::instruments::rates::cms_option::pricer::convexity_adjustment_with_frequency(
                atm_volatility,
                time_to_expiry,
                tenor_years,
                forward_rate,
                reference_swap.payments_per_year()?,
                payment_delay,
            )
        } else {
            0.0
        };
        let adjusted_forward_rate = forward_rate + convexity;
        if adjusted_forward_rate <= 0.0 || !adjusted_forward_rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CmsSpreadOption convexity-adjusted CMS rate must be positive and finite, got {}",
                adjusted_forward_rate
            )));
        }

        Ok(CmsSpreadLeg {
            forward_rate,
            adjusted_forward_rate,
            convexity_adjustment: convexity,
            atm_volatility,
            time_to_expiry,
        })
    }

    fn expected_payoff(
        &self,
        inst: &CmsSpreadOption,
        long_leg: &CmsSpreadLeg,
        short_leg: &CmsSpreadLeg,
    ) -> Result<f64> {
        let quadrature = GaussHermiteQuadrature::new(self.quadrature_order)?;
        let rho = inst.correlation;
        let short_stddev = short_leg.atm_volatility * short_leg.time_to_expiry.sqrt();
        let conditional_stddev = short_stddev * (1.0 - rho * rho).sqrt();

        let strike = inst.strike_rate()?;
        let expected = quadrature.integrate(|z_long| {
            let long_rate = quantile_from_gaussian(long_leg, z_long);
            let short_conditional_mean = short_leg.adjusted_forward_rate
                * (short_stddev * rho * z_long - 0.5 * (short_stddev * rho).powi(2)).exp();
            // Conditional on the long-rate normal, the short rate is
            // lognormal. Integrating it with Black removes the payoff kink
            // from the inner dimension and leaves one Gaussian expectation.
            conditional_spread_payoff(
                short_conditional_mean,
                long_rate - strike,
                conditional_stddev,
                inst.option_type,
            )
        });

        if !expected.is_finite() || expected < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CmsSpreadOption expected payoff is invalid: {}",
                expected
            )));
        }
        Ok(expected)
    }

    pub(crate) fn price_internal(
        &self,
        inst: &CmsSpreadOption,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        let data = self.price_data(inst, market, as_of)?;
        Money::new(
            data.expected_payoff * data.discount_factor * inst.notional.amount(),
            inst.notional.currency(),
        )
    }
}

impl Default for CmsSpreadOptionPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl CmsSpreadLeg {
    fn zero() -> Self {
        Self {
            forward_rate: 0.0,
            adjusted_forward_rate: 0.0,
            convexity_adjustment: 0.0,
            atm_volatility: 0.0,
            time_to_expiry: 0.0,
        }
    }
}

impl Pricer for CmsSpreadOptionPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CmsSpreadOption, ModelKey::StaticReplication)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let option = crate::pricer::expect_inst::<CmsSpreadOption>(
            instrument,
            InstrumentType::CmsSpreadOption,
        )?;
        let data = self.price_data(option, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(instrument)
                    .model(ModelKey::StaticReplication)
                    .curve_ids([
                        option.discount_curve_id.as_str().to_string(),
                        option.forward_curve_id.as_str().to_string(),
                        option.long_vol_surface_id.as_str().to_string(),
                        option.short_vol_surface_id.as_str().to_string(),
                    ]),
            )
        })?;

        let value = Money::new(
            data.expected_payoff * data.discount_factor * option.notional.amount(),
            option.notional.currency(),
        )
        .map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(instrument),
            )
        })?;
        let mut result = ValuationResult::stamped(option.id.as_str(), as_of, value);
        result.measures.insert(
            MetricId::custom("long_cms_forward"),
            data.long_leg.forward_rate,
        );
        result.measures.insert(
            MetricId::custom("short_cms_forward"),
            data.short_leg.forward_rate,
        );
        result.measures.insert(
            MetricId::custom("long_cms_convexity_adjustment"),
            data.long_leg.convexity_adjustment,
        );
        result.measures.insert(
            MetricId::custom("short_cms_convexity_adjustment"),
            data.short_leg.convexity_adjustment,
        );
        result.measures.insert(
            MetricId::custom("cms_spread_forward"),
            data.long_leg.adjusted_forward_rate - data.short_leg.adjusted_forward_rate,
        );
        result.measures.insert(
            MetricId::custom("cms_spread_correlation"),
            option.correlation,
        );
        Ok(result)
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        let option = crate::pricer::expect_inst::<CmsSpreadOption>(
            instrument,
            InstrumentType::CmsSpreadOption,
        )?;
        self.price_internal(option, market, as_of)
            .map(|m| m.amount())
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(instrument)
                        .model(ModelKey::StaticReplication),
                )
            })
    }
}

fn cms_spread_payoff(long_rate: f64, short_rate: f64, strike: f64, option_type: OptionType) -> f64 {
    let spread = long_rate - short_rate;
    match option_type {
        OptionType::Call => (spread - strike).max(0.0),
        OptionType::Put => (strike - spread).max(0.0),
    }
}

fn conditional_spread_payoff(
    short_mean: f64,
    long_less_strike: f64,
    short_stddev: f64,
    option_type: OptionType,
) -> f64 {
    // A spread call is a put on the conditional short rate, and vice versa.
    // A nonpositive threshold cannot exceed a positive short rate; this also
    // handles negative spread strikes without taking a logarithm of zero.
    if long_less_strike <= 0.0 {
        return match option_type {
            OptionType::Call => 0.0,
            OptionType::Put => short_mean - long_less_strike,
        };
    }
    match option_type {
        OptionType::Call => black_put(short_mean, long_less_strike, short_stddev, 1.0),
        OptionType::Put => black_call(short_mean, long_less_strike, short_stddev, 1.0),
    }
}

fn quantile_from_gaussian(leg: &CmsSpreadLeg, z: f64) -> f64 {
    let variance = leg.atm_volatility * leg.atm_volatility * leg.time_to_expiry;
    // Exact lognormal quantile: E[exp(sigma Z - sigma^2/2)] = 1. The
    // distribution's mean is therefore the same CMS forward reported to callers.
    leg.adjusted_forward_rate * (variance.sqrt() * z - 0.5 * variance).exp()
}

fn validate_flat_marginal(
    provider: &VolSource,
    expiry: f64,
    tenor: f64,
    atm_volatility: f64,
) -> Result<()> {
    let unsupported = || {
        finstack_quant_core::Error::Validation(format!(
        "CmsSpreadOption requires a flat-strike Black volatility surface; source {} has an unsupported smile (SABR/payment-measure smile mapping is not implemented)",
        provider.get_id()
    ))
    };
    let VolSource::Surface(surface) = provider else {
        return Err(unsupported());
    };
    // A tenor axis already represents one strike-independent volatility per
    // expiry/tenor. For strike surfaces, every knot must agree; linear strike
    // interpolation and flat extrapolation then preserve that constant slice.
    if surface.secondary_axis() == VolSurfaceAxis::Strike {
        for &strike in surface.strikes() {
            let vol = clean_volatility(provider, expiry, tenor, strike)?;
            if (vol - atm_volatility).abs() > FLAT_VOL_TOLERANCE {
                return Err(unsupported());
            }
        }
    }
    Ok(())
}

fn clean_volatility(vol_provider: &VolSource, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
    if vol_provider.get_convention(expiry, tenor)?
        != finstack_quant_models::volatility::VolatilityConvention::Lognormal
    {
        return Err(finstack_quant_core::Error::Validation(
            "CmsSpreadOption requires unshifted Black volatility quotes".into(),
        ));
    }
    let vol =
        vol_provider.get_vol_clamped(expiry.max(0.0), tenor, strike.max(MIN_POSITIVE_RATE))?;
    if vol <= 0.0 || !vol.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CmsSpreadOption volatility source {} returned invalid vol {}",
            vol_provider.get_id(),
            vol
        )));
    }
    Ok(vol.max(MIN_VOL))
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod date_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/date.rs"
        ));
    }
    #[allow(dead_code, unused_imports)]
    mod discount_forward_curve_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/discount_forward_curves.rs"
        ));
    }

    use super::*;
    use date_support::date;
    use discount_forward_curve_support::flat_discount_with_tenor;
    use finstack_quant_core::market_data::fixings::cms_fixing_series_id;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::types::CurveId;

    fn flat_vol_surface(id: &str, vol: f64) -> VolSurface {
        let strikes = vec![0.005, 0.02, 0.04, 0.08];
        let expiries = vec![0.25, 1.0, 5.0, 10.0];
        let mut builder = VolSurface::builder(CurveId::new(id))
            .expiries(&expiries)
            .strikes(&strikes);
        for _ in 0..expiries.len() {
            builder = builder.row(&vec![vol; strikes.len()]);
        }
        builder.build().expect("vol surface")
    }

    /// A seasoned CMS spread option (expiry in the past, payment in the
    /// future) must resolve both legs from the recorded fixings — and a
    /// missing fixing series must be a hard error, never silent live-curve
    /// projection.
    #[test]
    fn seasoned_spread_option_uses_recorded_fixings() {
        let expiry = date(2024, 12, 1);
        let as_of = date(2025, 1, 1);
        let payment = date(2025, 3, 1);

        let mut inst = CmsSpreadOption::example().expect("example");
        inst.expiry = expiry;
        inst.payment_date = payment;
        inst.strike = rust_decimal::Decimal::ZERO;
        inst.option_type = OptionType::Call;

        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 1.0))
            .insert_surface(flat_vol_surface("USD-SWAPTION-VOL-10Y", 0.25))
            .insert_surface(flat_vol_surface("USD-SWAPTION-VOL-2Y", 0.25));

        // Without the fixing series the seasoned option must hard-error.
        let err = CmsSpreadOptionPricer::new()
            .price_internal(&inst, &market, as_of)
            .expect_err("missing CMS fixing series must be a hard error");
        assert!(
            err.to_string().contains("FIXING:CMS-10Y:USD-SOFR-3M"),
            "error must name the missing series: {err}"
        );

        // With both fixings recorded, the PV is the discounted intrinsic on
        // the observed spread.
        let long_observed = 0.045;
        let short_observed = 0.040;
        let market = market
            .insert_series(
                ScalarTimeSeries::new(
                    cms_fixing_series_id("USD-SOFR-3M", 10.0),
                    vec![(expiry, long_observed)],
                    None,
                )
                .expect("long fixing series"),
            )
            .insert_series(
                ScalarTimeSeries::new(
                    cms_fixing_series_id("USD-SOFR-3M", 2.0),
                    vec![(expiry, short_observed)],
                    None,
                )
                .expect("short fixing series"),
            );

        let pv = CmsSpreadOptionPricer::new()
            .price_internal(&inst, &market, as_of)
            .expect("seasoned spread option PV")
            .amount();
        let df = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .df_between_dates(as_of, payment)
            .expect("df");
        let expected = (long_observed - short_observed) * df * inst.notional.amount();
        assert!(
            (pv - expected).abs() < 0.01,
            "seasoned spread option must price intrinsic on the recorded \
             fixings: expected {expected}, got {pv}"
        );
    }

    fn future_market(as_of: Date) -> MarketContext {
        use finstack_quant_core::market_data::term_structures::ForwardCurve;
        MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.02, 30.0))
            .insert(
                ForwardCurve::builder("USD-SOFR-3M", 0.25)
                    .base_date(as_of)
                    .day_count(DayCount::Act360)
                    .knots([(0.0, 0.04), (30.0, 0.04)])
                    .build()
                    .expect("forward"),
            )
            .insert_surface(flat_vol_surface("USD-SWAPTION-VOL-10Y", 0.30))
            .insert_surface(flat_vol_surface("USD-SWAPTION-VOL-2Y", 0.20))
    }

    #[test]
    fn flat_marginals_match_reported_mean_and_spread_put_call_parity() {
        let as_of = date(2025, 1, 2);
        let mut inst = CmsSpreadOption::example().expect("example");
        inst.expiry = date(2026, 1, 2);
        inst.payment_date = date(2026, 1, 12);
        inst.strike = rust_decimal::Decimal::new(25, 4);
        let market = future_market(as_of);
        for order in [10, 15, 20] {
            let pricer = CmsSpreadOptionPricer {
                quadrature_order: order,
            };
            for correlation in [-1.0, -0.8, 0.0, 0.85, 1.0] {
                for strike in [-25, 0, 25] {
                    inst.correlation = correlation;
                    inst.strike = rust_decimal::Decimal::new(strike, 4);
                    inst.option_type = OptionType::Call;
                    let call = pricer.price_dyn(&inst, &market, as_of).expect("call");
                    inst.option_type = OptionType::Put;
                    let put = pricer.price_dyn(&inst, &market, as_of).expect("put");
                    let mean = call.measures[&MetricId::custom("cms_spread_forward")];
                    let df = market
                        .get_discount("USD-OIS")
                        .expect("discount")
                        .df_between_dates(as_of, inst.payment_date)
                        .expect("df");
                    let expected =
                        inst.notional.amount() * df * (mean - inst.strike_rate().expect("strike"));
                    assert!(
                        (call.value.amount() - put.value.amount() - expected).abs() < 1.0e-6,
                        "order {order}, correlation {correlation}, strike {strike}"
                    );
                }
            }
        }
    }

    #[test]
    fn conditional_payoff_handles_nonpositive_threshold_and_zero_variance() {
        for threshold in [-0.02, 0.0, 0.02, 0.06] {
            let mean = 0.04;
            let call = conditional_spread_payoff(mean, threshold, 0.0, OptionType::Call);
            let put = conditional_spread_payoff(mean, threshold, 0.0, OptionType::Put);
            assert_eq!(call, (threshold - mean).max(0.0));
            assert_eq!(put, (mean - threshold).max(0.0));
            if threshold <= 0.0 {
                assert_eq!(
                    conditional_spread_payoff(mean, threshold, 0.8, OptionType::Call),
                    0.0
                );
                assert_eq!(
                    conditional_spread_payoff(mean, threshold, 0.8, OptionType::Put),
                    mean - threshold
                );
            }
        }
    }

    #[test]
    fn identical_perfectly_correlated_legs_have_no_spread_option_value() {
        let leg = CmsSpreadLeg {
            forward_rate: 0.04,
            adjusted_forward_rate: 0.041,
            convexity_adjustment: 0.001,
            atm_volatility: 0.30,
            time_to_expiry: 2.0,
        };
        let mut inst = CmsSpreadOption::example().expect("example");
        inst.correlation = 1.0;
        inst.strike = rust_decimal::Decimal::ZERO;
        for option_type in [OptionType::Call, OptionType::Put] {
            inst.option_type = option_type;
            let expected = CmsSpreadOptionPricer::new()
                .expected_payoff(&inst, &leg, &leg)
                .expect("identical legs");
            assert!(expected < 1.0e-15, "expected zero, got {expected}");
        }
    }

    #[test]
    fn conditional_quadrature_matches_independent_cms_fixture_integral() {
        // Independent 1000-node Gauss-Legendre integral over [-10, 10], with
        // the conditional lognormal expectation evaluated using math.erfc.
        // Reproducible in audit evidence/implementation-cms-oracle.py.
        let reference = 0.001_625_713_844_676_493_9;
        let long_leg = CmsSpreadLeg {
            forward_rate: 0.04,
            adjusted_forward_rate: 0.041_670_924_675_754_06,
            convexity_adjustment: 0.0,
            atm_volatility: 0.20,
            time_to_expiry: 3.252_054_794_520_548,
        };
        let short_leg = CmsSpreadLeg {
            adjusted_forward_rate: 0.040_912_523_730_776_02,
            ..long_leg
        };
        let mut inst = CmsSpreadOption::example().expect("example");
        inst.correlation = 0.85;
        inst.strike = rust_decimal::Decimal::new(5, 3);
        let mut estimates = Vec::new();
        for order in [10, 15, 20] {
            let pricer = CmsSpreadOptionPricer {
                quadrature_order: order,
            };
            let estimate = pricer
                .expected_payoff(&inst, &long_leg, &short_leg)
                .expect("conditional expectation");
            assert!(
                (estimate - reference).abs() < 1.0e-9,
                "order {order}: {estimate} versus {reference}"
            );
            estimates.push(estimate);
        }
        assert!((estimates[2] - estimates[1]).abs() < 1.0e-9);
    }

    #[test]
    fn flat_lognormal_quantiles_are_monotone_and_integrate_to_the_declared_mean() {
        let leg = CmsSpreadLeg {
            forward_rate: 0.04,
            adjusted_forward_rate: 0.041,
            convexity_adjustment: 0.001,
            atm_volatility: 0.30,
            time_to_expiry: 2.0,
        };
        let quadrature = GaussHermiteQuadrature::new(20).expect("quadrature");
        assert!((quadrature.integrate(|_| 1.0) - 1.0).abs() < 1.0e-14);
        let mean = quadrature.integrate(|z| quantile_from_gaussian(&leg, z));
        assert!((mean - leg.adjusted_forward_rate).abs() < 1.0e-12);
        let mut previous = 0.0;
        for z in [-8.0, -4.0, -1.0, 0.0, 1.0, 4.0, 8.0] {
            let rate = quantile_from_gaussian(&leg, z);
            assert!(rate > previous);
            previous = rate;
        }
    }

    #[test]
    fn nonflat_smiles_are_rejected_by_public_pricing() {
        let as_of = date(2025, 1, 2);
        let mut inst = CmsSpreadOption::example().expect("example");
        inst.expiry = date(2026, 1, 2);
        inst.payment_date = date(2026, 1, 12);
        let smile = VolSurface::builder("USD-SWAPTION-VOL-10Y")
            .expiries(&[0.5, 2.0])
            .strikes(&[0.01, 0.04, 0.10])
            .row(&[0.20, 0.30, 0.20])
            .row(&[0.20, 0.30, 0.20])
            .build()
            .expect("smile");
        let market = future_market(as_of).insert_surface(smile);
        let error = CmsSpreadOptionPricer::new()
            .price_dyn(&inst, &market, as_of)
            .expect_err("unsupported smile must fail");
        assert!(error.to_string().contains("flat-strike Black"), "{error}");
    }

    #[test]
    fn sabr_cube_smiles_are_rejected_instead_of_using_a_flat_vol_cdf() {
        use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};
        let params = SabrParameterData::new(0.05, 0.5, -0.2, 0.4).expect("SABR");
        let cube = VolCube::builder("SABR")
            .expiries(&[0.5, 2.0])
            .tenors(&[2.0, 10.0])
            .node(params, 0.04)
            .node(params, 0.04)
            .node(params, 0.04)
            .node(params, 0.04)
            .build()
            .expect("cube");
        let error =
            validate_flat_marginal(&VolSource::Cube(std::sync::Arc::new(cube)), 1.0, 10.0, 0.25)
                .expect_err("unsupported cube must fail");
        assert!(error.to_string().contains("flat-strike Black"));
    }

    #[test]
    fn tenor_surface_can_have_distinct_volatilities_for_distinct_cms_tenors() {
        let surface = VolSurface::builder("ATM")
            .secondary_axis(VolSurfaceAxis::Tenor)
            .expiries(&[0.5, 2.0])
            .strikes(&[2.0, 10.0])
            .row(&[0.2, 0.3])
            .row(&[0.2, 0.3])
            .build()
            .expect("ATM surface");
        let source = VolSource::Surface(std::sync::Arc::new(surface));
        validate_flat_marginal(&source, 1.0, 2.0, 0.2).expect("short tenor");
        validate_flat_marginal(&source, 1.0, 10.0, 0.3).expect("long tenor");
    }

    #[test]
    fn shifted_quotes_cannot_feed_unshifted_cms_marginals() {
        let surface = VolSurface::builder("SHIFTED")
            .expiries(&[1.0])
            .strikes(&[0.03])
            .row(&[0.20])
            .build()
            .expect("surface")
            .with_displacements(&[0.02])
            .expect("displacement");
        let source = VolSource::Surface(std::sync::Arc::new(surface));
        assert!(clean_volatility(&source, 1.0, 5.0, 0.03)
            .expect_err("unshifted marginal rejects displaced quote")
            .to_string()
            .contains("unshifted Black"));
    }
}
