use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements_analytics::templates::real_estate::{
    self, LeaseGrowthConvention, LeaseSpec, ManagementFeeBase, ManagementFeeSpec,
    PropertyTemplateNodes, RenewalSpec, RentRollOutputNodes, RentStepSpec, SimpleLeaseSpec,
};
use finstack_quant_statements_analytics::templates::RealEstateExtension;

#[test]
fn real_estate_noi_and_ncf_templates_compute_expected_values() {
    let model = ModelBuilder::new("re_template")
        .periods("2025Q1..Q2", None)
        .expect("periods should parse")
        .value(
            "rent",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
            ],
        )
        .value(
            "other_income",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(12.0)),
            ],
        )
        .value(
            "taxes",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(20.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(22.0)),
            ],
        )
        .value(
            "repairs",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(5.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(6.0)),
            ],
        )
        .value(
            "capex",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(3.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(4.0)),
            ],
        )
        .add_noi_buildup(
            "total_revenue",
            &["rent", "other_income"],
            "total_expenses",
            &["taxes", "repairs"],
            "noi",
        )
        .expect("noi template")
        .add_ncf_buildup("noi", &["capex"], "ncf")
        .expect("ncf template")
        .build()
        .expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    let q1 = PeriodId::quarter(2025, 1);
    let q2 = PeriodId::quarter(2025, 2);

    let noi = results.get_node("noi").expect("noi node");
    let ncf = results.get_node("ncf").expect("ncf node");

    // NOI = (rent + other_income) - (taxes + repairs)
    assert_eq!(noi[&q1], (100.0 + 10.0) - (20.0 + 5.0));
    assert_eq!(noi[&q2], (110.0 + 12.0) - (22.0 + 6.0));

    // NCF = NOI - capex
    assert_eq!(ncf[&q1], ((100.0 + 10.0) - (20.0 + 5.0)) - 3.0);
    assert_eq!(ncf[&q2], ((110.0 + 12.0) - (22.0 + 6.0)) - 4.0);
}

#[test]
fn real_estate_rent_roll_template_builds_lease_series_and_total_rent() {
    let leases = vec![
        SimpleLeaseSpec {
            node_id: "lease_a_rent".into(),
            start: PeriodId::quarter(2025, 1),
            end: Some(PeriodId::quarter(2025, 4)),
            base_rent: 100.0,
            growth_rate: 0.0,
            free_rent_periods: 0,
            occupancy: 1.0,
        },
        SimpleLeaseSpec {
            node_id: "lease_b_rent".into(),
            start: PeriodId::quarter(2025, 3),
            end: Some(PeriodId::quarter(2025, 4)),
            base_rent: 50.0,
            growth_rate: 0.0,
            free_rent_periods: 1, // free rent in first active period (Q3)
            occupancy: 0.9,
        },
    ];

    let builder = ModelBuilder::new("re_rent_roll")
        .periods("2025Q1..Q4", None)
        .expect("periods should parse");
    let model = real_estate::add_rent_roll_rental_revenue(builder, &leases, "rent_total")
        .expect("rent roll template")
        .build()
        .expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    let q1 = PeriodId::quarter(2025, 1);
    let q2 = PeriodId::quarter(2025, 2);
    let q3 = PeriodId::quarter(2025, 3);
    let q4 = PeriodId::quarter(2025, 4);

    let rent_total = results.get_node("rent_total").expect("rent_total node");
    assert_eq!(rent_total[&q1], 100.0);
    assert_eq!(rent_total[&q2], 100.0);
    // Lease B is free in Q3.
    assert_eq!(rent_total[&q3], 100.0);
    assert_eq!(rent_total[&q4], 145.0);
}

