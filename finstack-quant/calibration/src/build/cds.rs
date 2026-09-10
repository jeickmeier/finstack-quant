//! Builders for credit instruments from market quotes.

use crate::build::helpers::resolve_spot_date;
use crate::build::BuildCtx;
use crate::quotes::cds::CdsQuote;
use crate::quotes::ids::Pillar;
use finstack_quant_core::dates::{
    next_cds_date, next_semiannual_cds_maturity, prev_cds_semiannual_roll, BusinessDayConvention,
    Date, DateExt,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::{
    Attributes, Instrument, PayReceive, PremiumLegSpec, ProtectionLegSpec,
};
use finstack_quant_valuations::market::conventions::ids::CdsConventionKey;
use finstack_quant_valuations::market::conventions::{CdsConventionSpec, ConventionRegistry};
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
    let conv = registry.resolve_cds(convention_key)?;
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
    conv: &CdsConventionSpec,
    pillar: &Pillar,
) -> Result<CdsResolvedDates> {
    let spot = resolve_spot_date(
        ctx.as_of(),
        &conv.calendar_id,
        i32::from(conv.settlement_days),
        conv.business_day_convention,
    )?;
    // CDS Start: Market standard is the prior CDS roll (20th of Mar/Jun/Sep/Dec).
    // Use the CDS IMM roll date on or before spot.
    let roll_anchor = spot.add_months(-3);
    let start = next_cds_date(roll_anchor);

    let maturity = match pillar {
        Pillar::Tenor(t) => {
            // Post-2015 ISDA semi-annual roll convention.
            let roll = prev_cds_semiannual_roll(ctx.as_of());
            let raw = t.add_to_date(roll, None, BusinessDayConvention::Unadjusted)?;
            next_semiannual_cds_maturity(raw)
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
/// Premium accrual starts on the quarterly CDS roll date (20th of March, June,
/// September, December) on or before spot. For tenor pillars, the scheduled
/// termination date follows the post-2015 ISDA semi-annual roll: the
/// semi-annual roll anchor (20-Mar/20-Sep) on or before the trade date, plus
/// the tenor, extended to the next standard maturity (20-Jun/20-Dec). Explicit
/// date pillars are snapped to the quarterly IMM grid unchanged.
///
/// # Examples
///
/// Building from a par spread quote:
/// ```rust,ignore
/// use finstack_quant_calibration::build::BuildCtx;
/// use finstack_quant_calibration::build::build_cds_instrument;
/// use finstack_quant_calibration::quotes::cds::CdsQuote;
/// use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
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
/// ```rust,ignore
/// use finstack_quant_calibration::build::BuildCtx;
/// use finstack_quant_calibration::build::build_cds_instrument;
/// use finstack_quant_calibration::quotes::cds::CdsQuote;
/// use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
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
/// - [`CdsQuote`] for supported quote types
/// - [`BuildCtx`] for build context configuration
pub fn build_cds_instrument(quote: &CdsQuote, ctx: &BuildCtx) -> Result<Box<dyn Instrument>> {
    tracing::debug!(quote_id = %quote.id(), "building CDS instrument");
    quote.validate()?;
    let registry = ConventionRegistry::try_global()?;

    // Normalize both quote styles onto a shared running-coupon path before building.
    let spread_bp = quote.quoted_running_spread_bp();

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

    ProtectionLegSpec::validate_recovery_rate(recovery_rate)?;

    let conv = registry.resolve_cds(convention_key)?;
    let dates = resolve_cds_dates(ctx, conv, pillar)?;

    let discount_id = ctx.require_curve_id("discount")?.to_string();

    // Credit curve ID: usually defaulted to entity name if not mapped
    let credit_id = ctx.require_curve_id("credit")?.to_string();

    // Amount = Notional * pct; Date = Spot (Settlement)
    let upfront_payment = upfront
        .map(|pct| {
            Ok::<_, finstack_quant_core::Error>((
                dates.spot,
                Money::new(ctx.notional() * pct, convention_key.currency)?,
            ))
        })
        .transpose()?;

    let cds = CreditDefaultSwap {
        id: InstrumentId::new(id.as_str()),
        notional: Money::new(ctx.notional(), convention_key.currency)?,
        // Calibration instruments buy protection and pay premium.
        side: PayReceive::Pay,
        convention: conv.family,
        premium: PremiumLegSpec {
            standard_imm_dates: true,
            start: dates.start,
            end: dates.maturity,
            frequency: conv.frequency,
            stub: conv.stub,
            business_day_convention: conv.business_day_convention,
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
            settlement_delay: conv.settlement_days,
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
    use crate::quotes::cds::CdsQuote;
    use crate::quotes::ids::{Pillar, QuoteId};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::HashMap;
    use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
    use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    use time::Month;

    fn cds_build_ctx() -> BuildCtx {
        let as_of =
            Date::from_calendar_date(2024, Month::January, 2).expect("valid CDS build date");
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
        let explicit_maturity =
            Date::from_calendar_date(2026, Month::June, 20).expect("valid CDS maturity date");

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

    /// On-the-run tenor pillars must land on the semi-annual maturity grid:
    /// 20 June or 20 December only, never March or September (post-2015
    /// ISDA semi-annual roll).
    #[test]
    fn test_cds_tenor_pillar_aligns_to_semiannual_maturity() -> Result<()> {
        let ctx = cds_build_ctx();

        let quote = CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-TEST-5Y"),
            entity: "Test Corp".to_string(),
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
            pillar: Pillar::Tenor("5Y".parse().expect("valid 5Y CDS tenor")),
            spread_bp: 100.0,
            recovery_rate: 0.40,
        };

        let instrument = build_cds_instrument(&quote, &ctx)?;

        let cds = instrument
            .as_any()
            .downcast_ref::<CreditDefaultSwap>()
            .expect("Expected CreditDefaultSwap");

        assert_eq!(
            cds.premium.end.day(),
            20,
            "CDS maturity should be on the 20th"
        );
        assert!(
            matches!(cds.premium.end.month(), Month::June | Month::December),
            "on-the-run CDS maturity must be 20-Jun or 20-Dec, got {:?}",
            cds.premium.end.month()
        );

        Ok(())
    }

    /// Semi-annual maturity roll across all four calendar windows.
    #[test]
    fn test_cds_5y_maturity_all_four_quarters() -> Result<()> {
        let cases = [
            // (trade date, expected 5Y scheduled termination)
            ((2024, Month::January, 15), (2028, Month::December, 20)),
            ((2026, Month::May, 2), (2031, Month::June, 20)),
            ((2026, Month::July, 1), (2031, Month::June, 20)),
            ((2024, Month::October, 1), (2029, Month::December, 20)),
            ((2025, Month::March, 20), (2030, Month::June, 20)),
            ((2025, Month::March, 19), (2029, Month::December, 20)),
        ];

        for ((ty, tm, td), (ey, em, ed)) in cases {
            let as_of =
                Date::from_calendar_date(ty, tm, td).expect("valid CDS trade-date test case");
            let mut curve_ids = HashMap::default();
            curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
            curve_ids.insert("credit".to_string(), "ABC-CORP".to_string());
            let ctx = BuildCtx::new(as_of, 10_000_000.0, curve_ids);

            let quote = CdsQuote::CdsParSpread {
                id: QuoteId::new("CDS-TEST-5Y"),
                entity: "Test Corp".to_string(),
                convention: CdsConventionKey {
                    currency: Currency::USD,
                    doc_clause: CdsDocClause::IsdaNa,
                },
                pillar: Pillar::Tenor("5Y".parse().expect("valid 5Y CDS tenor")),
                spread_bp: 100.0,
                recovery_rate: 0.40,
            };

            let instrument = build_cds_instrument(&quote, &ctx)?;
            let cds = instrument
                .as_any()
                .downcast_ref::<CreditDefaultSwap>()
                .expect("Expected CreditDefaultSwap");

            let expected =
                Date::from_calendar_date(ey, em, ed).expect("valid expected CDS maturity");
            assert_eq!(
                cds.premium.end, expected,
                "trade {as_of}: expected 5Y maturity {expected}, got {}",
                cds.premium.end
            );
        }

        Ok(())
    }

    #[test]
    fn test_cds_tenor_pillar_runs_from_spot_not_prior_imm_start() -> Result<()> {
        let as_of = Date::from_calendar_date(2026, Month::May, 2)
            .expect("valid CDS spot-date regression date");
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
            pillar: Pillar::Tenor("5Y".parse().expect("valid 5Y CDS tenor")),
            spread_bp: 60.5,
            recovery_rate: 0.40,
        };

        let instrument = build_cds_instrument(&quote, &ctx)?;
        use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
        let cds = instrument
            .as_any()
            .downcast_ref::<CreditDefaultSwap>()
            .expect("Expected CreditDefaultSwap");

        assert_eq!(
            cds.premium.end,
            Date::from_calendar_date(2031, Month::June, 20).expect("valid expected CDS maturity")
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
            pillar: Pillar::Tenor("5Y".parse().expect("valid 5Y CDS tenor")),
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
            pillar: Pillar::Tenor("5Y".parse().expect("valid 5Y CDS tenor")),
            spread_bp: 100.0,
            recovery_rate: -0.1,
        };
        let result = build_cds_instrument(&quote, &ctx);
        assert!(result.is_err(), "negative recovery_rate should be rejected");
    }
}
