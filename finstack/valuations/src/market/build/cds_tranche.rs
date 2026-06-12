//! Builders for CDS Tranche instruments from market quotes.

use crate::cashflow::builder::ScheduleParams;
use crate::instruments::credit_derivatives::cds_tranche::parameters::CDSTrancheParams;
use crate::instruments::credit_derivatives::cds_tranche::{CDSTranche, TrancheSide};
use crate::instruments::Instrument;
use crate::market::build::helpers::{resolve_calendar, resolve_spot_date};
use crate::market::conventions::registry::ConventionRegistry;
use crate::market::quotes::cds_tranche::CDSTrancheQuote;
use crate::market::BuildCtx;
use finstack_core::dates::{
    adjust, next_cds_date, BusinessDayConvention, DateExt, DayCount, StubKind, Tenor,
};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_core::{Error, Result};

/// Overrides for CDS tranche schedule and index metadata.
///
/// Allows customization of schedule parameters and index series when building CDS tranche
/// instruments from quotes. Fields default to convention values if not specified.
///
/// # Examples
///
/// ```text
/// use finstack_valuations::market::build::cds_tranche::CDSTrancheBuildOverrides;
///
/// // Use default overrides with only series specified
/// let overrides = CDSTrancheBuildOverrides::new(42);
///
/// // Customize payment frequency and day count
/// let mut overrides = CDSTrancheBuildOverrides::new(42);
/// overrides.frequency = Some("3M".parse().unwrap());
/// overrides.day_count = Some(finstack_core::dates::DayCount::Act360);
/// ```
#[derive(Debug, Clone)]
pub struct CDSTrancheBuildOverrides {
    /// Index series number.
    ///
    /// The series number identifies which version of the CDS index this tranche references
    /// (e.g., CDX.NA.IG Series 42).
    pub series: u16,
    /// Optional payment frequency override.
    ///
    /// If `None`, uses the payment frequency from the CDS convention.
    pub frequency: Option<Tenor>,
    /// Optional day count override.
    ///
    /// If `None`, uses the day count from the CDS convention.
    pub day_count: Option<DayCount>,
    /// Optional business day convention override.
    ///
    /// If `None`, uses the business day convention from the CDS convention.
    pub bdc: Option<BusinessDayConvention>,
    /// Optional calendar identifier override.
    ///
    /// If `None`, uses the calendar ID from the CDS convention.
    pub calendar_id: Option<String>,
    /// Whether to use standard IMM dates for the schedule.
    ///
    /// When `true`, payment dates are aligned to IMM dates (20th of Mar/Jun/Sep/Dec).
    /// When `false`, payment dates follow the standard schedule calculation.
    pub use_imm_dates: bool,
}

impl CDSTrancheBuildOverrides {
    /// Create overrides with only the series specified.
    ///
    /// Optional convention fields default to `None`; `use_imm_dates` defaults to `true`
    /// for standard CDS index-tranche schedules.
    ///
    /// # Arguments
    ///
    /// * `series` - The CDS index series number
    ///
    /// # Returns
    ///
    /// A new `CDSTrancheBuildOverrides` with default values.
    ///
    /// # Examples
    ///
    /// ```text
    /// use finstack_valuations::market::build::cds_tranche::CDSTrancheBuildOverrides;
    ///
    /// let overrides = CDSTrancheBuildOverrides::new(42);
    /// assert_eq!(overrides.series, 42);
    /// assert_eq!(overrides.frequency, None);
    /// assert!(overrides.use_imm_dates);
    /// ```
    pub fn new(series: u16) -> Self {
        Self {
            series,
            frequency: None,
            day_count: None,
            bdc: None,
            calendar_id: None,
            use_imm_dates: true,
        }
    }
}

