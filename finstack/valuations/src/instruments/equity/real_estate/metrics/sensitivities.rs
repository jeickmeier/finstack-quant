//! Real estate appraisal-style sensitivities (finite difference).
//!
//! These are *not* curve DV01 metrics. They are bump-and-reprice sensitivities for
//! real estate deal inputs like cap rates and appraisal discount rates.

use crate::instruments::equity::real_estate::{LeveredRealEstateEquity, RealEstateAsset};
use crate::instruments::Instrument;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Error as CoreError;

const DEFAULT_BUMP_ABS: f64 = 1e-4; // 1bp in fractional terms

#[derive(Debug, Clone, Copy)]
pub(super) struct CapRateSensitivity {
    /// Absolute bump size (e.g., 1bp = 1e-4).
    pub(super) bump_abs: f64,
}

impl Default for CapRateSensitivity {
    fn default() -> Self {
        Self {
            bump_abs: DEFAULT_BUMP_ABS,
        }
    }
}

impl CapRateSensitivity {
    fn bump_asset(mut a: RealEstateAsset, bump: f64) -> finstack_core::Result<RealEstateAsset> {
        match a.valuation_method {
            crate::instruments::equity::real_estate::RealEstateValuationMethod::DirectCap => {
                let Some(r) = a.cap_rate else {
                    return Err(CoreError::Validation(
                        "CapRateSensitivity: missing cap_rate (DirectCap)".into(),
                    ));
                };
                let bumped = r + bump;
                if bumped <= 0.0 {
                    return Err(CoreError::Validation(
                        "CapRateSensitivity: bumped cap_rate must be positive".into(),
                    ));
                }
                a.cap_rate = Some(bumped);
                Ok(a)
            }
            crate::instruments::equity::real_estate::RealEstateValuationMethod::Dcf => {
                // If terminal proceeds are explicitly set (sale_price), cap-rate sensitivity is 0.
                if a.sale_price.is_some() || a.terminal_cap_rate.is_none() {
                    return Ok(a);
                }
                let Some(r) = a.terminal_cap_rate else {
                    // Defensive: treat missing terminal cap rate as "not applicable".
                    return Ok(a);
                };
                let bumped = r + bump;
                if bumped <= 0.0 {
                    return Err(CoreError::Validation(
                        "CapRateSensitivity: bumped terminal_cap_rate must be positive".into(),
                    ));
                }
                a.terminal_cap_rate = Some(bumped);
                Ok(a)
            }
        }
    }

    fn eval_asset(
        &self,
        ctx: &MetricContext,
        base: &RealEstateAsset,
        bump: f64,
    ) -> finstack_core::Result<f64> {
        let bumped = Self::bump_asset(base.clone(), bump)?;
        Ok(bumped.value(ctx.curves.as_ref(), ctx.as_of)?.amount())
    }
}

impl MetricCalculator for CapRateSensitivity {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        if !self.bump_abs.is_finite() || self.bump_abs <= 0.0 {
            return Err(CoreError::Validation(
                "CapRateSensitivity: bump_abs must be positive and finite".into(),
            ));
        }

        // Dispatch by instrument type.
        if let Some(asset) = context
            .instrument
            .as_any()
            .downcast_ref::<RealEstateAsset>()
        {
            // If this configuration does not use a cap rate (no terminal proceeds), return not-applicable.
            match asset.valuation_method {
                crate::instruments::equity::real_estate::RealEstateValuationMethod::DirectCap => {
                    if asset.cap_rate.is_none() {
                        return Err(CoreError::Validation(
                            "CapRateSensitivity: not applicable for DirectCap without cap_rate"
                                .into(),
                        ));
                    }
                }
                crate::instruments::equity::real_estate::RealEstateValuationMethod::Dcf => {
                    if asset.sale_price.is_some() || asset.terminal_cap_rate.is_none() {
                        return Err(CoreError::Validation(
                            "CapRateSensitivity: not applicable for DCF with explicit sale_price or missing terminal_cap_rate".into(),
                        ));
                    }
                }
            }

            let v_up = self.eval_asset(context, asset, self.bump_abs)?;
            let v_dn = self.eval_asset(context, asset, -self.bump_abs)?;
            return Ok((v_up - v_dn) / (2.0 * self.bump_abs));
        }

        if let Some(levered) = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
        {
            // Same logic, but cap rate lives on the underlying asset.
            let asset = &levered.asset;
            match asset.valuation_method {
                crate::instruments::equity::real_estate::RealEstateValuationMethod::DirectCap => {
                    if asset.cap_rate.is_none() {
                        return Err(CoreError::Validation(
                            "CapRateSensitivity: not applicable for DirectCap without cap_rate"
                                .into(),
                        ));
                    }
                }
                crate::instruments::equity::real_estate::RealEstateValuationMethod::Dcf => {
                    if asset.sale_price.is_some() || asset.terminal_cap_rate.is_none() {
                        return Err(CoreError::Validation(
                            "CapRateSensitivity: not applicable for DCF with explicit sale_price or missing terminal_cap_rate".into(),
                        ));
                    }
                }
            }

            let mut up = levered.clone();
            up.asset = Self::bump_asset(up.asset.clone(), self.bump_abs)?;
            let v_up = up.value(context.curves.as_ref(), context.as_of)?.amount();

            let mut dn = levered.clone();
            dn.asset = Self::bump_asset(dn.asset.clone(), -self.bump_abs)?;
            let v_dn = dn.value(context.curves.as_ref(), context.as_of)?.amount();

            return Ok((v_up - v_dn) / (2.0 * self.bump_abs));
        }

