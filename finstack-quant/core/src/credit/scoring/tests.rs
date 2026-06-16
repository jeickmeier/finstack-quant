//! Tests for credit scoring models.

#[cfg(test)]
mod altman_tests {
    use crate::credit::scoring::{
        altman_z_double_prime, altman_z_prime, altman_z_score, AltmanZDoublePrimeInput,
        AltmanZPrimeInput, AltmanZScoreInput, CreditScoringError, ScoringZone,
    };

    /// Textbook example: healthy manufacturing firm.
    /// Z = 1.2(0.10) + 1.4(0.20) + 3.3(0.15) + 0.6(1.50) + 1.0(1.80)
    ///   = 0.12 + 0.28 + 0.495 + 0.90 + 1.80 = 3.595
    #[test]
    fn z_score_healthy_firm() {
        let input = AltmanZScoreInput {
            working_capital_to_total_assets: 0.10,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: 0.15,
            market_equity_to_total_liabilities: 1.50,
            sales_to_total_assets: 1.80,
        };
        let result = altman_z_score(&input).unwrap();
        assert!(
            (result.score - 3.595).abs() < 1e-10,
            "score={}",
            result.score
        );
        assert_eq!(result.zone, ScoringZone::Safe);
        assert!(result.implied_pd < 0.01);
        assert_eq!(result.model, "Altman Z-Score (1968)");
    }

    /// Distressed firm: negative working capital, low earnings, high leverage.
    /// Z = 1.2(-0.15) + 1.4(-0.10) + 3.3(-0.05) + 0.6(0.20) + 1.0(0.50)
    ///   = -0.18 + -0.14 + -0.165 + 0.12 + 0.50 = 0.135
    #[test]
    fn z_score_distressed_firm() {
        let input = AltmanZScoreInput {
            working_capital_to_total_assets: -0.15,
            retained_earnings_to_total_assets: -0.10,
            ebit_to_total_assets: -0.05,
            market_equity_to_total_liabilities: 0.20,
            sales_to_total_assets: 0.50,
        };
        let result = altman_z_score(&input).unwrap();
        assert!(
            (result.score - 0.135).abs() < 1e-10,
            "score={}",
            result.score
        );
        assert_eq!(result.zone, ScoringZone::Distress);
        assert!(result.implied_pd > 0.50);
    }

    /// Grey zone: borderline firm.
    /// Z = 1.2(0.05) + 1.4(0.10) + 3.3(0.08) + 0.6(0.80) + 1.0(1.00)
    ///   = 0.06 + 0.14 + 0.264 + 0.48 + 1.00 = 1.944
    #[test]
    fn z_score_grey_zone() {
        let input = AltmanZScoreInput {
            working_capital_to_total_assets: 0.05,
            retained_earnings_to_total_assets: 0.10,
            ebit_to_total_assets: 0.08,
            market_equity_to_total_liabilities: 0.80,
            sales_to_total_assets: 1.00,
        };
        let result = altman_z_score(&input).unwrap();
        assert!(
            (result.score - 1.944).abs() < 1e-10,
            "score={}",
            result.score
        );
        assert_eq!(result.zone, ScoringZone::Grey);
        assert!(result.implied_pd > 0.01 && result.implied_pd < 0.50);
    }

    /// Zone boundary: score just above safe threshold (> 2.99) is Safe.
    #[test]
    fn z_score_above_safe_boundary() {
        let input = AltmanZScoreInput {
            working_capital_to_total_assets: 0.10,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: 0.15,
            market_equity_to_total_liabilities: 1.50,
            sales_to_total_assets: 1.30, // Z = 3.095 > 2.99
        };
        let result = altman_z_score(&input).unwrap();
        assert_eq!(result.zone, ScoringZone::Safe);
    }

    /// Zone boundary: score just below distress threshold (< 1.81) is Distress.
    #[test]
    fn z_score_below_distress_boundary() {
        let input = AltmanZScoreInput {
            working_capital_to_total_assets: 0.10,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: 0.15,
            market_equity_to_total_liabilities: 1.50,
            sales_to_total_assets: 0.0, // Z = 1.795 < 1.81
        };
        let result = altman_z_score(&input).unwrap();
        assert_eq!(result.zone, ScoringZone::Distress);
    }

