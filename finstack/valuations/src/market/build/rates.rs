//! Builders for interest rate instruments from market quotes.

use crate::instruments::rates::deposit::Deposit;
use crate::instruments::rates::fra::ForwardRateAgreement;
use crate::instruments::rates::ir_future::{FutureContractSpecs, InterestRateFuture, Position};
use crate::instruments::rates::irs::{InterestRateSwap, IrsLegConventions};
use crate::instruments::DynInstrument;
use crate::market::build::helpers::{resolve_calendar, resolve_spot_date};
use crate::market::conventions::defs::{RateIndexConventions, RateIndexKind};
use crate::market::conventions::registry::ConventionRegistry;
use crate::market::quotes::ids::Pillar;
use crate::market::quotes::rates::RateQuote;
use crate::market::BuildCtx;
use finstack_core::dates::{adjust, Date, DateExt, TenorUnit};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_core::{Error, InputError, Result};
use rust_decimal::Decimal;

/// Build an interest rate instrument from a [`RateQuote`].
///
/// This function resolves conventions, calculates accrual dates, and constructs a concrete
/// instrument instance based on the quote type. Supported quote types include deposits, FRAs,
/// interest rate futures, and swaps (both term and overnight).
///
/// # Arguments
///
/// * `quote` - The market quote containing rate/price and pillar information
/// * `ctx` - Build context with valuation date, notional, and curve mappings
///
/// # Returns
///
/// `Ok(Box<DynInstrument>)` with the constructed instrument, or `Err` if:
/// - Convention lookup fails (missing index or future contract)
/// - Calendar resolution fails
/// - Date calculations fail (invalid tenor, business day adjustment)
/// - Instrument construction fails (invalid parameters)
///
/// # Examples
///
/// Building a deposit:
/// ```rust
/// use finstack_valuations::market::BuildCtx;
/// use finstack_valuations::market::build_rate_instrument;
/// use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
/// use finstack_valuations::market::quotes::rates::RateQuote;
/// use finstack_valuations::market::conventions::ids::IndexId;
/// use finstack_core::dates::Date;
/// use finstack_core::HashMap;
///
/// # fn example() -> finstack_core::Result<()> {
/// let ctx = BuildCtx::new(
///     Date::from_calendar_date(2024, time::Month::January, 2).unwrap(),
///     1_000_000.0,
///     HashMap::default(),
/// );
///
/// let quote = RateQuote::Deposit {
///     id: QuoteId::new("USD-SOFR-DEP-1M"),
///     index: IndexId::new("USD-SOFR-1M"),
///     pillar: Pillar::Tenor("1M".parse().unwrap()),
///     rate: 0.0525,
/// };
///
/// let instrument = build_rate_instrument(&quote, &ctx)?;
/// # Ok(())
/// # }
/// ```
///
/// Building a swap:
/// ```rust
/// use finstack_valuations::market::BuildCtx;
/// use finstack_valuations::market::build_rate_instrument;
/// use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
/// use finstack_valuations::market::quotes::rates::RateQuote;
/// use finstack_valuations::market::conventions::ids::IndexId;
/// use finstack_core::dates::Date;
/// use finstack_core::HashMap;
///
/// # fn example() -> finstack_core::Result<()> {
/// let ctx = BuildCtx::new(
///     Date::from_calendar_date(2024, time::Month::January, 2).unwrap(),
///     1_000_000.0,
///     HashMap::default(),
/// );
///
/// let quote = RateQuote::Swap {
///     id: QuoteId::new("USD-OIS-SWAP-5Y"),
///     index: IndexId::new("USD-SOFR-OIS"),
///     pillar: Pillar::Tenor("5Y".parse().unwrap()),
///     rate: 0.0450,
///     spread_decimal: None,
/// };
///
/// let instrument = build_rate_instrument(&quote, &ctx)?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`RateQuote`](crate::market::quotes::rates::RateQuote) for supported quote types
/// - [`BuildCtx`](crate::market::BuildCtx) for build context configuration
pub fn build_rate_instrument(quote: &RateQuote, ctx: &BuildCtx) -> Result<Box<DynInstrument>> {
    tracing::debug!(quote_id = %quote.id(), "building rate instrument");
    let registry = ConventionRegistry::try_global()?;

    match quote {
        RateQuote::Deposit { .. } => build_deposit(quote, ctx, registry),
        RateQuote::Fra { .. } => build_fra(quote, ctx, registry),
        RateQuote::Futures { .. } => build_future(quote, ctx, registry),
        RateQuote::Swap { .. } => build_swap(quote, ctx, registry),
    }
}

