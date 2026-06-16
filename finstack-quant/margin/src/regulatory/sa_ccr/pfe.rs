//! PFE (Potential Future Exposure) computation for SA-CCR.
//!
//! PFE = multiplier * AddOn_aggregate

use super::add_on::asset_class_add_on;
use super::types::{SaCcrAssetClass, SaCcrNettingSetConfig, SaCcrTrade};
use finstack_quant_core::HashMap;

/// PFE multiplier floor per BCBS 279 paragraph 149.
const MULTIPLIER_FLOOR: f64 = 0.05;

/// Compute PFE components: (multiplier, add_on_aggregate, add_on_by_class).
///
/// `multiplier = min(1, floor + (1 - floor) * exp(V_minus_C / (2 * (1-floor) * AddOn)))`
/// `PFE = multiplier * AddOn_aggregate`
///
/// The per-asset-class add-ons compute their own per-trade maturity
/// factors from `config` (see [`super::add_on::asset_class_add_on`]), so
/// this function no longer collapses MF to a netting-set-level scalar.
///
/// # Arguments
///
/// * `config` - Netting-set collateral and margin-agreement terms.
/// * `trades` - Derivative trades in the netting set.
///
/// # Returns
///
/// Tuple of `(multiplier, add_on_aggregate, add_on_by_asset_class)`.
///
/// # References
///
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
pub fn pfe(
    config: &SaCcrNettingSetConfig,
    trades: &[SaCcrTrade],
) -> (f64, f64, HashMap<SaCcrAssetClass, f64>) {
    // Compute add-on per asset class.
    let mut add_on_by_class = HashMap::default();
    let mut add_on_aggregate = 0.0;
    for &ac in SaCcrAssetClass::ALL {
        let ao = asset_class_add_on(ac, trades, config);
        if ao > 0.0 {
            add_on_by_class.insert(ac, ao);
            add_on_aggregate += ao;
        }
    }

    // Compute multiplier. Basel 279 para 149: the multiplier input is
    // V - C where C is *total* net collateral, i.e. VM + NICA.
    let v: f64 = trades.iter().map(|t| t.mtm).sum();
    let v_minus_c = v - config.collateral - config.nica;
    let mult = multiplier(v_minus_c, add_on_aggregate);

    (mult, add_on_aggregate, add_on_by_class)
}

/// PFE multiplier per BCBS 279 paragraph 149.
///
/// Recognizes excess collateral by scaling AddOn below 1.0
/// when the netting set is over-collateralized (V - C < 0).
/// Floor of 5% prevents the multiplier from reaching zero.
///
/// # Arguments
///
/// * `v_minus_c` - Net current exposure after collateral.
/// * `add_on` - Aggregate add-on before multiplier.
///
/// # Returns
///
/// PFE multiplier in `[0.05, 1.0]`.
///
/// # References
///
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
pub fn multiplier(v_minus_c: f64, add_on: f64) -> f64 {
    if add_on <= 0.0 {
        return MULTIPLIER_FLOOR;
    }
    let ratio = v_minus_c / (2.0 * (1.0 - MULTIPLIER_FLOOR) * add_on);
    f64::min(
        1.0,
        MULTIPLIER_FLOOR + (1.0 - MULTIPLIER_FLOOR) * ratio.exp(),
    )
}
