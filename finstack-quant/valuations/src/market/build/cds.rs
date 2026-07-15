//! Builders for credit instruments from market quotes.

use crate::instruments::common_impl::parameters::legs::{
    PayReceive, PremiumLegSpec, ProtectionLegSpec,
};
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::credit_derivatives::cds::{CDSConvention, CreditDefaultSwap};
use crate::instruments::Instrument;
use crate::market::build::helpers::{resolve_calendar, resolve_spot_date};
use crate::market::conventions::ids::CdsConventionKey;
use crate::market::conventions::registry::ConventionRegistry;
use crate::market::conventions::CdsConventions;
use crate::market::quotes::cds::CdsQuote;
use crate::market::quotes::ids::Pillar;
use crate::market::BuildCtx;
use finstack_quant_core::dates::{next_cds_date, BusinessDayConvention, Date, DateExt, StubKind};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;
use rust_decimal::Decimal;

/// Resolved CDS dates used by both builders and calibration prepared quotes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CdsResolvedDates {
    pub(crate) spot: Date,
    pub(crate) start: Date,
    pub(crate) maturity: Date,
}

/// Resolve CDS quote dates without requiring callers to inspect the boxed instrument.
pub(crate) fn resolve_cds_quote_dates(
    quote: &CdsQuote,
    ctx: &BuildCtx,
) -> Result<CdsResolvedDates> {
    let (convention_key, pillar) = cds_quote_convention_and_pillar(quote);
    let registry = ConventionRegistry::try_global()?;
    let conv = registry.require_cds(convention_key)?;
    resolve_cds_dates(ctx, conv, pillar)
}

fn cds_quote_convention_and_pillar(quote: &CdsQuote) -> (&CdsConventionKey, &Pillar) {
    match quote {
        CdsQuote::CdsParSpread {
            convention, pillar, ..
        }
        | CdsQuote::CdsUpfront {
            convention, pillar, ..
        } => (convention, pillar),
    }
}

fn resolve_cds_dates(
    ctx: &BuildCtx,
    conv: &CdsConventions,
    pillar: &Pillar,
) -> Result<CdsResolvedDates> {
    let spot = resolve_spot_date(
        ctx.as_of(),
        &conv.calendar_id,
        conv.settlement_days,
        conv.bdc,
    )?;
    let cal = resolve_calendar(&conv.calendar_id)?;

    // CDS Start: Market standard is the prior CDS roll (20th of Mar/Jun/Sep/Dec).
    // Use the CDS IMM roll date on or before spot.
    let roll_anchor = spot.add_months(-3);
    let start = next_cds_date(roll_anchor);

    let maturity = match pillar {
        Pillar::Tenor(t) => {
            // Market CDS tenors run from the trade/spot date and then roll to
            // the next standard IMM-20 maturity. They do not run from the prior
            // accrual start date; doing so turns a May 5Y quote into the March
            // roll instead of the Bloomberg/ISDA June roll.
            let raw = t.add_to_date(spot, Some(cal), BusinessDayConvention::Unadjusted)?;
            next_cds_date(raw - time::Duration::days(1))
        }
        Pillar::Date(d) => {
            // Enforce IMM alignment using the unadjusted input date.
            // Do NOT business-day adjust before roll selection, as that can push
            // the date past the 20th and cause next_cds_date to return the next quarter.
            next_cds_date(*d - time::Duration::days(1))
        }
    };

    Ok(CdsResolvedDates {
        spot,
        start,
        maturity,
    })
}