/// Resolved date set for an interest-rate market quote.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RateResolvedDates {
    /// Deposit accrual dates.
    Deposit {
        /// Accrual start date.
        start: Date,
        /// Accrual end date.
        end: Date,
    },
    /// FRA accrual and fixing dates.
    Fra {
        /// Fixing date.
        fixing: Date,
        /// Accrual start date.
        start: Date,
        /// Accrual end date.
        end: Date,
    },
    /// Interest-rate future dates.
    Future {
        /// Adjusted contract expiry.
        expiry: Date,
        /// Underlying period start.
        period_start: Date,
        /// Underlying period end.
        period_end: Date,
        /// Underlying rate fixing date.
        fixing: Date,
    },
    /// Swap accrual dates plus calibration pillar date.
    Swap {
        /// Swap start date.
        start: Date,
        /// Swap maturity used by the instrument.
        maturity: Date,
        /// Pillar date used by calibration.
        pillar_date: Date,
    },
}

impl RateResolvedDates {
    /// Date used as the quote pillar in calibration time axes.
    pub(crate) fn pillar_date(self) -> Date {
        match self {
            Self::Deposit { end, .. } | Self::Fra { end, .. } => end,
            Self::Future { period_end, .. } => period_end,
            Self::Swap { pillar_date, .. } => pillar_date,
        }
    }
}

/// Resolve rate quote dates without requiring callers to inspect the boxed instrument.
pub(crate) fn resolve_rate_quote_dates(
    quote: &RateQuote,
    ctx: &BuildCtx,
    swap_use_payment_delay: bool,
) -> Result<RateResolvedDates> {
    let registry = ConventionRegistry::try_global()?;
    match quote {
        RateQuote::Deposit { index, pillar, .. } => {
            let conv = registry.require_rate_index(index)?;
            let spot = resolve_spot_date(
                ctx.as_of(),
                &conv.market_calendar_id,
                conv.market_settlement_days,
                conv.market_business_day_convention,
            )?;
            let cal = resolve_calendar(&conv.market_calendar_id)?;
            let end = match pillar {
                Pillar::Tenor(t) => {
                    t.add_to_date(spot, Some(cal), conv.market_business_day_convention)?
                }
                Pillar::Date(d) => adjust(*d, conv.market_business_day_convention, cal)?,
            };
            Ok(RateResolvedDates::Deposit { start: spot, end })
        }
        RateQuote::Fra {
            index,
            start: start_pillar,
            end: end_pillar,
            ..
        } => {
            let conv = registry.require_rate_index(index)?;
            let spot = resolve_spot_date(
                ctx.as_of(),
                &conv.market_calendar_id,
                conv.market_settlement_days,
                conv.market_business_day_convention,
            )?;
            let cal = resolve_calendar(&conv.market_calendar_id)?;
            let start = match start_pillar {
                Pillar::Tenor(t) => {
                    t.add_to_date(spot, Some(cal), conv.market_business_day_convention)?
                }
                Pillar::Date(d) => adjust(*d, conv.market_business_day_convention, cal)?,
            };
            let end = match end_pillar {
                Pillar::Tenor(t) => {
                    t.add_to_date(spot, Some(cal), conv.market_business_day_convention)?
                }
                Pillar::Date(d) => adjust(*d, conv.market_business_day_convention, cal)?,
            };
            let fixing = resolve_fixing_date(start, conv)?;
            Ok(RateResolvedDates::Fra { fixing, start, end })
        }
        RateQuote::Futures {
            contract, expiry, ..
        } => {
            let fut_conv = registry.require_ir_future(contract)?;
            let idx_conv = registry.require_rate_index(&fut_conv.index_id)?;
            let cal = resolve_calendar(&fut_conv.calendar_id)?;
            let bdc = idx_conv.market_business_day_convention;
            let expiry = adjust(*expiry, bdc, cal)?;
            let period_start_unadj = expiry.add_business_days(fut_conv.settlement_days, cal)?;
            let period_start = adjust(period_start_unadj, bdc, cal)?;
            let delivery_tenor = finstack_core::dates::Tenor::new(
                fut_conv.delivery_months as u32,
                TenorUnit::Months,
            );
            let period_end = delivery_tenor.add_to_date(period_start, Some(cal), bdc)?;
            let fixing = resolve_fixing_date(period_start, idx_conv)?;
            Ok(RateResolvedDates::Future {
                expiry,
                period_start,
                period_end,
                fixing,
            })
        }
        RateQuote::Swap { index, pillar, .. } => {
            let conv = registry.require_rate_index(index)?;
            let start = resolve_spot_date(
                ctx.as_of(),
                &conv.market_calendar_id,
                conv.market_settlement_days,
                conv.market_business_day_convention,
            )?;
            let cal = resolve_calendar(&conv.market_calendar_id)?;
            let maturity = match pillar {
                Pillar::Tenor(t) => {
                    t.add_to_date(start, Some(cal), conv.market_business_day_convention)?
                }
                Pillar::Date(d) => adjust(*d, conv.market_business_day_convention, cal)?,
            };
            let pillar_date = if swap_use_payment_delay {
                crate::instruments::common_impl::pricing::swap_legs::add_payment_delay(
                    maturity,
                    conv.default_payment_lag_days,
                    Some(&conv.market_calendar_id),
                )?
            } else {
                maturity
            };
            Ok(RateResolvedDates::Swap {
                start,
                maturity,
                pillar_date,
            })
        }
    }
}

