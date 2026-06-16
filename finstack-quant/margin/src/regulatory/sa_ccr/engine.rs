//! SA-CCR engine for computing Exposure at Default.
//!
//! Implements BCBS 279 (March 2014, rev. April 2014):
//!   `EAD = alpha * (RC + PFE)`
//!   where alpha = 1.4

use super::maturity_factor::{maturity_factor_margined, maturity_factor_unmargined};
use super::pfe::pfe;
use super::replacement_cost::replacement_cost;
use super::types::{EadResult, SaCcrNettingSetConfig, SaCcrTrade};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;

/// SA-CCR engine for computing Exposure at Default.
#[derive(Debug)]
pub struct SaCcrEngine {
    /// Alpha multiplier (regulatory: 1.4, supervisory override possible).
    alpha: f64,
    /// Reporting currency.
    #[allow(dead_code)]
    reporting_currency: Currency,
}

impl SaCcrEngine {
    /// Create a builder for configuring the engine.
    #[must_use]
    pub fn builder() -> SaCcrEngineBuilder {
        SaCcrEngineBuilder::default()
    }

    /// Compute EAD for a netting set.
    ///
    /// `EAD = alpha * (RC + PFE)` where `PFE = multiplier * AddOn_aggregate`.
    ///
    /// Each trade is validated via [`SaCcrTrade::validate`] before aggregation
    /// so that direction / supervisory-delta / option-type inconsistencies
    /// surface as a validation error rather than a silently reversed add-on
    /// contribution.
    ///
    /// # Arguments
    ///
    /// * `config` - Netting-set collateral, threshold, MTA, NICA, and MPOR terms.
    /// * `trades` - Derivative trades in the netting set.
    ///
    /// # Returns
    ///
    /// [`EadResult`] with replacement cost, PFE, multiplier, add-on breakdown,
    /// alpha, and a reporting maturity factor.
    ///
    /// # Errors
    ///
    /// Returns an error if any trade fails [`SaCcrTrade::validate`].
    ///
    /// # References
    ///
    /// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
    pub fn calculate_ead(
        &self,
        config: &SaCcrNettingSetConfig,
        trades: &[SaCcrTrade],
    ) -> Result<EadResult> {
        for trade in trades {
            trade.validate()?;
        }
        let rc = replacement_cost(config, trades);
        let (mult, add_on_agg, add_on_by_class) = pfe(config, trades);
        let pfe_value = mult * add_on_agg;
        let ead = self.alpha * (rc + pfe_value);

        // `maturity_factor` on `EadResult` is a single summary number for
        // reporting. For margined sets it is the (per-trade shared)
        // MPOR MF. For unmargined sets the true per-trade MFs are
        // applied inside the add-on; this is a tenor-weighted average
        // for reporting only and is not used in the EAD arithmetic.
        let mf = if config.is_margined {
            maturity_factor_margined(config.mpor_days)
        } else if trades.is_empty() {
            maturity_factor_unmargined(10.0 / 250.0)
        } else {
            let avg_maturity: f64 = trades
                .iter()
                .map(|t| {
                    let days = (t.end_date - config.as_of).whole_days().max(0) as f64;
                    days / 365.0
                })
                .sum::<f64>()
                / trades.len() as f64;
            maturity_factor_unmargined(avg_maturity)
        };

        Ok(EadResult {
            ead,
            rc,
            pfe: pfe_value,
            multiplier: mult,
            add_on_aggregate: add_on_agg,
            add_on_by_asset_class: add_on_by_class,
            alpha: self.alpha,
            maturity_factor: mf,
        })
    }
}

/// Builder for `SaCcrEngine`.
#[derive(Default)]
pub struct SaCcrEngineBuilder {
    alpha: Option<f64>,
    reporting_currency: Option<Currency>,
}

