//! Long-format DataFrame row builders for PnlAttribution export.

/// Long-format row for the unified detail DataFrame (see
/// [`super::pnl_attribution::PyPnlAttribution::to_long_dataframe`]). Currency is
/// owned because `Currency::Display` allocates; the row is dropped immediately
/// after JSON serialization so the per-row String is cheap.
///
/// Each row's `currency` is taken from its OWN `Money` value, never from the
/// parent factor aggregate: detail maps are not currency-validated by
/// `validate_currencies`, so stamping the parent's currency could silently
/// mislabel a mixed-currency payload (quant review MO-B3).
#[derive(serde::Serialize)]
pub(super) struct LongDetailRow {
    kind: &'static str,
    factor: &'static str,
    key_a: String,
    key_b: Option<String>,
    amount: f64,
    currency: String,
}

pub(super) fn build_long_detail_rows(
    attribution: &finstack_attribution::PnlAttribution,
) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();

    if let Some(detail) = &attribution.rates_detail {
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow {
                kind: "rates.by_curve",
                factor: "rates",
                key_a: curve_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
        for ((curve_id, tenor), money) in &detail.by_tenor {
            rows.push(LongDetailRow {
                kind: "rates.by_tenor",
                factor: "rates",
                key_a: curve_id.as_str().to_string(),
                key_b: Some(tenor.clone()),
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
        rows.push(LongDetailRow {
            kind: "rates.discount_total",
            factor: "rates",
            key_a: String::new(),
            key_b: None,
            amount: detail.discount_total.amount(),
            currency: detail.discount_total.currency().to_string(),
        });
        rows.push(LongDetailRow {
            kind: "rates.forward_total",
            factor: "rates",
            key_a: String::new(),
            key_b: None,
            amount: detail.forward_total.amount(),
            currency: detail.forward_total.currency().to_string(),
        });
    }

    if let Some(detail) = &attribution.credit_detail {
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow {
                kind: "credit.by_curve",
                factor: "credit",
                key_a: curve_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
        for ((curve_id, tenor), money) in &detail.by_tenor {
            rows.push(LongDetailRow {
                kind: "credit.by_tenor",
                factor: "credit",
                key_a: curve_id.as_str().to_string(),
                key_b: Some(tenor.clone()),
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
    }

    if let Some(detail) = &attribution.inflation_detail {
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow {
                kind: "inflation.by_curve",
                factor: "inflation",
                key_a: curve_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
        if let Some(by_tenor) = &detail.by_tenor {
            for ((curve_id, tenor), money) in by_tenor {
                rows.push(LongDetailRow {
                    kind: "inflation.by_tenor",
                    factor: "inflation",
                    key_a: curve_id.as_str().to_string(),
                    key_b: Some(tenor.clone()),
                    amount: money.amount(),
                    currency: money.currency().to_string(),
                });
            }
        }
    }

    if let Some(detail) = &attribution.correlations_detail {
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow {
                kind: "correlations.by_curve",
                factor: "correlations",
                key_a: curve_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
    }

    if let Some(detail) = &attribution.fx_detail {
        for ((from, to), money) in &detail.by_pair {
            rows.push(LongDetailRow {
                kind: "fx.by_pair",
                factor: "fx",
                key_a: from.to_string(),
                key_b: Some(to.to_string()),
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
    }

    if let Some(detail) = &attribution.vol_detail {
        for (surface_id, money) in &detail.by_surface {
            rows.push(LongDetailRow {
                kind: "vol.by_surface",
                factor: "vol",
                key_a: surface_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
    }

    if let Some(detail) = &attribution.cross_factor_detail {
        for (pair_label, money) in &detail.by_pair {
            rows.push(LongDetailRow {
                kind: "cross_factor.by_pair",
                factor: "cross_factor",
                key_a: pair_label.clone(),
                key_b: None,
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
    }

    if let Some(detail) = &attribution.scalars_detail {
        let mut push_scalar_map = |kind: &'static str,
                                   map: &indexmap::IndexMap<
            finstack_core::types::CurveId,
            finstack_core::money::Money,
        >| {
            for (id, money) in map {
                rows.push(LongDetailRow {
                    kind,
                    factor: "scalars",
                    key_a: id.as_str().to_string(),
                    key_b: None,
                    amount: money.amount(),
                    currency: money.currency().to_string(),
                });
            }
        };
        push_scalar_map("scalars.dividends", &detail.dividends);
        push_scalar_map("scalars.inflation", &detail.inflation);
        push_scalar_map("scalars.equity_prices", &detail.equity_prices);
        push_scalar_map("scalars.commodity_prices", &detail.commodity_prices);
    }

    if let Some(detail) = &attribution.model_params_detail {
        let mut push_opt = |key: &'static str, money: &Option<finstack_core::money::Money>| {
            if let Some(m) = money {
                rows.push(LongDetailRow {
                    kind: "model_params.named",
                    factor: "model_params",
                    key_a: key.to_string(),
                    key_b: None,
                    amount: m.amount(),
                    currency: m.currency().to_string(),
                });
            }
        };
        push_opt("prepayment", &detail.prepayment);
        push_opt("default_rate", &detail.default_rate);
        push_opt("recovery_rate", &detail.recovery_rate);
        push_opt("conversion_ratio", &detail.conversion_ratio);
        for (k, money) in &detail.other {
            rows.push(LongDetailRow {
                kind: "model_params.other",
                factor: "model_params",
                key_a: k.clone(),
                key_b: None,
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
    }

    // Carry detail folded into the long view alongside the typed accessor.
    rows.extend(build_carry_detail_rows(attribution));

    // Credit-factor hierarchy folded into the long view alongside the typed
    // accessor. Per-bucket rows go through the same dotted-key convention as
    // the typed accessor for symmetry.
    rows.extend(build_credit_factor_rows(attribution));

    rows
}

pub(super) fn build_carry_detail_rows(
    attribution: &finstack_attribution::PnlAttribution,
) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();
    let Some(detail) = &attribution.carry_detail else {
        return rows;
    };

    let mut push = |kind: &'static str, key_a: &str, money: &finstack_core::money::Money| {
        rows.push(LongDetailRow {
            kind,
            factor: "carry",
            key_a: key_a.to_string(),
            key_b: None,
            amount: money.amount(),
            currency: money.currency().to_string(),
        });
    };

    push("carry.total", "total", &detail.total);
    if let Some(theta) = &detail.theta {
        push("carry.theta", "theta", theta);
    }
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

pub(super) fn build_credit_factor_rows(
    attribution: &finstack_attribution::PnlAttribution,
) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();
    let Some(detail) = &attribution.credit_factor_detail else {
        return rows;
    };

    rows.push(LongDetailRow {
        kind: "credit_factor.generic",
        factor: "credit_factor",
        key_a: "generic".to_string(),
        key_b: None,
        amount: detail.generic_pnl.amount(),
        currency: detail.generic_pnl.currency().to_string(),
    });
    for level in &detail.levels {
        rows.push(LongDetailRow {
            kind: "credit_factor.level",
            factor: "credit_factor",
            key_a: level.level_name.clone(),
            key_b: None,
            amount: level.total.amount(),
            currency: level.total.currency().to_string(),
        });
        for (bucket, money) in &level.by_bucket {
            rows.push(LongDetailRow {
                kind: "credit_factor.level.by_bucket",
                factor: "credit_factor",
                key_a: level.level_name.clone(),
                key_b: Some(bucket.clone()),
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
    }
    rows.push(LongDetailRow {
        kind: "credit_factor.adder",
        factor: "credit_factor",
        key_a: "adder".to_string(),
        key_b: None,
        amount: detail.adder_pnl_total.amount(),
        currency: detail.adder_pnl_total.currency().to_string(),
    });
    rows.push(LongDetailRow {
        kind: "credit_factor.curve_shape",
        factor: "credit_factor",
        key_a: "curve_shape".to_string(),
        key_b: None,
        amount: detail.curve_shape_pnl.amount(),
        currency: detail.curve_shape_pnl.currency().to_string(),
    });
    if let Some(by_issuer) = &detail.adder_pnl_by_issuer {
        for (issuer_id, money) in by_issuer {
            rows.push(LongDetailRow {
                kind: "credit_factor.adder_by_issuer",
                factor: "credit_factor",
                key_a: "adder".to_string(),
                key_b: Some(issuer_id.as_str().to_string()),
                amount: money.amount(),
                currency: money.currency().to_string(),
            });
        }
    }

    rows
}