    #[test]
    fn z_score_rejects_nan() {
        let input = AltmanZScoreInput {
            working_capital_to_total_assets: f64::NAN,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: 0.15,
            market_equity_to_total_liabilities: 1.50,
            sales_to_total_assets: 1.80,
        };
        let err = altman_z_score(&input).unwrap_err();
        assert!(matches!(err, CreditScoringError::NonFiniteInput { .. }));
    }

    #[test]
    fn z_score_rejects_infinity() {
        let input = AltmanZScoreInput {
            working_capital_to_total_assets: 0.10,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: f64::INFINITY,
            market_equity_to_total_liabilities: 1.50,
            sales_to_total_assets: 1.80,
        };
        let err = altman_z_score(&input).unwrap_err();
        assert!(matches!(err, CreditScoringError::NonFiniteInput { .. }));
    }

    /// Z'-Score for a private firm.
    /// Z' = 0.717(0.10) + 0.847(0.20) + 3.107(0.15) + 0.420(1.00) + 0.998(1.50)
    ///    = 0.0717 + 0.1694 + 0.46605 + 0.42 + 1.497 = 2.62415
    #[test]
    fn z_prime_private_firm() {
        let input = AltmanZPrimeInput {
            working_capital_to_total_assets: 0.10,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: 0.15,
            book_equity_to_total_liabilities: 1.00,
            sales_to_total_assets: 1.50,
        };
        let result = altman_z_prime(&input).unwrap();
        assert!(
            (result.score - 2.62415).abs() < 1e-10,
            "score={}",
            result.score
        );
        assert_eq!(result.zone, ScoringZone::Grey);
    }

    /// Z'-Score safe zone classification.
    #[test]
    fn z_prime_safe_zone() {
        let input = AltmanZPrimeInput {
            working_capital_to_total_assets: 0.20,
            retained_earnings_to_total_assets: 0.30,
            ebit_to_total_assets: 0.20,
            book_equity_to_total_liabilities: 2.00,
            sales_to_total_assets: 2.00,
        };
        let result = altman_z_prime(&input).unwrap();
        assert!(result.score > 2.90);
        assert_eq!(result.zone, ScoringZone::Safe);
    }

    /// Z''-Score for a healthy non-manufacturer (constant-free non-EM model;
    /// the +3.25 EM-Score constant was removed per
    ///  Major — credit).
    /// Z'' = 6.56(0.10) + 3.26(0.20) + 6.72(0.15) + 1.05(1.00)
    ///     = 0.656 + 0.652 + 1.008 + 1.05 = 3.366
    #[test]
    fn z_double_prime_non_manufacturer() {
        let input = AltmanZDoublePrimeInput {
            working_capital_to_total_assets: 0.10,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: 0.15,
            book_equity_to_total_liabilities: 1.00,
        };
        let result = altman_z_double_prime(&input).unwrap();
        assert!(
            (result.score - 3.366).abs() < 1e-10,
            "score={}",
            result.score
        );
        assert_eq!(result.zone, ScoringZone::Safe);
    }

    /// Z''-Score distress zone.
    #[test]
    fn z_double_prime_distress() {
        let input = AltmanZDoublePrimeInput {
            working_capital_to_total_assets: -0.30,
            retained_earnings_to_total_assets: -0.20,
            ebit_to_total_assets: -0.10,
            book_equity_to_total_liabilities: -0.50,
        };
        let result = altman_z_double_prime(&input).unwrap();
        assert!(result.score < 1.10, "score={}", result.score);
        assert_eq!(result.zone, ScoringZone::Distress);
    }

    /// Regression: a firm with all ratios at zero must NOT be classified
    /// Safe. Under the old +3.25-constant/non-EM-cutoff mix it scored
    /// 3.25 -> Safe ().
    #[test]
    fn z_double_prime_all_zero_ratios_is_distress() {
        let input = AltmanZDoublePrimeInput {
            working_capital_to_total_assets: 0.0,
            retained_earnings_to_total_assets: 0.0,
            ebit_to_total_assets: 0.0,
            book_equity_to_total_liabilities: 0.0,
        };
        let result = altman_z_double_prime(&input).unwrap();
        assert!(result.score.abs() < 1e-12, "score={}", result.score);
        assert_eq!(result.zone, ScoringZone::Distress);
        assert!(result.implied_pd > 0.5, "implied_pd={}", result.implied_pd);
    }

