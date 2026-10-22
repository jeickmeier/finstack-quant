//! Builders for CDS Tranche instruments from market quotes.

use crate::build::cds::cds_accrual_start;
use crate::build::helpers::resolve_spot_date;
use crate::build::BuildCtx;
use crate::quotes::CdsTrancheQuote;
use finstack_quant_cashflows::builder::RollRule;
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::dates::calendar_by_id_strict;
use finstack_quant_core::dates::{
    adjust, next_cds_date, BusinessDayConvention, DateExt, DayCount, Tenor,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::market::conventions::ConventionRegistry;
use finstack_quant_valuations_credit::cds_tranche::{CdsTranche, CdsTrancheParams};

/// Overrides for CDS tranche schedule conventions.
///
/// The typed quote owns index identity and series. Overrides are limited to
/// schedule construction and default to the convention registry.
#[derive(Debug, Clone)]
pub(crate) struct CdsTrancheBuildOverrides {
    /// Optional payment frequency override.
    ///
    /// If `None`, uses the payment frequency from the CDS convention.
    pub(crate) frequency: Option<Tenor>,
    /// Optional day count override.
    ///
    /// If `None`, uses the day count from the CDS convention.
    pub(crate) day_count: Option<DayCount>,
    /// Optional business day convention override.
    ///
    /// If `None`, uses the business day convention from the CDS convention.
    pub(crate) business_day_convention: Option<BusinessDayConvention>,
    /// Optional calendar identifier override.
    ///
    /// If `None`, uses the calendar ID from the CDS convention.
    pub(crate) calendar_id: Option<String>,
    /// Coupon roll-date grid for the built tranche.
    ///
    /// `CdsImm` aligns payment dates to CDS IMM dates (20th of
    /// Mar/Jun/Sep/Dec); `None` follows the convention frequency and stub.
    /// `Imm` is rejected.
    pub(crate) roll_rule: RollRule,
}

impl Default for CdsTrancheBuildOverrides {
    fn default() -> Self {
        Self {
            frequency: None,
            day_count: None,
            business_day_convention: None,
            calendar_id: None,
            roll_rule: RollRule::CdsImm,
        }
    }
}

/// Build a CDS Tranche instrument from a [`CdsTrancheQuote`].
pub(crate) fn build_cds_tranche_instrument(
    quote: &CdsTrancheQuote,
    ctx: &BuildCtx,
    overrides: &CdsTrancheBuildOverrides,
) -> Result<CdsTranche> {
    tracing::debug!(quote_id = %quote.id(), "building CDS tranche instrument");
    quote.validate()?;
    let registry = ConventionRegistry::try_global()?;

    let id = &quote.id;
    let convention_key = &quote.convention;
    let index = &quote.index;
    let series = quote.series;
    let attachment = quote.attachment;
    let detachment = quote.detachment;
    let maturity = quote.maturity;
    let coupon_bp = quote.coupon_bp;
    let upfront_pct = quote.upfront_pct;

    let conv = registry.resolve_cds(convention_key)?;
    let spot = resolve_spot_date(
        ctx.as_of(),
        &conv.calendar_id,
        i32::from(conv.settlement_days),
        conv.business_day_convention,
    )?;

    let cal = calendar_by_id_strict(&conv.calendar_id)?;

    let discount_id = ctx.require_curve_id("discount")?.to_string();

    // Index curve ID: usually defaulted to index name if not mapped
    let credit_id = ctx.require_curve_id("credit")?.to_string();

    let normalization_factor = detachment - attachment;
    let notional_amt = ctx.notional() * normalization_factor;

    // `upfront_pct` is expressed as a decimal fraction (e.g. -0.025 means -2.5% of tranche notional).
    let upfront = (upfront_pct.abs() > 0.0)
        .then(|| {
            Ok::<_, finstack_quant_core::Error>((
                spot,
                Money::new(notional_amt * upfront_pct, convention_key.currency)?,
            ))
        })
        .transpose()?;

    let (effective_date, maturity_date) = match overrides.roll_rule {
        RollRule::CdsImm => {
            // Premium accrual follows the trade-date coupon period, even when
            // upfront cash settlement crosses the next quarterly roll.
            let effective_date = cds_accrual_start(ctx.as_of())?;
            // Use unadjusted maturity date for IMM roll selection to prevent BDC
            // from pushing the date past the 20th into the next quarter.
            let maturity_imm = next_cds_date(maturity.add_days(-1)?)?;
            (effective_date, maturity_imm)
        }
        RollRule::None => {
            let maturity_adj = adjust(maturity, conv.business_day_convention, cal)?;
            (spot, maturity_adj)
        }
        other => {
            return Err(Error::Validation(format!(
                "CDS tranche roll_rule must be cds_imm or none, got {other:?}"
            )));
        }
    };

    let tranche_params = CdsTrancheParams {
        index_name: index.clone(),
        series,
        attach_pct: attachment * 100.0, // Params expect percent
        detach_pct: detachment * 100.0, // Params expect percent
        notional: Money::new(notional_amt, convention_key.currency)?,
        maturity: maturity_date,
        coupon_bp,
        realized_loss: 0.0,
    };

    let schedule_params = ScheduleParams {
        frequency: overrides.frequency.unwrap_or(conv.frequency),
        day_count: overrides.day_count.unwrap_or(conv.day_count),
        business_day_convention: overrides
            .business_day_convention
            .unwrap_or(conv.business_day_convention),
        calendar_id: overrides
            .calendar_id
            .clone()
            .unwrap_or_else(|| conv.calendar_id.clone())
            .into(),
        stub: conv.stub,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: overrides.roll_rule,
    };

    // Quote-built tranches are constructed as buy-protection (pay premium),
    // the standard orientation for calibration instruments.
    let side = PayReceive::Pay;

    let mut instrument = CdsTranche::new(
        InstrumentId::new(id.as_str()),
        &tranche_params,
        &schedule_params,
        CurveId::new(discount_id),
        CurveId::new(credit_id),
        side,
    )?;
    instrument.start_date = Some(effective_date);
    instrument.upfront = upfront;

    Ok(instrument)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quotes::CdsTrancheQuote;
    use crate::quotes::QuoteId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{adjust, Date};
    use finstack_quant_core::FxHashMap;
    use finstack_quant_valuations_core::market::conventions::ids::{
        CdsConventionKey, CdsDocClause,
    };
    use time::Month;

    #[test]
    fn build_non_imm_tranche_allows_custom_schedule() {
        let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let mut curve_ids = FxHashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        let ctx = BuildCtx::new(as_of, 1_000_000.0, curve_ids);

        let convention_key = CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        };
        let maturity = Date::from_calendar_date(2029, Month::January, 15).expect("valid date");

        let quote = CdsTrancheQuote {
            id: QuoteId::new("CDX-IG-3-7"),
            index: "CDX.NA.IG".to_string(),
            series: 1,
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: -0.025, // -2.5% as decimal fraction
            coupon_bp: 500.0,
            convention: convention_key.clone(),
        };

        let overrides = CdsTrancheBuildOverrides {
            roll_rule: RollRule::None,
            ..CdsTrancheBuildOverrides::default()
        };

        let instrument = build_cds_tranche_instrument(&quote, &ctx, &overrides)
            .expect("non-IMM tranche build should succeed");
        let tranche = instrument;

        assert_eq!(tranche.roll_rule, RollRule::None);

        let conv = ConventionRegistry::try_global()
            .expect("registry")
            .resolve_cds(&convention_key)
            .expect("convention should exist");
        let spot = resolve_spot_date(
            as_of,
            &conv.calendar_id,
            i32::from(conv.settlement_days),
            conv.business_day_convention,
        )
        .expect("spot date");
        let cal = calendar_by_id_strict(&conv.calendar_id).expect("calendar");
        let maturity_adj =
            adjust(maturity, conv.business_day_convention, cal).expect("maturity adjustment");

        assert_eq!(tranche.start_date, Some(spot));
        assert_eq!(tranche.maturity, maturity_adj);
    }

    /// Regression test: verify upfront_pct uses decimal fraction semantics.
    /// `-0.025` should produce an upfront payment of -2.5% of tranche notional.
    #[test]
    fn test_upfront_decimal_fraction_semantics() {
        let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let mut curve_ids = FxHashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        // Base notional of 100M -> tranche notional = 100M * (0.07 - 0.03) = 4M
        let ctx = BuildCtx::new(as_of, 100_000_000.0, curve_ids);

        let convention_key = CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        };
        let maturity = Date::from_calendar_date(2029, Month::June, 20).expect("valid date");

        let quote = CdsTrancheQuote {
            id: QuoteId::new("CDX-IG-3-7"),
            index: "CDX.NA.IG".to_string(),
            series: 1,
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: -0.025, // -2.5% as decimal fraction
            coupon_bp: 500.0,
            convention: convention_key,
        };

        let overrides = CdsTrancheBuildOverrides::default();
        let instrument = build_cds_tranche_instrument(&quote, &ctx, &overrides)
            .expect("tranche build should succeed");
        let tranche = instrument;

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
        let mut curve_ids = FxHashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        let ctx = BuildCtx::new(as_of, 100_000_000.0, curve_ids);

        let convention_key = CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        };
        let maturity = Date::from_calendar_date(2029, Month::June, 20).expect("valid date");

        // Use old percentage-point notation (-2.5 instead of -0.025)
        let quote = CdsTrancheQuote {
            id: QuoteId::new("CDX-IG-3-7"),
            index: "CDX.NA.IG".to_string(),
            series: 1,
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: -2.5, // WRONG: this is percentage-point notation
            coupon_bp: 500.0,
            convention: convention_key,
        };

        let overrides = CdsTrancheBuildOverrides::default();
        let result = build_cds_tranche_instrument(&quote, &ctx, &overrides);

        assert!(result.is_err(), "Should reject upfront_pct with abs > 1.0");

        let err_str = result.expect_err("should be error").to_string();
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
        let mut curve_ids = FxHashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        let ctx = BuildCtx::new(as_of, 100_000_000.0, curve_ids);

        let convention_key = CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        };
        let maturity = Date::from_calendar_date(2029, Month::June, 20).expect("valid date");

        let quote = CdsTrancheQuote {
            id: QuoteId::new("CDX-IG-3-7"),
            index: "CDX.NA.IG".to_string(),
            series: 1,
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: 0.0, // No upfront
            coupon_bp: 500.0,
            convention: convention_key,
        };

        let overrides = CdsTrancheBuildOverrides::default();
        let instrument = build_cds_tranche_instrument(&quote, &ctx, &overrides)
            .expect("tranche build should succeed");
        let tranche = instrument;

        assert!(
            tranche.upfront.is_none(),
            "Zero upfront should result in no upfront payment"
        );
    }
}
