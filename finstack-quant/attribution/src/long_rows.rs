//! Long-format detail-row projection of a [`PnlAttribution`].
//!
//! This module is the canonical definition of the "long" tabular view of an
//! attribution result: one row per populated detail entry, identified by a
//! dotted `kind` taxonomy (e.g. `"rates.by_curve"`, `"carry.coupon_income"`,
//! `"credit_factor.level.by_bucket"`). Host bindings (Python DataFrame
//! exports, future WASM tables) delegate here so the row schema and taxonomy
//! cannot drift per host.
//!
//! # Row schema
//!
//! | Column | Meaning |
//! |---|---|
//! | `kind` | Dotted path identifying the row's origin (closed taxonomy below) |
//! | `factor` | Parent factor family (`"rates"`, `"credit"`, `"fx"`, ...) |
//! | `sub` | `kind` with the `factor.` prefix removed (`"by_curve"`, `"coupon_income.rates"`, ...) |
//! | `key_a` | Primary identifier (curve id, pair label, level name, ...) |
//! | `key_b` | Secondary key when present (tenor, `to` currency, bucket path) |
//! | `amount` | Signed P&L amount as `f64` |
//! | `currency` | ISO-4217 code of the row's own `Money` value |
//!
//! Each row's `currency` is taken from its OWN `Money` value, never from the
//! parent factor aggregate: detail maps are not currency-validated by
//! [`PnlAttribution::validate_currencies`], so stamping the parent's currency
//! could silently mislabel a mixed-currency payload.
//!
//! # Kind taxonomy
//!
//! - `rates.by_curve`, `rates.by_tenor`, `rates.discount_total`,
//!   `rates.forward_total`
//! - `credit.by_curve`, `credit.by_tenor`
//! - `inflation.by_curve`, `inflation.by_tenor`
//! - `correlations.by_curve`
//! - `fx.by_pair`
//! - `vol.by_surface`
//! - `cross_factor.by_pair`
//! - `scalars.dividends`, `scalars.inflation`, `scalars.equity_prices`,
//!   `scalars.commodity_prices`
//! - `model_params.named`, `model_params.other`
//! - `carry.total`, `carry.coupon_income`, `carry.coupon_income.rates`,
//!   `carry.coupon_income.credit`, `carry.pull_to_par`, `carry.roll_down`,
//!   `carry.roll_down.rates`, `carry.roll_down.credit`, `carry.funding_cost`
//! - `credit_factor.generic`, `credit_factor.level`,
//!   `credit_factor.level.by_bucket`, `credit_factor.adder`,
//!   `credit_factor.curve_shape`, `credit_factor.adder_by_issuer`
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_attribution::{
//!     pnl_attribution_long_rows, AttributionMethod, PnlAttribution,
//! };
//! use finstack_quant_core::{currency::Currency, dates::create_date, money::Money};
//! use time::Month;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let attribution = PnlAttribution::new(
//!     Money::from((500_i64, Currency::USD)),
//!     "AAPL",
//!     create_date(2025, Month::January, 15)?,
//!     create_date(2025, Month::January, 16)?,
//!     AttributionMethod::Parallel,
//! );
//! // No detail maps are populated on a bare result, so the projection is empty.
//! assert!(pnl_attribution_long_rows(&attribution).is_empty());
//! # Ok(())
//! # }
//! ```

use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use indexmap::IndexMap;
use serde::Serialize;

use crate::PnlAttribution;

/// One long-format detail row projected from a [`PnlAttribution`].
///
/// `kind` and `factor` come from the closed taxonomy documented at the
/// [module level](self); `key_a`/`key_b` identify the row within its kind.
/// `currency` is owned because `Currency::Display` allocates; rows are
/// typically serialized immediately after construction so the per-row
/// `String` is cheap.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LongDetailRow {
    /// Dotted-path row origin (e.g. `"rates.by_curve"`); closed taxonomy.
    pub kind: &'static str,
    /// Parent factor family (e.g. `"rates"`, `"carry"`, `"credit_factor"`).
    pub factor: &'static str,
    /// Sub-component of `kind` after the `factor` prefix (e.g. `"by_curve"`,
    /// `"coupon_income.rates"`, `"level.by_bucket"`).
    pub sub: &'static str,
    /// Primary identifier: curve id, pair label, level name, or component.
    pub key_a: String,
    /// Secondary key when present: tenor, `to` currency, or bucket path.
    pub key_b: Option<String>,
    /// Signed P&L amount.
    pub amount: f64,
    /// ISO-4217 currency code of this row's own `Money` value.
    pub currency: String,
}

