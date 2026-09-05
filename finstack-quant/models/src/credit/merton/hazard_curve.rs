use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::{Error, InputError, Result};

use super::{AssetDynamics, MertonModel};

impl MertonModel {
    /// Generate a [`HazardCurve`] compatible with existing pricing engines.
    ///
    /// Converts structural model default probabilities to piecewise-constant
    /// hazard rates at the specified tenor grid.
    ///
    /// # Measure
    ///
    /// The curve carries **risk-neutral** hazard rates, because it is built
    /// from [`default_probability`](Self::default_probability). That is what
    /// pricing engines want; it is not a physical default-intensity curve.
    ///
    /// # Algorithm
    ///
    /// 1. Compute survival probability S(t) = 1 - PD(t) at each tenor.
    /// 2. Back out piecewise-constant hazard rates between consecutive tenors:
    ///    - λ_0 = -ln(S(t_0)) / t_0
    ///    - λ_i = -ln(S(t_i) / S(t_{i-1})) / (t_i - t_{i-1}) for i >= 1
    /// 3. Build via `HazardCurve::builder`.
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier assigned to the resulting [`HazardCurve`],
    ///   used as the lookup key in a market context
    /// * `base_date` - Valuation date the curve's year fractions are measured
    ///   from
    /// * `tenors` - Tenor grid in years from `base_date`. Must be non-empty
    ///   and strictly positive, and must be distinct; it need not be sorted,
    ///   as it is sorted internally
    /// * `recovery` - Recovery rate stored on the curve as a decimal fraction
    ///   of notional (`0.40` is the senior-unsecured market convention). Must
    ///   lie in `[0, 1]`; for `CreditGrades` dynamics it must equal the
    ///   model's own `mean_recovery`, since that value already determines the
    ///   barrier and a different recovery here would price the same default
    ///   event two ways
    /// * `day_count` - Day-count convention the curve uses to turn dates into
    ///   year fractions. Pass the convention of the discount curve the hazard
    ///   curve will be paired with; [`DayCount::Act365F`] matches the
    ///   year-fraction axis this model's horizons are expressed on
    ///
    /// # Errors
    ///
    /// Returns [`InputError::TooFewPoints`] if `tenors` is empty,
    /// [`InputError::NonPositiveValue`] if any tenor is non-positive, and
    /// [`Error::Validation`] if `recovery` is out of range or contradicts the
    /// model's `mean_recovery`, if tenors are not strictly increasing, if the
    /// implied survival curve is non-monotonic, or if survival reaches zero
    /// at some tenor (no finite hazard rate exists there). Propagates
    /// `HazardCurve` builder errors, including the hazard-rate ceiling.
    pub fn to_hazard_curve(
        &self,
        id: &str,
        base_date: time::Date,
        tenors: &[f64],
        recovery: f64,
        day_count: DayCount,
    ) -> Result<HazardCurve> {
        if tenors.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        if !(0.0..=1.0).contains(&recovery) {
            return Err(Error::Validation(format!(
                "to_hazard_curve: recovery must be in [0, 1], got {recovery}"
            )));
        }
        // The CreditGrades barrier is `debt * mean_recovery`, so the model
        // already embeds a recovery assumption. Letting the exported curve
        // carry a different one would price the same default event under two
        // inconsistent loss assumptions.
        if let AssetDynamics::CreditGrades { mean_recovery, .. } = self.dynamics {
            if (recovery - mean_recovery).abs() > 1e-12 {
                return Err(Error::Validation(format!(
                    "to_hazard_curve: recovery {recovery} contradicts the model's \
                     CreditGrades mean_recovery {mean_recovery}; the barrier is derived \
                     from mean_recovery, so the exported curve must use the same value"
                )));
            }
        }

        let mut sorted_tenors: Vec<f64> = tenors.to_vec();
        sorted_tenors.sort_by(|a, b| a.total_cmp(b));

        if sorted_tenors[0] <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }

        // Survival of exactly zero has no finite hazard rate. Clamping it to a
        // tiny epsilon would bury a total-loss model behind an arbitrary
        // 34,000% hazard rate, so report it instead.
        let survivals: Vec<f64> = sorted_tenors
            .iter()
            .map(|&t| {
                let survival = 1.0 - self.default_probability(t);
                if survival <= 0.0 {
                    return Err(Error::Validation(format!(
                        "Merton hazard bootstrap: survival is zero at {t:.6}y (default \
                         probability is numerically 1), so no finite hazard rate exists. \
                         Shorten the tenor grid or reduce leverage/volatility."
                    )));
                }
                Ok(survival.min(1.0))
            })
            .collect::<Result<Vec<f64>>>()?;

        let mut knots: Vec<(f64, f64)> = Vec::with_capacity(sorted_tenors.len());

        // First point: λ_0 = -ln(S(t_0)) / t_0
        let lambda_0 = -survivals[0].ln() / sorted_tenors[0];
        knots.push((sorted_tenors[0], lambda_0));

