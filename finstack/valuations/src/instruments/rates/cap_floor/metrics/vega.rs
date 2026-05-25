//! Vega calculator for interest rate options (caps/floors/caplets/floorlets).

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::rates::cap_floor::{CapFloor, CapFloorVolType};
use crate::metrics::{MetricCalculator, MetricContext};
use crate::pricer::ModelKey;
use finstack_core::Result;

use super::common::CapletInputs;

const DEFAULT_HW_VEGA_BUMP: f64 = 0.0001;

/// Vega calculator (model-consistent vega per 1% vol, aggregated for caps/floors).
///
/// Dispatches to the appropriate model based on `vol_type`:
/// - `Lognormal`: Black-76 vega = F·n(d₁)·√T / 100
/// - `ShiftedLognormal`: Black-76 vega on shifted rates
/// - `Normal`: Bachelier vega = n(d)·√T / 100
///
/// For Normal vol, the 1% bump is in absolute rate terms (e.g., 1bp normal vol).
///
/// # RFR Convention
///
/// For RFR-indexed options, the vega formula's `T` argument is the
/// observation-window midpoint (`c.risk_t`), not the option fixing date — this
/// matches the actual rate-observation window. Sigma lookup uses the fixing
/// time `c.fixing_t` (which is also what the pricer uses).
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        if matches!(
            context.clone_pricer_dispatch().0,
            Some(ModelKey::HullWhite1F)
        ) {
            return hull_white_tree_vega_per_pct(option, context);
        }
        let strike = option.strike_f64()?;
        let vol_type = option.vol_type;
        let vol_shift = option.resolved_vol_shift();
        super::common::aggregate_over_caplets(option, context, |c: CapletInputs| {
            caplet_vega(vol_type, strike, vol_shift, c)
        })
    }
}

fn caplet_vega(vol_type: CapFloorVolType, strike: f64, vol_shift: f64, c: CapletInputs) -> f64 {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};
    // RFR options' vega risk reflects the observation-window midpoint, not the
    // option fixing date; non-RFR options have risk_t == fixing_t.
    let t = c.risk_t;
    match vol_type {
        CapFloorVolType::Lognormal => black::vega_per_pct(strike, c.forward, c.sigma, t),
        CapFloorVolType::ShiftedLognormal => {
            black::vega_per_pct(strike + vol_shift, c.forward + vol_shift, c.sigma, t)
        }
        CapFloorVolType::Normal => normal::vega_per_pct(strike, c.forward, c.sigma, t),
        CapFloorVolType::Auto => {
            if c.forward > 0.0 && strike > 0.0 {
                black::vega_per_pct(strike, c.forward, c.sigma, t)
            } else {
                normal::vega_per_pct(strike, c.forward, c.sigma, t)
            }
        }
    }
}

fn hull_white_tree_vega_per_pct(option: &CapFloor, context: &MetricContext) -> Result<f64> {
    let base_vol = option
        .pricing_overrides
        .market_quotes
        .implied_volatility
        .unwrap_or_else(|| HullWhiteParams::default().sigma);
    if base_vol <= DEFAULT_HW_VEGA_BUMP {
        return Ok(0.0);
    }

    let bump = DEFAULT_HW_VEGA_BUMP;
    let mut up = option.clone();
    up.pricing_overrides.market_quotes.implied_volatility = Some(base_vol + bump);
    let pv_up = context.reprice_instrument_raw(&up, context.curves.as_ref(), context.as_of)?;

    let mut down = option.clone();
    down.pricing_overrides.market_quotes.implied_volatility = Some(base_vol - bump);
    let pv_down = context.reprice_instrument_raw(&down, context.curves.as_ref(), context.as_of)?;

    Ok((pv_up - pv_down) / (2.0 * bump) * 0.01)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::periods::SchedulePeriod;
    use crate::instruments::rates::cap_floor::RateOptionType;
    use crate::instruments::{ExerciseStyle, SettlementType};
    use finstack_core::currency::Currency;
    use finstack_core::dates::{BusinessDayConvention, DayCount, DayCountContext, StubKind, Tenor};
    use finstack_core::money::Money;
    use rust_decimal::Decimal;
    use time::macros::date;

    #[test]
    fn rfr_vega_time_uses_actual_observation_window_midpoint() {
        let option = CapFloor {
            id: "RFR-VEGA-TIME".into(),
            rate_option_type: RateOptionType::Caplet,
            notional: Money::new(1_000_000.0, Currency::USD),
            strike: Decimal::try_from(0.05).expect("valid decimal"),
            start_date: date!(2024 - 01 - 03),
            maturity: date!(2024 - 04 - 03),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            stub: StubKind::None,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            exercise_style: ExerciseStyle::European,
            settlement: SettlementType::Cash,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-OIS".into(),
            vol_surface_id: "USD-CAP-VOL".into(),
            vol_type: CapFloorVolType::Lognormal,
            vol_shift: 0.0,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Default::default(),
        };
        let period = SchedulePeriod {
            accrual_start: date!(2024 - 01 - 03),
            accrual_end: date!(2024 - 04 - 03),
            payment_date: date!(2024 - 04 - 05),
            reset_date: None,
            accrual_year_fraction: 91.0 / 360.0,
        };

        let actual = super::super::common::rfr_observation_midpoint_time(
            &option,
            date!(2024 - 01 - 03),
            &period,
            DayCountContext::default(),
        )
        .expect("rfr timing");

        assert!((actual - (91.0 / 360.0) * 0.5).abs() < 1e-12);
    }
}
