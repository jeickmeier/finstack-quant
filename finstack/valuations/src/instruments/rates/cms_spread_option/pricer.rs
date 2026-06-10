//! Gaussian-copula pricer for CMS spread options.
//!
//! The model treats each CMS rate as a convexity-adjusted SABR marginal and
//! couples the two terminal CMS rates with the instrument's Gaussian rank
//! correlation. Volatility sources are resolved through `VolProvider`, so a
//! market can provide full SABR `VolCube`s or simpler 2D volatility surfaces.

use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_option::pricer::convexity_adjustment;
use crate::instruments::rates::cms_spread_option::{CmsSpreadOption, CmsSpreadOptionType};
use crate::instruments::rates::exotics_shared::forward_swap_rate::{
    calculate_forward_swap_rate, ForwardSwapRateInputs,
};
use crate::metrics::MetricId;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DateExt, DayCountContext, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::traits::VolProvider;
use finstack_core::math::{norm_cdf, GaussHermiteQuadrature};
use finstack_core::money::Money;
use finstack_core::Result;

const DEFAULT_QUADRATURE_ORDER: usize = 10;
const MIN_POSITIVE_RATE: f64 = 1.0e-8;
const MIN_VOL: f64 = 1.0e-8;
const QUANTILE_ITERS: usize = 48;
const TAIL_PROB_EPS: f64 = 1.0e-10;

#[derive(Debug, Clone, Copy)]
struct CmsSpreadLeg {
    tenor_years: f64,
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

/// CMS spread option pricer using Gaussian copula and SABR marginals.
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