        // Subsequent points: λ_i = -ln(S(t_{i+1}) / S(t_i)) / (t_{i+1} - t_i)
        for i in 1..sorted_tenors.len() {
            if survivals[i] > survivals[i - 1] {
                return Err(Error::Validation(format!(
                    "Merton hazard bootstrap produced non-monotonic survival: \
                     S({:.6}y)={:.12} > S({:.6}y)={:.12}",
                    sorted_tenors[i],
                    survivals[i],
                    sorted_tenors[i - 1],
                    survivals[i - 1]
                )));
            }
            let dt = sorted_tenors[i] - sorted_tenors[i - 1];
            // Duplicate/non-increasing tenors give dt == 0; the equal survivals
            // pass the monotonic check above, so guard here to avoid emitting a
            // NaN hazard knot (-ln(1)/0 = 0/0) into the curve.
            if dt <= 0.0 {
                return Err(Error::Validation(format!(
                    "Merton hazard bootstrap requires strictly increasing tenors; \
                     got duplicate or non-increasing tenor {:.6}y",
                    sorted_tenors[i]
                )));
            }
            let lambda_i = -(survivals[i] / survivals[i - 1]).ln() / dt;
            knots.push((sorted_tenors[i], lambda_i));
        }

        HazardCurve::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .knots(knots)
            .recovery_rate(recovery)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use finstack_quant_core::dates::DayCount;

    use super::super::MertonModel;

    #[test]
    fn to_hazard_curve_survival_matches_pd() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let hc = m
            .to_hazard_curve(
                "TEST",
                base,
                &[1.0, 3.0, 5.0, 7.0, 10.0],
                0.40,
                DayCount::Act365F,
            )
            .expect("hc");
        // Survival at 5Y should match 1 - PD(5)
        let sp5 = hc.sp(5.0);
        let pd5 = m.default_probability(5.0);
        assert!(
            (sp5 - (1.0 - pd5)).abs() < 0.02,
            "sp5={sp5}, 1-pd5={}",
            1.0 - pd5
        );
    }

    #[test]
    fn to_hazard_curve_hazard_rates_positive() {
        let m = MertonModel::new(100.0, 0.30, 80.0, 0.04).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let hc = m
            .to_hazard_curve("TEST2", base, &[1.0, 3.0, 5.0], 0.40, DayCount::Act365F)
            .expect("hc");
        // All hazard rates should be positive for a risky firm
        for t in [0.5, 1.0, 2.0, 3.0, 4.0, 5.0] {
            let hr = hc.hazard_rate(t);
            assert!(
                hr > 0.0,
                "Hazard rate at t={t} should be positive, got {hr}"
            );
        }
    }

    #[test]
    fn to_hazard_curve_riskier_firm_higher_hazard() {
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let m_safe = MertonModel::new(100.0, 0.15, 50.0, 0.04).expect("valid");
        let m_risky = MertonModel::new(100.0, 0.30, 85.0, 0.04).expect("valid");
        let hc_safe = m_safe
            .to_hazard_curve("SAFE", base, &[1.0, 5.0, 10.0], 0.40, DayCount::Act365F)
            .expect("hc");
        let hc_risky = m_risky
            .to_hazard_curve("RISKY", base, &[1.0, 5.0, 10.0], 0.40, DayCount::Act365F)
            .expect("hc");
        assert!(
            hc_risky.hazard_rate(3.0) > hc_safe.hazard_rate(3.0),
            "Riskier firm should have higher hazard rate"
        );
    }

    #[test]
    fn to_hazard_curve_rejects_non_monotonic_survival() {
        // V < B with positive drift can make terminal PD fall with horizon,
        // which implies increasing survival and a negative hazard segment.
        let m = MertonModel::new(98.0, 0.30, 100.0, 0.10).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");

        let survival_1y = 1.0 - m.default_probability(1.0);
        let survival_5y = 1.0 - m.default_probability(5.0);
        assert!(
            survival_5y > survival_1y,
            "fixture must have increasing survival: 1Y={survival_1y}, 5Y={survival_5y}"
        );

        assert!(m
            .to_hazard_curve("BAD", base, &[1.0, 5.0], 0.40, DayCount::Act365F)
            .is_err());
    }

    #[test]
    fn to_hazard_curve_rejects_duplicate_tenors() {
        // Duplicate tenors (after sorting) give dt == 0 with equal survivals,
        // which previously slipped past the monotonic-survival check and emitted
        // a NaN hazard knot (-ln(1)/0). It must now be rejected.
        let m = MertonModel::new(120.0, 0.25, 100.0, 0.05).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let result = m.to_hazard_curve("DUP", base, &[1.0, 5.0, 5.0], 0.40, DayCount::Act365F);
        assert!(
            result.is_err(),
            "duplicate tenors must be rejected, got {result:?}"
        );
    }

    // Hazard curve export

    #[test]
    fn to_hazard_curve_rejects_recovery_inconsistent_with_credit_grades() {
        let m = MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.30, 0.40).expect("cg");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        assert!(m
            .to_hazard_curve("CG", base, &[1.0, 5.0], 0.60, DayCount::Act365F)
            .is_err());
        assert!(m
            .to_hazard_curve("CG", base, &[1.0, 5.0], 0.40, DayCount::Act365F)
            .is_ok());
    }

    #[test]
    fn to_hazard_curve_honours_the_requested_day_count() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let hc = m
            .to_hazard_curve("DC", base, &[1.0, 5.0], 0.40, DayCount::Act360)
            .expect("hc");
        assert_eq!(hc.day_count(), DayCount::Act360);
    }
}
