//! Deterministic JSON wire representation for FRTB sensitivities.

use finstack_quant_core::currency::Currency;

use super::types::{DrcPosition, FrtbSensitivities, RraoPosition};

type CurrencyTenor = (Currency, String, f64);
type CurrencyValue = (Currency, f64);
type CurrencyVega = (Currency, String, String, f64);
type CurrencyCurvature = (Currency, f64, f64);
type LabelBucketBasis = (String, u8, String, String, f64);
type LabelBucketTenor = (String, u8, String, f64);
type LabelBucketCurvature = (String, u8, f64, f64);
type CurrencyPairValue = (Currency, Currency, f64);
type CurrencyPairVega = (Currency, Currency, String, f64);
type CurrencyPairCurvature = (Currency, Currency, f64, f64);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FrtbSensitivitiesWire {
    base_currency: Currency,
    girr_delta: Vec<CurrencyTenor>,
    girr_inflation_delta: Vec<CurrencyValue>,
    girr_xccy_basis_delta: Vec<CurrencyValue>,
    girr_vega: Vec<CurrencyVega>,
    girr_curvature: Vec<CurrencyCurvature>,
    csr_nonsec_delta: Vec<LabelBucketBasis>,
    csr_nonsec_vega: Vec<LabelBucketTenor>,
    csr_nonsec_curvature: Vec<LabelBucketCurvature>,
    csr_sec_ctp_delta: Vec<LabelBucketBasis>,
    csr_sec_ctp_vega: Vec<LabelBucketTenor>,
    csr_sec_ctp_curvature: Vec<LabelBucketCurvature>,
    csr_sec_nonctp_delta: Vec<LabelBucketBasis>,
    csr_sec_nonctp_vega: Vec<LabelBucketTenor>,
    csr_sec_nonctp_curvature: Vec<LabelBucketCurvature>,
    equity_delta: Vec<(String, u8, f64)>,
    equity_repo_delta: Vec<(String, u8, f64)>,
    equity_vega: Vec<LabelBucketTenor>,
    equity_curvature: Vec<LabelBucketCurvature>,
    commodity_delta: Vec<LabelBucketBasis>,
    commodity_vega: Vec<LabelBucketTenor>,
    commodity_curvature: Vec<LabelBucketCurvature>,
    fx_delta: Vec<CurrencyPairValue>,
    fx_vega: Vec<CurrencyPairVega>,
    fx_curvature: Vec<CurrencyPairCurvature>,
    drc_positions: Vec<DrcPosition>,
    rrao_exotic_notionals: Vec<RraoPosition>,
}

