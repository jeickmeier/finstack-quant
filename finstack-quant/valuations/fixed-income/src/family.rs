//! The fixed-income family: bonds, loans, agency MBS and their derivatives.
//!
//! Structured credit, CMOs and asset-backed facilities register with the
//! `STRUCTURED_CREDIT` family in the structured-credit crate.

use finstack_quant_valuations_core::metrics::{
    MetricRegistry, MetricRegistryError, PricerPublishedMetric,
};
use finstack_quant_valuations_core::pricer::{
    Family, InstrumentType, ModelKey, PricerRegistry, PricingError,
};
use finstack_quant_valuations_core::register_generic;

/// Registration unit for the fixed-income instruments.
pub static FIXED_INCOME: Family = Family::new("fixed_income", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    // Bonds and bond futures.
    for model in [
        ModelKey::Discounting,
        ModelKey::Tree,
        ModelKey::HazardRate,
        ModelKey::RatesCredit,
    ] {
        registry.register(crate::bond::pricing::BondPricer::new(model))?;
    }
    registry.register(crate::bond::pricing::SimpleBondMertonMcPricer)?;
    registry.register(crate::bond_future::pricer::BondFuturePricer)?;

    register_generic!(
        registry,
        InstrumentType::FiIndexTotalReturnSwap,
        crate::fi_trs::FiIndexTotalReturnSwap
    );
    registry.register(crate::convertible::pricing::ConvertibleTreePricer)?;
    register_generic!(
        registry,
        InstrumentType::InflationLinkedBond,
        crate::inflation_linked_bond::InflationLinkedBond
    );

    // Loans and facilities.
    registry.register(
        crate::revolving_credit::pricing::RevolvingCreditPricer::new(ModelKey::Discounting),
    )?;
    registry.register(
        crate::revolving_credit::pricing::RevolvingCreditPricer::new(
            ModelKey::MonteCarloThreeFactor,
        ),
    )?;
    registry.register(crate::term_loan::pricing::TermLoanDiscountingPricer)?;
    registry.register(crate::term_loan::pricing::TermLoanTreePricer)?;

    // Agency MBS products price through `Instrument::base_value`.
    register_generic!(
        registry,
        InstrumentType::AgencyMbsPassthrough,
        crate::mbs_passthrough::AgencyMbsPassthrough
    );
    register_generic!(registry, InstrumentType::AgencyTba, crate::tba::AgencyTba);
    register_generic!(
        registry,
        InstrumentType::DollarRoll,
        crate::dollar_roll::DollarRoll
    );
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::bond::metrics::register_bond_metrics(registry)?;
    crate::bond_future::metrics::register_bond_future_metrics(registry)?;
    crate::convertible::metrics::register_convertible_metrics(registry)?;
    crate::mbs_passthrough::metrics::register_mbs_passthrough_metrics(registry)?;
    crate::tba::metrics::register_tba_metrics(registry)?;
    crate::dollar_roll::metrics::register_dollar_roll_metrics(registry)?;
    crate::inflation_linked_bond::metrics::register_ilb_metrics(registry)?;
    crate::term_loan::metrics::register_term_loan_metrics(registry)?;
    crate::revolving_credit::metrics::register_revolving_credit_metrics(registry)?;
    crate::fi_trs::metrics::register_fi_trs_metrics(registry)?;
    // The Merton-MC bond engine publishes `expected_loss` (path-simulated
    // credit loss) from its own pricing pass; the entry records that
    // ownership so the applicability table stays truthful.
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Bond,
        metrics: [(
            ExpectedLoss,
            PricerPublishedMetric::new(finstack_quant_valuations_core::metrics::MetricId::ExpectedLoss)
        )]
    }
    Ok(())
}