/// Build a Credit Default Swap instrument from a [`CdsQuote`].
///
/// This function resolves CDS conventions, calculates IMM roll dates, and constructs a CDS
/// instrument with premium and protection legs configured according to market standards.
/// Supports both par spread quotes and upfront + running spread quotes.
///
/// # Arguments
///
/// * `quote` - The CDS market quote (either par spread or upfront + running)
/// * `ctx` - Build context with valuation date, notional, and curve mappings
///
/// # Returns
///
/// `Ok(Box<dyn Instrument>)` with the constructed CDS instrument, or `Err` if:
/// - Convention lookup fails (missing CDS convention key)
/// - Calendar resolution fails
/// - Date calculations fail (invalid pillar, IMM roll date resolution)
/// - Instrument construction fails (invalid parameters)
///
/// # CDS Date Conventions
///
/// CDS instruments use IMM roll dates (20th of March, June, September, December) for
/// start dates. The start date is set to the IMM date on or before spot, and maturity
/// is adjusted to the next IMM date after the pillar date.
///
/// # Examples
///
/// Building from a par spread quote:
/// ```rust
/// use finstack_quant_valuations::market::BuildCtx;
/// use finstack_quant_valuations::market::build_cds_instrument;
/// use finstack_quant_valuations::market::quotes::cds::CdsQuote;
/// use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
/// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::HashMap;
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let ctx = BuildCtx::new(
///     Date::from_calendar_date(2024, time::Month::January, 2).unwrap(),
///     10_000_000.0,
///     HashMap::default(),
/// );
///
/// let quote = CdsQuote::CdsParSpread {
///     id: QuoteId::new("CDS-ABC-CORP-5Y"),
///     entity: "ABC Corp".to_string(),
///     convention: CdsConventionKey {
///         currency: Currency::USD,
///         doc_clause: CdsDocClause::Cr14,
///     },
///     pillar: Pillar::Tenor("5Y".parse().unwrap()),
///     spread_bp: 150.0,
///     recovery_rate: 0.40,
/// };
///
/// let instrument = build_cds_instrument(&quote, &ctx)?;
/// # Ok(())
/// # }
/// ```
///
/// Building from an upfront quote:
/// ```rust
/// use finstack_quant_valuations::market::BuildCtx;
/// use finstack_quant_valuations::market::build_cds_instrument;
/// use finstack_quant_valuations::market::quotes::cds::CdsQuote;
/// use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
/// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::HashMap;
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let ctx = BuildCtx::new(
///     Date::from_calendar_date(2024, time::Month::January, 2).unwrap(),
///     10_000_000.0,
///     HashMap::default(),
/// );
///
/// let quote = CdsQuote::CdsUpfront {
///     id: QuoteId::new("CDS-ABC-CORP-5Y"),
///     entity: "ABC Corp".to_string(),
///     convention: CdsConventionKey {
///         currency: Currency::USD,
///         doc_clause: CdsDocClause::Cr14,
///     },
///     pillar: Pillar::Tenor("5Y".parse().unwrap()),
///     running_spread_bp: 500.0,
///     upfront_pct: 0.02, // 2% upfront
///     recovery_rate: 0.40,
/// };
///
/// let instrument = build_cds_instrument(&quote, &ctx)?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`CdsQuote`](crate::market::quotes::cds::CdsQuote) for supported quote types
/// - [`BuildCtx`](crate::market::BuildCtx) for build context configuration
pub fn build_cds_instrument(quote: &CdsQuote, ctx: &BuildCtx) -> Result<Box<dyn Instrument>> {
    tracing::debug!(quote_id = %quote.id(), "building CDS instrument");
    quote.validate_market_conventions()?;
    let registry = ConventionRegistry::try_global()?;

    // Normalize both quote styles onto a shared running-coupon path before building.
    let spread_bp = quote.quoted_running_spread_bp();

    // Extract the remaining fields.
    let (id, convention_key, entity, pillar, recovery_rate, upfront) = match quote {
        CdsQuote::CdsParSpread {
            id,
            entity,
            convention,
            pillar,
            recovery_rate,
            ..
        } => (id, convention, entity, pillar, *recovery_rate, None),
        CdsQuote::CdsUpfront {
            id,
            entity,
            convention,
            pillar,
            upfront_pct,
            recovery_rate,
            ..
        } => (
            id,
            convention,
            entity,
            pillar,
            *recovery_rate,
            Some(*upfront_pct),
        ),
    };

    crate::instruments::common_impl::validation::validate_recovery_rate(recovery_rate)?;

    let conv = registry.require_cds(convention_key)?;
    let dates = resolve_cds_dates(ctx, conv, pillar)?;

    let discount_id = ctx.require_curve_id("discount")?.to_string();

    // Credit curve ID: usually defaulted to entity name if not mapped
    let credit_id = ctx.require_curve_id("credit")?.to_string();

    // Calculate upfront amount if present
    // Amount = Notional * pct; Date = Spot (Settlement)
    let upfront_payment = upfront.map(|pct| {
        (
            dates.spot,
            Money::new(ctx.notional() * pct, convention_key.currency),
        )
    });

    let convention_enum = CDSConvention::detect_from_currency(convention_key.currency);

    let cds = CreditDefaultSwap {
        id: InstrumentId::new(id.as_str()),
        notional: Money::new(ctx.notional(), convention_key.currency),
        side: PayReceive::Pay, // Standard: Quote implies we buy protection (pay premium/spread) ? Or we are pricing the contract?
        // Usually "Par Spread" implies the spread we pay.
        // Default to Buy Protection (Pay Premium).
        convention: convention_enum,
        premium: PremiumLegSpec {
            start: dates.start,
            end: dates.maturity,
            frequency: conv.frequency,
            stub: StubKind::None, // Default to None or derive?
            bdc: conv.bdc,
            calendar_id: Some(conv.calendar_id.clone()),
            day_count: conv.day_count,
            spread_bp: Decimal::try_from(spread_bp).map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "spread_bp {} cannot be represented as Decimal: {}",
                    spread_bp, e
                ))
            })?,
            discount_curve_id: CurveId::new(discount_id),
        },
        protection: ProtectionLegSpec {
            credit_curve_id: CurveId::new(credit_id),
            recovery_rate,
            settlement_delay: conv.settlement_days as u16,
        },
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        valuation_convention: ctx.cds_valuation_convention().unwrap_or_default(),
        upfront: upfront_payment,
        doc_clause: Some(convention_key.doc_clause),
        protection_effective_date: None,
        margin_spec: None,
        attributes: Attributes::new().with_meta("entity", entity.as_str()),
    };

    Ok(Box::new(cds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    use crate::market::quotes::cds::CdsQuote;
    use crate::market::quotes::ids::{Pillar, QuoteId};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::HashMap;
    use time::Month;

    fn cds_build_ctx() -> BuildCtx {
        let as_of = Date::from_calendar_date(2024, Month::January, 2).unwrap();
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "ABC-CORP".to_string());
        BuildCtx::new(as_of, 10_000_000.0, curve_ids)
    }

    /// Regression test: CDS maturity roll alignment should not jump to the next quarter
    /// when the 20th of the target month falls on a weekend.
    ///
    /// Example: June 20, 2026 is a Saturday. The CDS maturity should still be 2026-06-20,
    /// not 2026-09-20 (which would happen if we business-day adjusted before roll selection).
    #[test]
    fn test_cds_maturity_roll_does_not_jump_quarter_on_weekend() -> Result<()> {
        let ctx = cds_build_ctx();

        // June 20, 2026 is a Saturday - pick this as our explicit maturity date
        let explicit_maturity = Date::from_calendar_date(2026, Month::June, 20).unwrap();

        let quote = CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-TEST-5Y"),
            entity: "Test Corp".to_string(),
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
            pillar: Pillar::Date(explicit_maturity),
            spread_bp: 100.0,
            recovery_rate: 0.40,
        };

        let instrument = build_cds_instrument(&quote, &ctx)?;

        use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
        let cds = instrument
            .as_any()
            .downcast_ref::<CreditDefaultSwap>()
            .expect("Expected CreditDefaultSwap");

        // The maturity should be June 20, 2026 (the IMM date), NOT September 20, 2026
        // next_cds_date(June 19) returns June 20
        assert_eq!(
            cds.premium.end.month(),
            Month::June,
            "CDS maturity should be in June, not jumped to September"
        );
        assert_eq!(
            cds.premium.end, explicit_maturity,
            "CDS maturity should be exactly 2026-06-20"
        );

        Ok(())
    }

    /// Test that tenor-based CDS pillar also correctly aligns to IMM dates
    #[test]
    fn test_cds_tenor_pillar_aligns_to_imm() -> Result<()> {
        let ctx = cds_build_ctx();

        let quote = CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-TEST-5Y"),
            entity: "Test Corp".to_string(),
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
            pillar: Pillar::Tenor("5Y".parse().unwrap()),
            spread_bp: 100.0,
            recovery_rate: 0.40,
        };

        let instrument = build_cds_instrument(&quote, &ctx)?;

        use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
        let cds = instrument
            .as_any()
            .downcast_ref::<CreditDefaultSwap>()
            .expect("Expected CreditDefaultSwap");

        // Maturity should be on the 20th (CDS IMM date)
        assert_eq!(
            cds.premium.end.day(),
            20,
            "CDS maturity should be on the 20th (IMM date)"
        );

        // Should be in a quarterly month (Mar, Jun, Sep, Dec)
        let maturity_month = cds.premium.end.month();
        assert!(
            matches!(
                maturity_month,
                Month::March | Month::June | Month::September | Month::December
            ),
            "CDS maturity should be in a quarterly month, got {:?}",
            maturity_month
        );

        Ok(())
    }

    #[test]
    fn test_cds_tenor_pillar_runs_from_spot_not_prior_imm_start() -> Result<()> {
        let as_of = Date::from_calendar_date(2026, Month::May, 2).unwrap();
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "IBM-USD-SENIOR".to_string());
        let ctx = BuildCtx::new(as_of, 10_000_000.0, curve_ids);

        let quote = CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-IBM-5Y"),
            entity: "IBM".to_string(),
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
            pillar: Pillar::Tenor("5Y".parse().unwrap()),
            spread_bp: 60.5,
            recovery_rate: 0.40,
        };

        let instrument = build_cds_instrument(&quote, &ctx)?;
        use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
        let cds = instrument
            .as_any()
            .downcast_ref::<CreditDefaultSwap>()
            .expect("Expected CreditDefaultSwap");

        assert_eq!(
            cds.premium.end,
            Date::from_calendar_date(2031, Month::June, 20).unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_cds_rejects_recovery_rate_above_one() {
        let ctx = cds_build_ctx();
        let quote = CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-TEST"),
            entity: "Test Corp".to_string(),
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
            pillar: Pillar::Tenor("5Y".parse().unwrap()),
            spread_bp: 100.0,
            recovery_rate: 1.5,
        };
        let result = build_cds_instrument(&quote, &ctx);
        assert!(result.is_err(), "recovery_rate > 1.0 should be rejected");
    }

    #[test]
    fn test_cds_rejects_negative_recovery_rate() {
        let ctx = cds_build_ctx();
        let quote = CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-TEST"),
            entity: "Test Corp".to_string(),
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
            pillar: Pillar::Tenor("5Y".parse().unwrap()),
            spread_bp: 100.0,
            recovery_rate: -0.1,
        };
        let result = build_cds_instrument(&quote, &ctx);
        assert!(result.is_err(), "negative recovery_rate should be rejected");
    }
}