    /// Implied PD is always in [0, 1].
    #[test]
    fn implied_pd_bounds() {
        // Very safe firm
        let safe_input = AltmanZScoreInput {
            working_capital_to_total_assets: 0.50,
            retained_earnings_to_total_assets: 0.60,
            ebit_to_total_assets: 0.40,
            market_equity_to_total_liabilities: 5.00,
            sales_to_total_assets: 3.00,
        };
        let safe_result = altman_z_score(&safe_input).unwrap();
        assert!(safe_result.implied_pd >= 0.0 && safe_result.implied_pd <= 1.0);

        // Very distressed firm
        let dist_input = AltmanZScoreInput {
            working_capital_to_total_assets: -1.00,
            retained_earnings_to_total_assets: -1.00,
            ebit_to_total_assets: -0.50,
            market_equity_to_total_liabilities: 0.01,
            sales_to_total_assets: 0.10,
        };
        let dist_result = altman_z_score(&dist_input).unwrap();
        assert!(dist_result.implied_pd >= 0.0 && dist_result.implied_pd <= 1.0);
    }
}

#[cfg(test)]
mod ohlson_tests {
    use crate::credit::scoring::{ohlson_o_score, CreditScoringError, OhlsonOScoreInput};

    /// Ohlson O-Score: healthy firm with low leverage and good profitability.
    /// Should produce a low O-score (low PD).
    #[test]
    fn o_score_healthy_firm() {
        let input = OhlsonOScoreInput {
            log_total_assets_adjusted: 8.0, // ~$3B in assets
            total_liabilities_to_total_assets: 0.40,
            working_capital_to_total_assets: 0.20,
            current_liabilities_to_current_assets: 0.50,
            liabilities_exceed_assets: 0.0,
            net_income_to_total_assets: 0.10,
            funds_from_operations_to_total_liabilities: 0.30,
            negative_net_income_two_years: 0.0,
            net_income_change: 0.10,
        };
        let result = ohlson_o_score(&input).unwrap();
        // O-score should be negative (safe), PD should be low
        assert!(result.implied_pd < 0.50, "pd={}", result.implied_pd);
    }

    /// Ohlson O-Score: distressed firm with high leverage and losses.
    #[test]
    fn o_score_distressed_firm() {
        let input = OhlsonOScoreInput {
            log_total_assets_adjusted: 4.0, // small firm
            total_liabilities_to_total_assets: 0.90,
            working_capital_to_total_assets: -0.10,
            current_liabilities_to_current_assets: 2.0,
            liabilities_exceed_assets: 0.0,
            net_income_to_total_assets: -0.15,
            funds_from_operations_to_total_liabilities: -0.05,
            negative_net_income_two_years: 1.0,
            net_income_change: -0.50,
        };
        let result = ohlson_o_score(&input).unwrap();
        assert!(result.implied_pd > 0.50, "pd={}", result.implied_pd);
        // PD far above Ohlson's optimal cutoff P* = 0.038 → Distress
        // (PD-based zones per the ).
        assert_eq!(result.zone, crate::credit::scoring::ScoringZone::Distress);
    }