/// Build a CDS Tranche instrument from a [`CDSTrancheQuote`].
///
/// This function resolves CDS conventions, calculates tranche notional based on attachment
/// and detachment points, and constructs a CDS tranche instrument with upfront and running
/// spread payments.
///
/// # Arguments
///
/// * `quote` - The CDS tranche market quote with attachment/detachment and pricing
/// * `ctx` - Build context with valuation date, notional, and curve mappings
/// * `overrides` - Overrides for schedule parameters and index series
///
/// # Returns
///
/// `Ok(Box<dyn Instrument>)` with the constructed CDS tranche instrument, or `Err` if:
/// - Convention lookup fails (missing CDS convention key)
/// - Calendar resolution fails
/// - Invalid tranche width (detachment <= attachment or non-finite values)
/// - Instrument construction fails (invalid parameters)
///
/// # Tranche Notional Calculation
///
/// The tranche notional is calculated as:
/// ```text
/// tranche_notional = base_notional * (detachment - attachment)
/// ```
///
/// The upfront payment (if present) is calculated as:
/// ```text
/// upfront_amount = tranche_notional * upfront_pct
/// ```
///
/// Note: `upfront_pct` is a decimal fraction (e.g., -0.025 for -2.5%).
///
/// # Examples
///
/// ```text
/// use finstack_valuations::market::BuildCtx;
/// use finstack_valuations::market::build::cds_tranche::{build_cds_tranche_instrument, CDSTrancheBuildOverrides};
/// use finstack_valuations::market::quotes::cds_tranche::CDSTrancheQuote;
/// use finstack_valuations::market::quotes::ids::QuoteId;
/// use finstack_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
/// use finstack_core::dates::Date;
/// use finstack_core::currency::Currency;
/// use finstack_core::HashMap;
///
/// # fn example() -> finstack_core::Result<()> {
/// let ctx = BuildCtx::new(
///     Date::from_calendar_date(2024, time::Month::January, 2).unwrap(),
///     100_000_000.0, // Base notional
///     HashMap::default(),
/// );
///
/// let quote = CDSTrancheQuote::CDSTranche {
///     id: QuoteId::new("CDX-IG-3-7"),
///     index: "CDX.NA.IG".to_string(),
///     attachment: 0.03,  // 3%
///     detachment: 0.07,   // 7%
///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
///     upfront_pct: -0.025, // -2.5% upfront (decimal fraction)
///     running_spread_bp: 500.0,
///     convention: CdsConventionKey {
///         currency: Currency::USD,
///         doc_clause: CdsDocClause::Cr14,
///     },
/// };
///
/// let overrides = CDSTrancheBuildOverrides::new(42);
/// let instrument = build_cds_tranche_instrument(&quote, &ctx, &overrides)?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`CDSTrancheQuote`](crate::market::quotes::cds_tranche::CDSTrancheQuote) for quote structure
/// - [`BuildCtx`](crate::market::BuildCtx) for build context configuration
/// - [`CDSTrancheBuildOverrides`](CDSTrancheBuildOverrides) for override options
pub fn build_cds_tranche_instrument(
    quote: &CDSTrancheQuote,
    ctx: &BuildCtx,
    overrides: &CDSTrancheBuildOverrides,
) -> Result<Box<dyn Instrument>> {
    tracing::debug!(quote_id = %quote.id(), "building CDS tranche instrument");
    let registry = ConventionRegistry::try_global()?;

    // Extract fields
    let (
        id,
        convention_key,
        index,
        attachment,
        detachment,
        maturity,
        running_spread_bp,
        upfront_pct,
    ) = match quote {
        CDSTrancheQuote::CDSTranche {
            id,
            index,
            attachment,
            detachment,
            maturity,
            running_spread_bp,
            upfront_pct,
            convention,
            ..
        } => (
            id,
            convention,
            index,
            *attachment,
            *detachment,
            *maturity,
            *running_spread_bp,
            *upfront_pct,
        ),
    };

    let conv = registry.require_cds(convention_key)?;
    let spot = resolve_spot_date(
        ctx.as_of(),
        &conv.calendar_id,
        conv.settlement_days,
        conv.bdc,
    )?;

    // Resolve calendar for tenor addition
    let cal = resolve_calendar(&conv.calendar_id)?;

    let discount_id = ctx.require_curve_id("discount")?.to_string();

    // Index curve ID: usually defaulted to index name if not mapped
    let credit_id = ctx.require_curve_id("credit")?.to_string();

    let normalization_factor = detachment - attachment;
    if !normalization_factor.is_finite() || normalization_factor <= 0.0 {
        return Err(Error::Validation(format!(
            "Invalid tranche width: attachment={} detachment={}",
            attachment, detachment
        )));
    }
    let notional_amt = ctx.notional() * normalization_factor;

    // Validate upfront_pct is in decimal fraction format (not percentage points).
    // Reject values > 1.0 to prevent accidental misuse of old percentage-point notation.
    if upfront_pct.abs() > 1.0 {
        return Err(Error::Validation(format!(
            "upfront_pct must be a decimal fraction (e.g., -0.025 for -2.5%), got {}; \
             values with abs() > 1.0 are rejected to prevent unit confusion",
            upfront_pct
        )));
    }

    // `upfront_pct` is expressed as a decimal fraction (e.g. -0.025 means -2.5% of tranche notional).
    let upfront_payment = (upfront_pct.abs() > 0.0).then(|| {
        (
            spot,
            Money::new(notional_amt * upfront_pct, convention_key.currency),
        )
    });

    let (effective_date, maturity_date, standard_imm_dates) = if overrides.use_imm_dates {
        // CDS-style effective date (prior IMM) and IMM-aligned maturity.
        let roll_anchor = spot.add_months(-3);
        let effective_date = next_cds_date(roll_anchor);
        // Use unadjusted maturity date for IMM roll selection to prevent BDC
        // from pushing the date past the 20th into the next quarter.
        let maturity_imm = next_cds_date(maturity - time::Duration::days(1));
        (effective_date, maturity_imm, true)
    } else {
        let maturity_adj = adjust(maturity, conv.bdc, cal)?;
        (spot, maturity_adj, false)
    };

    // Construct Params
    let tranche_params = CDSTrancheParams {
        index_name: index.clone(),
        series: overrides.series,
        attach_pct: attachment * 100.0, // Params expect percent
        detach_pct: detachment * 100.0, // Params expect percent
        notional: Money::new(notional_amt, convention_key.currency),
        maturity: maturity_date,
        running_coupon_bp: running_spread_bp,
        accumulated_loss: 0.0,
    };

    let schedule_params = ScheduleParams {
        freq: overrides.frequency.unwrap_or(conv.frequency),
        dc: overrides.day_count.unwrap_or(conv.day_count),
        bdc: overrides.bdc.unwrap_or(conv.bdc),
        calendar_id: overrides
            .calendar_id
            .clone()
            .unwrap_or_else(|| conv.calendar_id.clone()),
        stub: StubKind::ShortFront,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };

    // Side: Quote usually implies we are observing market price.
    // If we build instrument to price it, we usually align with "Buy Protection" logic (pay premium).
    let side = TrancheSide::BuyProtection;

    let mut instrument = CDSTranche::new(
        InstrumentId::new(id.as_str()),
        &tranche_params,
        &schedule_params,
        CurveId::new(discount_id),
        CurveId::new(credit_id),
        side,
    )?;
    instrument.standard_imm_dates = standard_imm_dates;
    instrument.effective_date = Some(effective_date);
    instrument.upfront = upfront_payment;

    Ok(Box::new(instrument))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    use crate::market::quotes::cds_tranche::CDSTrancheQuote;
    use crate::market::quotes::ids::QuoteId;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{adjust, Date};
    use finstack_core::HashMap;
    use time::Month;

    #[test]
    fn build_non_imm_tranche_allows_custom_schedule() {
        let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        let ctx = BuildCtx::new(as_of, 1_000_000.0, curve_ids);

        let convention_key = CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        };
        let maturity = Date::from_calendar_date(2029, Month::January, 15).expect("valid date");

        let quote = CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("CDX-IG-3-7"),
            index: "CDX.NA.IG".to_string(),
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: -0.025, // -2.5% as decimal fraction
            running_spread_bp: 500.0,
            convention: convention_key.clone(),
        };

        let mut overrides = CDSTrancheBuildOverrides::new(42);
        overrides.use_imm_dates = false;

        let instrument = build_cds_tranche_instrument(&quote, &ctx, &overrides)
            .expect("non-IMM tranche build should succeed");
        let tranche = instrument
            .as_any()
            .downcast_ref::<CDSTranche>()
            .expect("should be CDSTranche");

        assert!(!tranche.standard_imm_dates);

        let conv = ConventionRegistry::try_global()
            .expect("registry")
            .require_cds(&convention_key)
            .expect("convention should exist");
        let spot = resolve_spot_date(as_of, &conv.calendar_id, conv.settlement_days, conv.bdc)
            .expect("spot date");
        let cal = resolve_calendar(&conv.calendar_id).expect("calendar");
        let maturity_adj = adjust(maturity, conv.bdc, cal).expect("maturity adjustment");

        assert_eq!(tranche.effective_date, Some(spot));
        assert_eq!(tranche.maturity, maturity_adj);
    }

    /// Regression test: verify upfront_pct uses decimal fraction semantics.
    /// `-0.025` should produce an upfront payment of -2.5% of tranche notional.
    #[test]
    fn test_upfront_decimal_fraction_semantics() {
        let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        // Base notional of 100M -> tranche notional = 100M * (0.07 - 0.03) = 4M
        let ctx = BuildCtx::new(as_of, 100_000_000.0, curve_ids);

        let convention_key = CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        };
        let maturity = Date::from_calendar_date(2029, Month::June, 20).expect("valid date");

        let quote = CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("CDX-IG-3-7"),
            index: "CDX.NA.IG".to_string(),
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: -0.025, // -2.5% as decimal fraction
            running_spread_bp: 500.0,
            convention: convention_key,
        };

        let overrides = CDSTrancheBuildOverrides::new(42);
        let instrument = build_cds_tranche_instrument(&quote, &ctx, &overrides)
            .expect("tranche build should succeed");
        let tranche = instrument
            .as_any()
            .downcast_ref::<CDSTranche>()
            .expect("should be CDSTranche");

        // Tranche notional = 100M * 0.04 = 4M
        // Upfront = 4M * (-0.025) = -100,000 USD
        let (_, upfront_money) = tranche.upfront.expect("should have upfront payment");
        let upfront_amount = upfront_money.amount();

        // Verify sign is negative (protection buyer receives upfront)
        assert!(upfront_amount < 0.0, "Upfront should be negative");

        // Verify magnitude: 4M * 0.025 = 100,000
        let expected_amount = -100_000.0;
        let tolerance = 1.0; // Allow for minor floating point differences
        assert!(
            (upfront_amount - expected_amount).abs() < tolerance,
            "Upfront amount should be ~{}, got {}",
            expected_amount,
            upfront_amount
        );
    }

    /// Regression test: verify that upfront_pct > 1.0 is rejected (prevents old percentage-point notation)
    #[test]
    fn test_upfront_rejects_percentage_point_notation() {
        let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        let ctx = BuildCtx::new(as_of, 100_000_000.0, curve_ids);

        let convention_key = CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        };
        let maturity = Date::from_calendar_date(2029, Month::June, 20).expect("valid date");

        // Use old percentage-point notation (-2.5 instead of -0.025)
        let quote = CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("CDX-IG-3-7"),
            index: "CDX.NA.IG".to_string(),
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: -2.5, // WRONG: this is percentage-point notation
            running_spread_bp: 500.0,
            convention: convention_key,
        };

        let overrides = CDSTrancheBuildOverrides::new(42);
        let result = build_cds_tranche_instrument(&quote, &ctx, &overrides);

        assert!(result.is_err(), "Should reject upfront_pct with abs > 1.0");

        let err_str = result.err().expect("should be error").to_string();
        assert!(
            err_str.contains("decimal fraction") || err_str.contains("upfront_pct"),
            "Error should mention decimal fraction format: {}",
            err_str
        );
    }

    /// Test that zero upfront is handled correctly
    #[test]
    fn test_zero_upfront_no_payment() {
        let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        let ctx = BuildCtx::new(as_of, 100_000_000.0, curve_ids);

        let convention_key = CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        };
        let maturity = Date::from_calendar_date(2029, Month::June, 20).expect("valid date");

        let quote = CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("CDX-IG-3-7"),
            index: "CDX.NA.IG".to_string(),
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: 0.0, // No upfront
            running_spread_bp: 500.0,
            convention: convention_key,
        };

        let overrides = CDSTrancheBuildOverrides::new(42);
        let instrument = build_cds_tranche_instrument(&quote, &ctx, &overrides)
            .expect("tranche build should succeed");
        let tranche = instrument
            .as_any()
            .downcast_ref::<CDSTranche>()
            .expect("should be CDSTranche");

        assert!(
            tranche.upfront.is_none(),
            "Zero upfront should result in no upfront payment"
        );
    }
}