#[test]
fn real_estate_rent_roll_handles_steps_free_rent_and_renewal_downtime() {
    let nodes = RentRollOutputNodes {
        rent_pgi_node: "rent_pgi".into(),
        free_rent_node: "free_rent".into(),
        vacancy_loss_node: "vacancy_loss".into(),
        rent_effective_node: "rent_effective".into(),
    };

    let leases = vec![
        LeaseSpec {
            node_id: "lease_a".into(),
            start: PeriodId::quarter(2025, 1),
            end: Some(PeriodId::quarter(2025, 2)),
            base_rent: 100.0,
            growth_rate: 0.0,
            growth_convention: LeaseGrowthConvention::PerPeriod,
            rent_steps: vec![RentStepSpec {
                start: PeriodId::quarter(2025, 2),
                rent: 120.0,
            }],
            free_rent_periods: 0,
            free_rent_windows: vec![],
            occupancy: 0.8,
            renewal: Some(RenewalSpec {
                downtime_periods: 1, // Q3 downtime
                term_periods: 2,     // Q4.. (Q5 if present)
                probability: 0.5,
                rent_factor: 1.10,
                free_rent_periods: 1, // free at renewal start (Q4)
            }),
        },
        LeaseSpec {
            node_id: "lease_b".into(),
            start: PeriodId::quarter(2025, 1),
            end: Some(PeriodId::quarter(2025, 4)),
            base_rent: 50.0,
            growth_rate: 0.0,
            growth_convention: LeaseGrowthConvention::PerPeriod,
            rent_steps: vec![],
            free_rent_periods: 1, // Q1 free
            free_rent_windows: vec![],
            occupancy: 1.0,
            renewal: None,
        },
    ];

    let model = ModelBuilder::new("re_rent_roll")
        .periods("2025Q1..Q4", None)
        .expect("periods should parse")
        .add_rent_roll(&leases, &nodes)
        .expect("rent roll v2 template")
        .build()
        .expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    let q1 = PeriodId::quarter(2025, 1);
    let q2 = PeriodId::quarter(2025, 2);
    let q3 = PeriodId::quarter(2025, 3);
    let q4 = PeriodId::quarter(2025, 4);

    // Lease A contractual: Q1=100, Q2=120 (step), Q3 downtime, Q4 renewal starts at 120*1.10=132 but free at renewal start => 0.
    let a_pgi = results.get_node("lease_a.pgi").expect("a pgi node");
    assert_eq!(a_pgi[&q1], 100.0);
    assert_eq!(a_pgi[&q2], 120.0);
    assert_eq!(a_pgi[&q3], 0.0);
    assert_eq!(a_pgi[&q4], 132.0);

    let a_eff = results
        .get_node("lease_a.effective_rent")
        .expect("a effective node");
    // Effective rent applies occupancy (0.8) in initial term; renewal is additionally probability-weighted (0.5) and Q4 is free.
    assert_eq!(a_eff[&q1], 100.0 * 0.8);
    assert_eq!(a_eff[&q2], 120.0 * 0.8);
    assert_eq!(a_eff[&q3], 0.0);
    assert_eq!(a_eff[&q4], 0.0);

    // Lease B contractual: Q1=50 but free => 0 effective, Q2..Q4 = 50.
    let b_eff = results
        .get_node("lease_b.effective_rent")
        .expect("b effective node");
    assert_eq!(b_eff[&q1], 0.0);
    assert_eq!(b_eff[&q2], 50.0);
    assert_eq!(b_eff[&q3], 50.0);
    assert_eq!(b_eff[&q4], 50.0);

    // Totals: rent_effective = rent_pgi - free_rent - vacancy_loss
    let rent_eff = results
        .get_node("rent_effective")
        .expect("rent_effective node");
    assert!(rent_eff[&q2].is_finite(), "rent_effective should compute");
}

#[test]
fn real_estate_rent_roll_rejects_non_finite_growth_output() {
    let leases = vec![SimpleLeaseSpec {
        node_id: "lease_overflow_rent".into(),
        start: PeriodId::quarter(2025, 1),
        end: Some(PeriodId::quarter(2025, 4)),
        base_rent: 1.0e308,
        growth_rate: 1.0,
        free_rent_periods: 0,
        occupancy: 1.0,
    }];

    let builder = ModelBuilder::new("re_rent_roll_overflow")
        .periods("2025Q1..Q4", None)
        .expect("periods should parse");
    let result = real_estate::add_rent_roll_rental_revenue(builder, &leases, "rent_total");

    assert!(result.is_err());
    assert!(result
        .expect_err("overflowing rent growth should fail")
        .to_string()
        .contains("rent growth overflow"));
}