    /// Zone cutoffs are PD-based : Distress
    /// boundary at Ohlson's published optimal cutoff P* = 0.038 (O ≈ −3.23),
    /// Safe/Grey boundary at P*/2 = 0.019. The previous raw-O cutoffs
    /// {0.38, 0.50} labeled PD ≈ 59% firms "Safe".
    #[test]
    fn o_score_pd_based_zone_cutoffs() {
        use crate::credit::scoring::ScoringZone;

        let base = OhlsonOScoreInput {
            log_total_assets_adjusted: 8.0,
            total_liabilities_to_total_assets: 0.40,
            working_capital_to_total_assets: 0.20,
            current_liabilities_to_current_assets: 0.50,
            liabilities_exceed_assets: 0.0,
            net_income_to_total_assets: 0.10,
            funds_from_operations_to_total_liabilities: 0.30,
            negative_net_income_two_years: 0.0,
            net_income_change: 0.10,
        };

        // O ≈ −3.250 → PD ≈ 0.0373 ∈ [0.019, 0.038] → Grey.
        let grey = ohlson_o_score(&base).unwrap();
        assert!(
            grey.implied_pd > 0.019 && grey.implied_pd < 0.038,
            "pd={}",
            grey.implied_pd
        );
        assert_eq!(grey.zone, ScoringZone::Grey);

        // Larger firm: O ≈ −4.064 → PD ≈ 0.0169 < 0.019 → Safe.
        let safe = ohlson_o_score(&OhlsonOScoreInput {
            log_total_assets_adjusted: 10.0,
            ..base
        })
        .unwrap();
        assert!(safe.implied_pd < 0.019, "pd={}", safe.implied_pd);
        assert_eq!(safe.zone, ScoringZone::Safe);

        // More levered firm: O ≈ −2.045 → PD ≈ 0.115 > 0.038 → Distress
        // (the old raw-O cutoffs would have labeled this O < 0.38 "Safe").
        let distress = ohlson_o_score(&OhlsonOScoreInput {
            total_liabilities_to_total_assets: 0.60,
            ..base
        })
        .unwrap();
        assert!(distress.implied_pd > 0.038, "pd={}", distress.implied_pd);
        assert!(distress.score < 0.38, "score={}", distress.score);
        assert_eq!(distress.zone, ScoringZone::Distress);
    }

    /// PD is always in [0, 1] for the logistic transform.
    #[test]
    fn o_score_pd_bounds() {
        // Extreme inputs
        let input = OhlsonOScoreInput {
            log_total_assets_adjusted: 20.0,
            total_liabilities_to_total_assets: 0.01,
            working_capital_to_total_assets: 0.50,
            current_liabilities_to_current_assets: 0.10,
            liabilities_exceed_assets: 0.0,
            net_income_to_total_assets: 0.50,
            funds_from_operations_to_total_liabilities: 1.0,
            negative_net_income_two_years: 0.0,
            net_income_change: 0.90,
        };
        let result = ohlson_o_score(&input).unwrap();
        assert!(result.implied_pd >= 0.0 && result.implied_pd <= 1.0);
    }

    /// Verify coefficient signs: higher leverage should increase PD.
    #[test]
    fn o_score_leverage_monotonicity() {
        let base = OhlsonOScoreInput {
            log_total_assets_adjusted: 6.0,
            total_liabilities_to_total_assets: 0.40,
            working_capital_to_total_assets: 0.10,
            current_liabilities_to_current_assets: 0.80,
            liabilities_exceed_assets: 0.0,
            net_income_to_total_assets: 0.05,
            funds_from_operations_to_total_liabilities: 0.15,
            negative_net_income_two_years: 0.0,
            net_income_change: 0.0,
        };
        let result_low = ohlson_o_score(&base).unwrap();

        let high_leverage = OhlsonOScoreInput {
            total_liabilities_to_total_assets: 0.90,
            ..base
        };
        let result_high = ohlson_o_score(&high_leverage).unwrap();

        assert!(
            result_high.implied_pd > result_low.implied_pd,
            "Higher leverage should increase PD: low={}, high={}",
            result_low.implied_pd,
            result_high.implied_pd
        );
    }

    #[test]
    fn o_score_rejects_nan() {
        let input = OhlsonOScoreInput {
            log_total_assets_adjusted: f64::NAN,
            total_liabilities_to_total_assets: 0.40,
            working_capital_to_total_assets: 0.10,
            current_liabilities_to_current_assets: 0.80,
            liabilities_exceed_assets: 0.0,
            net_income_to_total_assets: 0.05,
            funds_from_operations_to_total_liabilities: 0.15,
            negative_net_income_two_years: 0.0,
            net_income_change: 0.0,
        };
        let err = ohlson_o_score(&input).unwrap_err();
        assert!(matches!(err, CreditScoringError::NonFiniteInput { .. }));
    }
}