impl From<&FrtbSensitivities> for FrtbSensitivitiesWire {
    fn from(value: &FrtbSensitivities) -> Self {
        let mut girr_delta: Vec<_> = value
            .girr_delta
            .iter()
            .map(|((currency, tenor), amount)| (*currency, tenor.clone(), *amount))
            .collect();
        girr_delta.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

        let mut girr_inflation_delta: Vec<_> = value
            .girr_inflation_delta
            .iter()
            .map(|(currency, amount)| (*currency, *amount))
            .collect();
        girr_inflation_delta.sort_by_key(|entry| entry.0);

        let mut girr_xccy_basis_delta: Vec<_> = value
            .girr_xccy_basis_delta
            .iter()
            .map(|(currency, amount)| (*currency, *amount))
            .collect();
        girr_xccy_basis_delta.sort_by_key(|entry| entry.0);

        let mut girr_vega: Vec<_> = value
            .girr_vega
            .iter()
            .map(|((currency, maturity, tenor), amount)| {
                (*currency, maturity.clone(), tenor.clone(), *amount)
            })
            .collect();
        girr_vega.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });

        let mut girr_curvature: Vec<_> = value
            .girr_curvature
            .iter()
            .map(|(currency, (up, down))| (*currency, *up, *down))
            .collect();
        girr_curvature.sort_by_key(|entry| entry.0);

        let csr_nonsec_delta = label_bucket_basis(&value.csr_nonsec_delta);
        let mut csr_nonsec_vega = label_bucket_tenor(&value.csr_nonsec_vega);
        let mut csr_nonsec_curvature = label_bucket_curvature(&value.csr_nonsec_curvature);
        let csr_sec_ctp_delta = label_bucket_basis(&value.csr_sec_ctp_delta);
        let mut csr_sec_ctp_vega = label_bucket_tenor(&value.csr_sec_ctp_vega);
        let mut csr_sec_ctp_curvature = label_bucket_curvature(&value.csr_sec_ctp_curvature);
        let csr_sec_nonctp_delta = label_bucket_basis(&value.csr_sec_nonctp_delta);
        let mut csr_sec_nonctp_vega = label_bucket_tenor(&value.csr_sec_nonctp_vega);
        let mut csr_sec_nonctp_curvature = label_bucket_curvature(&value.csr_sec_nonctp_curvature);
        let mut equity_vega = label_bucket_tenor(&value.equity_vega);
        let mut equity_curvature = label_bucket_curvature(&value.equity_curvature);
        let commodity_delta = label_bucket_basis(&value.commodity_delta);
        let mut commodity_vega = label_bucket_tenor(&value.commodity_vega);
        let mut commodity_curvature = label_bucket_curvature(&value.commodity_curvature);
        for entries in [
            &mut csr_nonsec_vega,
            &mut csr_sec_ctp_vega,
            &mut csr_sec_nonctp_vega,
            &mut equity_vega,
            &mut commodity_vega,
        ] {
            sort_label_bucket_tenor(entries);
        }
        for entries in [
            &mut csr_nonsec_curvature,
            &mut csr_sec_ctp_curvature,
            &mut csr_sec_nonctp_curvature,
            &mut equity_curvature,
            &mut commodity_curvature,
        ] {
            sort_label_bucket_curvature(entries);
        }

        let mut equity_delta: Vec<_> = value
            .equity_delta
            .iter()
            .map(|((label, bucket), amount)| (label.clone(), *bucket, *amount))
            .collect();
        equity_delta.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

        let mut equity_repo_delta: Vec<_> = value
            .equity_repo_delta
            .iter()
            .map(|((label, bucket), amount)| (label.clone(), *bucket, *amount))
            .collect();
        equity_repo_delta
            .sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

        let mut fx_delta: Vec<_> = value
            .fx_delta
            .iter()
            .map(|((base, quote), amount)| (*base, *quote, *amount))
            .collect();
        fx_delta.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

        let mut fx_vega: Vec<_> = value
            .fx_vega
            .iter()
            .map(|((base, quote, maturity), amount)| (*base, *quote, maturity.clone(), *amount))
            .collect();
        fx_vega.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });

        let mut fx_curvature: Vec<_> = value
            .fx_curvature
            .iter()
            .map(|((base, quote), (up, down))| (*base, *quote, *up, *down))
            .collect();
        fx_curvature.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

        Self {
            base_currency: value.base_currency,
            girr_delta,
            girr_inflation_delta,
            girr_xccy_basis_delta,
            girr_vega,
            girr_curvature,
            csr_nonsec_delta,
            csr_nonsec_vega,
            csr_nonsec_curvature,
            csr_sec_ctp_delta,
            csr_sec_ctp_vega,
            csr_sec_ctp_curvature,
            csr_sec_nonctp_delta,
            csr_sec_nonctp_vega,
            csr_sec_nonctp_curvature,
            equity_delta,
            equity_repo_delta,
            equity_vega,
            equity_curvature,
            commodity_delta,
            commodity_vega,
            commodity_curvature,
            fx_delta,
            fx_vega,
            fx_curvature,
            drc_positions: value.drc_positions.clone(),
            rrao_exotic_notionals: value.rrao_exotic_notionals.clone(),
        }
    }
}

