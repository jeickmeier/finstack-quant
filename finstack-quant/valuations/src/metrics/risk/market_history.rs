//! Historical scenario application and tabular export for `MarketHistory`.

use super::{MarketHistory, MarketScenario, RiskFactorType, UnshiftedHazardPolicy};
use crate::metrics::validate_buckets_strictly_increasing;
use crate::recalibration::{
    HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump, RecalibrationProvider,
    provider_missing,
};
use finstack_quant_core::Result;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use std::cmp::Ordering;
use std::sync::Arc;

struct VolPointShift {
    expiry: f64,
    strike: f64,
    amount: f64,
}

// Coordinates use exact numeric equality: signed zeros coincide, adjacent
// floating-point values remain distinct, and NaN never matches a grid node.
pub(crate) fn same_grid_coordinate(left: f64, right: f64) -> bool {
    left.partial_cmp(&right) == Some(Ordering::Equal)
}

/// Apply `scenario` to a base market context.
///
/// Creates a new `MarketContext` with all risk factor shifts applied.
/// Rate shifts use triangular key-rate bumps. Credit spreads are additive
/// par-quote changes, replayed together for each hazard curve through the
/// supplied calibration provider against the shifted dependency market.
/// Equity and volatility shifts are multiplicative and additive, respectively.
/// Volatility coordinates must exactly match source-surface expiry and strike
/// nodes. Repeated coordinates sum their absolute volatility changes before
/// application; every other node is unchanged. Off-grid coordinates and
/// shifts producing negative volatility are rejected rather than mapped or floored.
///
/// # Arguments
///
/// * `scenario` - Historical market scenario carrying the effective date, shifts,
///   and hazard replay policies.
/// * `base_market` - Current market state, including the source hazard recipes
///   and discount curves needed to verify and replay credit spread quotes.
/// * `provider` - Quote-recalibration service. Required for a nonzero credit
///   spread shock or unshifted calibrated hazard whose discount curve moved;
///   other scenarios accept `None`.
///
/// # Errors
///
/// Returns an error for non-finite shifts, invalid credit tenors, missing
/// dependencies or providers, invalid volatility coordinates or resulting
/// volatilities, or failed quote recalibration.
pub fn apply_market_scenario(
    scenario: &MarketScenario,
    base_market: &MarketContext,
    provider: Option<&dyn RecalibrationProvider>,
) -> Result<MarketContext> {
    let mut bumps: Vec<MarketBump> = Vec::with_capacity(scenario.shifts.len());

    let mut vol_shifts_by_surface: Vec<(CurveId, Vec<VolPointShift>)> = Vec::new();
    let mut credit_shifts: Vec<(CurveId, Vec<(f64, f64)>)> = Vec::new();

    for shift in &scenario.shifts {
        if !shift.shift.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "historical scenario shifts must be finite".to_string(),
            ));
        }
        let bump = match &shift.factor {
            RiskFactorType::DiscountRate {
                curve_id,
                tenor_years,
                grid_years,
            } => {
                let (id, spec) = key_rate_bp_bump(curve_id, *tenor_years, grid_years, shift.shift)?;
                MarketBump::DiscountCurve { id, spec }
            }
            RiskFactorType::ForwardRate {
                curve_id,
                tenor_years,
                grid_years,
            } => {
                let (id, spec) = key_rate_bp_bump(curve_id, *tenor_years, grid_years, shift.shift)?;
                MarketBump::ForwardCurve { id, spec }
            }
            RiskFactorType::CreditSpread {
                curve_id,
                tenor_years,
            } => {
                if !tenor_years.is_finite() || *tenor_years <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "historical credit tenor for '{curve_id}' must be finite and positive"
                    )));
                }
                if shift.shift == 0.0 {
                    continue;
                }
                let target = (*tenor_years, shift.shift * 10_000.0);
                if let Some((_, targets)) = credit_shifts.iter_mut().find(|(id, _)| id == curve_id)
                {
                    targets.push(target);
                } else {
                    credit_shifts.push((curve_id.clone(), vec![target]));
                }
                continue;
            }
            RiskFactorType::EquitySpot { ticker } => {
                let id = CurveId::from(ticker.as_str());
                let spec = BumpSpec::multiplier(1.0 + shift.shift);
                if base_market.get_price(id.as_str())?.is_price() {
                    MarketBump::PriceScalar { id, spec }
                } else {
                    MarketBump::UnitlessScalar { id, spec }
                }
            }
            RiskFactorType::FxSpot { base, quote } => MarketBump::FxPct {
                base: *base,
                quote: *quote,
                pct: shift.shift * 100.0,
                as_of: scenario.date,
                policy: finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
            },
            RiskFactorType::ImpliedVol {
                vol_surface_id,
                expiry_years,
                strike,
            } => {
                let point = VolPointShift {
                    expiry: *expiry_years,
                    strike: *strike,
                    amount: shift.shift,
                };
                match vol_shifts_by_surface
                    .iter_mut()
                    .find(|(id, _)| id == vol_surface_id)
                {
                    Some((_, shifts)) => {
                        if let Some(existing) = shifts.iter_mut().find(|existing| {
                            same_grid_coordinate(existing.expiry, *expiry_years)
                                && same_grid_coordinate(existing.strike, *strike)
                        }) {
                            existing.amount += shift.shift;
                        } else {
                            shifts.push(point);
                        }
                    }
                    None => vol_shifts_by_surface.push((vol_surface_id.clone(), vec![point])),
                }
                continue;
            }
        };

        bumps.push(bump);
    }

    let mut bumped_market = base_market.clone();
    let mut remaining = bumps;
    while !remaining.is_empty() {
        let mut round: Vec<MarketBump> = Vec::with_capacity(remaining.len());
        let mut deferred: Vec<MarketBump> = Vec::new();
        let mut seen: Vec<CurveId> = Vec::new();
        for bump in remaining {
            match bump_curve_id(&bump) {
                Some(id) if seen.contains(&id) => deferred.push(bump),
                Some(id) => {
                    seen.push(id);
                    round.push(bump);
                }
                None => round.push(bump),
            }
        }
        bumped_market = bumped_market.bump(round)?;
        remaining = deferred;
    }

    for (surface_id, shifts) in vol_shifts_by_surface {
        let mut surface = base_market.get_surface(&surface_id)?.as_ref().clone();
        for VolPointShift {
            expiry,
            strike,
            amount,
        } in shifts
        {
            let row = surface
                .expiries()
                .iter()
                .position(|value| same_grid_coordinate(*value, expiry));
            let column = surface
                .secondary_values()
                .iter()
                .position(|value| same_grid_coordinate(*value, strike));
            let (Some(row), Some(column)) = (row, column) else {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "historical volatility point ({expiry}, {strike}) must exactly match a grid node on '{surface_id}'"
                )));
            };
            let new_vol = surface.vols()[row * surface.secondary_values().len() + column] + amount;
            if !new_vol.is_finite() || new_vol < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "historical volatility point ({expiry}, {strike}) on '{surface_id}' must remain finite and nonnegative"
                )));
            }
            surface.bump_point_absolute_in_place(expiry, strike, amount)?;
        }
        bumped_market = bumped_market.insert_surface(surface);
    }

    let discount_changes: std::collections::BTreeSet<_> = scenario
        .shifts
        .iter()
        .filter_map(|shift| match &shift.factor {
            RiskFactorType::DiscountRate { curve_id, .. } if shift.shift != 0.0 => Some(curve_id),
            _ => None,
        })
        .collect();
    if !discount_changes.is_empty() {
        let hazards: Vec<_> = base_market
            .curve_ids()
            .into_iter()
            .filter_map(|id| base_market.get_hazard(id).ok())
            .filter(|hazard| hazard.hazard_calibration().is_some())
            .filter(|hazard| !credit_shifts.iter().any(|(id, _)| id == hazard.id()))
            .collect();
        if !hazards.is_empty() {
            let provider =
                provider.ok_or_else(|| provider_missing("historical_hazard_rate_dependency"))?;
            let source_market = Arc::new(base_market.clone());
            for hazard in hazards {
                let discount_curve_id = provider.get_hazard_discount_curve_id(&hazard)?;
                if !discount_changes.contains(&discount_curve_id) {
                    continue;
                }
                let policy = scenario.unshifted_hazard_policies.get(hazard.id()).ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "historical scenario requires an explicit hazard replay policy for '{}' after discount curve '{}' moves",
                        hazard.id(), discount_curve_id
                    ))
                })?;
                if *policy == UnshiftedHazardPolicy::RefitOriginalQuotes {
                    let rebuilt = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
                        hazard,
                        source_market: Arc::clone(&source_market),
                        target_market: Arc::new(bumped_market.clone()),
                        discount_curve_id,
                        doc_clause: None,
                        cds_valuation_convention: None,
                        deal_quote_override: None,
                        action: HazardRecalibrationAction::DependencyMarketReplay,
                    })?;
                    bumped_market.try_insert_mut(rebuilt.as_ref().clone())?;
                }
            }
        }
    }

    if !credit_shifts.is_empty() {
        let provider = provider.ok_or_else(|| provider_missing("historical_credit_spread"))?;
        let source_market = Arc::new(base_market.clone());
        for (curve_id, targets) in credit_shifts {
            let hazard = base_market.get_hazard(&curve_id)?;
            let discount_curve_id = provider.get_hazard_discount_curve_id(&hazard)?;
            let rebuilt = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
                hazard,
                source_market: Arc::clone(&source_market),
                target_market: Arc::new(bumped_market.clone()),
                discount_curve_id,
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
                action: HazardRecalibrationAction::SpreadBump(QuoteBump::TenorsBp(targets)),
            })?;
            bumped_market.try_insert_mut(rebuilt.as_ref().clone())?;
        }
    }
    Ok(bumped_market)
}

