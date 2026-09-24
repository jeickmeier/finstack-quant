//! Pricer registrations for fixed-income instruments.
//!
//! Covers: FIIndexTotalReturnSwap, Convertible, InflationLinkedBond,
//! RevolvingCredit, TermLoan, AgencyMbsPassthrough, AgencyTba, DollarRoll,
//! AgencyCmo.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for additional fixed-income instruments (convertibles, MBS,
/// revolving credit, term loans) not included in the minimal rates set.
pub(crate) fn register_fixed_income_pricers(
    registry: &mut PricerRegistry,
) -> std::result::Result<(), crate::pricer::PricingError> {
    // FI Index TRS
    register_generic!(
        registry,
        InstrumentType::FiIndexTotalReturnSwap,
        crate::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap
    );

    // Convertible Bond
    registry
        .register(crate::instruments::fixed_income::convertible::pricing::ConvertibleTreePricer)?;

    // Inflation Linked Bond
    register_generic!(
        registry,
        InstrumentType::InflationLinkedBond,
        crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond
    );

    // Revolving Credit
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

    // Asset-backed facility — priced off the synthetic deal's lender flows via
    // Instrument::base_value.
    register_generic!(
        registry,
        InstrumentType::AssetBackedFacility,
        crate::instruments::fixed_income::asset_backed_facility::AssetBackedFacility
    );

    // Term Loan (including DDTL)
    registry.register(
        crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer,
    )?;
    registry.register(
        crate::instruments::fixed_income::term_loan::pricing::TermLoanTreePricer::default(),
    )?;

    // Agency MBS Passthrough — uses Instrument::base_value via GenericInstrumentPricer.
    // Per-instrument *DiscountingPricer wrappers were trivial pass-throughs with no
    // behavior beyond delegating to the same base_value path; collapsed to the
    // generic pricer to remove ~100 LoC of boilerplate (FI-TRS and inflation linker
    // already use the same pattern).
    register_generic!(
        registry,
        InstrumentType::AgencyMbsPassthrough,
        crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough
    );

    // Agency TBA
    register_generic!(
        registry,
        InstrumentType::AgencyTba,
        crate::instruments::fixed_income::tba::AgencyTba
    );

    // Dollar Roll
    register_generic!(
        registry,
        InstrumentType::DollarRoll,
        crate::instruments::fixed_income::dollar_roll::DollarRoll
    );

    // Agency CMO
    register_generic!(
        registry,
        InstrumentType::AgencyCmo,
        crate::instruments::fixed_income::cmo::AgencyCmo
    );
    Ok(())
}