/// Strip the `factor.` prefix from a dotted `kind`, leaving the sub-component.
fn kind_sub(kind: &'static str, factor: &'static str) -> &'static str {
    kind.strip_prefix(factor)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or("")
}

impl LongDetailRow {
    /// Build a row from a `Money` value, stamping that value's own currency.
    fn from_money(
        kind: &'static str,
        factor: &'static str,
        key_a: String,
        key_b: Option<String>,
        money: &Money,
    ) -> Self {
        Self {
            kind,
            factor,
            sub: kind_sub(kind, factor),
            key_a,
            key_b,
            amount: money.amount(),
            currency: money.currency().to_string(),
        }
    }
}

/// One wide (single-row) projection of a [`PnlAttribution`] aggregate fields.
///
/// Used by the Python `to_dataframe` export. Every amount is taken from its
/// own `Money` value after [`PnlAttribution::validate_currencies`] so the
/// single `currency` column labels comparable units.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PnlAttributionWideRow {
    /// Instrument identifier from attribution metadata.
    pub instrument_id: String,
    /// Canonical snake-case method name.
    pub method: String,
    /// Opening valuation date as `YYYY-MM-DD`.
    pub t0: String,
    /// Closing valuation date as `YYYY-MM-DD`.
    pub t1: String,
    /// ISO-4217 code of `total_pnl` (and every factor, after validation).
    pub currency: String,
    /// Total reported P&L amount.
    pub total_pnl: f64,
    /// Raw mark-to-market P&L when the method recorded it.
    pub mark_to_market_pnl: Option<f64>,
    /// Carry P&L amount.
    pub carry: f64,
    /// Rates-curves P&L amount.
    pub rates_curves_pnl: f64,
    /// Credit-curves P&L amount.
    pub credit_curves_pnl: f64,
    /// Inflation-curves P&L amount.
    pub inflation_curves_pnl: f64,
    /// Correlations P&L amount.
    pub correlations_pnl: f64,
    /// Pricing-impact FX P&L amount.
    pub fx_pnl: f64,
    /// Reporting-currency FX translation P&L amount.
    pub fx_translation_pnl: f64,
    /// Volatility P&L amount.
    pub vol_pnl: f64,
    /// Cross-factor interaction P&L amount.
    pub cross_factor_pnl: f64,
    /// Model-parameter P&L amount.
    pub model_params_pnl: f64,
    /// Market-scalars P&L amount.
    pub market_scalars_pnl: f64,
    /// Residual P&L amount.
    pub residual: f64,
    /// Residual as a percentage of total P&L.
    pub residual_pct: f64,
    /// Number of repricings performed.
    pub num_repricings: usize,
    /// True when residual or a factor is non-finite.
    pub result_invalid: bool,
}

