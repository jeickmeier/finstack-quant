use super::calculator::*;
use super::params::*;
use crate::calculators::traits::{ImCalculator, ImResult};
use crate::types::{
    ImMethodology, SimmCreditSector, SimmRiskClass, SimmSensitivities, ordered_tenor_pair,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{FxHashMap, Result};

mod unit_tests {
    use super::*;
    use crate::Marginable;
    use crate::registry::validate_simm_params;
    use crate::types::SimmCurvatureSensitivity;
    fn test_date() -> Date {
        Date::from_calendar_date(2025, time::Month::January, 15).expect("date")
    }
    fn usd_tenors(values: &FxHashMap<String, f64>) -> FxHashMap<(Currency, String), f64> {
        values
            .iter()
            .map(|(tenor, amount)| ((Currency::USD, tenor.clone()), *amount))
            .collect()
    }
    fn single_credit(amount: f64) -> FxHashMap<(String, String), f64> {
        [(("NAME".to_string(), "5Y".to_string()), amount)]
            .into_iter()
            .collect()
    }

    #[test]
    fn simm_version_display() {
        assert_eq!(SimmVersion::V2_6.to_string(), "SIMM v2.6");
        assert_eq!(SimmVersion::V2_6.as_str(), "v2_6");
        assert!(
            "v2_5".parse::<SimmVersion>().is_err(),
            "v2.5 tables are not shipped, so the version is not selectable"
        );
        for noncanonical in ["SIMM 2.6", "2_6", "v2.6", " V2_6"] {
            assert!(noncanonical.parse::<SimmVersion>().is_err());
        }
    }

    #[test]
    fn embedded_simm_registries_pass_validation() {
        SimmCalculator::new(SimmVersion::V2_6)
            .expect("embedded SIMM v2.6 registry should validate");
    }

    #[test]
    fn validate_rejects_missing_ir_tenor_pair() {
        let mut params = SimmCalculator::new(SimmVersion::V2_6)
            .expect("registry should load")
            .params;
        let mut tenors = params.ir_delta_weights.keys();
        let a = tenors
            .next()
            .expect("embedded SIMM registry should define at least two IR tenors");
        let b = tenors
            .next()
            .expect("embedded SIMM registry should define at least two IR tenors");
        let pair = ordered_tenor_pair(a, b);
        params.ir_tenor_correlations.remove(&pair);
        let err =
            validate_simm_params(&params).expect_err("should reject missing ir_tenor_correlations");
        let msg = err.to_string();
        assert!(
            msg.contains("ir_tenor_correlations"),
            "error should name the map: {msg}"
        );
    }

    #[test]
    fn ir_delta_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        // Single-tenor: correlation matrix is 1.0 on diagonal so
        // result = sqrt((dv01 * weight)^2) = |dv01 * weight|
        let dv01_by_tenor: FxHashMap<String, f64> = [
            ("5Y".to_string(), 100_000.0), // $100K DV01 at 5y
        ]
        .into_iter()
        .collect();

        let ir_margin = calc.aggregate_ir(&usd_tenors(&dv01_by_tenor), false, None);

        // ISDA SIMM v2.6 D.1 p14: regular-currency 5Y weight is 60bp.
        assert!((ir_margin - 100_000.0 * 60.0).abs() < 1.0);
    }

    #[test]
    fn credit_non_qualifying_delta_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01 = 50_000.0;
        let cnq_margin = calc.non_qualifying_margin(&single_credit(cs01), false, None);

        // ISDA SIMM v2.6 F.1 p18: unclassified non-qualifying credit
        // uses the 1300bp residual weight, below the USD500K/bp threshold.
        assert!((cnq_margin - 50_000.0 * 1300.0).abs() < 1.0);
    }

    #[test]
    fn fx_delta_bucketed_preserves_partial_offset() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut fx_delta = FxHashMap::default();
        fx_delta.insert(Currency::EUR, 100.0);
        fx_delta.insert(Currency::JPY, -100.0);

        let actual = calc.calculate_fx_delta_bucketed(&fx_delta, None);
        let expected = 100.0 * calc.params.fx_delta_weight;

        assert!(
            (actual - expected).abs() < 1e-9,
            "opposite FX deltas should aggregate with rho=0.5: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn params_loaded() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        assert_eq!(calc.version(), SimmVersion::V2_6);
        assert!(calc.params.ir_delta_weights.contains_key("5Y"));
        assert!(
            calc.params
                .cq_bucket_weights
                .contains_key(&SimmCreditSector::Financial)
        );
    }

    #[test]
    fn aggregation() {
        let calc = SimmCalculator::new(SimmVersion::default()).expect("embedded registry");

        let risk_class_margins: FxHashMap<SimmRiskClass, f64> = [
            (SimmRiskClass::InterestRate, 1_000_000.0),
            (SimmRiskClass::CreditQualifying, 500_000.0),
        ]
        .into_iter()
        .collect();

        let total = calc.aggregate_risk_classes(&risk_class_margins);

        // ISDA v2.6 K: IR/CQ correlation is 4%.
        let expected = (1e12_f64 + 0.25e12 + 2.0 * 0.04 * 1e6 * 0.5e6).sqrt();
        assert!((total - expected).abs() < 1e-6);
    }

    #[test]
    fn calculate_from_sensitivities_uses_risk_class_correlation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 100_000.0);
        sens.add_equity_delta("AAPL", 100_000.0);

        let result = calc
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        let total_im = result.amount.amount();
        let breakdown = &result.breakdown;

        let ir_margin = breakdown
            .get("IR_Delta")
            .expect("IR margin present")
            .amount();
        let eq_margin = breakdown
            .get("Equity_Delta")
            .expect("Equity margin present")
            .amount();

        let expected =
            (ir_margin * ir_margin + eq_margin * eq_margin + 2.0 * 0.07 * ir_margin * eq_margin)
                .sqrt();
        assert!((total_im - expected).abs() < 1.0);
    }

    #[test]
    fn ir_delta_multi_tenor_with_correlations() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let dv01_by_tenor: FxHashMap<String, f64> = [
            ("5Y".to_string(), 100_000.0),
            ("10Y".to_string(), -80_000.0), // Partially hedged
        ]
        .into_iter()
        .collect();

        let ir_margin = calc.aggregate_ir(&usd_tenors(&dv01_by_tenor), false, None);

        // ws_5y = 100K*51 = 5.1M, ws_10y = -80K*51 = -4.08M
        // With high tenor correlation (~0.96), the hedge offsets most of the risk
        // so margin should be much less than the uncorrelated sqrt(5.1^2 + 4.08^2) ≈ 6.53M
        assert!(ir_margin > 1_000_000.0);
        assert!(ir_margin < 3_000_000.0);
    }

    #[test]
    fn ir_vega_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let vega_by_tenor: FxHashMap<String, f64> =
            [("5Y".to_string(), 500_000.0)].into_iter().collect();

        let ir_vega_margin = calc.aggregate_ir(&usd_tenors(&vega_by_tenor), true, None);
        // D.1 paragraph 35: single tenor uses VRW 0.23; no IR vega HVR.
        assert!((ir_vega_margin - 115_000.0).abs() < 1.0);
    }

    #[test]
    fn m15_ir_vega_multi_currency_preserves_same_tenor_exposures() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut sensitivities = SimmSensitivities::new(Currency::USD);
        sensitivities.add_ir_vega(Currency::USD, "5Y", 500_000.0);
        sensitivities.add_ir_vega(Currency::EUR, "5Y", 500_000.0);

        let result = calc
            .calculate_from_sensitivities(&sensitivities, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        let ir_vega_margin = result
            .breakdown
            .get("IR_Vega")
            .expect("M-15: IR vega margin should be present")
            .amount();

        let single_currency_margin = calc.aggregate_ir(
            &usd_tenors(&[("5Y".to_string(), 500_000.0)].into_iter().collect()),
            true,
            None,
        );
        assert!(
            ir_vega_margin > single_currency_margin,
            "M-15: same-tenor multi-currency IR vega must not collapse to one exposure"
        );
    }

    #[test]
    fn curvature_is_computed_within_each_class_before_class_correlation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry");
        let inputs = [
            SimmCurvatureSensitivity {
                risk_class: SimmRiskClass::InterestRate,
                bucket: "USD".into(),
                factor: "OIS".into(),
                risk_tenor: Some("5Y".into()),
                expiry_tenor: "2W".into(),
                volatility_weighted_vega: 1000.0,
            },
            SimmCurvatureSensitivity {
                risk_class: SimmRiskClass::Equity,
                bucket: "residual".into(),
                factor: "ACME".into(),
                risk_tenor: None,
                expiry_tenor: "2W".into(),
                volatility_weighted_vega: 500.0,
            },
        ];
        let z2 = 2.575_829_303_548_900_4_f64.powi(2);
        let ir = 500.0 * z2 / 0.47_f64.powi(2);
        let equity = 250.0 * z2;
        let expected = (ir * ir + equity * equity + 2.0 * 0.07 * ir * equity).sqrt();
        let actual = calc.aggregate_risk_classes(
            &calc
                .curvature_by_risk_class(&inputs, None)
                .expect("curvature"),
        );
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn commodity_delta_uses_bucket_correlations() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let delta_by_bucket: FxHashMap<String, f64> =
            [("2".to_string(), 100_000.0), ("3".to_string(), -100_000.0)]
                .into_iter()
                .collect();

        let actual = calc.aggregate_commodity(&delta_by_bucket, false, None);
        let bucket_2 = 100_000.0 * calc.params.commodity_bucket_weights["2"];
        let bucket_3 = -100_000.0 * calc.params.commodity_bucket_weights["3"];
        let rho_23 = 0.92_f64;
        let expected =
            (bucket_2 * bucket_2 + bucket_3 * bucket_3 + 2.0 * rho_23 * bucket_2 * bucket_3).sqrt();

        assert!(
            (actual - expected).abs() < 1.0,
            "expected correlated commodity margin {}, got {}",
            expected,
            actual
        );
    }

    #[derive(Clone)]
    struct MarginableTestInstrument {
        id: String,
        value: Money,
        sensitivities: SimmSensitivities,
    }

    impl MarginableTestInstrument {
        fn new(value: Money, sensitivities: SimmSensitivities) -> Self {
            Self {
                id: "SIMM-TEST".to_string(),
                value,
                sensitivities,
            }
        }
    }

    impl Marginable for MarginableTestInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn margin_spec(&self) -> Option<&crate::types::OtcMarginSpec> {
            None
        }

        fn netting_set_id(&self) -> Option<crate::types::NettingSetId> {
            None
        }

        fn simm_sensitivities(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> Result<SimmSensitivities> {
            Ok(self.sensitivities.clone())
        }

        fn mtm_for_vm(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
            Ok(self.value)
        }
    }

    #[test]
    fn public_calculate_matches_full_simm_sensitivities() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");

        let mut sensitivities = SimmSensitivities::new(Currency::USD);
        sensitivities.add_ir_delta(Currency::USD, "5Y", 50_000.0);
        sensitivities.add_equity_delta("AAPL", 100_000.0);
        sensitivities.add_fx_delta(Currency::EUR, 80_000.0);

        let instrument = MarginableTestInstrument::new(
            Money::from((1_000_000_i64, Currency::USD)),
            sensitivities.clone(),
        );
        let market = MarketContext::new();

        let expected = calc
            .calculate_from_sensitivities(&sensitivities, Currency::USD, as_of)
            .expect("valid sensitivity fixture");
        let actual = calc
            .calculate(&instrument, &market, as_of)
            .expect("SIMM calculation should succeed");

        assert!(
            (actual.amount.amount() - expected.amount.amount()).abs() < 1e-2,
            "expected total {}, got {} with breakdown {:?}",
            expected.amount.amount(),
            actual.amount.amount(),
            actual.breakdown
        );
        for (key, expected_amount) in &expected.breakdown {
            let actual_amount = actual
                .breakdown
                .get(key)
                .expect("expected breakdown entry should be present");
            assert!(
                (actual_amount.amount() - expected_amount.amount()).abs() < 1e-2,
                "breakdown mismatch for {key}: expected {}, got {}",
                expected_amount.amount(),
                actual_amount.amount()
            );
        }
        assert!(actual.breakdown.contains_key("Equity_Delta"));
        assert!(actual.breakdown.contains_key("FX_Delta"));
    }

    // Bucketed credit qualifying delta tests

    #[test]
    fn bucketed_single_bucket_uses_sector_weight_and_concentration() {
        // When all sensitivities are in one bucket with one name, the bucketed
        // aggregation should produce K = |cs01 * sector_weight| multiplied by
        // the bucket concentration factor.
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01 = 50_000.0;
        let sector = SimmCreditSector::BasicMaterials;
        let weight = calc.params.cq_bucket_weight(sector);
        let raw_ws = cs01 * weight;
        let cf = concentration(cs01, calc.params.cq_concentration_thresholds[&sector]);

        // Bucketed path: single name in one bucket.
        let mut bucketed: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        bucketed.insert((sector, "ISSUER_A".to_string(), "5Y".to_string()), cs01);
        let bucketed_margin = calc.aggregate_credit_qualifying(
            &bucketed,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );

        let expected = (raw_ws * cf).abs();
        assert!(
            (bucketed_margin - expected).abs() < 1.0,
            "bucketed mismatch: expected {expected}, got {bucketed_margin}"
        );
    }

    #[test]
    fn bucketed_diversification_reduces_margin() {
        // A diversified portfolio across multiple sectors should produce LOWER
        // margin than the equivalent scalar approach (which sums everything
        // into one bucket with no diversification benefit).
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01_per_name = 1_000.0;

        // Bucketed path: spread across four different sectors.
        let mut bucketed: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        bucketed.insert(
            (
                SimmCreditSector::Sovereign,
                "GOVT_A".to_string(),
                "5Y".to_string(),
            ),
            cs01_per_name,
        );
        bucketed.insert(
            (
                SimmCreditSector::Financial,
                "BANK_A".to_string(),
                "5Y".to_string(),
            ),
            cs01_per_name,
        );
        bucketed.insert(
            (
                SimmCreditSector::BasicMaterials,
                "MINING_A".to_string(),
                "5Y".to_string(),
            ),
            cs01_per_name,
        );
        bucketed.insert(
            (
                SimmCreditSector::TechnologyMedia,
                "TECH_A".to_string(),
                "5Y".to_string(),
            ),
            cs01_per_name,
        );
        let bucketed_margin = calc.aggregate_credit_qualifying(
            &bucketed,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );
        let gross_weighted: f64 = bucketed
            .iter()
            .map(|((sector, _, _), cs01)| (cs01 * calc.params.cq_bucket_weight(*sector)).abs())
            .sum();

        assert!(
            bucketed_margin < gross_weighted,
            "diversified bucketed margin ({bucketed_margin}) should be less \
             than gross weighted margin ({gross_weighted})"
        );
        // The bucketed margin should still be positive.
        assert!(bucketed_margin > 0.0, "bucketed margin should be positive");
    }

    #[test]
    fn bucketed_inter_bucket_correlation_formula() {
        // Verify the inter-bucket aggregation formula directly.
        // Two buckets with known K values and inter-bucket correlation gamma.
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01_a = 100_000.0;
        let cs01_b = 80_000.0;
        let sector_a = SimmCreditSector::Sovereign;
        let sector_b = SimmCreditSector::Financial;

        let weight_a = calc.params.cq_bucket_weight(sector_a);
        let weight_b = calc.params.cq_bucket_weight(sector_b);

        let ws_a = cs01_a * weight_a;
        let ws_b = cs01_b * weight_b;
        let cf_a = concentration(cs01_a, calc.params.cq_concentration_thresholds[&sector_a]);
        let cf_b = concentration(cs01_b, calc.params.cq_concentration_thresholds[&sector_b]);

        // Single-name per bucket: K_b = |cs01 * weight * concentration_factor|
        let k_a = (ws_a * cf_a).abs();
        let k_b = (ws_b * cf_b).abs();
        let s_a = (ws_a * cf_a).clamp(-k_a, k_a);
        let s_b = (ws_b * cf_b).clamp(-k_b, k_b);

        let gamma = calc.params.cq_inter_bucket_correlation(sector_a, sector_b);

        // Expected: sqrt(K_a^2 + K_b^2 + 2*gamma*S_a*S_b)
        let expected = (k_a * k_a + k_b * k_b + 2.0 * gamma * s_a * s_b).sqrt();

        let mut bucketed: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        bucketed.insert((sector_a, "GOVT_A".to_string(), "5Y".to_string()), cs01_a);
        bucketed.insert((sector_b, "BANK_A".to_string(), "5Y".to_string()), cs01_b);
        let actual = calc.aggregate_credit_qualifying(
            &bucketed,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );

        assert!(
            (actual - expected).abs() < 1.0,
            "inter-bucket formula: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn bucketed_intra_bucket_two_names() {
        // Verify intra-bucket aggregation with two names in the same sector.
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01_1 = 60_000.0;
        let cs01_2 = 40_000.0;
        let sector = SimmCreditSector::Financial;
        let weight = calc.params.cq_bucket_weight(sector);
        let rho = calc.params.cq_intra_bucket_correlation;

        let ws_1 = cs01_1 * weight;
        let ws_2 = cs01_2 * weight;
        let cf_1 = concentration(cs01_1, calc.params.cq_concentration_thresholds[&sector]);
        let cf_2 = concentration(cs01_2, calc.params.cq_concentration_thresholds[&sector]);

        // K_b = sqrt((cf*ws_1)^2 + (cf*ws_2)^2 + 2*rho*(cf*ws_1)*(cf*ws_2))
        let scaled_1 = cf_1 * ws_1;
        let scaled_2 = cf_2 * ws_2;
        let expected = (scaled_1 * scaled_1
            + scaled_2 * scaled_2
            + 2.0 * rho * (cf_1.min(cf_2) / cf_1.max(cf_2)) * scaled_1 * scaled_2)
            .sqrt();

        let mut bucketed: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        bucketed.insert((sector, "BANK_A".to_string(), "5Y".to_string()), cs01_1);
        bucketed.insert((sector, "BANK_B".to_string(), "5Y".to_string()), cs01_2);
        let actual = calc.aggregate_credit_qualifying(
            &bucketed,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );

        assert!(
            (actual - expected).abs() < 1.0,
            "intra-bucket two names: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn calculate_from_sensitivities_uses_sector_bucketed_credit_qualifying_delta() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_credit_qualifying_delta(SimmCreditSector::Sovereign, "GOVT_A", "5Y", 50_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Financial, "BANK_A", "5Y", 50_000.0);

        let result = calc
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        let total_im = result.amount.amount();
        let breakdown = &result.breakdown;
        assert!(total_im > 0.0, "total IM should be positive");
        assert!(
            breakdown.contains_key("Credit_Qualifying_Delta"),
            "breakdown should contain Credit_Qualifying_Delta"
        );

        // The bucketed margin should match the direct bucketed calculation.
        let expected = calc.aggregate_credit_qualifying(
            &sens.credit_qualifying_delta,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );
        let actual = breakdown
            .get("Credit_Qualifying_Delta")
            .expect("CQ delta breakdown entry")
            .amount();
        assert!(
            (actual - expected).abs() < 1.0,
            "calculate_from_sensitivities should delegate to bucketed: \
             expected {expected}, got {actual}"
        );
    }

    /// Netting set exercising every newly wired vega risk class.
    ///
    /// An IR delta anchor is included so the correlated risk-class
    /// aggregation is exercised end to end rather than degenerating to a
    /// single-class square root.
    fn credit_commodity_vega_sensitivities() -> SimmSensitivities {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 10_000.0);
        sens.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 25_000.0);
        sens.add_credit_non_qualifying_vega("RMBS_A", "5Y", 15_000.0);
        sens.add_commodity_vega("Crude", 20_000.0);
        sens
    }

    /// Guard against the registry weight going silently inert: doubling the
    /// weight MUST move the margin number.
    #[test]
    fn cq_vega_weight_is_read_by_the_calculation() {
        let base = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut bumped = base.clone();
        bumped.params.cq_vega_weight = base.params.cq_vega_weight * 2.0;

        let sens = credit_commodity_vega_sensitivities();
        let baseline = base
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();
        let with_bump = bumped
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();

        assert!(baseline > 0.0, "baseline margin must be positive");
        assert_ne!(
            baseline, with_bump,
            "cq_vega_weight must change the SIMM margin"
        );
        assert!(with_bump > baseline, "a larger vega weight must raise IM");
    }

    /// Guard against the registry weight going silently inert: doubling the
    /// weight MUST move the margin number.
    #[test]
    fn cnq_vega_weight_is_read_by_the_calculation() {
        let base = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut bumped = base.clone();
        bumped.params.cnq_vega_weight = base.params.cnq_vega_weight * 2.0;

        let sens = credit_commodity_vega_sensitivities();
        let baseline = base
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();
        let with_bump = bumped
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();

        assert!(baseline > 0.0, "baseline margin must be positive");
        assert_ne!(
            baseline, with_bump,
            "cnq_vega_weight must change the SIMM margin"
        );
        assert!(with_bump > baseline, "a larger vega weight must raise IM");
    }

    /// Guard against the registry weight going silently inert: doubling the
    /// weight MUST move the margin number.
    #[test]
    fn commodity_vega_weight_is_read_by_the_calculation() {
        let base = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut bumped = base.clone();
        bumped.params.commodity_vega_weight = base.params.commodity_vega_weight * 2.0;

        let sens = credit_commodity_vega_sensitivities();
        let baseline = base
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();
        let with_bump = bumped
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();

        assert!(baseline > 0.0, "baseline margin must be positive");
        assert_ne!(
            baseline, with_bump,
            "commodity_vega_weight must change the SIMM margin"
        );
        assert!(with_bump > baseline, "a larger vega weight must raise IM");
    }

    /// Each vega sensitivity family must move the aggregate margin, proving the
    /// input path reaches the calculation rather than being dropped.
    #[test]
    fn credit_and_commodity_vega_sensitivities_change_total_margin() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut anchor = SimmSensitivities::new(Currency::USD);
        anchor.add_ir_delta(Currency::USD, "5Y", 10_000.0);
        let baseline = calc
            .calculate_from_sensitivities(&anchor, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();

        let mut with_cq = anchor.clone();
        with_cq.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 25_000.0);
        let cq = calc
            .calculate_from_sensitivities(&with_cq, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        assert_ne!(baseline, cq.amount.amount(), "CQ vega must change total IM");
        assert!(cq.breakdown.contains_key("Credit_Qualifying_Vega"));

        let mut with_cnq = anchor.clone();
        with_cnq.add_credit_non_qualifying_vega("RMBS_A", "5Y", 15_000.0);
        let cnq = calc
            .calculate_from_sensitivities(&with_cnq, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        assert_ne!(
            baseline,
            cnq.amount.amount(),
            "CNQ vega must change total IM"
        );
        assert!(cnq.breakdown.contains_key("Credit_NonQualifying_Vega"));

        let mut with_commodity = anchor.clone();
        with_commodity.add_commodity_vega("Crude", 20_000.0);
        let commodity = calc
            .calculate_from_sensitivities(&with_commodity, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        assert_ne!(
            baseline,
            commodity.amount.amount(),
            "commodity vega must change total IM"
        );
        assert!(commodity.breakdown.contains_key("Commodity_Vega"));
    }

    #[test]
    fn zero_credit_and_commodity_vega_contributes_nothing() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut anchor = SimmSensitivities::new(Currency::USD);
        anchor.add_ir_delta(Currency::USD, "5Y", 10_000.0);

        let mut zeroed = anchor.clone();
        zeroed.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 0.0);
        zeroed.add_credit_non_qualifying_vega("RMBS_A", "5Y", 0.0);
        zeroed.add_commodity_vega("Crude", 0.0);

        let baseline = calc
            .calculate_from_sensitivities(&anchor, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();
        let with_zero = calc
            .calculate_from_sensitivities(&zeroed, Currency::USD, test_date())
            .expect("valid sensitivity fixture");

        assert_eq!(
            baseline,
            with_zero.amount.amount(),
            "zero vega must not move the margin"
        );
        assert!(!with_zero.breakdown.contains_key("Credit_Qualifying_Vega"));
        assert!(
            !with_zero
                .breakdown
                .contains_key("Credit_NonQualifying_Vega")
        );
        assert!(!with_zero.breakdown.contains_key("Commodity_Vega"));

        assert_eq!(
            calc.aggregate_credit_qualifying(
                &FxHashMap::default(),
                |_| calc.params.cq_vega_weight,
                false,
                None,
            ),
            0.0
        );
        assert_eq!(
            calc.non_qualifying_margin(&single_credit(0.0), true, None),
            0.0
        );
        assert_eq!(
            calc.aggregate_commodity(&FxHashMap::default(), true, None),
            0.0
        );
    }

    /// A single name / single bucket reduces to `|vega * VRW|` for each of the
    /// three risk classes, mirroring the equivalent single-factor delta cases.
    #[test]
    fn single_factor_credit_and_commodity_vega_match_closed_form() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut cq: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        cq.insert(
            (
                SimmCreditSector::Financial,
                "BANK_A".to_string(),
                "5Y".to_string(),
            ),
            25_000.0,
        );
        let expected_cq = 25_000.0 * calc.params.cq_vega_weight;
        assert!(
            (calc.aggregate_credit_qualifying(&cq, |_| calc.params.cq_vega_weight, false, None)
                - expected_cq)
                .abs()
                < 1e-6
        );

        let expected_cnq = 15_000.0 * calc.params.cnq_vega_weight;
        assert!(
            (calc.non_qualifying_margin(&single_credit(15_000.0), true, None) - expected_cnq).abs()
                < 1e-6
        );
        assert!(
            (calc.non_qualifying_margin(&single_credit(-15_000.0), true, None) - expected_cnq)
                .abs()
                < 1e-6,
            "vega margin is sign-insensitive, matching the delta path"
        );

        let mut commodity: FxHashMap<String, f64> = FxHashMap::default();
        commodity.insert("Crude".to_string(), 20_000.0);
        let expected_commodity = 20_000.0 * 0.74 * calc.params.commodity_vega_weight;
        assert!(
            (calc.aggregate_commodity(&commodity, true, None) - expected_commodity).abs() < 1e-6
        );
    }

    #[test]
    fn registry_ir_tenors_match_the_published_simm_tenor_set() {
        let calc = SimmCalculator::new(SimmVersion::default()).expect("registry should load");
        let mut registry: Vec<&str> = calc
            .params
            .ir_delta_weights
            .keys()
            .map(String::as_str)
            .collect();
        registry.sort_unstable();
        let mut published: Vec<&str> = crate::types::SIMM_TENORS.to_vec();
        published.sort_unstable();
        assert_eq!(registry, published, "registry tenors drifted");
    }

    #[test]
    fn typed_calculation_rejects_unknown_tenor_instead_of_zero_margin() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let as_of = test_date();
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "7Y", 50_000.0);
        let err = calc
            .calculate_from_sensitivities(&sens, Currency::USD, as_of)
            .expect_err("7Y must be rejected");
        assert!(err.to_string().contains("7Y"), "{err}");

        let mut good = SimmSensitivities::new(Currency::USD);
        good.add_ir_delta(Currency::USD, "5Y", 50_000.0);
        let result = calc
            .calculate_from_sensitivities(&good, Currency::USD, as_of)
            .expect("valid tenor prices");
        assert!(result.amount.amount() > 0.0);
        assert_eq!(result.mpor_days, calc.mpor_days());
        assert!(result.breakdown.contains_key("IR_Delta"));
    }

    fn detail_fixture() -> SimmSensitivities {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "2Y", 4.0e9); // breaches the concentration threshold
        sens.add_ir_delta(Currency::USD, "10Y", -2.5e9);
        sens.add_ir_delta(Currency::EUR, "5Y", 60_000.0);
        sens.add_ir_delta(Currency::JPY, "5Y", -45_000.0);
        sens.add_ir_vega(Currency::USD, "5Y", 500_000.0);
        sens.add_ir_vega(Currency::EUR, "10Y", -200_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Financial, "BANK_A", "5Y", 30_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Financial, "BANK_B", "5Y", -12_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Sovereign, "SOV_A", "10Y", 9_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Residual, "UNRATED", "5Y", 2_000.0);
        sens.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 7_000.0);
        sens.add_credit_non_qualifying_delta("RMBS_1", "5Y", 15_000.0);
        sens.add_credit_non_qualifying_delta("RMBS_2", "5Y", -4_000.0);
        sens.add_credit_non_qualifying_vega("RMBS_1", "5Y", 3_000.0);
        sens.add_equity_delta("AAPL", 100_000.0);
        sens.add_equity_delta("MSFT", -70_000.0);
        sens.add_equity_vega("AAPL", 40_000.0);
        sens.add_fx_delta(Currency::EUR, 400_000.0);
        sens.add_fx_delta(Currency::BRL, -150_000.0);
        sens.add_fx_vega(Currency::EUR, Currency::USD, 90_000.0);
        sens.add_fx_vega(Currency::USD, Currency::JPY, -35_000.0);
        sens.add_commodity_delta("2", 80_000.0);
        sens.add_commodity_delta("12", -30_000.0);
        sens.add_commodity_vega("2", 20_000.0);
        sens.add_curvature(SimmCurvatureSensitivity {
            risk_class: SimmRiskClass::InterestRate,
            bucket: "USD".into(),
            factor: "OIS".into(),
            risk_tenor: Some("5Y".into()),
            expiry_tenor: "2W".into(),
            volatility_weighted_vega: 1000.0,
        });
        sens.add_curvature(SimmCurvatureSensitivity {
            risk_class: SimmRiskClass::Equity,
            bucket: "residual".into(),
            factor: "ACME".into(),
            risk_tenor: None,
            expiry_tenor: "2W".into(),
            volatility_weighted_vega: 500.0,
        });
        sens
    }

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0)
    }

    #[test]
    fn simm_detail_rebuilds_breakdown_and_total() {
        let calc = SimmCalculator::new(SimmVersion::V2_6)
            .expect("registry should load")
            .with_mpor(20);
        let sens = detail_fixture();
        let as_of = Date::from_calendar_date(2025, time::Month::January, 15).expect("date");
        let result = calc
            .calculate_from_sensitivities(&sens, Currency::USD, as_of)
            .expect("SIMM");
        let detail = result.simm_detail.as_ref().expect("SIMM detail");

        // One component per breakdown key; margin × MPOR scale is the breakdown amount.
        assert!(close(detail.mpor_scale, 2.0_f64.sqrt()));
        let mut labels: Vec<&str> = detail
            .components
            .iter()
            .map(|c| c.component.as_str())
            .collect();
        labels.sort_unstable();
        assert_eq!(
            labels,
            result
                .breakdown
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            labels.len(),
            14,
            "every delta, vega and curvature component"
        );
        let mut by_class: std::collections::BTreeMap<SimmRiskClass, f64> =
            std::collections::BTreeMap::new();
        for component in &detail.components {
            assert!(close(
                component.margin * detail.mpor_scale,
                result.breakdown[&component.component].amount()
            ));
            *by_class.entry(component.risk_class).or_insert(0.0) += component.margin;

            // Weighted sensitivities replay from their recorded inputs.
            for bucket in &component.buckets {
                let mut sum = 0.0;
                for row in &bucket.weighted_sensitivities {
                    let hvr = if component.component.ends_with("_Curvature") {
                        1.0
                    } else {
                        component.historical_volatility_ratio
                    };
                    assert!(close(
                        row.weighted_sensitivity,
                        row.sensitivity * hvr * row.risk_weight * row.concentration_factor
                    ));
                    assert!(row.concentration_factor >= 1.0);
                    sum += row.weighted_sensitivity;
                }
                assert!(close(bucket.signed_sum, sum.clamp(-bucket.k, bucket.k)));
            }
        }

        // Risk-class margins are the component sums; the total is their correlated norm.
        assert_eq!(
            by_class.keys().collect::<Vec<_>>(),
            detail.risk_class_margins.keys().collect::<Vec<_>>()
        );
        for (class, margin) in &detail.risk_class_margins {
            assert!(close(*margin, by_class[class]));
        }
        let classes: Vec<(SimmRiskClass, f64)> = detail
            .risk_class_margins
            .iter()
            .map(|(class, margin)| (*class, *margin))
            .collect();
        let mut variance = 0.0;
        for (a, k_a) in &classes {
            for (b, k_b) in &classes {
                variance += calc.params.correlation(*a, *b) * k_a * k_b;
            }
        }
        assert!(close(
            variance.sqrt() * detail.mpor_scale,
            result.amount.amount()
        ));

        let component = |label: &str| {
            detail
                .components
                .iter()
                .find(|c| c.component == label)
                .unwrap_or_else(|| panic!("{label} detail"))
        };

        // IR delta: bucket K from the tenor correlations, then the currency aggregation.
        let ir = component("IR_Delta");
        let mut currencies: Vec<&str> = ir.buckets.iter().map(|b| b.bucket.as_str()).collect();
        currencies.sort_unstable();
        assert_eq!(currencies, ["EUR", "JPY", "USD"]);
        let mut ir_buckets = Vec::new();
        for bucket in &ir.buckets {
            let rows = &bucket.weighted_sensitivities;
            let mut k2 = 0.0;
            for a in rows {
                for b in rows {
                    let rho = match (&a.tenor, &b.tenor) {
                        (Some(x), Some(y)) if x == y => 1.0,
                        (Some(x), Some(y)) => {
                            calc.params.ir_tenor_correlations[&ordered_tenor_pair(x, y)]
                        }
                        _ => unreachable!("IR rows carry tenors"),
                    };
                    k2 += rho * a.weighted_sensitivity * b.weighted_sensitivity;
                }
            }
            assert!(close(bucket.k, k2.sqrt()));
            ir_buckets.push((bucket.k, bucket.signed_sum, rows[0].concentration_factor));
        }
        assert!(close(ir.margin, calc.aggregate_ir_currencies(&ir_buckets)));
        let usd = ir
            .buckets
            .iter()
            .find(|b| b.bucket == "USD")
            .expect("USD bucket");
        assert!(
            usd.weighted_sensitivities[0].concentration_factor > 1.0,
            "the USD position breaches its concentration threshold"
        );

        // Equity delta: one bucket per name, no correlation between names.
        let equity = component("Equity_Delta");
        assert_eq!(equity.buckets.len(), 2);
        let equity_k2: f64 = equity.buckets.iter().map(|b| b.k * b.k).sum();
        assert!(close(equity.margin, equity_k2.sqrt()));

        // Commodity delta: one net factor per bucket, correlated across buckets.
        let commodity = component("Commodity_Delta");
        let mut commodity_k2 = 0.0;
        for a in &commodity.buckets {
            for b in &commodity.buckets {
                let rho = if a.bucket == b.bucket {
                    1.0
                } else {
                    calc.params.commodity_inter_bucket_correlation(
                        a.bucket.parse().expect("bucket id"),
                        b.bucket.parse().expect("bucket id"),
                    )
                };
                commodity_k2 += rho * a.signed_sum * b.signed_sum;
            }
        }
        assert!(close(commodity.margin, commodity_k2.sqrt()));

        // Credit qualifying: sector buckets plus the residual bucket added outside the norm.
        let cq = component("Credit_Qualifying_Delta");
        assert_eq!(
            cq.buckets
                .iter()
                .map(|b| b.bucket.as_str())
                .collect::<Vec<_>>(),
            ["sovereign", "financial", "residual"]
        );
        let (sov, fin, residual) = (&cq.buckets[0], &cq.buckets[1], &cq.buckets[2]);
        let gamma = calc
            .params
            .cq_inter_bucket_correlation(SimmCreditSector::Sovereign, SimmCreditSector::Financial);
        let cq_expected = residual.k
            + (sov.k * sov.k + fin.k * fin.k + 2.0 * gamma * sov.signed_sum * fin.signed_sum)
                .sqrt();
        assert!(close(cq.margin, cq_expected));

        // Single-bucket components: the bucket K is the component margin.
        for label in [
            "Credit_NonQualifying_Delta",
            "Credit_NonQualifying_Vega",
            "FX_Delta",
            "FX_Vega",
        ] {
            let single = component(label);
            assert_eq!(single.buckets.len(), 1);
            assert_eq!(single.buckets[0].k, single.margin);
        }
        assert_eq!(
            component("FX_Vega").historical_volatility_ratio,
            calc.params.fx_historical_volatility_ratio
        );

        // The detail survives the wire and is absent for other methodologies.
        let json = serde_json::to_string(&result).expect("serialize");
        let back: ImResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.simm_detail.map(|d| d.components.len()),
            Some(detail.components.len())
        );
        let schedule = ImResult::simple(result.amount, ImMethodology::Schedule, as_of, 10);
        assert!(schedule.simm_detail.is_none());
        assert!(
            !serde_json::to_string(&schedule)
                .expect("serialize")
                .contains("simm_detail")
        );
    }
}

