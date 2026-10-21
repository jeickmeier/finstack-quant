//! Builders for cross-currency swap instruments from market quotes.

use crate::build::BuildCtx;
use crate::quotes::ids::Pillar;
use crate::quotes::xccy::XccyQuote;
use finstack_quant_core::dates::fx::fx_spot_date_for_pair;
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::dates::{adjust, resolve_joint_calendar};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::pricing::overnight_conventions::compounding_from_conventions;
use finstack_quant_valuations_core::instruments::FloatLegSpec;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::market::conventions::ConventionRegistry;
use finstack_quant_valuations_rates::xccy_swap::{XccySwap, XccySwapLeg};
use rust_decimal::Decimal;

/// Build a cross-currency swap instrument from an [`XccyQuote`].
pub(crate) fn build_xccy_instrument(quote: &XccyQuote, ctx: &BuildCtx) -> Result<XccySwap> {
    tracing::debug!(quote_id = %quote.id(), "building XCCY instrument");
    quote.validate()?;
    let registry = ConventionRegistry::try_global()?;

    let id = &quote.id;
    let convention = &quote.convention;
    let far_pillar = &quote.far_pillar;
    let basis_spread_bp = quote.basis_spread_bp;
    let spot_fx = quote.spot_fx;

    let conv = registry.require_xccy(convention)?;
    let base_index = registry.require_rate_index(&conv.base_index_id)?;
    let quote_index = registry.require_rate_index(&conv.quote_index_id)?;

    let domestic_discount = ctx.require_curve_id("domestic_discount")?.to_string();
    let foreign_discount = ctx.require_curve_id("foreign_discount")?.to_string();
    // Each leg projects on its contractual index curve with that index's
    // compounding.
    let domestic_forward = conv.quote_index_id.to_string();
    let foreign_forward = conv.base_index_id.to_string();
    let foreign_compounding = compounding_from_conventions(base_index)?;
    let domestic_compounding = compounding_from_conventions(quote_index)?;

    let fx_spot = spot_fx.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "XCCY quote build requires `spot_fx` to derive FX-equivalent leg notionals".to_string(),
        )
    })?;

    // CLS-consistent spot roll: a US holiday on an intermediate day does
    // not delay a USD pair's spot date (2026-06-09 core quant review,
    // FX spot finding).
    let spot = fx_spot_date_for_pair(
        ctx.as_of(),
        conv.settlement_days,
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

    // Apply the quoted basis to the base leg; the quote leg is flat.
    let leg1 = XccySwapLeg {
        notional: Money::new(base_notional, conv.base_currency)?,
        side: PayReceive::Receive,
        leg: FloatLegSpec {
            forward_curve_id: CurveId::new(foreign_forward),
            discount_curve_id: CurveId::new(foreign_discount),
            start: spot,
            end: far,
            frequency: conv.payment_frequency,
            day_count: conv.day_count,
            business_day_convention: conv.business_day_convention,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            spread_bp: Decimal::try_from(basis_spread_bp)
                .map_err(|_| finstack_quant_core::InputError::ConversionOverflow)?,
            payment_lag_days: base_index.default_payment_lag_days,
            calendar_id: Some(conv.base_calendar_id.clone().into()),
            reset_lag_days: base_index.default_reset_lag_days,
            compounding: foreign_compounding,
            fixing_calendar_id: None,
            end_of_month: false,
        },
    };

    let leg2 = XccySwapLeg {
        notional: Money::new(quote_notional, conv.quote_currency)?,
        side: PayReceive::Pay,
        leg: FloatLegSpec {
            forward_curve_id: CurveId::new(domestic_forward),
            discount_curve_id: CurveId::new(domestic_discount),
            start: spot,
            end: far,
            frequency: conv.payment_frequency,
            day_count: conv.day_count,
            business_day_convention: conv.business_day_convention,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: quote_index.default_payment_lag_days,
            calendar_id: Some(conv.quote_calendar_id.clone().into()),
            reset_lag_days: quote_index.default_reset_lag_days,
            compounding: domestic_compounding,
            fixing_calendar_id: None,
            end_of_month: false,
        },
    };

    let swap = XccySwap::new(id.as_str(), leg1, leg2, conv.quote_currency)
        .with_notional_exchange(conv.notional_exchange);

    Ok(swap)
}

fn resolve_far_date(
    spot: finstack_quant_core::dates::Date,
    pillar: &Pillar,
    business_day_convention: BusinessDayConvention,
    base_calendar_id: &str,
    quote_calendar_id: &str,
) -> Result<finstack_quant_core::dates::Date> {
    let calendar = resolve_joint_calendar(Some(base_calendar_id), Some(quote_calendar_id))?;
    match pillar {
        Pillar::Tenor(tenor) => {
            let raw = tenor.add_to_date(spot, None, BusinessDayConvention::Unadjusted)?;
            adjust(raw, business_day_convention, calendar)
        }
        Pillar::Date(date) => adjust(*date, business_day_convention, calendar),
    }
}

#[cfg(test)]
mod tests;