/// Project aggregate attribution fields into one wide row.
///
/// # Arguments
///
/// * `attribution` - Attribution result whose aggregate Money fields are
///   flattened. Currencies must already agree or the call fails.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when any factor
/// currency differs from `total_pnl`.
pub fn pnl_attribution_wide_row(attribution: &PnlAttribution) -> Result<PnlAttributionWideRow> {
    attribution.validate_currencies()?;
    Ok(PnlAttributionWideRow {
        instrument_id: attribution.meta.instrument_id.clone(),
        method: attribution.meta.method.as_str().to_owned(),
        t0: attribution.meta.t0.to_string(),
        t1: attribution.meta.t1.to_string(),
        currency: attribution.total_pnl.currency().to_string(),
        total_pnl: attribution.total_pnl.amount(),
        mark_to_market_pnl: attribution.mark_to_market_pnl.map(|m| m.amount()),
        carry: attribution.carry.amount(),
        rates_curves_pnl: attribution.rates_curves_pnl.amount(),
        credit_curves_pnl: attribution.credit_curves_pnl.amount(),
        inflation_curves_pnl: attribution.inflation_curves_pnl.amount(),
        correlations_pnl: attribution.correlations_pnl.amount(),
        fx_pnl: attribution.fx_pnl.amount(),
        fx_translation_pnl: attribution.fx_translation_pnl.amount(),
        vol_pnl: attribution.vol_pnl.amount(),
        cross_factor_pnl: attribution.cross_factor_pnl.amount(),
        model_params_pnl: attribution.model_params_pnl.amount(),
        market_scalars_pnl: attribution.market_scalars_pnl.amount(),
        residual: attribution.residual.amount(),
        residual_pct: attribution.meta.residual_pct,
        num_repricings: attribution.meta.num_repricings,
        result_invalid: attribution.result_invalid,
    })
}

/// Project every populated detail breakdown of `attribution` into long rows.
///
/// The output concatenates, in order: rates, credit, inflation, correlations,
/// FX, vol, cross-factor, scalars, and model-params detail rows, followed by
/// the carry rows ([`pnl_attribution_carry_rows`]) and the credit-factor
/// hierarchy rows ([`pnl_attribution_credit_factor_rows`]). Detail maps keep
/// their stored (deterministic) iteration order.
///
/// # Arguments
///
/// * `attribution` - Attribution result whose detail maps are projected.
///
/// # Returns
///
/// One [`LongDetailRow`] per populated detail entry; empty when no detail
/// breakdown was populated.
pub fn pnl_attribution_long_rows(attribution: &PnlAttribution) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();

    if let Some(detail) = &attribution.rates_detail {
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow::from_money(
                "rates.by_curve",
                "rates",
                curve_id.as_str().to_string(),
                None,
                money,
            ));
        }
        for ((curve_id, tenor), money) in &detail.by_tenor {
            rows.push(LongDetailRow::from_money(
                "rates.by_tenor",
                "rates",
                curve_id.as_str().to_string(),
                Some(tenor.clone()),
                money,
            ));
        }
        rows.push(LongDetailRow::from_money(
            "rates.discount_total",
            "rates",
            String::new(),
            None,
            &detail.discount_total,
        ));
        rows.push(LongDetailRow::from_money(
            "rates.forward_total",
            "rates",
            String::new(),
            None,
            &detail.forward_total,
        ));
    }

    if let Some(detail) = &attribution.credit_detail {
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow::from_money(
                "credit.by_curve",
                "credit",
                curve_id.as_str().to_string(),
                None,
                money,
            ));
        }
        for ((curve_id, tenor), money) in &detail.by_tenor {
            rows.push(LongDetailRow::from_money(
                "credit.by_tenor",
                "credit",
                curve_id.as_str().to_string(),
                Some(tenor.clone()),
                money,
            ));
        }
    }

    if let Some(detail) = &attribution.inflation_detail {
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow::from_money(
                "inflation.by_curve",
                "inflation",
                curve_id.as_str().to_string(),
                None,
                money,
            ));
        }
        if let Some(by_tenor) = &detail.by_tenor {
            for ((curve_id, tenor), money) in by_tenor {
                rows.push(LongDetailRow::from_money(
                    "inflation.by_tenor",
                    "inflation",
                    curve_id.as_str().to_string(),
                    Some(tenor.clone()),
                    money,
                ));
            }
        }
    }

    if let Some(detail) = &attribution.correlations_detail {
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow::from_money(
                "correlations.by_curve",
                "correlations",
                curve_id.as_str().to_string(),
                None,
                money,
            ));
        }
    }

    if let Some(detail) = &attribution.fx_detail {
        for ((from, to), money) in &detail.by_pair {
            rows.push(LongDetailRow::from_money(
                "fx.by_pair",
                "fx",
                from.to_string(),
                Some(to.to_string()),
                money,
            ));
        }
    }

    if let Some(detail) = &attribution.vol_detail {
        for (vol_surface_id, money) in &detail.by_surface {
            rows.push(LongDetailRow::from_money(
                "vol.by_surface",
                "vol",
                vol_surface_id.as_str().to_string(),
                None,
                money,
            ));
        }
    }

    if let Some(detail) = &attribution.cross_factor_detail {
        for (pair_label, money) in &detail.by_pair {
            rows.push(LongDetailRow::from_money(
                "cross_factor.by_pair",
                "cross_factor",
                pair_label.clone(),
                None,
                money,
            ));
        }
    }

    if let Some(detail) = &attribution.scalars_detail {
        let mut push_scalar_map = |kind: &'static str, map: &IndexMap<CurveId, Money>| {
            for (id, money) in map {
                rows.push(LongDetailRow::from_money(
                    kind,
                    "scalars",
                    id.as_str().to_string(),
                    None,
                    money,
                ));
            }
        };
        push_scalar_map("scalars.dividends", &detail.dividends);
        push_scalar_map("scalars.inflation", &detail.inflation);
        push_scalar_map("scalars.equity_prices", &detail.equity_prices);
        push_scalar_map("scalars.commodity_prices", &detail.commodity_prices);
    }

    if let Some(detail) = &attribution.model_params_detail {
        let mut push_opt = |key: &'static str, money: &Option<Money>| {
            if let Some(m) = money {
                rows.push(LongDetailRow::from_money(
                    "model_params.named",
                    "model_params",
                    key.to_string(),
                    None,
                    m,
                ));
            }
        };
        push_opt("prepayment", &detail.prepayment);
        push_opt("default_rate", &detail.default_rate);
        push_opt("recovery_rate", &detail.recovery_rate);
        push_opt("conversion_ratio", &detail.conversion_ratio);
        for (k, money) in &detail.other {
            rows.push(LongDetailRow::from_money(
                "model_params.other",
                "model_params",
                k.clone(),
                None,
                money,
            ));
        }
    }

    // Carry detail folded into the long view alongside the typed accessor.
    rows.extend(pnl_attribution_carry_rows(attribution));

    // Credit-factor hierarchy folded into the long view alongside the typed
    // accessor. Per-bucket rows go through the same dotted-key convention as
    // the typed accessor for symmetry.
    rows.extend(pnl_attribution_credit_factor_rows(attribution));

    rows
}