fn build_deposit(
    quote: &RateQuote,
    ctx: &BuildCtx,
    registry: &ConventionRegistry,
) -> Result<Box<DynInstrument>> {
    let RateQuote::Deposit {
        id, index, rate, ..
    } = quote
    else {
        return Err(Error::Validation(
            "build_deposit called with non-deposit rate quote".to_string(),
        ));
    };

    let conv = registry.require_rate_index(index)?;
    let RateResolvedDates::Deposit { start, end } = resolve_rate_quote_dates(quote, ctx, false)?
    else {
        return Err(Error::Validation(
            "resolved non-deposit dates for deposit quote".to_string(),
        ));
    };

    let deposit = Deposit::builder()
        .id(InstrumentId::new(id.as_str()))
        .notional(Money::new(ctx.notional(), conv.currency))
        .start_date(start)
        .maturity(end)
        .day_count(conv.day_count)
        .quote_rate_opt(Some(
            Decimal::try_from(*rate).map_err(|_| InputError::ConversionOverflow)?,
        ))
        .discount_curve_id(CurveId::new(ctx.require_curve_id("discount")?.to_string()))
        .bdc(conv.market_business_day_convention)
        .calendar_id_opt(Some(conv.market_calendar_id.clone().into()))
        .attributes(Default::default())
        .build()?;

    Ok(Box::new(deposit))
}

fn build_fra(
    quote: &RateQuote,
    ctx: &BuildCtx,
    registry: &ConventionRegistry,
) -> Result<Box<DynInstrument>> {
    let RateQuote::Fra {
        id, index, rate, ..
    } = quote
    else {
        return Err(Error::Validation(
            "build_fra called with non-FRA rate quote".to_string(),
        ));
    };

    let conv = registry.require_rate_index(index)?;
    let RateResolvedDates::Fra {
        fixing,
        start: start_date,
        end: end_date,
    } = resolve_rate_quote_dates(quote, ctx, false)?
    else {
        return Err(Error::Validation(
            "resolved non-FRA dates for FRA quote".to_string(),
        ));
    };

    let fra = ForwardRateAgreement::builder()
        .id(InstrumentId::new(id.as_str()))
        .notional(Money::new(ctx.notional(), conv.currency))
        .fixing_date(fixing)
        .start_date(start_date)
        .maturity(end_date)
        .fixed_rate(Decimal::try_from(*rate).map_err(|_| InputError::ConversionOverflow)?)
        .day_count(conv.day_count)
        .reset_lag(conv.default_reset_lag_days)
        .discount_curve_id(CurveId::new(ctx.require_curve_id("discount")?.to_string()))
        .forward_curve_id(CurveId::new(ctx.require_curve_id("forward")?.to_string()))
        .side(crate::instruments::common_impl::parameters::legs::PayReceive::ReceiveFixed)
        .fixing_calendar_id_opt(Some(conv.market_calendar_id.clone().into()))
        .fixing_bdc_opt(Some(conv.market_business_day_convention))
        .attributes(Default::default())
        .build()?;

    Ok(Box::new(fra))
}

