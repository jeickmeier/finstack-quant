//! Internal model IM calculator.
//!
//! Stub implementation for bank internal models (VaR/ES-based).

use crate::calculators::traits::{ImCalculator, ImResult};
use crate::traits::Marginable;
use crate::types::ImMethodology;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::HashMap;
use finstack_core::Result;
use std::sync::Arc;

/// Internal model IM calculator.
///
/// Fields are private; use the builder methods ([`Self::new`],
/// [`Self::with_conservative_rate`], [`Self::with_mpor_days`],
/// [`Self::with_input_source`]) so invariants — most notably that
/// `conservative_rate` lies in `[0, 1]` — are enforced at the boundary.
#[derive(Clone)]
pub struct InternalModelImCalculator {
    mpor_days: u32,
    conservative_rate: f64,
    input_source: Option<Arc<dyn super::ExternalImSource>>,
}

impl std::fmt::Debug for InternalModelImCalculator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InternalModelImCalculator")
            .field("mpor_days", &self.mpor_days)
            .field("conservative_rate", &self.conservative_rate)
            .field(
                "input_source",
                &self.input_source.as_ref().map(|_| "<dyn ExternalImSource>"),
            )
            .finish()
    }
}

impl Default for InternalModelImCalculator {
    fn default() -> Self {
        Self {
            mpor_days: 10,
            conservative_rate: 0.05,
            input_source: None,
        }
    }
}

impl InternalModelImCalculator {
    /// Create a new internal model calculator with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Margin period of risk in days.
    #[must_use]
    pub fn mpor_days(&self) -> u32 {
        self.mpor_days
    }

    /// Conservative fallback rate applied to a notional proxy.
    #[must_use]
    pub fn conservative_rate(&self) -> f64 {
        self.conservative_rate
    }

    /// Attach an internal-model input source.
    #[must_use]
    pub fn with_input_source(mut self, source: Arc<dyn super::ExternalImSource>) -> Self {
        self.input_source = Some(source);
        self
    }

    /// Override the conservative fallback rate.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if `rate` is not a
    /// finite value in `[0, 1]` — a negative or NaN rate would silently
    /// produce a negative or non-finite IM, which would in turn corrupt
    /// any downstream margin call.
    pub fn with_conservative_rate(mut self, rate: f64) -> Result<Self> {
        if !rate.is_finite() || !(0.0..=1.0).contains(&rate) {
            return Err(finstack_core::Error::Validation(format!(
                "InternalModel conservative_rate must be a finite value in [0, 1], got {rate}"
            )));
        }
        self.conservative_rate = rate;
        Ok(self)
    }

    /// Override the MPOR in days.
    #[must_use]
    pub fn with_mpor_days(mut self, days: u32) -> Self {
        self.mpor_days = days;
        self
    }

    /// Calculate IM using conservative estimate.
    pub fn calculate_conservative(&self, exposure_base: Money) -> Money {
        super::conservative_im(exposure_base, self.conservative_rate)
    }

    /// Conservative `|exposure_base| × rate` fallback used when no
    /// external internal-model IM is available. Fails closed if the
    /// instrument cannot supply a regulatory exposure base.
    fn conservative_fallback(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        let exposure_base = super::require_im_exposure_base(
            "InternalModel",
            instrument,
            context,
            as_of,
            "an external IM source amount",
        )?;
        Ok(self.calculate_conservative(exposure_base))
    }
}

impl ImCalculator for InternalModelImCalculator {
    fn calculate(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<ImResult> {
        let mut mpor_days = self.mpor_days;
        let mut label = "internal_model".to_string();

        let im_amount = if let Some(source) = &self.input_source {
            if let Some(override_mpor) = source.external_mpor_days() {
                mpor_days = override_mpor;
            }
            if let Some(name) = source.external_model_name() {
                label = name;
            }
            source
                .external_initial_margin(instrument, context, as_of)
                .map_or_else(
                    || self.conservative_fallback(instrument, context, as_of),
                    Ok,
                )?
        } else {
            self.conservative_fallback(instrument, context, as_of)?
        };

        let mut breakdown = HashMap::default();
        breakdown.insert(label, im_amount);

        Ok(ImResult::with_breakdown(
            im_amount,
            ImMethodology::InternalModel,
            as_of,
            mpor_days,
            breakdown,
        ))
    }

    fn methodology(&self) -> ImMethodology {
        ImMethodology::InternalModel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use std::sync::Arc;

    #[derive(Clone)]
    struct TestInstrument {
        id: String,
        value: Money,
    }

    impl TestInstrument {
        fn new(value: Money) -> Self {
            Self {
                id: "TEST-INST".to_string(),
                value,
            }
        }
    }

    impl Marginable for TestInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn margin_spec(&self) -> Option<&crate::OtcMarginSpec> {
            None
        }

        fn netting_set_id(&self) -> Option<crate::NettingSetId> {
            None
        }

        fn simm_sensitivities(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> Result<crate::SimmSensitivities> {
            Ok(crate::SimmSensitivities::new(self.value.currency()))
        }

        fn mtm_for_vm(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
            Ok(self.value)
        }
    }

    #[derive(Debug)]
    struct TestInputSource {
        amount: Money,
    }

    impl super::super::ExternalImSource for TestInputSource {
        fn external_initial_margin(
            &self,
            _instrument: &dyn Marginable,
            _context: &MarketContext,
            _as_of: Date,
        ) -> Option<Money> {
            Some(self.amount)
        }

        fn external_model_name(&self) -> Option<String> {
            Some("internal_var".to_string())
        }
    }

    #[test]
    fn conservative_internal_model_calc() {
        let calc = InternalModelImCalculator::default();
        let notional = Money::new(100_000_000.0, Currency::USD);
        let im = calc.calculate_conservative(notional);
        assert_eq!(im.amount(), 5_000_000.0);
    }

    #[test]
    fn input_source_overrides_amount() {
        let calc =
            InternalModelImCalculator::default().with_input_source(Arc::new(TestInputSource {
                amount: Money::new(2_500_000.0, Currency::USD),
            }));
        let instrument = TestInstrument::new(Money::new(10_000_000.0, Currency::USD));
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");
        let result = calc.calculate(&instrument, &market, as_of).expect("im");

        assert_eq!(result.amount.amount(), 2_500_000.0);
        assert!(result.breakdown.contains_key("internal_var"));
    }

    #[test]
    fn fails_closed_without_model_source_or_exposure_base() {
        let calc = InternalModelImCalculator::default();
        let instrument = TestInstrument::new(Money::new(0.0, Currency::USD));
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");

        let err = calc
            .calculate(&instrument, &market, as_of)
            .expect_err("internal-model IM must not use zero MtM as a notional proxy");

        assert!(
            err.to_string().contains("external IM source")
                && err.to_string().contains("exposure base"),
            "expected missing source/exposure-base error, got {err}"
        );
    }
}