#[test]
fn real_estate_rich_rent_roll_rejects_non_finite_growth_output() {
    let nodes = RentRollOutputNodes {
        rent_pgi_node: "rent_pgi".into(),
        free_rent_node: "free_rent".into(),
        vacancy_loss_node: "vacancy_loss".into(),
        rent_effective_node: "rent_effective".into(),
    };

    let leases = vec![LeaseSpec {
        node_id: "lease_overflow".into(),
        start: PeriodId::quarter(2025, 1),
        end: Some(PeriodId::quarter(2025, 4)),
        base_rent: 1.0e308,
        growth_rate: 1.0,
        growth_convention: LeaseGrowthConvention::PerPeriod,
        rent_steps: vec![],
        free_rent_periods: 0,
        free_rent_windows: vec![],
        occupancy: 1.0,
        renewal: None,
    }];

    let result = ModelBuilder::new("re_rent_roll_overflow")
        .periods("2025Q1..Q4", None)
        .expect("periods should parse")
        .add_rent_roll(&leases, &nodes);

    assert!(result.is_err());
    assert!(result
        .expect_err("overflowing rent growth should fail")
        .to_string()
        .contains("rent growth overflow"));
}

#[test]
fn real_estate_full_property_template_computes_egi_noi_ncf() {
    let leases = vec![
        LeaseSpec {
            node_id: "lease_a".into(),
            start: PeriodId::quarter(2025, 1),
            end: Some(PeriodId::quarter(2025, 2)),
            base_rent: 100.0,
            growth_rate: 0.0,
            growth_convention: LeaseGrowthConvention::PerPeriod,
            rent_steps: vec![],
            free_rent_periods: 1, // Q1 free
            free_rent_windows: vec![],
            occupancy: 1.0,
            renewal: None,
        },
        LeaseSpec {
            node_id: "lease_b".into(),
            start: PeriodId::quarter(2025, 1),
            end: Some(PeriodId::quarter(2025, 2)),
            base_rent: 50.0,
            growth_rate: 0.0,
            growth_convention: LeaseGrowthConvention::PerPeriod,
            rent_steps: vec![],
            free_rent_periods: 0,
            free_rent_windows: vec![],
            occupancy: 0.8,
            renewal: None,
        },
    ];

    let nodes = PropertyTemplateNodes::default();

    let model = ModelBuilder::new("re_full_property")
        .periods("2025Q1..Q2", None)
        .expect("periods should parse")
        .value(
            "parking_income",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(10.0)),
            ],
        )
        .value(
            "taxes",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(5.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(5.0)),
            ],
        )
        .value(
            "repairs",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(2.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(2.0)),
            ],
        )
        .value(
            "capex",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(3.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(3.0)),
            ],
        )
        .add_property_operating_statement(
            &leases,
            &["parking_income"],
            &["taxes", "repairs"],
            &["capex"],
            Some(ManagementFeeSpec {
                rate: 0.10,
                base: ManagementFeeBase::Egi,
            }),
            &nodes,
        )
        .expect("property template")
        .build()
        .expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    let q1 = PeriodId::quarter(2025, 1);
    let q2 = PeriodId::quarter(2025, 2);

    let egi = results.get_node("egi").expect("egi node");
    let noi = results.get_node("noi").expect("noi node");
    let ncf = results.get_node("ncf").expect("ncf node");

    // Q1:
    // Lease A free => 0; Lease B effective = 50 * 0.8 = 40; other income 10 => EGI = 50.
    // Mgmt fee = 10% of EGI = 5. OpEx = taxes(5)+repairs(2)+mgmt(5)=12. NOI=38. NCF=NOI-capex(3)=35.
    assert_eq!(egi[&q1], 50.0);
    assert_eq!(noi[&q1], 38.0);
    assert_eq!(ncf[&q1], 35.0);

    // Q2:
    // Lease A effective = 100; Lease B effective = 40; other income 10 => EGI = 150.
    // Mgmt=15. OpEx=5+2+15=22. NOI=128. NCF=125.
    assert_eq!(egi[&q2], 150.0);
    assert_eq!(noi[&q2], 128.0);
    assert_eq!(ncf[&q2], 125.0);
}

