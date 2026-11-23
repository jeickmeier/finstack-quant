//! Bucket-share analytics, tolerance checks, and human-readable explanations for [`PnlAttribution`].

use super::pnl::PnlAttribution;
use super::types::{PnlBucket, PnlBucketShare};
use finstack_quant_core::money::Money;

impl PnlAttribution {
    /// Share of total P&L, in percent, that an amount represents.
    ///
    /// Computes `amount / total_pnl × 100`, the per-factor percentage shown by
    /// [`Self::explain`] and attribution reports. The zero test on
    /// `total_pnl` uses the attribution's stored `RoundingContext`, the same
    /// test as `meta.residual_pct`.
    ///
    /// # Arguments
    ///
    /// * `amount` - P&L amount in `total_pnl` currency units, normally one
    ///   factor field (`carry`, `rates_curves_pnl`, `residual`, …) or
    ///   `total_pnl` itself.
    ///
    /// # Returns
    ///
    /// Signed percentage points (`25.0` = 25% of total P&L), or `None` when
    /// `total_pnl` is effectively zero and the share is undefined.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_attribution::{AttributionMethod, PnlAttribution};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use time::macros::date;
    ///
    /// let mut pnl = PnlAttribution::new(
    ///     Money::from((200_i64, Currency::USD)),
    ///     "BOND-1",
    ///     date!(2025 - 01 - 15),
    ///     date!(2025 - 01 - 16),
    ///     AttributionMethod::Parallel,
    /// );
    /// assert_eq!(pnl.pct_of_total(50.0), Some(25.0));
    /// pnl.total_pnl = Money::from((0_i64, Currency::USD));
    /// assert_eq!(pnl.pct_of_total(50.0), None);
    /// ```
    #[must_use]
    pub fn pct_of_total(&self, amount: f64) -> Option<f64> {
        let total = &self.total_pnl;
        if self.meta.rounding.is_effectively_zero(
            total.amount(),
            finstack_quant_core::config::ZeroKind::Money(total.currency()),
        ) {
            None
        } else {
            Some(amount / total.amount() * 100.0)
        }
    }

    /// Every money bucket with its share of total P&L, in the struct's field order.
    ///
    /// Each share is [`Self::pct_of_total`] of the bucket's own amount. Lists the
    /// eleven factor buckets and `residual`; the headline totals and endpoint
    /// values (`total_pnl`, `mark_to_market_pnl`, `pv_t0`, `pv_t1`) are not
    /// buckets. A host reads this list instead of naming the fields, so a bucket
    /// added to this struct cannot silently go missing from a report.
    ///
    /// # Returns
    ///
    /// Twelve records in field order; `pct_of_total` is `None` on every record
    /// when `total_pnl` is effectively zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_attribution::{AttributionMethod, PnlAttribution, PnlBucket};
    /// use finstack_quant_core::{currency::Currency, money::Money};
    /// use time::macros::date;
    ///
    /// let mut pnl = PnlAttribution::new(
    ///     Money::from((200_i64, Currency::USD)),
    ///     "BOND-1",
    ///     date!(2025 - 01 - 15),
    ///     date!(2025 - 01 - 16),
    ///     AttributionMethod::Parallel,
    /// );
    /// pnl.carry = Money::from((50_i64, Currency::USD));
    /// let buckets = pnl.pct_of_total_by_bucket();
    /// assert_eq!(buckets[0].field, PnlBucket::Carry);
    /// assert_eq!(buckets[0].pct_of_total, Some(25.0));
    /// ```
    #[must_use]
    pub fn pct_of_total_by_bucket(&self) -> Vec<PnlBucketShare> {
        [
            (PnlBucket::Carry, &self.carry),
            (PnlBucket::RatesCurvesPnl, &self.rates_curves_pnl),
            (PnlBucket::CreditCurvesPnl, &self.credit_curves_pnl),
            (PnlBucket::InflationCurvesPnl, &self.inflation_curves_pnl),
            (PnlBucket::CorrelationsPnl, &self.correlations_pnl),
            (PnlBucket::FxPnl, &self.fx_pnl),
            (PnlBucket::FxTranslationPnl, &self.fx_translation_pnl),
            (PnlBucket::VolPnl, &self.vol_pnl),
            (PnlBucket::CrossFactorPnl, &self.cross_factor_pnl),
            (PnlBucket::ModelParamsPnl, &self.model_params_pnl),
            (PnlBucket::MarketScalarsPnl, &self.market_scalars_pnl),
            (PnlBucket::Residual, &self.residual),
        ]
        .into_iter()
        .map(|(field, amount)| PnlBucketShare {
            field,
            amount: *amount,
            pct_of_total: self.pct_of_total(amount.amount()),
        })
        .collect()
    }