fn build_future(
    quote: &RateQuote,
    ctx: &BuildCtx,
    registry: &ConventionRegistry,
) -> Result<Box<DynInstrument>> {
    let RateQuote::Futures {
        id,
        contract,
        price,
        convexity_adjustment,
        vol_surface_id,
        ..
    } = quote
    else {
        return Err(Error::Validation(
            "build_future called with non-future rate quote".to_string(),
        ));
    };

    let fut_conv = registry.require_ir_future(contract)?;
    let idx_conv = registry.require_rate_index(&fut_conv.index_id)?;
    let RateResolvedDates::Future {
        expiry: expiry_date,
        period_start,
        period_end,
        fixing,
    } = resolve_rate_quote_dates(quote, ctx, false)?
    else {
        return Err(Error::Validation(
            "resolved non-future dates for futures quote".to_string(),
        ));
    };

    let contract_specs = FutureContractSpecs {
        face_value: fut_conv.face_value,
        tick_size: fut_conv.tick_size,
        tick_value: fut_conv.tick_value,
        delivery_months: fut_conv.delivery_months,
        convexity_adjustment: (*convexity_adjustment).or(fut_conv.convexity_adjustment),
    };

    let future = InterestRateFuture::builder()
        .id(InstrumentId::new(id.as_str()))
        .notional(Money::new(ctx.notional(), idx_conv.currency))
        .expiry(expiry_date)
        .fixing_date(fixing)
        .period_start(period_start)
        .period_end(period_end)
        .quoted_price(*price)
        .day_count(idx_conv.day_count)
        .position(Position::Long)
        .contract_specs(contract_specs)
        .discount_curve_id(CurveId::new(ctx.require_curve_id("discount")?.to_string()))
        .forward_curve_id(CurveId::new(ctx.require_curve_id("forward")?.to_string()))
        .vol_surface_id_opt(vol_surface_id.clone())
        .attributes(Default::default())
        .build()?;

    Ok(Box::new(future))
}