#[test]
fn real_estate_annual_escalator_bumps_on_lease_anniversary() {
    // Quarterly model spanning 2 years: 8 quarters.
    // base_rent = 100, growth_rate = 0.10, AnnualEscalator
    // Year 1 (Q1..Q4) => 100 flat.  Year 2 (Q1..Q4) => 110 flat.

    let nodes = RentRollOutputNodes::default();

    let leases = vec![LeaseSpec {
        node_id: "lease_ann".into(),
        start: PeriodId::quarter(2025, 1),
        end: Some(PeriodId::quarter(2026, 4)),
        base_rent: 100.0,
        growth_rate: 0.10,
        growth_convention: LeaseGrowthConvention::AnnualEscalator,
        rent_steps: vec![],
        free_rent_periods: 0,
        free_rent_windows: vec![],
        occupancy: 1.0,
        renewal: None,
    }];

    let model = ModelBuilder::new("re_annual_esc")
        .periods("2025Q1..2026Q4", None)
        .expect("periods should parse")
        .add_rent_roll(&leases, &nodes)
        .expect("rent roll v2 template")
        .build()
        .expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    let pgi = results
        .get_node("lease_ann.pgi")
        .expect("lease_ann.pgi node");

    // Year 1: periods 0..3 => exponent = floor(0/4)=0 => 100.0
    for q in 1..=4 {
        assert_eq!(
            pgi[&PeriodId::quarter(2025, q)],
            100.0,
            "2025 Q{q} should be 100 (year 1, no escalation)"
        );
    }

    // Year 2: periods 4..7 => exponent = floor(4/4)=1 => 100 * 1.10 = 110.0
    for q in 1..=4 {
        let expected = 100.0 * 1.10;
        assert!(
            (pgi[&PeriodId::quarter(2026, q)] - expected).abs() < 1e-10,
            "2026 Q{q} should be {expected} (year 2, 1 escalation)"
        );
    }
}

#[test]
fn real_estate_annual_escalator_resets_at_rent_step() {
    // Quarterly model spanning 2 years.
    // base_rent = 100, growth_rate = 0.10, AnnualEscalator
    // Rent step at Q3 2025 sets rent to 200.
    //
    // Expected:
    // Q1-Q2 2025: segment starts at Q1 => exponent=floor(0/4)=0 => 100
    // Q3-Q4 2025: new segment at Q3 => exponent=floor(0/4)=0,floor(1/4)=0 => 200
    // Q1-Q4 2026: segment still from Q3 => periods 2,3,4,5 => exponent=floor(2/4)=0,floor(3/4)=0,floor(4/4)=1,floor(5/4)=1 => 200, 200, 220, 220

    let nodes = RentRollOutputNodes::default();

    let leases = vec![LeaseSpec {
        node_id: "lease_step".into(),
        start: PeriodId::quarter(2025, 1),
        end: Some(PeriodId::quarter(2026, 4)),
        base_rent: 100.0,
        growth_rate: 0.10,
        growth_convention: LeaseGrowthConvention::AnnualEscalator,
        rent_steps: vec![RentStepSpec {
            start: PeriodId::quarter(2025, 3),
            rent: 200.0,
        }],
        free_rent_periods: 0,
        free_rent_windows: vec![],
        occupancy: 1.0,
        renewal: None,
    }];

    let model = ModelBuilder::new("re_annual_step_reset")
        .periods("2025Q1..2026Q4", None)
        .expect("periods should parse")
        .add_rent_roll(&leases, &nodes)
        .expect("rent roll v2 template")
        .build()
        .expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    let pgi = results
        .get_node("lease_step.pgi")
        .expect("lease_step.pgi node");

    // Q1, Q2 2025: base segment, exponent 0 => 100
    assert_eq!(pgi[&PeriodId::quarter(2025, 1)], 100.0);
    assert_eq!(pgi[&PeriodId::quarter(2025, 2)], 100.0);

    // Q3, Q4 2025: new segment from Q3, 0 and 1 periods elapsed => exponent 0 => 200
    assert_eq!(pgi[&PeriodId::quarter(2025, 3)], 200.0);
    assert_eq!(pgi[&PeriodId::quarter(2025, 4)], 200.0);

    // Q1, Q2 2026: 2 and 3 periods from step start => exponent 0 => 200
    assert_eq!(pgi[&PeriodId::quarter(2026, 1)], 200.0);
    assert_eq!(pgi[&PeriodId::quarter(2026, 2)], 200.0);

    // Q3, Q4 2026: 4 and 5 periods from step start => exponent 1 => 200 * 1.10 = 220
    let expected_yr2 = 200.0 * 1.10;
    assert!(
        (pgi[&PeriodId::quarter(2026, 3)] - expected_yr2).abs() < 1e-10,
        "2026 Q3 should be {expected_yr2}"
    );
    assert!(
        (pgi[&PeriodId::quarter(2026, 4)] - expected_yr2).abs() < 1e-10,
        "2026 Q4 should be {expected_yr2}"
    );
}