    /// Check if residual is within tolerance.
    ///
    /// Tolerance is either percentage-based (relative to total P&L) or
    /// absolute, whichever is larger.
    ///
    /// # Arguments
    ///
    /// * `pct_tolerance` - Percentage tolerance (e.g., `Some(0.1)` for 0.1%);
    ///   `None` uses the run's configured `meta.tolerance_pct`.
    /// * `abs_tolerance` - Absolute tolerance in `total_pnl` currency units
    ///   (e.g., `Some(100.0)` for $100); `None` uses `meta.tolerance_abs`.
    ///
    /// # Returns
    ///
    /// `true` if residual is within tolerance.
    pub fn residual_within_tolerance(
        &self,
        pct_tolerance: Option<f64>,
        abs_tolerance: Option<f64>,
    ) -> bool {
        let pct_tolerance = pct_tolerance.unwrap_or(self.meta.tolerance_pct);
        let abs_tolerance = abs_tolerance.unwrap_or(self.meta.tolerance_abs);
        // If residual computation failed, `residual` was reset to 0
        // and a clean tolerance check would falsely succeed. Refuse to claim
        // "within tolerance" when the attribution is flagged invalid.
        if self.result_invalid {
            return false;
        }

        let abs_residual = self.residual.amount().abs();
        let abs_total = self.total_pnl.amount().abs();

        // Tolerance is the larger of percentage-based or absolute. The
        // "is total_pnl effectively zero?" gate uses the attribution's stored
        // RoundingContext (MI4) rather than a hardcoded 1e-10 — keeps the
        // semantic consistent with the rest of `PnlAttribution`'s zero checks.
        let total_is_zero = self.meta.rounding.is_effectively_zero(
            abs_total,
            finstack_quant_core::config::ZeroKind::Money(self.total_pnl.currency()),
        );
        let tolerance = if total_is_zero {
            abs_tolerance
        } else {
            (abs_total * pct_tolerance / 100.0).max(abs_tolerance)
        };

        abs_residual <= tolerance
    }

    /// Generate a structured tree explanation of P&L attribution.
    ///
    /// Creates a human-readable tree showing the total P&L broken down by factor.
    ///
    /// # Arguments
    ///
    /// * `show_zeros` - `false` omits zero-valued factors for a clean
    ///   presentation; `true` lists every factor, which helps verify that
    ///   all factors were computed.
    ///
    /// # Returns
    ///
    /// Multi-line string with tree structure.
    ///
    /// # Examples
    ///
    /// ```text
    /// Total P&L: $125,430
    ///   ├─ Carry: $45,000 (35.8%)
    ///   ├─ Rates Curves: $65,000 (51.7%)
    ///   │   ├─ USD-OIS: $50,000
    ///   │   └─ EUR-OIS: $15,000
    ///   ├─ Credit Curves: $5,000 (4.0%)
    ///   ├─ FX: $12,000 (9.5%)
    ///   ├─ Vol: $2,000 (1.6%)
    ///   └─ Residual: -$1,570 (-1.2%)
    /// ```
    pub fn explain(&self, show_zeros: bool) -> String {
        let rc = &self.meta.rounding;

        let fmt = |amount: &Money| -> String {
            let pct = self.pct_of_total(amount.amount()).unwrap_or(0.0);
            format!("{} ({:.1}%)", amount, pct)
        };

        let show = |m: &Money| -> bool {
            show_zeros
                || !rc.is_effectively_zero(
                    m.amount(),
                    finstack_quant_core::config::ZeroKind::Money(m.currency()),
                )
        };

        let child = |label: &dyn std::fmt::Display, pnl: &Money| format!("  │   ├─ {label}: {pnl}");
        let mut carry_lines = Vec::new();
        if let Some(detail) = &self.carry_detail {
            if let Some(coupon_income) = &detail.coupon_income {
                carry_lines.push(child(&"Coupon Income", &coupon_income.total));
            }
            if let Some(pull_to_par) = &detail.pull_to_par {
                carry_lines.push(child(&"Pull-to-Par", pull_to_par));
            }
            if let Some(roll_down) = &detail.roll_down {
                carry_lines.push(child(&"Roll-Down", &roll_down.total));
            }
            if let Some(funding_cost) = &detail.funding_cost {
                carry_lines.push(format!("  │   └─ Funding Cost: {funding_cost}"));
            }
        }
        let by_curve = |by_curve: Option<
            &indexmap::IndexMap<finstack_quant_core::types::CurveId, Money>,
        >|
         -> Vec<String> {
            by_curve
                .into_iter()
                .flatten()
                .map(|(curve_id, pnl)| child(curve_id, pnl))
                .collect()
        };
        let cross_lines = self
            .cross_factor_detail
            .iter()
            .flat_map(|detail| &detail.by_pair)
            .map(|(pair, pnl)| child(pair, pnl))
            .collect();

        let mut lines = vec![format!("Total P&L: {}", self.total_pnl)];
        for (label, amount, children) in [
            ("Carry", &self.carry, carry_lines),
            (
                "Rates Curves",
                &self.rates_curves_pnl,
                by_curve(self.rates_detail.as_ref().map(|d| &d.by_curve)),
            ),
            (
                "Credit Curves",
                &self.credit_curves_pnl,
                by_curve(self.credit_detail.as_ref().map(|d| &d.by_curve)),
            ),
            ("Inflation Curves", &self.inflation_curves_pnl, Vec::new()),
            ("Correlations", &self.correlations_pnl, Vec::new()),
            ("FX", &self.fx_pnl, Vec::new()),
            ("FX Translation", &self.fx_translation_pnl, Vec::new()),
            ("Vol", &self.vol_pnl, Vec::new()),
            ("Cross-Factor", &self.cross_factor_pnl, cross_lines),
            ("Model Params", &self.model_params_pnl, Vec::new()),
            ("Market Scalars", &self.market_scalars_pnl, Vec::new()),
        ] {
            if show(amount) {
                lines.push(format!("  ├─ {label}: {}", fmt(amount)));
                lines.extend(children);
            }
        }

        lines.push(format!("  └─ Residual: {}", fmt(&self.residual)));

        lines.join("\n")
    }
}
