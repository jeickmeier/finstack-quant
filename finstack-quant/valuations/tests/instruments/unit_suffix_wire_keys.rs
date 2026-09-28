//! `_pct` means percent points only: decimal-valued fields carry `_decimal`,
//! a ratio noun (`share`, `fraction_of_*`) or `_bp`. Each retired `_pct` wire
//! key is rejected by `deny_unknown_fields`.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::pe_fund::{ClawbackSettle, ClawbackSpec};
use finstack_quant_valuations::instruments::equity::{EquityTotalReturnFuture, RealEstateAsset};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    CardPortfolioSpec, ExcessSpreadSpec, IncentiveFeeSpec, ReinvestmentCriteria,
    ShiftingInterestStep, StructuredCredit, TargetOcSpec,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Rename the first object key `from` to `to` anywhere in `value`.
fn rename_key(value: &mut Value, from: &str, to: &str) -> bool {
    match value {
        Value::Object(map) => {
            if let Some(inner) = map.remove(from) {
                map.insert(to.to_string(), inner);
                return true;
            }
            map.values_mut().any(|child| rename_key(child, from, to))
        }
        Value::Array(items) => items.iter_mut().any(|child| rename_key(child, from, to)),
        _ => false,
    }
}

/// Serialize `instrument`, respell `canonical` as `retired`, and require the
/// deserializer to reject it.
fn assert_rejects<T: Serialize + DeserializeOwned>(instrument: &T, canonical: &str, retired: &str) {
    let mut value = serde_json::to_value(instrument).expect("serialize");
    assert!(
        rename_key(&mut value, canonical, retired),
        "{canonical} missing from the serialized value"
    );
    let err = serde_json::from_value::<T>(value)
        .err()
        .unwrap_or_else(|| panic!("retired key {retired} must be rejected"));
    assert!(err.to_string().contains(retired), "{retired}: {err}");
}

#[test]
// schema-rejection-test: every retired decimal-valued `_pct` key
fn retired_decimal_pct_keys_are_rejected() {
    let mut deal = StructuredCredit::example().expect("example");
    deal.cleanup_call_decimal = Some(0.10);
    assert_rejects(&deal, "cleanup_call_decimal", "cleanup_call_pct");

    let fee = IncentiveFeeSpec {
        hurdle_irr: 0.12,
        share: 0.2,
    };
    assert_rejects(&fee, "share", "share_pct");

    let mut card = CardPortfolioSpec::new(0.15, 0.18, 0.05);
    card.fixed_allocation_decimal = Some(0.9);
    assert_rejects(&card, "fixed_allocation_decimal", "fixed_allocation_pct");

    let step = ShiftingInterestStep {
        months_from_closing: 60,
        senior_decimal: 0.7,
    };
    assert_rejects(&step, "senior_decimal", "senior_pct");

    let target = TargetOcSpec {
        fraction_of_current: 0.12,
        floor_fraction_of_original: 0.015,
    };
    assert_rejects(&target, "fraction_of_current", "pct_of_current");
    assert_rejects(
        &target,
        "floor_fraction_of_original",
        "floor_pct_of_original",
    );

    let spread = ExcessSpreadSpec {
        target_balance: Money::new(1_000_000.0, Currency::USD).expect("money"),
        trap_loss_decimal: Some(0.05),
    };
    assert_rejects(&spread, "trap_loss_decimal", "trap_loss_pct");

    let mut asset = RealEstateAsset::example().expect("example");
    asset.disposition_cost_decimal = Some(0.02);
    assert_rejects(&asset, "disposition_cost_decimal", "disposition_cost_pct");

    let clawback = ClawbackSpec {
        holdback_decimal: Some(0.2),
        settle_on: ClawbackSettle::FundEnd,
    };
    assert_rejects(&clawback, "holdback_decimal", "holdback_pct");

    // Percent-of-par price gains its `_pct` suffix.
    assert_rejects(
        &ReinvestmentCriteria::default(),
        "max_price_pct",
        "max_price",
    );

    let trf = EquityTotalReturnFuture::example().expect("example");
    assert_rejects(&trf, "spread_bp_id", "spread_basis_points_id");
}