/// Independent transcription of historical ISDA SIMM v2.6 tables.
///
/// Source: https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf
/// PDF SHA256 57e9d2e9e080e27abc92197923b019e40eb2fe60d9c82ce2a0f8238b7c4bf8e0.
/// Reread 2026-09-09. Provenance: tests/fixtures/simm_v26_provenance.md.
mod simm_schedule_parity_tests {
    use super::{SimmCalculator, SimmVersion};
    use crate::types::{SIMM_TENORS, SimmRiskClass};

    #[test]
    fn published_ir_weights_and_correlations() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry");
        let p = calc.get_params();
        // D.1 tables 1-3, printed page 14.
        for (table, expected) in [
            (
                &p.ir_delta_weights,
                [109., 105., 90., 71., 66., 66., 64., 60., 60., 61., 61., 67.],
            ),
            (
                &p.ir_delta_weights_low,
                [15., 18., 9., 11., 13., 15., 19., 23., 23., 22., 22., 23.],
            ),
            (
                &p.ir_delta_weights_high,
                [
                    163., 109., 87., 89., 102., 96., 101., 97., 97., 102., 106., 101.,
                ],
            ),
        ] {
            assert_eq!(table.len(), 12);
            for (tenor, weight) in SIMM_TENORS.iter().zip(expected) {
                assert_eq!(table[*tenor], weight);
            }
        }
        // D.2 paragraph 36, printed pages 14-15, strict upper triangle in tenor order.
        let rows: &[&[f64]] = &[
            &[77., 67., 59., 48., 39., 34., 30., 25., 23., 21., 20.],
            &[84., 74., 56., 43., 36., 31., 26., 21., 19., 19.],
            &[88., 69., 55., 47., 40., 34., 27., 25., 25.],
            &[86., 73., 65., 57., 49., 40., 38., 37.],
            &[94., 87., 79., 68., 60., 57., 55.],
            &[96., 91., 80., 74., 70., 69.],
            &[97., 88., 81., 77., 76.],
            &[95., 90., 86., 85.],
            &[97., 94., 94.],
            &[98., 97.],
            &[99.],
        ];
        assert_eq!(p.ir_tenor_correlations.len(), 66);
        for (i, row) in rows.iter().enumerate() {
            for (offset, percent) in row.iter().enumerate() {
                let a = SIMM_TENORS[i].to_owned();
                let b = SIMM_TENORS[i + offset + 1].to_owned();
                let actual = p
                    .ir_tenor_correlations
                    .get(&(a.clone(), b.clone()))
                    .or_else(|| p.ir_tenor_correlations.get(&(b, a)))
                    .expect("pair");
                assert_eq!(*actual, percent / 100.0);
            }
        }
        assert_eq!(p.ir_inter_currency_correlation, 0.32);
        assert_eq!(p.ir_subcurve_correlation, 0.993);
        assert_eq!(p.ir_historical_volatility_ratio, 0.47);
        assert_eq!(p.ir_vega_weight, 0.23);
    }

    #[test]
    fn published_non_ir_weights_and_historical_ratios() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
        let p = calc.get_params();
        // E through I: selected supported residual classes and every commodity bucket.
        assert_eq!(p.cnq_delta_weight, 1300.0);
        assert_eq!(p.equity_delta_weight, 50.0);
        assert_eq!(p.fx_delta_weight, 7.4);
        assert_eq!(p.fx_high_delta_weight, 14.7);
        assert_eq!(p.cq_vega_weight, 0.76);
        assert_eq!(p.cnq_vega_weight, 0.76);
        assert_eq!(p.equity_vega_weight, 0.45);
        assert_eq!(p.fx_vega_weight, 0.48);
        assert_eq!(p.commodity_vega_weight, 0.55);
        assert_eq!(p.equity_historical_volatility_ratio, 0.60);
        assert_eq!(p.fx_historical_volatility_ratio, 0.57);
        assert_eq!(p.commodity_historical_volatility_ratio, 0.74);
        assert_eq!(p.cq_same_issuer_correlation, 0.93);
        assert_eq!(p.cq_intra_bucket_correlation, 0.46);
        assert_eq!(p.credit_residual_correlation, 0.50);
        assert_eq!(p.fx_intra_bucket_correlation, 0.50);
        assert_eq!(p.fx_regular_high_correlation, 0.25);
        assert_eq!(p.fx_high_high_correlation, -0.05);
        let weights = [
            48., 29., 33., 25., 35., 30., 60., 52., 68., 63., 21., 21., 15., 16., 13., 68., 17.,
        ];
        let correlations = [
            83., 97., 93., 97., 98., 90., 98., 49., 80., 46., 58., 53., 62., 16., 18., 0., 38.,
        ];
        for i in 0..17 {
            let key = (i + 1).to_string();
            assert_eq!(p.commodity_bucket_weights[&key], weights[i]);
            assert_eq!(
                p.commodity_intra_bucket_correlations[&key],
                correlations[i] / 100.0
            );
        }
    }

    #[test]
    fn published_raw_concentration_thresholds() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
        let p = calc.get_params();
        // J, printed pages 26-28. Table units USD millions, per bp/% for delta.
        for (key, delta, vega) in [
            ("high", 30., 74.),
            ("regular_well_traded", 330., 4900.),
            ("regular_less_traded", 130., 520.),
            ("low", 61., 970.),
        ] {
            assert_eq!(p.ir_delta_concentration_thresholds[key], delta * 1e6);
            assert_eq!(p.ir_vega_concentration_thresholds[key], vega * 1e6);
        }
        for (key, threshold) in [("1", 3300.), ("2", 880.), ("3", 170.)] {
            assert_eq!(p.fx_delta_concentration_thresholds[key], threshold * 1e6);
        }
        for (key, threshold) in [
            ("1_1", 2800.),
            ("1_2", 1400.),
            ("1_3", 590.),
            ("2_2", 520.),
            ("2_3", 340.),
            ("3_3", 210.),
        ] {
            assert_eq!(p.fx_vega_concentration_thresholds[key], threshold * 1e6);
        }
        let delta = [
            310., 2100., 1700., 1700., 1700., 2800., 2800., 2700., 2700., 52., 530., 1300., 100.,
            100., 100., 52., 4000.,
        ];
        let vega = [
            390., 2900., 310., 310., 310., 6300., 6300., 1200., 1200., 120., 390., 1300., 590.,
            590., 590., 69., 69.,
        ];
        for i in 0..17 {
            let key = (i + 1).to_string();
            assert_eq!(
                p.commodity_delta_concentration_thresholds[&key],
                delta[i] * 1e6
            );
            assert_eq!(
                p.commodity_vega_concentration_thresholds[&key],
                vega[i] * 1e6
            );
        }
        for (class, delta, vega) in [
            (SimmRiskClass::Equity, 0.37, 39.),
            (SimmRiskClass::CreditNonQualifying, 0.50, 70.),
        ] {
            assert_eq!(p.concentration_thresholds[&class], delta * 1e6);
            assert_eq!(p.vega_concentration_thresholds[&class], vega * 1e6);
        }
        assert_eq!(
            p.vega_concentration_thresholds[&SimmRiskClass::CreditQualifying],
            360e6
        );
    }

    #[test]
    fn published_cross_risk_class_correlations() {
        use SimmRiskClass::*;
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
        let p = calc.get_params();
        let classes = [
            InterestRate,
            CreditQualifying,
            CreditNonQualifying,
            Equity,
            Commodity,
            Fx,
        ];
        let rows: &[&[f64]] = &[
            &[4., 4., 7., 37., 14.],
            &[54., 70., 27., 37.],
            &[46., 24., 15.],
            &[35., 39.],
            &[35.],
        ];
        for (i, row) in rows.iter().enumerate() {
            for (j, percent) in row.iter().enumerate() {
                let a = classes[i];
                let b = classes[i + j + 1];
                assert_eq!(
                    *p.risk_class_correlations
                        .get(&(a, b))
                        .or_else(|| p.risk_class_correlations.get(&(b, a)))
                        .expect("pair"),
                    percent / 100.0
                );
            }
        }
    }
}