    /// Create a pricer with an explicit Gauss-Hermite quadrature order.
    pub fn with_quadrature_order(quadrature_order: usize) -> Self {
        Self { quadrature_order }
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
                long_leg: CmsSpreadLeg::zero(inst.long_cms_tenor.to_years_simple()),
                short_leg: CmsSpreadLeg::zero(inst.short_cms_tenor.to_years_simple()),
                discount_factor: 0.0,
                expected_payoff: 0.0,
            });
        }

        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
        let discount_factor =
            relative_df_discount_curve(discount_curve.as_ref(), as_of, inst.payment_date)?;

        // Seasoned options (expiry already past) have zero time to expiry;
        // the year fraction is only defined for `as_of <= expiry`.
        let time_to_expiry = if inst.expiry_date <= as_of {
            0.0
        } else {
            inst.day_count
                .year_fraction(as_of, inst.expiry_date, DayCountContext::default())?
                .max(0.0)
        };

        let long_vol = market.get_vol_provider(inst.long_vol_surface_id.as_ref())?;
        let short_vol = market.get_vol_provider(inst.short_vol_surface_id.as_ref())?;
        let long_leg = self.resolve_leg(
            inst,
            market,
            as_of,
            inst.long_cms_tenor,
            time_to_expiry,
            long_vol.as_ref(),
        )?;
        let short_leg = self.resolve_leg(
            inst,
            market,
            as_of,
            inst.short_cms_tenor,
            time_to_expiry,
            short_vol.as_ref(),
        )?;

        let expected_payoff = if time_to_expiry <= 0.0 {
            cms_spread_payoff(
                long_leg.forward_rate,
                short_leg.forward_rate,
                inst.strike,
                inst.option_type,
            )
        } else {
            self.expected_payoff(
                inst,
                &long_leg,
                long_vol.as_ref(),
                &short_leg,
                short_vol.as_ref(),
            )?
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
        vol_provider: &dyn VolProvider,
    ) -> Result<CmsSpreadLeg> {
        let tenor_years = tenor.to_years_simple();

        // Seasoned option: both CMS rates fixed at the (past) expiry. Resolve
        // the leg from the recorded fixing (mirroring the cap/floor pricer) —
        // never re-project from the live curve, which books phantom P&L. The
        // rate is known, so there is no convexity adjustment and the payoff
        // collapses to intrinsic on the observed rates.
        if inst.expiry_date < as_of {
            let observed =
                crate::instruments::rates::exotics_shared::fixings::historical_cms_fixing(
                    market,
                    &inst.forward_curve_id,
                    tenor_years,
                    inst.expiry_date,
                )?;
            return Ok(CmsSpreadLeg {
                tenor_years,
                forward_rate: observed,
                adjusted_forward_rate: observed,
                convexity_adjustment: 0.0,
                atm_volatility: 0.0,
                time_to_expiry: 0.0,
            });
        }

        let tenor_months = tenor.months().ok_or_else(|| {
            finstack_core::Error::Validation(format!(
                "CmsSpreadOption tenor {} must be month- or year-based",
                tenor
            ))
        })?;
        let swap_end = inst.expiry_date.add_months(tenor_months as i32);
        // Project the CMS forward swap rate on the instrument's actual swap
        // conventions. Hard-coding USD conventions (semi/30360 fixed,
        // quarterly/Act360 float) mis-prices the annuity and accrual basis for
        // non-USD CMS spreads (e.g. EUR: annual/30360 fixed, annual/Act360
        // float). `resolved_swap_*` falls back to the USD market standard when
        // neither `swap_convention` nor an explicit field is set.
        let (forward_rate, _) = calculate_forward_swap_rate(ForwardSwapRateInputs {
            market,
            discount_curve_id: &inst.discount_curve_id,
            forward_curve_id: &inst.forward_curve_id,
            as_of,
            start: inst.expiry_date,
            end: swap_end,
            fixed_freq: inst.resolved_swap_fixed_freq(),
            fixed_day_count: inst.resolved_swap_day_count(),
            float_freq: inst.resolved_swap_float_freq(),
            float_day_count: inst.resolved_swap_float_day_count(),
        })?;
        if forward_rate <= 0.0 || !forward_rate.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "CmsSpreadOption forward CMS rate must be positive and finite, got {}",
                forward_rate
            )));
        }

        let atm_volatility =
            clean_volatility(vol_provider, time_to_expiry, tenor_years, forward_rate)?;
        let convexity = if time_to_expiry > 0.0 {
            convexity_adjustment(atm_volatility, time_to_expiry, tenor_years, forward_rate)
        } else {
            0.0
        };
        let adjusted_forward_rate = forward_rate + convexity;
        if adjusted_forward_rate <= 0.0 || !adjusted_forward_rate.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "CmsSpreadOption convexity-adjusted CMS rate must be positive and finite, got {}",
                adjusted_forward_rate
            )));
        }

        Ok(CmsSpreadLeg {
            tenor_years,
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
        long_vol: &dyn VolProvider,
        short_leg: &CmsSpreadLeg,
        short_vol: &dyn VolProvider,
    ) -> Result<f64> {
        let quadrature = GaussHermiteQuadrature::new(self.quadrature_order)?;
        let rho = inst.spread_correlation.clamp(-0.999_999, 0.999_999);
        let rho_complement = (1.0 - rho * rho).sqrt();

        let expected = quadrature.integrate(|z_long| {
            quadrature.integrate(|z_independent| {
                let z_short = rho * z_long + rho_complement * z_independent;
                let long_rate = quantile_from_gaussian(long_vol, long_leg, z_long);
                let short_rate = quantile_from_gaussian(short_vol, short_leg, z_short);
                cms_spread_payoff(long_rate, short_rate, inst.strike, inst.option_type)
            })
        });

        if !expected.is_finite() || expected < 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "CmsSpreadOption expected payoff is invalid: {}",
                expected
            )));
        }
        Ok(expected)
    }

    fn price_internal(
        &self,
        inst: &CmsSpreadOption,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        let data = self.price_data(inst, market, as_of)?;
        Ok(Money::new(
            data.expected_payoff * data.discount_factor * inst.notional.amount(),
            inst.notional.currency(),
        ))
    }
}

impl Default for CmsSpreadOptionPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl CmsSpreadLeg {
    fn zero(tenor_years: f64) -> Self {
        Self {
            tenor_years,
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
        let option = instrument
            .as_any()
            .downcast_ref::<CmsSpreadOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CmsSpreadOption, instrument.key())
            })?;
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
        );
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
            option.spread_correlation,
        );
        Ok(result)
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        let option = instrument
            .as_any()
            .downcast_ref::<CmsSpreadOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CmsSpreadOption, instrument.key())
            })?;
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

