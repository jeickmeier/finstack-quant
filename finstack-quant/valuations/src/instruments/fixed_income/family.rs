//! The fixed-income family: bonds, loans, agency MBS and their derivatives.
//!
//! Structured credit is its own family ([`super::structured_credit::STRUCTURED_CREDIT`]).

use std::sync::Arc;

use crate::metrics::{MetricId, MetricRegistry, MetricRegistryError, PricerPublishedMetric};
use crate::pricer::{
    register_generic, Family, InstrumentType, ModelKey, PricerRegistry, PricingError,
};

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
        registry.register(
            crate::instruments::fixed_income::bond::pricing::engine::BondPricer::new(model),
        )?;
    }
    registry.register(
        crate::instruments::fixed_income::bond::pricing::engine::SimpleBondMertonMcPricer,
    )?;
    registry.register(crate::instruments::fixed_income::bond_future::pricer::BondFuturePricer)?;

    register_generic!(
        registry,
        InstrumentType::FiIndexTotalReturnSwap,
        crate::instruments::fixed_income::fi_trs::FiIndexTotalReturnSwap
    );
    registry
        .register(crate::instruments::fixed_income::convertible::pricing::ConvertibleTreePricer)?;
    register_generic!(
        registry,
        InstrumentType::InflationLinkedBond,
        crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond
    );

    // Loans and facilities.
    registry.register(
        crate::instruments::fixed_income::revolving_credit::pricing::RevolvingCreditPricer::new(
            ModelKey::Discounting,
        ),
    )?;
    registry.register(
        crate::instruments::fixed_income::revolving_credit::pricing::RevolvingCreditPricer::new(
            ModelKey::MonteCarloThreeFactor,
        ),
    )?;
    // Asset-backed facility: priced off the synthetic deal's lender flows via
    // `Instrument::base_value`.
    register_generic!(
        registry,
        InstrumentType::AssetBackedFacility,
        crate::instruments::fixed_income::asset_backed_facility::AssetBackedFacility
    );
    registry.register(
        crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer,
    )?;
    registry.register(crate::instruments::fixed_income::term_loan::pricing::TermLoanTreePricer)?;

    // Agency MBS products price through `Instrument::base_value`.
    register_generic!(
        registry,
        InstrumentType::AgencyMbsPassthrough,
        crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough
    );
    register_generic!(
        registry,
        InstrumentType::AgencyTba,
        crate::instruments::fixed_income::tba::AgencyTba
    );
    register_generic!(
        registry,
        InstrumentType::DollarRoll,
        crate::instruments::fixed_income::dollar_roll::DollarRoll
    );
    register_generic!(
        registry,
        InstrumentType::AgencyCmo,
        crate::instruments::fixed_income::cmo::AgencyCmo
    );
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::instruments::fixed_income::bond::metrics::register_bond_metrics(registry)?;
    crate::instruments::fixed_income::bond_future::metrics::register_bond_future_metrics(registry)?;
    crate::instruments::fixed_income::convertible::metrics::register_convertible_metrics(registry)?;
    crate::instruments::fixed_income::mbs_passthrough::metrics::register_mbs_passthrough_metrics(
        registry,
    )?;
    crate::instruments::fixed_income::tba::metrics::register_tba_metrics(registry)?;
    crate::instruments::fixed_income::dollar_roll::metrics::register_dollar_roll_metrics(registry)?;
    crate::instruments::fixed_income::cmo::metrics::register_cmo_metrics(registry)?;
    crate::instruments::fixed_income::inflation_linked_bond::metrics::register_ilb_metrics(
        registry,
    )?;
    crate::instruments::fixed_income::term_loan::metrics::register_term_loan_metrics(registry)?;
    crate::instruments::fixed_income::revolving_credit::metrics::register_revolving_credit_metrics(
        registry,
    )?;
    crate::instruments::fixed_income::asset_backed_facility::metrics::register_asset_backed_facility_metrics(registry)?;
    crate::instruments::fixed_income::fi_trs::metrics::register_fi_trs_metrics(registry)?;
    // The Merton-MC bond engine publishes `expected_loss` (path-simulated
    // credit loss) from its own pricing pass; the entry records that
    // ownership so the applicability table stays truthful.
    registry.register_metric(
        MetricId::ExpectedLoss,
        Arc::new(PricerPublishedMetric::new(MetricId::ExpectedLoss)),
        &[InstrumentType::Bond],
    )?;
    Ok(())
}