#[test]
fn real_estate_per_period_growth_convention_is_default_and_unchanged() {
    // Verify that the default convention (PerPeriod) matches the original behavior.
    // Quarterly model, base_rent=100, growth_rate=0.10 => each quarter compounds.

    let nodes = RentRollOutputNodes::default();

    let leases = vec![LeaseSpec {
        node_id: "lease_pp".into(),
        start: PeriodId::quarter(2025, 1),
        end: Some(PeriodId::quarter(2025, 4)),
        base_rent: 100.0,
        growth_rate: 0.10,
        growth_convention: LeaseGrowthConvention::PerPeriod,
        rent_steps: vec![],
        free_rent_periods: 0,
        free_rent_windows: vec![],
        occupancy: 1.0,
        renewal: None,
    }];

    let model = ModelBuilder::new("re_per_period")
        .periods("2025Q1..Q4", None)
        .expect("periods should parse")
        .add_rent_roll(&leases, &nodes)
        .expect("rent roll v2 template")
        .build()
        .expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    let pgi = results.get_node("lease_pp.pgi").expect("lease_pp.pgi node");

    // Per-period compounding: rent = 100 * 1.10^n where n = 0, 1, 2, 3.
    for (q, n) in [(1u8, 0i32), (2, 1), (3, 2), (4, 3)] {
        let expected = 100.0 * 1.10_f64.powi(n);
        assert!(
            (pgi[&PeriodId::quarter(2025, q)] - expected).abs() < 1e-10,
            "Q{q}: expected {expected}, got {}",
            pgi[&PeriodId::quarter(2025, q)]
        );
    }
}

// --- Parity: SimpleLeaseSpec vs LeaseSpec produce same simple effective rent ---

#[test]
fn parity_rent_roll_simple_and_rich_match_for_simple_leases() {
    use finstack_quant_statements_analytics::templates::real_estate::{
        LeaseSpec, RentRollOutputNodes, SimpleLeaseSpec,
    };

    let start = PeriodId::quarter(2025, 1);

    // Simple lease spec
    let simple_lease = SimpleLeaseSpec {
        node_id: "lease1".into(),
        start,
        end: None,
        base_rent: 100.0,
        growth_rate: 0.05,
        free_rent_periods: 0,
        occupancy: 1.0,
    };

    // Rich lease spec with the same economic parameters
    let rich_lease = LeaseSpec {
        node_id: "lease1".into(),
        start,
        end: None,
        base_rent: 100.0,
        growth_rate: 0.05,
        growth_convention:
            finstack_quant_statements_analytics::templates::real_estate::LeaseGrowthConvention::PerPeriod,
        rent_steps: vec![],
        free_rent_periods: 0,
        free_rent_windows: vec![],
        occupancy: 1.0,
        renewal: None,
    };

    let make_base = || {
        ModelBuilder::new("parity-rent")
            .periods("2025Q1..Q4", None)
            .expect("valid periods")
    };

    // Build simple model
    let model_simple =
        real_estate::add_rent_roll_rental_revenue(make_base(), &[simple_lease], "total_rent")
            .expect("simple rent roll")
            .build()
            .expect("valid model");

    // Build rich model
    let nodes = RentRollOutputNodes::default();
    let model_rich = make_base()
        .add_rent_roll(&[rich_lease], &nodes)
        .expect("rich rent roll")
        .build()
        .expect("valid model");

    let mut eval = Evaluator::new();
    let r_simple = eval.evaluate(&model_simple).expect("simple eval");
    let r_rich = eval.evaluate(&model_rich).expect("rich eval");

    // The simple total_rent and rich rent_effective_node should match
    for q in 1u8..=4 {
        let period = PeriodId::quarter(2025, q);
        let simple_val = r_simple
            .get("total_rent", &period)
            .expect("simple total_rent");
        let rich_val = r_rich
            .get(&nodes.rent_effective_node, &period)
            .expect("rich rent_effective");
        assert!(
            (simple_val - rich_val).abs() < 1e-9,
            "Q{q}: simple total_rent ({simple_val}) must match rich rent_effective ({rich_val})"
        );
    }
}
