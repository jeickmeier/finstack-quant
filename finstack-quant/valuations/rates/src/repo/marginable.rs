//! `Marginable` implementation for [`Repo`].

use super::Repo;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::CALENDAR_DAYS_PER_YEAR;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_margin::constants;
use finstack_quant_margin::types::NettingSetId;
use finstack_quant_margin::types::OtcMarginSpec;
use finstack_quant_margin::types::RepoMarginSpec;
use finstack_quant_margin::types::SimmSensitivities;
use finstack_quant_margin::Marginable;

impl Marginable for Repo {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        // Repos don't use OtcMarginSpec - they use RepoMarginSpec
        None
    }

    fn repo_margin_spec(&self) -> Option<&RepoMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        // Repos typically have their own netting arrangements
        // Use the repo ID as a simple netting set identifier
        Some(NettingSetId::bilateral(self.id.as_str(), "REPO_NETTING"))
    }

    fn simm_sensitivities(
        &self,
        _market: &MarketContext,
        as_of: Date,
    ) -> Result<SimmSensitivities> {
        let currency = self.cash_amount.currency();
        let mut sens = SimmSensitivities::new(currency);

        // Repos have limited rate sensitivity - mainly to the repo rate
        // Short-term IR sensitivity
        let days_to_maturity = (self.maturity - as_of).whole_days().max(1) as f64;
        let years_to_maturity = days_to_maturity / CALENDAR_DAYS_PER_YEAR;

        // DV01 approximation for short-term lending
        let dv01 = self.cash_amount.amount() * years_to_maturity * ONE_BASIS_POINT;

        // Assign to shortest tenor bucket (3M for very short, otherwise 6M)
        let tenor = if years_to_maturity <= constants::tenor_buckets::BUCKET_3M {
            "3M"
        } else {
            "6M"
        };

        sens.add_ir_delta(currency, tenor, dv01);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.pv(market, as_of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_marginable() {
        let repo = Repo::example().expect("example");

        // Repo uses repo_margin_spec, not margin_spec
        assert!(repo.margin_spec().is_none());

        // Should have netting set
        let netting_set = repo.netting_set_id();
        assert!(netting_set.is_some());
    }
}