fn cms_spread_payoff(
    long_rate: f64,
    short_rate: f64,
    strike: f64,
    option_type: CmsSpreadOptionType,
) -> f64 {
    let spread = long_rate - short_rate;
    match option_type {
        CmsSpreadOptionType::Call => (spread - strike).max(0.0),
        CmsSpreadOptionType::Put => (strike - spread).max(0.0),
    }
}

fn quantile_from_gaussian(vol_provider: &dyn VolProvider, leg: &CmsSpreadLeg, z: f64) -> f64 {
    let u = norm_cdf(z).clamp(TAIL_PROB_EPS, 1.0 - TAIL_PROB_EPS);
    sabr_marginal_quantile(vol_provider, leg, u)
}

fn sabr_marginal_quantile(vol_provider: &dyn VolProvider, leg: &CmsSpreadLeg, target: f64) -> f64 {
    if leg.time_to_expiry <= 0.0 || leg.atm_volatility <= MIN_VOL {
        return leg.adjusted_forward_rate;
    }

    let std_dev = leg.atm_volatility * leg.time_to_expiry.sqrt();
    let mut low = (leg.adjusted_forward_rate * (-10.0 * std_dev).exp()).max(MIN_POSITIVE_RATE);
    let mut high = (leg.adjusted_forward_rate * (10.0 * std_dev).exp())
        .max(leg.adjusted_forward_rate * 4.0)
        .max(MIN_POSITIVE_RATE * 10.0);

    for _ in 0..8 {
        if marginal_cdf(vol_provider, leg, high) >= target {
            break;
        }
        high *= 2.0;
    }

    if marginal_cdf(vol_provider, leg, low) > target {
        low = MIN_POSITIVE_RATE;
    }

    for _ in 0..QUANTILE_ITERS {
        let mid = 0.5 * (low + high);
        if marginal_cdf(vol_provider, leg, mid) < target {
            low = mid;
        } else {
            high = mid;
        }
    }
    0.5 * (low + high)
}

fn marginal_cdf(vol_provider: &dyn VolProvider, leg: &CmsSpreadLeg, strike: f64) -> f64 {
    if strike <= MIN_POSITIVE_RATE {
        return 0.0;
    }
    let vol = clean_volatility_or_atm(vol_provider, leg, strike);
    let sigma_sqrt_t = vol * leg.time_to_expiry.sqrt();
    if sigma_sqrt_t <= MIN_VOL {
        return if strike < leg.adjusted_forward_rate {
            0.0
        } else {
            1.0
        };
    }
    let d2 = ((leg.adjusted_forward_rate / strike).ln() - 0.5 * vol * vol * leg.time_to_expiry)
        / sigma_sqrt_t;
    norm_cdf(-d2)
}

fn clean_volatility(
    vol_provider: &dyn VolProvider,
    expiry: f64,
    tenor: f64,
    strike: f64,
) -> Result<f64> {
    let vol = vol_provider.vol_clamped(expiry.max(0.0), tenor, strike.max(MIN_POSITIVE_RATE));
    if vol <= 0.0 || !vol.is_finite() {
        return Err(finstack_core::Error::Validation(format!(
            "CmsSpreadOption volatility source {} returned invalid vol {}",
            vol_provider.vol_id(),
            vol
        )));
    }
    Ok(vol.max(MIN_VOL))
}

fn clean_volatility_or_atm(vol_provider: &dyn VolProvider, leg: &CmsSpreadLeg, strike: f64) -> f64 {
    let vol = vol_provider.vol_clamped(
        leg.time_to_expiry.max(0.0),
        leg.tenor_years,
        strike.max(MIN_POSITIVE_RATE),
    );
    if vol.is_finite() && vol > 0.0 {
        vol.max(MIN_VOL)
    } else {
        leg.atm_volatility.max(MIN_VOL)
    }
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use finstack_core::market_data::fixings::cms_fixing_series_id;
    use finstack_core::market_data::scalars::ScalarTimeSeries;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::types::CurveId;
    use test_utils::{date, flat_discount_with_tenor};

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

        let mut inst = CmsSpreadOption::example();
        inst.expiry_date = expiry;
        inst.payment_date = payment;
        inst.strike = 0.0;
        inst.option_type = CmsSpreadOptionType::Call;

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
}