impl SaCcrEngineBuilder {
    /// Override alpha (default: 1.4). Must be >= 1.0.
    #[must_use]
    pub fn alpha(mut self, alpha: f64) -> Self {
        self.alpha = Some(alpha);
        self
    }

    /// Set reporting currency (default: USD).
    #[must_use]
    pub fn reporting_currency(mut self, ccy: Currency) -> Self {
        self.reporting_currency = Some(ccy);
        self
    }

    /// Build the engine, validating configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if alpha is less than 1.0.
    pub fn build(self) -> Result<SaCcrEngine> {
        let alpha = self.alpha.unwrap_or(1.4);
        if alpha < 1.0 {
            return Err(finstack_quant_core::Error::Validation(
                "SA-CCR alpha must be >= 1.0".into(),
            ));
        }
        Ok(SaCcrEngine {
            alpha,
            reporting_currency: self.reporting_currency.unwrap_or(Currency::USD),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regulatory::sa_ccr::types::*;
    use crate::types::NettingSetId;
    use finstack_quant_core::dates::Date;

    fn make_date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(
            year,
            time::Month::try_from(month).expect("valid month"),
            day,
        )
        .expect("valid date")
    }

    fn simple_ir_trade(trade_id: &str, notional: f64, direction: f64, mtm: f64) -> SaCcrTrade {
        SaCcrTrade {
            trade_id: trade_id.to_string(),
            asset_class: SaCcrAssetClass::InterestRate,
            notional,
            start_date: make_date(2024, 1, 15),
            end_date: make_date(2029, 1, 15),
            underlier: "USD".to_string(),
            hedging_set: "USD-IR".to_string(),
            direction,
            supervisory_delta: direction,
            mtm,
            is_option: false,
            option_type: None,
        }
    }

    fn unmargined_config(collateral: f64) -> SaCcrNettingSetConfig {
        SaCcrNettingSetConfig::unmargined(
            NettingSetId::bilateral("BANK_A", "CSA-001"),
            collateral,
            make_date(2024, 1, 15),
        )
    }

    fn margined_config(
        collateral: f64,
        threshold: f64,
        mta: f64,
        nica: f64,
    ) -> SaCcrNettingSetConfig {
        SaCcrNettingSetConfig::margined(
            NettingSetId::bilateral("BANK_A", "CSA-001"),
            collateral,
            threshold,
            mta,
            nica,
            10,
            make_date(2024, 1, 15),
        )
    }

    // -----------------------------------------------------------------------
    // Builder tests
    // -----------------------------------------------------------------------

    #[test]
    fn builder_default() {
        let engine = SaCcrEngine::builder().build().expect("default build");
        assert!((engine.alpha - 1.4).abs() < 1e-10);
    }

    #[test]
    fn builder_custom_alpha() {
        let engine = SaCcrEngine::builder()
            .alpha(1.5)
            .build()
            .expect("custom alpha");
        assert!((engine.alpha - 1.5).abs() < 1e-10);
    }

    #[test]
    fn builder_rejects_low_alpha() {
        let err = SaCcrEngine::builder()
            .alpha(0.9)
            .build()
            .expect_err("low alpha");
        assert!(err.to_string().contains("alpha"));
    }

    // -----------------------------------------------------------------------
    // Empty netting set
    // -----------------------------------------------------------------------

    #[test]
    fn empty_netting_set_zero_ead() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = unmargined_config(0.0);
        let result = engine.calculate_ead(&config, &[]).expect("calculate");
        assert!(
            result.ead.abs() < 1e-10,
            "empty netting set: EAD = {}",
            result.ead
        );
        assert!(result.rc.abs() < 1e-10);
    }

    // -----------------------------------------------------------------------
    // Replacement cost: unmargined
    // -----------------------------------------------------------------------

    #[test]
    fn rc_unmargined_positive_mtm_no_collateral() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = unmargined_config(0.0);
        let trade = simple_ir_trade("T1", 100_000_000.0, 1.0, 2_500_000.0);
        let result = engine.calculate_ead(&config, &[trade]).expect("calculate");
        // RC = max(V - C, 0) = max(2.5M - 0, 0) = 2.5M.
        assert!(
            (result.rc - 2_500_000.0).abs() < 1.0,
            "RC unmargined: expected 2.5M, got {}",
            result.rc
        );
    }