        Err(CoreError::Validation(
            "CapRateSensitivity: instrument type mismatch".into(),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DiscountRateSensitivity {
    /// Absolute bump size (e.g., 1bp = 1e-4).
    pub(super) bump_abs: f64,
}

impl Default for DiscountRateSensitivity {
    fn default() -> Self {
        Self {
            bump_abs: DEFAULT_BUMP_ABS,
        }
    }
}

impl DiscountRateSensitivity {
    /// DCF valuations always discount at `discount_rate` 
    /// DCF valuations always discount at `discount_rate`, so the only
    /// precondition is that the rate is set.
    fn ensure_dcf_rate_present(a: &RealEstateAsset) -> finstack_core::Result<()> {
        if a.valuation_method
            != crate::instruments::equity::real_estate::RealEstateValuationMethod::Dcf
        {
            return Ok(());
        }
        if a.discount_rate.is_none() {
            return Err(CoreError::Validation(
                "DiscountRateSensitivity: missing discount_rate for DCF".into(),
            ));
        }
        Ok(())
    }

    fn eval_asset(
        ctx: &MetricContext,
        base: &RealEstateAsset,
        bump: f64,
    ) -> finstack_core::Result<f64> {
        let mut a = base.clone();
        if a.valuation_method
            == crate::instruments::equity::real_estate::RealEstateValuationMethod::Dcf
        {
            let r = a.discount_rate.ok_or_else(|| {
                CoreError::Validation("DiscountRateSensitivity: missing discount_rate".into())
            })?;
            let bumped = r + bump;
            if bumped <= -1.0 {
                return Err(CoreError::Validation(
                    "DiscountRateSensitivity: bumped discount_rate must be > -100%".into(),
                ));
            }
            a.discount_rate = Some(bumped);
        }
        Ok(a.value(ctx.curves.as_ref(), ctx.as_of)?.amount())
    }
}

impl MetricCalculator for DiscountRateSensitivity {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        if !self.bump_abs.is_finite() || self.bump_abs <= 0.0 {
            return Err(CoreError::Validation(
                "DiscountRateSensitivity: bump_abs must be positive and finite".into(),
            ));
        }

        if let Some(asset) = context
            .instrument
            .as_any()
            .downcast_ref::<RealEstateAsset>()
        {
            if asset.valuation_method
                != crate::instruments::equity::real_estate::RealEstateValuationMethod::Dcf
            {
                return Err(CoreError::Validation(
                    "DiscountRateSensitivity: not applicable for non-DCF valuation".into(),
                ));
            }
            Self::ensure_dcf_rate_present(asset)?;
            let v_up = Self::eval_asset(context, asset, self.bump_abs)?;
            let v_dn = Self::eval_asset(context, asset, -self.bump_abs)?;
            return Ok((v_up - v_dn) / (2.0 * self.bump_abs));
        }

        if let Some(levered) = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
        {
            if levered.asset.valuation_method
                != crate::instruments::equity::real_estate::RealEstateValuationMethod::Dcf
            {
                return Err(CoreError::Validation(
                    "DiscountRateSensitivity: not applicable for non-DCF valuation".into(),
                ));
            }
            Self::ensure_dcf_rate_present(&levered.asset)?;

            let mut up = levered.clone();
            let r = up
                .asset
                .discount_rate
                .ok_or_else(|| CoreError::Validation("missing discount_rate".into()))?;
            let up_bumped = r + self.bump_abs;
            if up_bumped <= -1.0 {
                return Err(CoreError::Validation(
                    "DiscountRateSensitivity: bumped discount_rate must be > -100%".into(),
                ));
            }
            up.asset.discount_rate = Some(up_bumped);
            let v_up = up.value(context.curves.as_ref(), context.as_of)?.amount();

            let mut dn = levered.clone();
            let r = dn
                .asset
                .discount_rate
                .ok_or_else(|| CoreError::Validation("missing discount_rate".into()))?;
            let dn_bumped = r - self.bump_abs;
            if dn_bumped <= -1.0 {
                return Err(CoreError::Validation(
                    "DiscountRateSensitivity: bumped discount_rate must be > -100%".into(),
                ));
            }
            dn.asset.discount_rate = Some(dn_bumped);
            let v_dn = dn.value(context.curves.as_ref(), context.as_of)?.amount();

            return Ok((v_up - v_dn) / (2.0 * self.bump_abs));
        }

        Err(CoreError::Validation(
            "DiscountRateSensitivity: instrument type mismatch".into(),
        ))
    }
}
