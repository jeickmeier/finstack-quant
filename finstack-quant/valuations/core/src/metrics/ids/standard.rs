use super::MetricId;

impl MetricId {
    // Pricer registry: spread / yield metrics on cash-equivalent cashflows

    /// Metrics computed on **cash-equivalent** cashflows when pricing with a
    /// non-discounting model (hazard, tree, Monte Carlo, etc.).
    ///
    /// This list must stay aligned with the spread/yield split in
    /// [`crate::pricer::PricerRegistry::price_with_metrics`].
    pub const SPREAD_EQUIVALENT_METRICS: &'static [MetricId] = &[
        MetricId::Ytm,
        MetricId::Ytw,
        MetricId::JapaneseSimpleYield,
        MetricId::MoosmullerYtm,
        MetricId::ZSpread,
        MetricId::ISpread,
        MetricId::DiscountMargin,
        MetricId::Oas,
        MetricId::ASWPar,
        MetricId::ASWMarket,
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
        MetricId::EmbeddedOptionValue,
    ];

    /// Return all standard metric IDs in group display order.
    ///
    /// The catalogue is derived from named group membership, including diagnostic
    /// outputs, so discovery and strict parsing use the same inventory.
    pub fn get_standard() -> &'static [MetricId] {
        static STANDARD: std::sync::OnceLock<Vec<MetricId>> = std::sync::OnceLock::new();
        STANDARD.get_or_init(|| {
            super::MetricGroup::ALL
                .iter()
                .flat_map(|group| group.metrics().iter().cloned())
                .collect()
        })
    }
}
