//! Deterministic regulatory-margin wire-format tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_margin::regulatory::frtb::{
    DrcAssetType, DrcPosition, DrcSector, DrcSeniority, FrtbSensitivities, RraoPosition,
};
use std::hash::Hash;

fn insert_entries<K, V>(
    map: &mut finstack_quant_core::HashMap<K, V>,
    mut entries: [(K, V); 2],
    reverse: bool,
) where
    K: Eq + Hash,
{
    if reverse {
        entries.reverse();
    }
    map.extend(entries);
}

fn sample_sensitivities(reverse: bool) -> FrtbSensitivities {
    let mut sensitivities = FrtbSensitivities::new(Currency::USD);
    insert_entries(
        &mut sensitivities.girr_delta,
        [
            ((Currency::USD, "10Y".to_string()), 125.0),
            ((Currency::EUR, "2Y".to_string()), -40.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.girr_inflation_delta,
        [(Currency::USD, 10.0), (Currency::EUR, 11.0)],
        reverse,
    );
    insert_entries(
        &mut sensitivities.girr_xccy_basis_delta,
        [(Currency::USD, 12.0), (Currency::EUR, 13.0)],
        reverse,
    );
    insert_entries(
        &mut sensitivities.girr_vega,
        [
            ((Currency::USD, "1Y".to_string(), "10Y".to_string()), 14.0),
            ((Currency::EUR, "6M".to_string(), "2Y".to_string()), 15.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.girr_curvature,
        [
            (Currency::USD, (16.0, -16.0)),
            (Currency::EUR, (17.0, -17.0)),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_nonsec_delta,
        [
            (
                ("ZETA".to_string(), 3, "5Y".to_string(), "basis".to_string()),
                18.0,
            ),
            (
                (
                    "ALPHA".to_string(),
                    1,
                    "3Y".to_string(),
                    "basis".to_string(),
                ),
                19.0,
            ),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_nonsec_vega,
        [
            (("ZETA".to_string(), 3, "1Y".to_string()), 20.0),
            (("ALPHA".to_string(), 1, "6M".to_string()), 21.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_nonsec_curvature,
        [
            (("ZETA".to_string(), 3), (22.0, -22.0)),
            (("ALPHA".to_string(), 1), (23.0, -23.0)),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_sec_ctp_delta,
        [
            (
                (
                    "CTP_Z".to_string(),
                    4,
                    "5Y".to_string(),
                    "basis".to_string(),
                ),
                24.0,
            ),
            (
                (
                    "CTP_A".to_string(),
                    2,
                    "3Y".to_string(),
                    "basis".to_string(),
                ),
                25.0,
            ),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_sec_ctp_vega,
        [
            (("CTP_Z".to_string(), 4, "1Y".to_string()), 26.0),
            (("CTP_A".to_string(), 2, "6M".to_string()), 27.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_sec_ctp_curvature,
        [
            (("CTP_Z".to_string(), 4), (28.0, -28.0)),
            (("CTP_A".to_string(), 2), (29.0, -29.0)),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_sec_nonctp_delta,
        [
            (
                (
                    "NCTP_Z".to_string(),
                    5,
                    "5Y".to_string(),
                    "basis".to_string(),
                ),
                30.0,
            ),
            (
                (
                    "NCTP_A".to_string(),
                    2,
                    "3Y".to_string(),
                    "basis".to_string(),
                ),
                31.0,
            ),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_sec_nonctp_vega,
        [
            (("NCTP_Z".to_string(), 5, "1Y".to_string()), 32.0),
            (("NCTP_A".to_string(), 2, "6M".to_string()), 33.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.csr_sec_nonctp_curvature,
        [
            (("NCTP_Z".to_string(), 5), (34.0, -34.0)),
            (("NCTP_A".to_string(), 2), (35.0, -35.0)),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.equity_delta,
        [
            (("MSFT".to_string(), 11), 36.0),
            (("AAPL".to_string(), 10), 37.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.equity_vega,
        [
            (("MSFT".to_string(), 11, "1Y".to_string()), 38.0),
            (("AAPL".to_string(), 10, "6M".to_string()), 39.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.equity_curvature,
        [
            (("MSFT".to_string(), 11), (40.0, -40.0)),
            (("AAPL".to_string(), 10), (41.0, -41.0)),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.commodity_delta,
        [
            (
                (
                    "Power".to_string(),
                    2,
                    "1Y".to_string(),
                    "basis".to_string(),
                ),
                42.0,
            ),
            (
                (
                    "Crude".to_string(),
                    1,
                    "6M".to_string(),
                    "basis".to_string(),
                ),
                43.0,
            ),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.commodity_vega,
        [
            (("Power".to_string(), 2, "1Y".to_string()), 44.0),
            (("Crude".to_string(), 1, "6M".to_string()), 45.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.commodity_curvature,
        [
            (("Power".to_string(), 2), (46.0, -46.0)),
            (("Crude".to_string(), 1), (47.0, -47.0)),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.fx_delta,
        [
            ((Currency::USD, Currency::JPY), 48.0),
            ((Currency::EUR, Currency::USD), 49.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.fx_vega,
        [
            ((Currency::USD, Currency::JPY, "1Y".to_string()), 50.0),
            ((Currency::EUR, Currency::USD, "6M".to_string()), 51.0),
        ],
        reverse,
    );
    insert_entries(
        &mut sensitivities.fx_curvature,
        [
            ((Currency::USD, Currency::JPY), (52.0, -52.0)),
            ((Currency::EUR, Currency::USD), (53.0, -53.0)),
        ],
        reverse,
    );
    sensitivities.drc_positions.push(DrcPosition {
        maturity_years: 1.0,
        issuer: "ACME".to_string(),
        jtd_amount: 1_000_000.0,
        rating_bucket: 4,
        sector: DrcSector::Corporate,
        seniority: DrcSeniority::SeniorUnsecured,
        asset_type: DrcAssetType::Corporate,
        pnl_adjustment: -10_000.0,
    });
    sensitivities.rrao_exotic_notionals.push(RraoPosition {
        instrument_id: "EXOTIC_1".to_string(),
        notional: 2_000_000.0,
        is_exotic: true,
    });
    sensitivities
}

#[test]
fn non_empty_frtb_sensitivities_roundtrip_deterministically() {
    let first = sample_sensitivities(false);
    let second = sample_sensitivities(true);

    let first_json = serde_json::to_vec(&first).expect("non-empty FRTB sensitivities serialize");
    let second_json = serde_json::to_vec(&second).expect("reverse insertion order serializes");

    assert_eq!(first_json, second_json);
    let restored: FrtbSensitivities =
        serde_json::from_slice(&first_json).expect("FRTB sensitivities deserialize");
    assert_eq!(restored, first);
}

#[test]
fn frtb_sensitivity_maps_reject_object_shapes() {
    let mut invalid = serde_json::to_value(FrtbSensitivities::new(Currency::USD))
        .expect("empty FRTB sensitivities serialize");
    invalid["girr_delta"] = serde_json::json!({});

    let error = serde_json::from_value::<FrtbSensitivities>(invalid)
        .expect_err("FRTB sensitivity collections must be arrays");
    assert!(error.to_string().contains("sequence"));
}