fn key_rate_bp_bump(
    curve_id: &CurveId,
    tenor_years: f64,
    grid_years: &[f64],
    shift: f64,
) -> Result<(CurveId, BumpSpec)> {
    if grid_years.iter().any(|&t| !t.is_finite() || t <= 0.0) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "historical rate grid for '{curve_id}' requires finite positive coordinates"
        )));
    }
    validate_buckets_strictly_increasing(grid_years)?;
    let index = grid_years
        .iter()
        .position(|&t| same_grid_coordinate(t, tenor_years))
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "historical rate tenor {tenor_years} must be an exact grid coordinate for '{curve_id}'"
            ))
        })?;
    let bp = shift * 10_000.0;
    if !bp.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "historical rate shock is outside the basis-point range for '{curve_id}'"
        )));
    }
    let spec = BumpSpec::triangular_key_rate_bucket_bp(
        grid_years[..index].last().copied(),
        tenor_years,
        grid_years.get(index + 1).copied(),
        bp,
    );
    Ok((curve_id.clone(), spec))
}

fn bump_curve_id(bump: &MarketBump) -> Option<CurveId> {
    bump.curve_id().cloned()
}

/// Project one table row per risk-factor shift in `history` in scenario and shift order.
///
/// Dates are ISO strings, shifts retain each factor's native units, and
/// coordinates absent for a factor are nullable. Empty histories keep all columns.
///
/// # Arguments
///
/// * `history` - Historical scenario sequence whose risk-factor shifts are flattened.
pub fn market_history_to_table(
    history: &MarketHistory,
) -> Result<finstack_quant_core::table::TableEnvelope> {
    use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};

    let row_count: usize = history.scenarios.iter().map(|s| s.shifts.len()).sum();
    let mut dates = Vec::with_capacity(row_count);
    let mut factor_types = Vec::with_capacity(row_count);
    let mut curve_ids = Vec::with_capacity(row_count);
    let mut tenor_years_col = Vec::with_capacity(row_count);
    let mut tickers = Vec::with_capacity(row_count);
    let mut bases = Vec::with_capacity(row_count);
    let mut quotes = Vec::with_capacity(row_count);
    let mut vol_surface_ids = Vec::with_capacity(row_count);
    let mut expiry_years_col = Vec::with_capacity(row_count);
    let mut strikes = Vec::with_capacity(row_count);
    let mut shifts = Vec::with_capacity(row_count);

    for scenario in &history.scenarios {
        let date_str = scenario.date.to_string();
        for shift in &scenario.shifts {
            dates.push(date_str.clone());
            shifts.push(shift.shift);
            let mut curve_id = None;
            let mut tenor_years = None;
            let mut ticker = None;
            let mut base = None;
            let mut quote = None;
            let mut vol_surface_id = None;
            let mut expiry_years = None;
            let mut strike = None;

            let factor_type = match &shift.factor {
                RiskFactorType::DiscountRate {
                    curve_id: cid,
                    tenor_years: t,
                    ..
                } => {
                    curve_id = Some(cid.as_str().to_owned());
                    tenor_years = Some(*t);
                    "discount_rate"
                }
                RiskFactorType::ForwardRate {
                    curve_id: cid,
                    tenor_years: t,
                    ..
                } => {
                    curve_id = Some(cid.as_str().to_owned());
                    tenor_years = Some(*t);
                    "forward_rate"
                }
                RiskFactorType::CreditSpread {
                    curve_id: cid,
                    tenor_years: t,
                } => {
                    curve_id = Some(cid.as_str().to_owned());
                    tenor_years = Some(*t);
                    "credit_spread"
                }
                RiskFactorType::EquitySpot { ticker: tkr } => {
                    ticker = Some(tkr.clone());
                    "equity_spot"
                }
                RiskFactorType::FxSpot { base: b, quote: q } => {
                    base = Some(b.to_string());
                    quote = Some(q.to_string());
                    "fx_spot"
                }
                RiskFactorType::ImpliedVol {
                    vol_surface_id: vid,
                    expiry_years: e,
                    strike: k,
                } => {
                    vol_surface_id = Some(vid.as_str().to_owned());
                    expiry_years = Some(*e);
                    strike = Some(*k);
                    "implied_vol"
                }
            };

            factor_types.push(Some(factor_type.to_owned()));
            curve_ids.push(curve_id);
            tenor_years_col.push(tenor_years);
            tickers.push(ticker);
            bases.push(base);
            quotes.push(quote);
            vol_surface_ids.push(vol_surface_id);
            expiry_years_col.push(expiry_years);
            strikes.push(strike);
        }
    }

    let columns = vec![
        TableColumn::new("date", TableColumnData::String(dates)),
        TableColumn::new("type", TableColumnData::NullableString(factor_types)),
        TableColumn::new("curve_id", TableColumnData::NullableString(curve_ids)),
        TableColumn::new(
            "tenor_years",
            TableColumnData::NullableFloat64(tenor_years_col),
        ),
        TableColumn::new("ticker", TableColumnData::NullableString(tickers)),
        TableColumn::new("base", TableColumnData::NullableString(bases)),
        TableColumn::new("quote", TableColumnData::NullableString(quotes)),
        TableColumn::new(
            "vol_surface_id",
            TableColumnData::NullableString(vol_surface_ids),
        ),
        TableColumn::new(
            "expiry_years",
            TableColumnData::NullableFloat64(expiry_years_col),
        ),
        TableColumn::new("strike", TableColumnData::NullableFloat64(strikes)),
        TableColumn::new("shift", TableColumnData::Float64(shifts)),
    ];
    TableEnvelope::new(columns, Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::risk::RiskFactorShift;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, FxQuery, SimpleFxProvider};
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn test_market_scenario_allows_multiple_key_rates() -> Result<()> {
        let base_date = date!(2024 - 01 - 01);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (5.0, 0.85)])
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()?;
        let base_market = MarketContext::new()
            .try_insert(curve)
            .expect("insert market curve");

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::from("USD-OIS"),
                        tenor_years: 5.0,
                        grid_years: crate::metrics::STANDARD_BUCKETS_YEARS.to_vec(),
                    },
                    shift: 0.0010,
                },
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::from("USD-OIS"),
                        tenor_years: 10.0,
                        grid_years: crate::metrics::STANDARD_BUCKETS_YEARS.to_vec(),
                    },
                    shift: -0.0005,
                },
            ],
        );

        let bumped = apply_market_scenario(&scenario, &base_market, None)?;
        assert!(bumped.get_discount("USD-OIS").is_ok());

        Ok(())
    }

    #[test]
    fn test_scenario_apply_creates_bumped_market() -> Result<()> {
        let base_date = date!(2024 - 01 - 01);

        let base_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (5.0, 0.85), (10.0, 0.70)])
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()?;

        let base_market = MarketContext::new()
            .try_insert(base_curve)
            .expect("insert market curve");

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: CurveId::from("USD-OIS"),
                    tenor_years: 5.0,
                    grid_years: crate::metrics::STANDARD_BUCKETS_YEARS.to_vec(),
                },
                shift: 0.0010, // +10bp
            }],
        );

        let bumped_market = apply_market_scenario(&scenario, &base_market, None)?;
        assert!(bumped_market.get_discount("USD-OIS").is_ok());

        Ok(())
    }

    #[test]
    fn test_equity_spot_shift_applied() -> Result<()> {
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let base_market = MarketContext::new().insert_price("AAPL", MarketScalar::unitless(100.0)?);

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![RiskFactorShift {
                factor: RiskFactorType::EquitySpot {
                    ticker: "AAPL".to_string(),
                },
                shift: 0.10, // +10%
            }],
        );

        let bumped = apply_market_scenario(&scenario, &base_market, None)?;
        let v = bumped
            .get_price("AAPL")?
            .get_unitless()
            .expect("unexpected scalar variant");
        assert!((v - 110.0).abs() < 1e-9);

        Ok(())
    }

    #[test]
    fn test_fx_spot_shift_applied() -> Result<()> {
        let provider = SimpleFxProvider::new();
        provider.set_quotes(&[(Currency::EUR, Currency::USD, 1.20)])?;
        let base_market = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider), None));

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![RiskFactorShift {
                factor: RiskFactorType::FxSpot {
                    base: Currency::EUR,
                    quote: Currency::USD,
                },
                shift: 0.10,
            }],
        );

        let bumped = apply_market_scenario(&scenario, &base_market, None)?;
        let rate = bumped
            .fx()
            .expect("FX matrix should be present")
            .rate(FxQuery::new(
                Currency::EUR,
                Currency::USD,
                date!(2024 - 01 - 02),
                finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
            ))?
            .rate;

        assert!((rate - 1.32).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_same_surface_vol_points_preserve_distinct_shocks() -> Result<()> {
        use finstack_quant_core::market_data::surfaces::{
            VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
        };

        let surface = VolSurface::builder("EQ-VOL")
            .expiries(&[0.5, 1.0])
            .secondary_values(&[100.0, 110.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.20, 0.20])
            .row(&[0.20, 0.20])
            .build()?;
        let base_market = MarketContext::new().insert_surface(surface);

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: CurveId::from("EQ-VOL"),
                        expiry_years: 0.5,
                        strike: 100.0,
                    },
                    shift: 0.02,
                },
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: CurveId::from("EQ-VOL"),
                        expiry_years: 1.0,
                        strike: 110.0,
                    },
                    shift: -0.02,
                },
            ],
        );

        let bumped = apply_market_scenario(&scenario, &base_market, None)?;
        let surface = bumped.get_surface("EQ-VOL")?;
        for (actual, expected) in surface.vols().iter().zip([0.22, 0.20, 0.20, 0.18]) {
            assert!((actual - expected).abs() < 1e-12);
        }
        assert_eq!(base_market.get_surface("EQ-VOL")?.vols(), &[0.20; 4]);
        Ok(())
    }

    #[test]
    fn test_distinct_surface_vol_shifts_stay_independent() -> Result<()> {
        use finstack_quant_core::market_data::surfaces::{
            VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
        };

        let eq = VolSurface::builder("EQ-VOL")
            .expiries(&[1.0])
            .secondary_values(&[100.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.20])
            .build()?;
        let fx = VolSurface::builder("FX-VOL")
            .expiries(&[1.0])
            .secondary_values(&[1.10])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.10])
            .build()?;
        let base_market = MarketContext::new().insert_surface(eq).insert_surface(fx);

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: CurveId::from("EQ-VOL"),
                        expiry_years: 1.0,
                        strike: 100.0,
                    },
                    shift: 0.02,
                },
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: CurveId::from("FX-VOL"),
                        expiry_years: 1.0,
                        strike: 1.10,
                    },
                    shift: -0.01,
                },
            ],
        );

        let bumped = apply_market_scenario(&scenario, &base_market, None)?;
        let eq_surface = bumped.get_surface("EQ-VOL")?;
        let eq_vol = finstack_quant_models::volatility::get_surface_vol(&eq_surface, 1.0, 100.0)
            .expect("grid point");
        let fx_surface = bumped.get_surface("FX-VOL")?;
        let fx_vol = finstack_quant_models::volatility::get_surface_vol(&fx_surface, 1.0, 1.10)
            .expect("grid point");
        assert!((eq_vol - 0.22).abs() < 1e-9, "EQ surface: got {eq_vol}");
        assert!((fx_vol - 0.09).abs() < 1e-9, "FX surface: got {fx_vol}");
        Ok(())
    }

    #[test]
    fn test_implied_vol_shift_applied() -> Result<()> {
        use finstack_quant_core::market_data::surfaces::{
            VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
        };

        let surface = VolSurface::builder("EQ-VOL")
            .expiries(&[0.5, 1.0])
            .secondary_values(&[100.0, 110.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.20, 0.22])
            .row(&[0.21, 0.23])
            .build()?;

        let base_market = MarketContext::new().insert_surface(surface);

        let scenario = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![RiskFactorShift {
                factor: RiskFactorType::ImpliedVol {
                    vol_surface_id: CurveId::from("EQ-VOL"),
                    expiry_years: 1.0,
                    strike: 100.0,
                },
                shift: 0.02, // +2 vol points
            }],
        );

        let bumped = apply_market_scenario(&scenario, &base_market, None)?;
        let bumped_surface = bumped.get_surface("EQ-VOL")?;
        let vol = finstack_quant_models::volatility::get_surface_vol(&bumped_surface, 1.0, 100.0)
            .expect("grid point lookup should succeed");
        assert!((vol - 0.23).abs() < 1e-9);
        assert_eq!(bumped_surface.vols()[0], 0.20);
        assert_eq!(bumped_surface.vols()[1], 0.22);
        assert_eq!(bumped_surface.vols()[3], 0.23);
        assert_eq!(
            base_market.get_surface("EQ-VOL")?.vols(),
            &[0.20, 0.22, 0.21, 0.23]
        );

        Ok(())
    }

    #[test]
    fn test_vol_point_coordinate_and_duplicate_policy() -> Result<()> {
        use finstack_quant_core::market_data::surfaces::{
            VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
        };

        assert!(same_grid_coordinate(0.0, -0.0));
        assert!(!same_grid_coordinate(f64::NAN, f64::NAN));
        let surface = VolSurface::builder("VOL")
            .expiries(&[0.5, 1.0])
            .secondary_values(&[100.0, 110.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.2, 0.2])
            .row(&[0.2, 0.2])
            .build()?;
        let market = MarketContext::new().insert_surface(surface);
        let point = |id: &str, expiry_years, strike, shift| RiskFactorShift {
            factor: RiskFactorType::ImpliedVol {
                vol_surface_id: id.into(),
                expiry_years,
                strike,
            },
            shift,
        };
        let duplicate = MarketScenario::new(
            date!(2024 - 01 - 02),
            vec![
                point("VOL", 0.5, 100.0, -0.3),
                point("VOL", 0.5, 100.0, 0.2),
            ],
        );
        let shocked = apply_market_scenario(&duplicate, &market, None)?;
        assert!((shocked.get_surface("VOL")?.vols()[0] - 0.1).abs() < 1e-12);
        assert_eq!(&shocked.get_surface("VOL")?.vols()[1..], &[0.2; 3]);

        for invalid in [
            point("VOL", 0.6, 100.0, 0.01),
            point("VOL", 0.5, 105.0, 0.01),
            point("VOL", 0.5, 100.0_f64.next_up(), 0.01),
            point("MISSING", 0.5, 100.0, 0.01),
            point("VOL", f64::NAN, 100.0, 0.01),
            point("VOL", 0.5, 100.0, -0.21),
        ] {
            let scenario = MarketScenario::new(
                date!(2024 - 01 - 02),
                vec![point("VOL", 1.0, 110.0, 0.05), invalid],
            );
            assert!(apply_market_scenario(&scenario, &market, None).is_err());
            assert_eq!(market.get_surface("VOL")?.vols(), &[0.2; 4]);
        }
        Ok(())
    }

    #[test]
    fn market_history_to_table_matches_all_factor_columns() -> Result<()> {
        use finstack_quant_core::table::TableColumnData;
        let as_of = date!(2024 - 01 - 02);
        let history = MarketHistory::new(
            as_of,
            5,
            vec![MarketScenario::new(
                as_of,
                vec![
                    RiskFactorShift {
                        factor: RiskFactorType::DiscountRate {
                            curve_id: "USD-OIS".into(),
                            tenor_years: 5.0,
                            grid_years: vec![5.0],
                        },
                        shift: 0.001,
                    },
                    RiskFactorShift {
                        factor: RiskFactorType::EquitySpot {
                            ticker: "AAPL".into(),
                        },
                        shift: -0.02,
                    },
                    RiskFactorShift {
                        factor: RiskFactorType::FxSpot {
                            base: Currency::EUR,
                            quote: Currency::USD,
                        },
                        shift: 0.01,
                    },
                    RiskFactorShift {
                        factor: RiskFactorType::ImpliedVol {
                            vol_surface_id: "SPX-VOL".into(),
                            expiry_years: 1.0,
                            strike: 100.0,
                        },
                        shift: 0.005,
                    },
                ],
            )],
        );
        let table = market_history_to_table(&history)?;
        assert_eq!(table.columns.len(), 11);
        assert_eq!(table.columns[1].name, "type");
        assert_eq!(
            table.columns[1].data,
            TableColumnData::NullableString(vec![
                Some("discount_rate".into()),
                Some("equity_spot".into()),
                Some("fx_spot".into()),
                Some("implied_vol".into()),
            ])
        );
        assert_eq!(table.columns[5].name, "base");
        assert_eq!(
            table.columns[5].data,
            TableColumnData::NullableString(vec![None, None, Some("EUR".into()), None])
        );
        Ok(())
    }
}