#[cfg(test)]
mod zmijewski_tests {
    use crate::credit::scoring::{
        zmijewski_score, CreditScoringError, ScoringZone, ZmijewskiInput,
    };

    /// Zmijewski score: healthy firm (positive ROA, moderate leverage, good liquidity).
    /// Y = -4.336 - 4.513(0.10) + 5.679(0.40) + 0.004(2.0)
    ///   = -4.336 - 0.4513 + 2.2716 + 0.008 = -2.5077
    /// PD = Phi(-2.5077) ~ 0.006
    #[test]
    fn zmijewski_healthy_firm() {
        let input = ZmijewskiInput {
            net_income_to_total_assets: 0.10,
            total_liabilities_to_total_assets: 0.40,
            current_assets_to_current_liabilities: 2.0,
        };
        let result = zmijewski_score(&input).unwrap();
        let expected_y = -4.336 - 4.513 * 0.10 + 5.679 * 0.40 + 0.004 * 2.0;
        assert!(
            (result.score - expected_y).abs() < 1e-10,
            "score={}, expected={}",
            result.score,
            expected_y
        );
        assert_eq!(result.zone, ScoringZone::Safe);
        assert!(result.implied_pd < 0.10, "pd={}", result.implied_pd);
    }

    /// Zmijewski score: distressed firm (negative ROA, high leverage).
    /// Y = -4.336 - 4.513(-0.10) + 5.679(0.90) + 0.004(0.50)
    ///   = -4.336 + 0.4513 + 5.1111 + 0.002 = 1.2284
    /// PD = Phi(1.2284) ~ 0.89
    #[test]
    fn zmijewski_distressed_firm() {
        let input = ZmijewskiInput {
            net_income_to_total_assets: -0.10,
            total_liabilities_to_total_assets: 0.90,
            current_assets_to_current_liabilities: 0.50,
        };
        let result = zmijewski_score(&input).unwrap();
        assert_eq!(result.zone, ScoringZone::Distress);
        assert!(result.implied_pd > 0.50, "pd={}", result.implied_pd);
    }

    /// PD is always in [0, 1] for probit transform.
    #[test]
    fn zmijewski_pd_bounds() {
        let extreme = ZmijewskiInput {
            net_income_to_total_assets: -1.0,
            total_liabilities_to_total_assets: 1.0,
            current_assets_to_current_liabilities: 0.01,
        };
        let result = zmijewski_score(&extreme).unwrap();
        assert!(result.implied_pd >= 0.0 && result.implied_pd <= 1.0);
    }

    /// Verify coefficient sign: higher leverage increases PD.
    #[test]
    fn zmijewski_leverage_monotonicity() {
        let low = ZmijewskiInput {
            net_income_to_total_assets: 0.05,
            total_liabilities_to_total_assets: 0.30,
            current_assets_to_current_liabilities: 1.5,
        };
        let high = ZmijewskiInput {
            total_liabilities_to_total_assets: 0.80,
            ..low
        };
        let pd_low = zmijewski_score(&low).unwrap().implied_pd;
        let pd_high = zmijewski_score(&high).unwrap().implied_pd;
        assert!(
            pd_high > pd_low,
            "Higher leverage should increase PD: low={}, high={}",
            pd_low,
            pd_high
        );
    }

    #[test]
    fn zmijewski_rejects_nan() {
        let input = ZmijewskiInput {
            net_income_to_total_assets: f64::NAN,
            total_liabilities_to_total_assets: 0.40,
            current_assets_to_current_liabilities: 2.0,
        };
        let err = zmijewski_score(&input).unwrap_err();
        assert!(matches!(err, CreditScoringError::NonFiniteInput { .. }));
    }

    #[test]
    fn zmijewski_rejects_neg_infinity() {
        let input = ZmijewskiInput {
            net_income_to_total_assets: 0.05,
            total_liabilities_to_total_assets: f64::NEG_INFINITY,
            current_assets_to_current_liabilities: 2.0,
        };
        let err = zmijewski_score(&input).unwrap_err();
        assert!(matches!(err, CreditScoringError::NonFiniteInput { .. }));
    }
}
