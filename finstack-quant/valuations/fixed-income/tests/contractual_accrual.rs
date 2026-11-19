//! Contractual earning terms the debt instruments expose for balance-sheet
//! accrual: the coupon frequency and accrual method, with no settlement
//! ex-coupon window and no PIK.

use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_fixed_income::bond::Bond;
use finstack_quant_valuations_fixed_income::convertible::ConvertibleBond;
use finstack_quant_valuations_fixed_income::revolving_credit::RevolvingCredit;
use finstack_quant_valuations_fixed_income::term_loan::TermLoan;

#[test]
fn debt_instruments_state_their_contractual_earning_terms() {
    let bond = Bond::example().expect("bond");
    let loan = TermLoan::example().expect("term loan");
    let revolver = RevolvingCredit::example().expect("revolver");
    let convertible = ConvertibleBond::example().expect("convertible");

    let instruments: [(&dyn Instrument, _); 4] = [
        (&bond, bond.cashflow_spec.frequency()),
        (&loan, loan.frequency),
        (&revolver, revolver.frequency),
        (&convertible, convertible.cashflow_spec.frequency()),
    ];
    for (instrument, frequency) in instruments {
        let terms = instrument
            .contractual_accrual_config()
            .expect("a coupon-bearing debt instrument has earning terms");
        assert_eq!(terms.frequency, Some(frequency), "{}", instrument.id());
        // The contractual basis has no settlement ex-coupon window and
        // excludes capitalized interest.
        assert!(terms.ex_coupon.is_none(), "{}", instrument.id());
        assert!(!terms.include_pik, "{}", instrument.id());
    }
    // The settlement quote basis keeps the bond's own accrual method.
    assert_eq!(
        bond.contractual_accrual_config().map(|terms| terms.method),
        Some(bond.accrual_config().method)
    );
}