fn build_swap(
    quote: &RateQuote,
    ctx: &BuildCtx,
    registry: &ConventionRegistry,
) -> Result<Box<DynInstrument>> {
    let RateQuote::Swap {
        id,
        index,
        rate,
        spread_decimal,
        ..
    } = quote
    else {
        return Err(Error::Validation(
            "build_swap called with non-swap rate quote".to_string(),
        ));
    };

    let conv = registry.require_rate_index(index)?;
    let RateResolvedDates::Swap {
        start, maturity, ..
    } = resolve_rate_quote_dates(quote, ctx, false)?
    else {
        return Err(Error::Validation(
            "resolved non-swap dates for swap quote".to_string(),
        ));
    };

    use crate::instruments::common_impl::parameters::legs::PayReceive;

    let end_of_month = start == start.end_of_month();
    let leg_conv = IrsLegConventions {
        fixed_freq: conv.default_fixed_leg_frequency,
        float_freq: conv.default_payment_frequency,
        fixed_dc: conv.default_fixed_leg_day_count,
        float_dc: conv.day_count,
        bdc: conv.market_business_day_convention,
        payment_calendar_id: Some(conv.market_calendar_id.clone()),
        fixing_calendar_id: Some(conv.market_calendar_id.clone()),
        stub: finstack_core::dates::StubKind::ShortFront,
        reset_lag_days: conv.default_reset_lag_days,
        payment_lag_days: conv.default_payment_lag_days,
    };

    let rate_decimal =
        Decimal::try_from(*rate).map_err(|_| finstack_core::InputError::ConversionOverflow)?;
    let compounding = match conv.kind {
        RateIndexKind::Term => crate::instruments::rates::irs::FloatingLegCompounding::Simple,
        RateIndexKind::OvernightRfr => match ctx.ois_compounding_override() {
            // Step-level override takes precedence over the per-index registry
            // default. This lets a calibration step match a vendor-specific
            // convention (e.g. Bloomberg SWPM SOFR uses CompoundedWithRateCutoff)
            // without changing the global registry.
            Some(override_compounding) => override_compounding.clone(),
            None => conv.ois_compounding.clone().ok_or_else(|| {
                Error::Validation(
                    "Overnight RFR index conventions must specify `ois_compounding`".to_string(),
                )
            })?,
        },
    };

    if matches!(conv.kind, RateIndexKind::OvernightRfr)
        && matches!(
            compounding,
            crate::instruments::rates::irs::FloatingLegCompounding::Simple
        )
    {
        return Err(Error::Validation(
            "OIS swap requires compounded-in-arrears floating compounding".to_string(),
        ));
    }

    let discount_id = ctx.require_curve_id("discount")?.to_string();
    let forward_id = ctx.require_curve_id("forward")?.to_string();
    let fixed = crate::instruments::common_impl::parameters::legs::FixedLegSpec {
        discount_curve_id: CurveId::new(discount_id.clone()),
        rate: rate_decimal,
        frequency: leg_conv.fixed_freq,
        day_count: leg_conv.fixed_dc,
        bdc: leg_conv.bdc,
        calendar_id: leg_conv.payment_calendar_id.clone(),
        stub: leg_conv.stub,
        start,
        end: maturity,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: leg_conv.payment_lag_days,
        end_of_month,
    };

    let float = crate::instruments::common_impl::parameters::legs::FloatLegSpec {
        discount_curve_id: CurveId::new(discount_id),
        forward_curve_id: CurveId::new(forward_id),
        spread_bp: Decimal::ZERO,
        frequency: leg_conv.float_freq,
        day_count: leg_conv.float_dc,
        bdc: leg_conv.bdc,
        calendar_id: leg_conv.payment_calendar_id,
        stub: leg_conv.stub,
        reset_lag_days: leg_conv.reset_lag_days,
        fixing_calendar_id: leg_conv.fixing_calendar_id,
        start,
        end: maturity,
        compounding,
        payment_lag_days: leg_conv.payment_lag_days,
        end_of_month,
    };

    let mut swap = InterestRateSwap::builder()
        .id(InstrumentId::new(id.as_str()))
        .notional(Money::new(ctx.notional(), conv.currency))
        .side(PayReceive::PayFixed)
        .fixed(fixed)
        .float(float)
        .build()?;

    apply_swap_spread(&mut swap, spread_decimal)?;
    Ok(Box::new(swap))
}

fn apply_swap_spread(swap: &mut InterestRateSwap, spread_decimal: &Option<f64>) -> Result<()> {
    let Some(spread_decimal) = spread_decimal else {
        return Ok(());
    };

    // `spread_decimal` is in decimal format (e.g. 0.0010 for 10bp).
    let spread_bp_f64 = *spread_decimal * 10_000.0;
    if !spread_bp_f64.is_finite() {
        let kind = if spread_bp_f64.is_nan() {
            finstack_core::NonFiniteKind::NaN
        } else if spread_bp_f64.is_sign_positive() {
            finstack_core::NonFiniteKind::PosInfinity
        } else {
            finstack_core::NonFiniteKind::NegInfinity
        };
        return Err(finstack_core::InputError::NonFiniteValue { kind }.into());
    }
    swap.float.spread_bp = Decimal::try_from(spread_bp_f64)
        .map_err(|_| finstack_core::InputError::ConversionOverflow)?;
    Ok(())
}

