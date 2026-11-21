//! Display units for `BondQuoteSet` fields.

use finstack_quant_core::currency::Currency;
use finstack_quant_valuations_core::metrics::{MetricId, MetricUnit};
use finstack_quant_valuations_core::results::MetricFieldUnit;

/// Units of the `BondQuoteSet` fields, in the order the struct declares them, the
/// one place a host reads them from.
///
/// The set mixes currency prices, a percent-of-par price and decimal yields and
/// spreads. Each field with a metric states that metric's [`MetricId::unit`], and
/// `clean_price_pct`, which has none, states its quote convention. Values are never
/// rescaled.
///
/// # Arguments
///
/// * `currency` - The currency of the bond whose set is being labelled; it names
///   the unit of `clean_price_currency` and `dirty_price_currency`.
///
/// # Returns
///
/// One record per field of `BondQuoteSet`.
pub fn bond_quote_set_field_units(currency: Currency) -> Vec<MetricFieldUnit> {
    let label = |metric: Option<MetricId>| match metric.map(|metric| metric.unit()) {
        None => "% of par".to_string(),
        Some(MetricUnit::Currency) => currency.to_string(),
        Some(unit) => unit.as_str().to_string(),
    };
    [
        ("clean_price_currency", Some(MetricId::CleanPrice)),
        ("clean_price_pct", None),
        ("dirty_price_currency", Some(MetricId::DirtyPrice)),
        ("ytm", Some(MetricId::Ytm)),
        ("ytw", Some(MetricId::Ytw)),
        ("z_spread", Some(MetricId::ZSpread)),
        ("discount_margin", Some(MetricId::DiscountMargin)),
        ("oas", Some(MetricId::Oas)),
        ("asw_par", Some(MetricId::ASWPar)),
        ("asw_market", Some(MetricId::ASWMarket)),
        ("i_spread", Some(MetricId::ISpread)),
        ("japanese_simple_yield", Some(MetricId::JapaneseSimpleYield)),
        ("moosmuller_ytm", Some(MetricId::MoosmullerYtm)),
    ]
    .into_iter()
    .map(|(field, metric)| MetricFieldUnit {
        field: field.to_string(),
        label: label(metric),
        source: format!("valuations::BondQuoteSet::{field}"),
    })
    .collect()
}