impl From<FrtbSensitivitiesWire> for FrtbSensitivities {
    fn from(wire: FrtbSensitivitiesWire) -> Self {
        let mut value = Self::new(wire.base_currency);
        for (currency, tenor, amount) in wire.girr_delta {
            value.girr_delta.insert((currency, tenor), amount);
        }
        value.girr_inflation_delta.extend(wire.girr_inflation_delta);
        value
            .girr_xccy_basis_delta
            .extend(wire.girr_xccy_basis_delta);
        for (currency, maturity, tenor, amount) in wire.girr_vega {
            value.girr_vega.insert((currency, maturity, tenor), amount);
        }
        for (currency, up, down) in wire.girr_curvature {
            value.girr_curvature.insert(currency, (up, down));
        }
        extend_label_bucket_basis(&mut value.csr_nonsec_delta, wire.csr_nonsec_delta);
        extend_label_bucket_tenor(&mut value.csr_nonsec_vega, wire.csr_nonsec_vega);
        extend_label_bucket_curvature(&mut value.csr_nonsec_curvature, wire.csr_nonsec_curvature);
        extend_label_bucket_basis(&mut value.csr_sec_ctp_delta, wire.csr_sec_ctp_delta);
        extend_label_bucket_tenor(&mut value.csr_sec_ctp_vega, wire.csr_sec_ctp_vega);
        extend_label_bucket_curvature(&mut value.csr_sec_ctp_curvature, wire.csr_sec_ctp_curvature);
        extend_label_bucket_basis(&mut value.csr_sec_nonctp_delta, wire.csr_sec_nonctp_delta);
        extend_label_bucket_tenor(&mut value.csr_sec_nonctp_vega, wire.csr_sec_nonctp_vega);
        extend_label_bucket_curvature(
            &mut value.csr_sec_nonctp_curvature,
            wire.csr_sec_nonctp_curvature,
        );
        for (label, bucket, amount) in wire.equity_delta {
            value.equity_delta.insert((label, bucket), amount);
        }
        for (label, bucket, amount) in wire.equity_repo_delta {
            value.equity_repo_delta.insert((label, bucket), amount);
        }
        extend_label_bucket_tenor(&mut value.equity_vega, wire.equity_vega);
        extend_label_bucket_curvature(&mut value.equity_curvature, wire.equity_curvature);
        extend_label_bucket_basis(&mut value.commodity_delta, wire.commodity_delta);
        extend_label_bucket_tenor(&mut value.commodity_vega, wire.commodity_vega);
        extend_label_bucket_curvature(&mut value.commodity_curvature, wire.commodity_curvature);
        for (base, quote, amount) in wire.fx_delta {
            value.fx_delta.insert((base, quote), amount);
        }
        for (base, quote, maturity, amount) in wire.fx_vega {
            value.fx_vega.insert((base, quote, maturity), amount);
        }
        for (base, quote, up, down) in wire.fx_curvature {
            value.fx_curvature.insert((base, quote), (up, down));
        }
        value.drc_positions = wire.drc_positions;
        value.rrao_exotic_notionals = wire.rrao_exotic_notionals;
        value
    }
}

pub(super) fn serialize<S>(value: &FrtbSensitivities, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serde::Serialize::serialize(&FrtbSensitivitiesWire::from(value), serializer)
}

pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<FrtbSensitivities, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let wire = <FrtbSensitivitiesWire as serde::Deserialize>::deserialize(deserializer)?;
    Ok(FrtbSensitivities::from(wire))
}

fn label_bucket_tenor(
    map: &finstack_quant_core::HashMap<(String, u8, String), f64>,
) -> Vec<LabelBucketTenor> {
    map.iter()
        .map(|((label, bucket, tenor), amount)| (label.clone(), *bucket, tenor.clone(), *amount))
        .collect()
}

fn sort_label_bucket_tenor(entries: &mut [LabelBucketTenor]) {
    entries.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
}

fn label_bucket_curvature(
    map: &finstack_quant_core::HashMap<(String, u8), (f64, f64)>,
) -> Vec<LabelBucketCurvature> {
    map.iter()
        .map(|((label, bucket), (up, down))| (label.clone(), *bucket, *up, *down))
        .collect()
}

fn sort_label_bucket_curvature(entries: &mut [LabelBucketCurvature]) {
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
}

fn extend_label_bucket_tenor(
    map: &mut finstack_quant_core::HashMap<(String, u8, String), f64>,
    entries: Vec<LabelBucketTenor>,
) {
    for (label, bucket, tenor, amount) in entries {
        map.insert((label, bucket, tenor), amount);
    }
}

fn extend_label_bucket_curvature(
    map: &mut finstack_quant_core::HashMap<(String, u8), (f64, f64)>,
    entries: Vec<LabelBucketCurvature>,
) {
    for (label, bucket, up, down) in entries {
        map.insert((label, bucket), (up, down));
    }
}

fn label_bucket_basis(
    map: &finstack_quant_core::HashMap<(String, u8, String, String), f64>,
) -> Vec<LabelBucketBasis> {
    let mut entries: Vec<_> = map
        .iter()
        .map(|((name, bucket, tenor, basis), amount)| {
            (name.clone(), *bucket, tenor.clone(), basis.clone(), *amount)
        })
        .collect();
    entries.sort_by(|a, b| (&a.0, a.1, &a.2, &a.3).cmp(&(&b.0, b.1, &b.2, &b.3)));
    entries
}
fn extend_label_bucket_basis(
    map: &mut finstack_quant_core::HashMap<(String, u8, String, String), f64>,
    entries: Vec<LabelBucketBasis>,
) {
    for (name, bucket, tenor, basis, amount) in entries {
        map.insert((name, bucket, tenor, basis), amount);
    }
}
