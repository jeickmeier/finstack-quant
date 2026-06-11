//! Builders for cross-currency swap instruments from market quotes.

use crate::instruments::common_impl::fx_dates::{adjust_joint_calendar, fx_spot_date_for_pair};
use crate::instruments::rates::xccy_swap::{LegSide, XccySwap, XccySwapLeg};
use crate::instruments::Instrument;
use crate::market::conventions::registry::ConventionRegistry;
use crate::market::quotes::ids::Pillar;
use crate::market::quotes::xccy::XccyQuote;
use crate::market::BuildCtx;
use finstack_core::dates::BusinessDayConvention;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_core::Result;
use rust_decimal::Decimal;

/// Build a cross-currency swap instrument from an [`XccyQuote`].
pub fn build_xccy_instrument(quote: &XccyQuote, ctx: &BuildCtx) -> Result<Box<dyn Instrument>> {
    tracing::debug!(quote_id = %quote.id(), "building XCCY instrument");
    let registry = ConventionRegistry::try_global()?;

    match quote {
        XccyQuote::BasisSwap {
            id,
            convention,
            far_pillar,
            basis_spread_bp,
            spot_fx,
        } => {
            let conv = registry.require_xccy(convention)?;
            let base_index = registry.require_rate_index(&conv.base_index_id)?;
            let quote_index = registry.require_rate_index(&conv.quote_index_id)?;

            let domestic_discount = ctx
                .curve_id("domestic_discount")
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}-OIS", conv.quote_currency));
            let foreign_discount = ctx
                .curve_id("foreign_discount")
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}-OIS", conv.base_currency));
            let domestic_forward = ctx
                .curve_id("domestic_forward")
                .map(|s| s.to_string())
                .unwrap_or_else(|| conv.quote_index_id.to_string());
            let foreign_forward = ctx
                .curve_id("foreign_forward")
                .map(|s| s.to_string())
                .unwrap_or_else(|| conv.base_index_id.to_string());

            let fx_spot = spot_fx.ok_or_else(|| {
                finstack_core::Error::Validation(
                    "XCCY quote build requires `spot_fx` to derive FX-equivalent leg notionals"
                        .to_string(),
                )
            })?;
            if !fx_spot.is_finite() || fx_spot <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "XCCY quote build requires positive finite `spot_fx`; got {}",
                    fx_spot
                )));
            }
            if !basis_spread_bp.is_finite() {
                let kind = if basis_spread_bp.is_nan() {
                    finstack_core::NonFiniteKind::NaN
                } else if basis_spread_bp.is_sign_positive() {
                    finstack_core::NonFiniteKind::PosInfinity
                } else {
                    finstack_core::NonFiniteKind::NegInfinity
                };
                return Err(finstack_core::InputError::NonFiniteValue { kind }.into());
            }

            // CLS-consistent spot roll: a US holiday on an intermediate day does
            // not delay a USD pair's spot date (2026-06-09 core quant review,
            // FX spot finding).
            let spot = fx_spot_date_for_pair(
                ctx.as_of(),
                conv.spot_lag_days as u32,
                conv.base_currency,
                conv.quote_currency,
                Some(&conv.base_calendar_id),
                Some(&conv.quote_calendar_id),
            )?;
            let far = resolve_far_date(
                spot,
                far_pillar,
                conv.business_day_convention,
                &conv.base_calendar_id,
                &conv.quote_calendar_id,
            )?;

            let quote_notional = ctx.notional();
            let base_notional = quote_notional / fx_spot;

            let leg1 = XccySwapLeg {
                currency: conv.base_currency,
                notional: Money::new(base_notional, conv.base_currency),
                side: LegSide::Receive,
                forward_curve_id: CurveId::new(foreign_forward),
                discount_curve_id: CurveId::new(foreign_discount),
                start: spot,
                end: far,
                frequency: conv.payment_frequency,
                day_count: conv.day_count,
                bdc: conv.business_day_convention,
                stub: finstack_core::dates::StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: base_index.default_payment_lag_days,
                calendar_id: Some(conv.base_calendar_id.clone()),
                reset_lag_days: Some(base_index.default_reset_lag_days),
                allow_calendar_fallback: false,
            };

            let leg2 = XccySwapLeg {
                currency: conv.quote_currency,
                notional: Money::new(quote_notional, conv.quote_currency),
                side: LegSide::Pay,
                forward_curve_id: CurveId::new(domestic_forward),
                discount_curve_id: CurveId::new(domestic_discount),
                start: spot,
                end: far,
                frequency: conv.payment_frequency,
                day_count: conv.day_count,
                bdc: conv.business_day_convention,
                stub: finstack_core::dates::StubKind::ShortFront,
                spread_bp: Decimal::try_from(*basis_spread_bp)
                    .map_err(|_| finstack_core::InputError::ConversionOverflow)?,
                payment_lag_days: quote_index.default_payment_lag_days,
                calendar_id: Some(conv.quote_calendar_id.clone()),
                reset_lag_days: Some(quote_index.default_reset_lag_days),
                allow_calendar_fallback: false,
            };

            let swap = XccySwap::new(id.as_str(), leg1, leg2, conv.quote_currency)
                .with_notional_exchange(conv.notional_exchange);

            Ok(Box::new(swap))
        }
    }
}

fn resolve_far_date(
    spot: finstack_core::dates::Date,
    pillar: &Pillar,
    bdc: BusinessDayConvention,
    base_calendar_id: &str,
    quote_calendar_id: &str,
) -> Result<finstack_core::dates::Date> {
    match pillar {
        Pillar::Tenor(tenor) => {
            let raw = tenor.add_to_date(spot, None, BusinessDayConvention::Unadjusted)?;
            adjust_joint_calendar(raw, bdc, Some(base_calendar_id), Some(quote_calendar_id))
        }
        Pillar::Date(date) => {
            adjust_joint_calendar(*date, bdc, Some(base_calendar_id), Some(quote_calendar_id))
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/market/build/xccy.rs"]
mod builder_integration_tests;