fn resolve_fixing_date(start: Date, conv: &RateIndexConventions) -> Result<Date> {
    let cal = resolve_calendar(&conv.market_calendar_id)?;
    let lag = conv.default_reset_lag_days;

    start.add_business_days(-lag, cal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::conventions::ids::IndexId;
    use crate::market::quotes::ids::{Pillar, QuoteId};
    use finstack_core::HashMap;

    fn usd_build_ctx() -> BuildCtx {
        let as_of = Date::from_calendar_date(2024, time::Month::January, 2).unwrap();
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("forward".to_string(), "USD-SOFR".to_string());
        BuildCtx::new(as_of, 1_000_000.0, curve_ids)
    }

    /// Test that spread_decimal is correctly converted to basis points
    #[test]
    fn test_swap_spread_decimal_conversion() -> Result<()> {
        let ctx = usd_build_ctx();

        // Create a swap quote with spread_decimal = 0.0010 (10bp)
        let quote = RateQuote::Swap {
            id: QuoteId::new("USD-SOFR-OIS-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: Some(0.0010), // 10bp in decimal
        };

        let instrument = build_rate_instrument(&quote, &ctx)?;

        // Downcast to InterestRateSwap to access spread_bp
        use crate::instruments::rates::irs::InterestRateSwap;
        let swap = instrument
            .as_any()
            .downcast_ref::<InterestRateSwap>()
            .expect("Expected InterestRateSwap");

        // Verify spread_decimal (0.0010) was converted to spread_bp (10.0)
        assert_eq!(
            swap.float.spread_bp,
            rust_decimal::Decimal::try_from(10.0).expect("valid"),
            "Expected spread_decimal of 0.0010 to convert to 10.0 basis points"
        );

        Ok(())
    }

    /// Test that swap with no spread works correctly
    #[test]
    fn test_swap_no_spread() -> Result<()> {
        let ctx = usd_build_ctx();

        let quote = RateQuote::Swap {
            id: QuoteId::new("USD-SOFR-OIS-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: None,
        };

        let instrument = build_rate_instrument(&quote, &ctx)?;

        // Should build successfully
        use crate::instruments::rates::irs::InterestRateSwap;
        let swap = instrument
            .as_any()
            .downcast_ref::<InterestRateSwap>()
            .expect("Expected InterestRateSwap");

        // Default spread_bp should be 0.0
        assert_eq!(
            swap.float.spread_bp,
            rust_decimal::Decimal::ZERO,
            "Expected default spread_bp to be 0.0"
        );

        Ok(())
    }

    /// Test spread conversion with various values
    #[test]
    fn test_swap_spread_various_values() -> Result<()> {
        let test_cases = vec![
            (0.0001, 1.0),    // 1bp
            (0.0010, 10.0),   // 10bp
            (0.0050, 50.0),   // 50bp
            (0.0100, 100.0),  // 100bp (1%)
            (-0.0010, -10.0), // -10bp (negative spread)
        ];

        for (spread_decimal, expected_bp) in test_cases {
            let ctx = usd_build_ctx();

            let quote = RateQuote::Swap {
                id: QuoteId::new("USD-SOFR-OIS-SWAP-5Y"),
                index: IndexId::new("USD-SOFR-OIS"),
                pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                    5,
                    finstack_core::dates::TenorUnit::Years,
                )),
                rate: 0.0450,
                spread_decimal: Some(spread_decimal),
            };

            let instrument = build_rate_instrument(&quote, &ctx)?;

            use crate::instruments::rates::irs::InterestRateSwap;
            let swap = instrument
                .as_any()
                .downcast_ref::<InterestRateSwap>()
                .expect("Expected InterestRateSwap");

            assert_eq!(
                swap.float.spread_bp,
                rust_decimal::Decimal::try_from(expected_bp).expect("valid"),
                "spread_decimal {} should convert to {} basis points",
                spread_decimal,
                expected_bp
            );
        }

        Ok(())
    }

    /// Test that NaN spread_decimal produces an error
    #[test]
    fn test_swap_spread_nan_returns_error() {
        let ctx = usd_build_ctx();

        let quote = RateQuote::Swap {
            id: QuoteId::new("USD-SOFR-OIS-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: Some(f64::NAN),
        };

        let result = build_rate_instrument(&quote, &ctx);
        assert!(result.is_err(), "NaN spread_decimal should return an error");

        // Verify error message contains expected text
        let err = result.err().expect("should be error");
        let err_str = err.to_string();
        assert!(
            err_str.contains("Non-finite") || err_str.contains("NaN"),
            "Error should mention non-finite value: {}",
            err_str
        );
    }

    /// Test that infinity spread_decimal produces an error
    #[test]
    fn test_swap_spread_infinity_returns_error() {
        let ctx = usd_build_ctx();

        // Test positive infinity
        let quote = RateQuote::Swap {
            id: QuoteId::new("USD-SOFR-OIS-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: Some(f64::INFINITY),
        };

        let result = build_rate_instrument(&quote, &ctx);
        assert!(
            result.is_err(),
            "Positive infinity spread_decimal should return an error"
        );

        // Test negative infinity
        let quote_neg = RateQuote::Swap {
            id: QuoteId::new("USD-SOFR-OIS-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: Some(f64::NEG_INFINITY),
        };

        let result_neg = build_rate_instrument(&quote_neg, &ctx);
        assert!(
            result_neg.is_err(),
            "Negative infinity spread_decimal should return an error"
        );
    }

    /// Regression test: verify that reset_lag_days uses positive T-minus semantics.
    /// With reset_lag_days = 2, the fixing date should be 2 business days BEFORE accrual start.
    #[test]
    fn test_fixing_date_is_t_minus_2() -> Result<()> {
        let ctx = usd_build_ctx();

        // Build an FRA with USD-SOFR-3M index (which has reset_lag_days = 2 in conventions)
        let quote = RateQuote::Fra {
            id: QuoteId::new("USD-SOFR-FRA-3x6"),
            index: IndexId::new("USD-SOFR-3M"),
            start: Pillar::Tenor("3M".parse().unwrap()),
            end: Pillar::Tenor("6M".parse().unwrap()),
            rate: 0.05,
        };

        let instrument = build_rate_instrument(&quote, &ctx)?;

        use crate::instruments::rates::fra::ForwardRateAgreement;
        let fra = instrument
            .as_any()
            .downcast_ref::<ForwardRateAgreement>()
            .expect("Expected ForwardRateAgreement");

        // If fixing_date is provided, it should be BEFORE the start date (T-2 semantics)
        if let Some(fixing_date) = fra.fixing_date {
            assert!(
                fixing_date < fra.start_date,
                "Fixing date {} should be before start date {} (T-minus semantics)",
                fixing_date,
                fra.start_date
            );

            // Verify it's approximately 2 business days before (exact depends on calendar)
            let days_diff = (fra.start_date - fixing_date).whole_days();
            assert!(
                (2..=4).contains(&days_diff),
                "Fixing date should be ~2 business days before start, got {} calendar days",
                days_diff
            );
        }

        Ok(())
    }

    #[test]
    fn test_swap_defaults_to_short_front_stub() -> Result<()> {
        let ctx = usd_build_ctx();

        let quote = RateQuote::Swap {
            id: QuoteId::new("USD-SOFR-OIS-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: None,
        };

        let instrument = build_rate_instrument(&quote, &ctx)?;
        use crate::instruments::rates::irs::InterestRateSwap;
        let swap = instrument
            .as_any()
            .downcast_ref::<InterestRateSwap>()
            .expect("Expected InterestRateSwap");

        assert_eq!(swap.fixed.stub, finstack_core::dates::StubKind::ShortFront);
        assert_eq!(swap.float.stub, finstack_core::dates::StubKind::ShortFront);

        Ok(())
    }

    #[test]
    fn test_swap_enables_eom_when_start_is_month_end() -> Result<()> {
        let as_of = Date::from_calendar_date(2024, time::Month::January, 31).unwrap();
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "GBP-SONIA-OIS".to_string());
        curve_ids.insert("forward".to_string(), "GBP-SONIA-OIS".to_string());
        let ctx = BuildCtx::new(as_of, 1_000_000.0, curve_ids);

        let quote = RateQuote::Swap {
            id: QuoteId::new("GBP-SONIA-OIS-SWAP-2Y"),
            index: IndexId::new("GBP-SONIA-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                2,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.04,
            spread_decimal: None,
        };

        let instrument = build_rate_instrument(&quote, &ctx)?;
        use crate::instruments::rates::irs::InterestRateSwap;
        let swap = instrument
            .as_any()
            .downcast_ref::<InterestRateSwap>()
            .expect("Expected InterestRateSwap");

        assert!(swap.fixed.start.month() == time::Month::January && swap.fixed.start.day() == 31);
        assert!(swap.fixed.end_of_month);
        assert!(swap.float.end_of_month);

        Ok(())
    }
}