    #[test]
    fn rc_unmargined_over_collateralized() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = unmargined_config(5_000_000.0);
        let trade = simple_ir_trade("T1", 100_000_000.0, 1.0, 2_500_000.0);
        let result = engine.calculate_ead(&config, &[trade]).expect("calculate");
        // RC = max(2.5M - 5M, 0) = 0.
        assert!(
            result.rc.abs() < 1e-10,
            "RC over-collateralized: expected 0, got {}",
            result.rc
        );
    }

    // -----------------------------------------------------------------------
    // Replacement cost: margined
    // -----------------------------------------------------------------------

    #[test]
    fn rc_margined_uses_margin_terms() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = margined_config(5_000_000.0, 0.0, 500_000.0, 1_000_000.0);
        let trade = simple_ir_trade("T1", 100_000_000.0, 1.0, 2_500_000.0);
        let result = engine.calculate_ead(&config, &[trade]).expect("calculate");
        // RC_margined = max(V - (VM + NICA), TH + MTA - NICA, 0)
        //             = max(2.5M - (5M + 1M), 0 + 0.5M - 1M, 0)
        //             = max(-3.5M, -0.5M, 0) = 0.
        assert!(
            result.rc.abs() < 1e-10,
            "RC margined over-collateralized: expected 0, got {}",
            result.rc
        );
    }

    /// NICA must reduce the multiplier input `V - C` (C = VM + NICA).
    /// Previously the multiplier ignored NICA, so a netting set holding
    /// substantial independent collateral still reported PFE as if the
    /// independent amount had no offsetting effect.
    #[test]
    fn nica_reduces_pfe_multiplier() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let trade = simple_ir_trade("T1", 100_000_000.0, 1.0, 2_500_000.0);

        let config_no_nica = margined_config(5_000_000.0, 0.0, 500_000.0, 0.0);
        let config_with_nica = margined_config(5_000_000.0, 0.0, 500_000.0, 3_000_000.0);

        let result_no = engine
            .calculate_ead(&config_no_nica, std::slice::from_ref(&trade))
            .expect("no NICA");
        let result_with = engine
            .calculate_ead(&config_with_nica, std::slice::from_ref(&trade))
            .expect("with NICA");

        // Adding NICA reduces V - C (more negative), which pushes the
        // multiplier further below 1 and therefore reduces PFE.
        assert!(
            result_with.multiplier <= result_no.multiplier,
            "NICA should not increase multiplier: no_nica={} with_nica={}",
            result_no.multiplier,
            result_with.multiplier,
        );
        assert!(
            result_with.pfe <= result_no.pfe,
            "NICA should not increase PFE: no_nica={} with_nica={}",
            result_no.pfe,
            result_with.pfe,
        );
    }

    // -----------------------------------------------------------------------
    // PFE multiplier
    // -----------------------------------------------------------------------

    #[test]
    fn multiplier_at_floor_when_over_collateralized() {
        use crate::regulatory::sa_ccr::pfe::multiplier;
        // When V - C is very negative relative to AddOn, multiplier -> floor.
        let mult = multiplier(-1_000_000.0, 100_000.0);
        assert!(mult > 0.04, "multiplier should be > floor");
        assert!(mult < 1.0, "multiplier should be < 1.0");
    }

    #[test]
    fn multiplier_is_one_when_uncollateralized() {
        use crate::regulatory::sa_ccr::pfe::multiplier;
        // When V - C > 0, multiplier = 1.0.
        let mult = multiplier(1_000_000.0, 100_000.0);
        assert!(
            (mult - 1.0).abs() < 1e-10,
            "multiplier = 1.0 when V > C, got {}",
            mult
        );
    }

    #[test]
    fn multiplier_floor_when_zero_addon() {
        use crate::regulatory::sa_ccr::pfe::multiplier;
        let mult = multiplier(0.0, 0.0);
        assert!((mult - 0.05).abs() < 1e-10, "multiplier floor = 0.05");
    }

    // -----------------------------------------------------------------------
    // Maturity factor
    // -----------------------------------------------------------------------

    #[test]
    fn maturity_factor_unmargined_one_year() {
        use crate::regulatory::sa_ccr::maturity_factor::maturity_factor_unmargined;
        let mf = maturity_factor_unmargined(1.0);
        assert!((mf - 1.0).abs() < 1e-10, "MF(1Y) = 1.0");
    }

    #[test]
    fn maturity_factor_unmargined_capped() {
        use crate::regulatory::sa_ccr::maturity_factor::maturity_factor_unmargined;
        let mf = maturity_factor_unmargined(5.0);
        // min(5, 1) = 1 => sqrt(1) = 1.0.
        assert!((mf - 1.0).abs() < 1e-10, "MF(5Y) capped at 1.0");
    }

    #[test]
    fn maturity_factor_unmargined_floor() {
        use crate::regulatory::sa_ccr::maturity_factor::maturity_factor_unmargined;
        let mf = maturity_factor_unmargined(0.0);
        // Floor: max(0, 10/250) = 0.04 => sqrt(0.04) = 0.2.
        assert!((mf - 0.2).abs() < 1e-4, "MF floor = 0.2, got {}", mf);
    }

    #[test]
    fn maturity_factor_margined_10d() {
        use crate::regulatory::sa_ccr::maturity_factor::maturity_factor_margined;
        let mf = maturity_factor_margined(10);
        // 1.5 * sqrt(10/250) = 1.5 * sqrt(0.04) = 1.5 * 0.2 = 0.3.
        assert!(
            (mf - 0.3).abs() < 1e-4,
            "MF margined(10d) = 0.3, got {}",
            mf
        );
    }

    // -----------------------------------------------------------------------
    // EAD structure
    // -----------------------------------------------------------------------

    #[test]
    fn ead_alpha_factor() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = unmargined_config(0.0);
        let trade = simple_ir_trade("T1", 100_000_000.0, 1.0, 2_500_000.0);
        let result = engine.calculate_ead(&config, &[trade]).expect("calculate");
        assert!((result.alpha - 1.4).abs() < 1e-10);
        // EAD = 1.4 * (RC + PFE).
        let expected_ead = 1.4 * (result.rc + result.pfe);
        assert!(
            (result.ead - expected_ead).abs() < 1.0,
            "EAD = alpha*(RC+PFE): expected {expected_ead}, got {}",
            result.ead
        );
    }

    #[test]
    fn ead_positive_for_single_trade() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = unmargined_config(0.0);
        let trade = simple_ir_trade("T1", 100_000_000.0, 1.0, 2_500_000.0);
        let result = engine.calculate_ead(&config, &[trade]).expect("calculate");
        assert!(result.ead > 0.0, "EAD should be positive");
        assert!(result.pfe > 0.0, "PFE should be positive");
    }

    // -----------------------------------------------------------------------
    // Margined vs. unmargined comparison
    // -----------------------------------------------------------------------

    #[test]
    fn margined_lower_ead_than_unmargined() {
        let engine = SaCcrEngine::builder().build().expect("build");

        let trades = vec![
            simple_ir_trade("T1", 100_000_000.0, 1.0, 2_500_000.0),
            simple_ir_trade("T2", 50_000_000.0, -1.0, -500_000.0),
        ];

        // Unmargined with zero collateral.
        let unmargined = unmargined_config(0.0);
        let result_u = engine
            .calculate_ead(&unmargined, &trades)
            .expect("unmargined");

        // Margined with matching collateral.
        let margined = margined_config(2_000_000.0, 0.0, 100_000.0, 500_000.0);
        let result_m = engine.calculate_ead(&margined, &trades).expect("margined");

        assert!(
            result_m.ead < result_u.ead,
            "margined EAD ({}) should be < unmargined EAD ({})",
            result_m.ead,
            result_u.ead
        );
    }

    // -----------------------------------------------------------------------
    // Add-on by asset class
    // -----------------------------------------------------------------------

    #[test]
    fn add_on_only_for_present_asset_classes() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = unmargined_config(0.0);
        let trade = simple_ir_trade("T1", 100_000_000.0, 1.0, 1_000_000.0);
        let result = engine.calculate_ead(&config, &[trade]).expect("calculate");

        // Only IR should have an add-on.
        assert!(
            result
                .add_on_by_asset_class
                .contains_key(&SaCcrAssetClass::InterestRate),
            "IR add-on present"
        );
        assert!(
            !result
                .add_on_by_asset_class
                .contains_key(&SaCcrAssetClass::Equity),
            "Equity add-on absent"
        );
    }

    // -----------------------------------------------------------------------
    // Multiple asset classes
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_asset_classes_aggregate() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = unmargined_config(0.0);

        let ir_trade = simple_ir_trade("T1", 100_000_000.0, 1.0, 1_000_000.0);

        let fx_trade = SaCcrTrade {
            trade_id: "T2".to_string(),
            asset_class: SaCcrAssetClass::ForeignExchange,
            notional: 50_000_000.0,
            start_date: make_date(2024, 1, 15),
            end_date: make_date(2025, 1, 15),
            underlier: "EURUSD".to_string(),
            hedging_set: "EURUSD".to_string(),
            direction: 1.0,
            supervisory_delta: 1.0,
            mtm: 500_000.0,
            is_option: false,
            option_type: None,
        };

        let result = engine
            .calculate_ead(&config, &[ir_trade, fx_trade])
            .expect("calculate");

        assert!(
            result.add_on_by_asset_class.len() >= 2,
            "should have add-ons for at least 2 asset classes"
        );
        // Aggregate add-on = sum of individual add-ons.
        let sum: f64 = result.add_on_by_asset_class.values().sum();
        assert!(
            (result.add_on_aggregate - sum).abs() < 1.0,
            "aggregate add-on = sum of per-class"
        );
    }

    // -----------------------------------------------------------------------
    // Property: EAD >= 0
    // -----------------------------------------------------------------------

    /// Invalid trades must surface at the `calculate_ead` boundary
    /// rather than silently miscontribute to the add-on via a sign-
    /// flipped adjusted notional.
    #[test]
    fn calculate_ead_rejects_trade_with_supervisory_delta_sign_mismatch() {
        let engine = SaCcrEngine::builder().build().expect("build");
        let config = unmargined_config(0.0);
        let mut bad = simple_ir_trade("BAD", 100_000_000.0, 1.0, 0.0);
        // Linear long direction with short supervisory_delta: caller bug.
        bad.supervisory_delta = -1.0;
        let err = engine
            .calculate_ead(&config, &[bad])
            .expect_err("sign mismatch must bubble up at the engine boundary");
        let msg = err.to_string();
        assert!(
            msg.contains("BAD") && msg.contains("agree in sign"),
            "expected engine boundary error: {msg}"
        );
    }

    #[test]
    fn ead_always_non_negative() {
        let engine = SaCcrEngine::builder().build().expect("build");

        // Trade with negative MTM and large collateral.
        let config = unmargined_config(10_000_000.0);
        let trade = simple_ir_trade("T1", 100_000_000.0, -1.0, -5_000_000.0);
        let result = engine.calculate_ead(&config, &[trade]).expect("calculate");
        assert!(
            result.ead >= 0.0,
            "EAD must be non-negative, got {}",
            result.ead
        );
    }
}
