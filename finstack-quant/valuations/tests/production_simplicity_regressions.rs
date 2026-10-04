//! Economic regressions for simplicity-review corrections.
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_core::{
    cashflow::{CFKind, CashFlow},
    currency::Currency,
    dates::{DayCount, Tenor},
    money::Money,
};
use time::macros::date;

#[test]
fn tied_custom_coupon_intervals_choose_shorter_frequency() {
    let dates = [
        date!(2025 - 01 - 01),
        date!(2025 - 04 - 01),
        date!(2025 - 10 - 01),
    ];
    let mut flows: Vec<_> = dates
        .iter()
        .map(|&date| {
            CashFlow::new(
                date,
                None,
                Money::from((10_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                Some(0.04),
            )
        })
        .collect();
    flows.push(CashFlow::new(
        dates[2],
        None,
        Money::from((1000_i64, Currency::USD)),
        CFKind::Notional,
        0.0,
        None,
    ));
    let schedule = CashFlowSchedule::from_parts(
        flows,
        Notional::par(1000.0, Currency::USD).expect("notional"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(date!(2024 - 10 - 01)),
            maturity: Some(dates[2]),
            ..CashFlowMeta::default()
        },
    );
    let schedule_json = serde_json::to_string(&schedule).expect("schedule JSON");
    let wire =
        finstack_quant_valuations::instruments::fixed_income::bond::bond_from_cashflows_json(
            "TIED",
            &schedule_json,
            "USD-OIS",
            None,
        )
        .expect("custom bond JSON");
    let envelope: finstack_quant_valuations::instruments::InstrumentEnvelope =
        serde_json::from_str(&wire).expect("bond envelope");
    let finstack_quant_valuations::instruments::InstrumentJson::Bond(bond) = envelope.instrument
    else {
        panic!("expected bond")
    };
    assert_eq!(bond.cashflow_spec.frequency(), Tenor::quarterly());
}

#[test]
fn structured_credit_json_acceptance_enforces_canonical_rules() {
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::AdvanceRate;
    let valid = serde_json::json!({"asset_class":"*", "rate":0.8});
    let rate: AdvanceRate = serde_json::from_value(valid).unwrap();
    assert_eq!(rate.rate, 0.8);
    assert!(serde_json::from_value::<AdvanceRate>(
        serde_json::json!({"asset_class":"*", "rate":1.1})
    )
    .is_err());
}

#[test]
fn tranche_table_retains_empty_schema_sums_duplicates_and_rejects_currency_loss() {
    use finstack_quant_core::table::TableColumnData;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCashflows;
    let zero = Money::from((0_i64, Currency::USD));
    let mut cashflows = TrancheCashflows {
        tranche_id: "TABLE".into(),
        cashflows: Vec::new(),
        detailed_flows: Vec::new(),
        accrual_periods: Vec::new(),
        interest_flows: Vec::new(),
        principal_flows: Vec::new(),
        pik_flows: Vec::new(),
        deferred_flows: Vec::new(),
        writedown_flows: Vec::new(),
        final_balance: zero,
        total_interest: zero,
        total_principal: zero,
        total_pik: zero,
        total_deferred: zero,
        total_writedown: zero,
    };
    let table = cashflows.to_table().unwrap();
    assert_eq!(table.row_count, 0);
    assert_eq!(table.columns.len(), 7);
    assert!(matches!(&table.columns[0].data, TableColumnData::String(values) if values.is_empty()));
    let date = date!(2025 - 01 - 02);
    cashflows.interest_flows = vec![
        (date, Money::from((2_i64, Currency::USD))),
        (date, Money::from((3_i64, Currency::USD))),
    ];
    let table = cashflows.to_table().unwrap();
    assert_eq!(table.row_count, 1);
    assert!(matches!(&table.columns[2].data, TableColumnData::Float64(values) if values == &[5.0]));
    cashflows
        .principal_flows
        .push((date, Money::from((1_i64, Currency::EUR))));
    assert!(matches!(
        cashflows.to_table(),
        Err(finstack_quant_core::Error::CurrencyMismatch { .. })
    ));
}

#[test]
fn retired_public_options_are_rejected_in_json() {
    use finstack_quant_valuations::instruments::{
        fixed_income::revolving_credit::{StochasticUtilizationSpec, UtilizationProcess},
        Swaption,
    };
    let mut swaption = serde_json::to_value(Swaption::example().unwrap()).unwrap();
    swaption["exercise_style"] = serde_json::json!("european");
    assert!(serde_json::from_value::<Swaption>(swaption)
        .unwrap_err()
        .to_string()
        .contains("exercise_style"));
    let spec = StochasticUtilizationSpec {
        utilization_process: UtilizationProcess::MeanReverting {
            theta: 0.5,
            kappa: 1.0,
            sigma: 0.1,
            spread_sensitivity: 0.0,
        },
        mc_config: None,
    };
    for value in [false, true] {
        let mut wire = serde_json::to_value(&spec).unwrap();
        wire["use_sobol_qmc"] = serde_json::json!(value);
        assert!(serde_json::from_value::<StochasticUtilizationSpec>(wire)
            .unwrap_err()
            .to_string()
            .contains("use_sobol_qmc"));
    }
}