/// Project the carry decomposition of `attribution` into long rows.
///
/// Emits `carry.total` plus one row per populated sub-line (`coupon_income`,
/// `pull_to_par`, `roll_down`, `funding_cost`), with `.rates` / `.credit`
/// split rows when a `CreditFactorModel` drove a typed split.
///
/// # Arguments
///
/// * `attribution` - Attribution result whose `carry_detail` is projected.
///
/// # Returns
///
/// Carry rows in partition order; empty when `carry_detail` is not populated.
pub fn pnl_attribution_carry_rows(attribution: &PnlAttribution) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();
    let Some(detail) = &attribution.carry_detail else {
        return rows;
    };

    let mut push = |kind: &'static str, key_a: &str, money: &Money| {
        rows.push(LongDetailRow::from_money(
            kind,
            "carry",
            key_a.to_string(),
            None,
            money,
        ));
    };

    push("carry.total", "total", &detail.total);
    if let Some(ci) = &detail.coupon_income {
        push("carry.coupon_income", "total", &ci.total);
        if let Some(r) = &ci.rates_part {
            push("carry.coupon_income.rates", "rates_part", r);
        }
        if let Some(c) = &ci.credit_part {
            push("carry.coupon_income.credit", "credit_part", c);
        }
    }
    if let Some(ptp) = &detail.pull_to_par {
        push("carry.pull_to_par", "pull_to_par", ptp);
    }
    if let Some(rd) = &detail.roll_down {
        push("carry.roll_down", "total", &rd.total);
        if let Some(r) = &rd.rates_part {
            push("carry.roll_down.rates", "rates_part", r);
        }
        if let Some(c) = &rd.credit_part {
            push("carry.roll_down.credit", "credit_part", c);
        }
    }
    if let Some(fc) = &detail.funding_cost {
        push("carry.funding_cost", "funding_cost", fc);
    }

    rows
}

