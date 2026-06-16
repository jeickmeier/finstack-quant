//! Public surface tests for valuations.

use finstack_quant_valuations::prelude::{
    BarrierOption, BasisSwap, CDSIndex, CDSTranche, ConvertibleBond, ExerciseStyle,
    InflationLinkedBond, OptionType, PayReceive, RevolvingCredit, SettlementType, StructuredCredit,
    TermLoan, VarianceSwap,
};

use finstack_quant_valuations::prelude::AsianOption;

#[test]
fn valuations_prelude_exposes_desk_quant_instruments_and_params() {
    fn assert_type<T>() {
        let _ = std::mem::size_of::<T>();
    }

    assert_type::<BasisSwap>();
    assert_type::<CDSIndex>();
    assert_type::<CDSTranche>();
    assert_type::<InflationLinkedBond>();
    assert_type::<ConvertibleBond>();
    assert_type::<StructuredCredit>();
    assert_type::<BarrierOption>();

    assert_type::<AsianOption>();
    assert_type::<VarianceSwap>();
    assert_type::<TermLoan>();
    assert_type::<RevolvingCredit>();
    assert_type::<OptionType>();
    assert_type::<ExerciseStyle>();
    assert_type::<PayReceive>();
    assert_type::<SettlementType>();
}