/// Project the credit-factor hierarchy decomposition of `attribution` into
/// long rows.
///
/// Emits `credit_factor.generic`, one `credit_factor.level` row per hierarchy
/// level (plus `credit_factor.level.by_bucket` rows when the per-bucket
/// breakdown is enabled), `credit_factor.adder`, `credit_factor.curve_shape`,
/// and `credit_factor.adder_by_issuer` rows when the per-issuer breakdown is
/// enabled.
///
/// # Arguments
///
/// * `attribution` - Attribution result whose `credit_factor_detail` is
///   projected.
///
/// # Returns
///
/// Credit-factor rows; empty when `credit_factor_detail` is not populated
/// (no `credit_factor_model` was supplied, or the instrument has no
/// resolvable issuer).
pub fn pnl_attribution_credit_factor_rows(attribution: &PnlAttribution) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();
    let Some(detail) = &attribution.credit_factor_detail else {
        return rows;
    };

    rows.push(LongDetailRow::from_money(
        "credit_factor.generic",
        "credit_factor",
        "generic".to_string(),
        None,
        &detail.generic_pnl,
    ));
    for level in &detail.levels {
        rows.push(LongDetailRow::from_money(
            "credit_factor.level",
            "credit_factor",
            level.level_name.clone(),
            None,
            &level.total,
        ));
        for (bucket, money) in &level.by_bucket {
            rows.push(LongDetailRow::from_money(
                "credit_factor.level.by_bucket",
                "credit_factor",
                level.level_name.clone(),
                Some(bucket.clone()),
                money,
            ));
        }
    }
    rows.push(LongDetailRow::from_money(
        "credit_factor.adder",
        "credit_factor",
        "adder".to_string(),
        None,
        &detail.adder_pnl_total,
    ));
    rows.push(LongDetailRow::from_money(
        "credit_factor.curve_shape",
        "credit_factor",
        "curve_shape".to_string(),
        None,
        &detail.curve_shape_pnl,
    ));
    if let Some(by_issuer) = &detail.adder_pnl_by_issuer {
        for (issuer_id, money) in by_issuer {
            rows.push(LongDetailRow::from_money(
                "credit_factor.adder_by_issuer",
                "credit_factor",
                "adder".to_string(),
                Some(issuer_id.as_str().to_string()),
                money,
            ));
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::detail::{
        CarryDetail, CreditFactorAttribution, LevelPnl, RatesCurvesAttribution, SourceLine,
    };
    use crate::AttributionMethod;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::create_date;
    use time::Month;

    fn usd(amount: f64) -> Money {
        Money::new(amount, Currency::USD).expect("valid money fixture")
    }

    fn base_attribution() -> PnlAttribution {
        PnlAttribution::new(
            usd(500.0),
            "TEST-BOND",
            create_date(2025, Month::January, 1).expect("valid test date"),
            create_date(2025, Month::January, 2).expect("valid test date"),
            AttributionMethod::Parallel,
        )
    }

    #[test]
    fn empty_attribution_projects_no_rows() {
        let attribution = base_attribution();
        assert!(pnl_attribution_long_rows(&attribution).is_empty());
        assert!(pnl_attribution_carry_rows(&attribution).is_empty());
        assert!(pnl_attribution_credit_factor_rows(&attribution).is_empty());
    }

    #[test]
    fn representative_details_project_expected_rows() {
        let mut attribution = base_attribution();

        let mut by_curve = IndexMap::new();
        by_curve.insert(CurveId::new("USD-OIS"), usd(120.0));
        let mut by_tenor = IndexMap::new();
        by_tenor.insert((CurveId::new("USD-OIS"), "5Y".to_string()), usd(80.0));
        attribution.rates_detail = Some(RatesCurvesAttribution {
            by_curve,
            by_tenor,
            discount_total: usd(100.0),
            forward_total: usd(20.0),
        });

        attribution.carry_detail = Some(CarryDetail {
            total: usd(50.0),
            coupon_income: Some(SourceLine::split(usd(40.0), usd(30.0), usd(10.0))),
            pull_to_par: Some(usd(5.0)),
            roll_down: Some(SourceLine::scalar(usd(5.0))),
            funding_cost: None,
        });

        attribution.credit_factor_detail = Some(CreditFactorAttribution {
            model_id: "2025-01-01/0000000000000000".to_string(),
            generic_pnl: usd(60.0),
            levels: vec![LevelPnl {
                level_name: "rating".to_string(),
                total: usd(25.0),
                by_bucket: [("IG".to_string(), usd(25.0))].into_iter().collect(),
            }],
            adder_pnl_total: usd(10.0),
            curve_shape_pnl: usd(5.0),
            adder_pnl_by_issuer: None,
            adder_magnitude: None,
        });

        let rows = pnl_attribution_long_rows(&attribution);
        let expected = vec![
            LongDetailRow {
                kind: "rates.by_curve",
                factor: "rates",
                sub: "by_curve",
                key_a: "USD-OIS".to_string(),
                key_b: None,
                amount: 120.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "rates.by_tenor",
                factor: "rates",
                sub: "by_tenor",
                key_a: "USD-OIS".to_string(),
                key_b: Some("5Y".to_string()),
                amount: 80.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "rates.discount_total",
                factor: "rates",
                sub: "discount_total",
                key_a: String::new(),
                key_b: None,
                amount: 100.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "rates.forward_total",
                factor: "rates",
                sub: "forward_total",
                key_a: String::new(),
                key_b: None,
                amount: 20.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "carry.total",
                factor: "carry",
                sub: "total",
                key_a: "total".to_string(),
                key_b: None,
                amount: 50.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "carry.coupon_income",
                factor: "carry",
                sub: "coupon_income",
                key_a: "total".to_string(),
                key_b: None,
                amount: 40.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "carry.coupon_income.rates",
                factor: "carry",
                sub: "coupon_income.rates",
                key_a: "rates_part".to_string(),
                key_b: None,
                amount: 30.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "carry.coupon_income.credit",
                factor: "carry",
                sub: "coupon_income.credit",
                key_a: "credit_part".to_string(),
                key_b: None,
                amount: 10.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "carry.pull_to_par",
                factor: "carry",
                sub: "pull_to_par",
                key_a: "pull_to_par".to_string(),
                key_b: None,
                amount: 5.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "carry.roll_down",
                factor: "carry",
                sub: "roll_down",
                key_a: "total".to_string(),
                key_b: None,
                amount: 5.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "credit_factor.generic",
                factor: "credit_factor",
                sub: "generic",
                key_a: "generic".to_string(),
                key_b: None,
                amount: 60.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "credit_factor.level",
                factor: "credit_factor",
                sub: "level",
                key_a: "rating".to_string(),
                key_b: None,
                amount: 25.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "credit_factor.level.by_bucket",
                factor: "credit_factor",
                sub: "level.by_bucket",
                key_a: "rating".to_string(),
                key_b: Some("IG".to_string()),
                amount: 25.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "credit_factor.adder",
                factor: "credit_factor",
                sub: "adder",
                key_a: "adder".to_string(),
                key_b: None,
                amount: 10.0,
                currency: "USD".to_string(),
            },
            LongDetailRow {
                kind: "credit_factor.curve_shape",
                factor: "credit_factor",
                sub: "curve_shape",
                key_a: "curve_shape".to_string(),
                key_b: None,
                amount: 5.0,
                currency: "USD".to_string(),
            },
        ];
        assert_eq!(rows, expected);
    }

    #[test]
    fn rows_serialize_with_stable_field_names() {
        let row = LongDetailRow::from_money(
            "rates.by_curve",
            "rates",
            "USD-OIS".to_string(),
            None,
            &usd(1.5),
        );
        let value = serde_json::to_value(&row).expect("row must serialize");
        assert_eq!(
            value,
            serde_json::json!({
                "kind": "rates.by_curve",
                "factor": "rates",
                "sub": "by_curve",
                "key_a": "USD-OIS",
                "key_b": null,
                "amount": 1.5,
                "currency": "USD",
            })
        );
    }
}
