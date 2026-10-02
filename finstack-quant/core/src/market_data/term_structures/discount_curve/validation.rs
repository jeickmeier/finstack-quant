//! Discount-factor and implied-forward validation policies.

/// Validation preset for
/// [`DiscountCurveBuilder::validation`](super::DiscountCurveBuilder::validation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValidationMode {
    /// Enforce monotonic (non-increasing) discount factors and a -50bp
    /// forward-rate floor. This is the recommended mode for production curves.
    MarketStandard,
    /// Relax monotonicity to support negative-rate regimes while keeping a
    /// safety floor on implied forwards.
    NegativeRateFriendly {
        /// Minimum allowed implied forward rate (in decimal).
        forward_floor: f64,
    },
    /// Fully raw mode for solver / calibration use: explicit over both
    /// monotonicity and (optional) forward-rate floor.
    Raw {
        /// Skip monotonicity checks when `true`.
        allow_non_monotonic: bool,
        /// Optional implied forward-rate floor.
        forward_floor: Option<f64>,
    },
}

impl ValidationMode {
    /// Resolve the public binding preset and its optional forward-rate floor.
    ///
    /// Bindings expose the two safe presets by name while keeping [`Self::Raw`]
    /// available only to canonical Rust callers.
    ///
    /// # Arguments
    ///
    /// * `name` - Preset name: `"market_standard"` or
    ///   `"negative_rate_friendly"`. `None` selects `"market_standard"`, the
    ///   [`DiscountCurve`](crate::market_data::term_structures::DiscountCurve)
    ///   builder default (monotone discount factors, −50bp implied-forward
    ///   floor).
    /// * `forward_floor` - Minimum implied forward rate as a decimal (for
    ///   example `-0.01` for −100bp). Required with `negative_rate_friendly`
    ///   and rejected otherwise, including when `name` is `None`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` for an unsupported preset, a floor supplied
    /// with `market_standard` (explicit or defaulted), a missing floor for
    /// `negative_rate_friendly`, or a non-finite floor.
    pub fn from_preset(name: Option<&str>, forward_floor: Option<f64>) -> crate::Result<Self> {
        match name.unwrap_or("market_standard") {
            "market_standard" => {
                if forward_floor.is_some() {
                    return Err(crate::Error::Validation(
                        "forward_floor is only valid with validation_mode='negative_rate_friendly'"
                            .to_string(),
                    ));
                }
                Ok(Self::MarketStandard)
            }
            "negative_rate_friendly" => {
                let forward_floor = forward_floor.ok_or_else(|| {
                    crate::Error::Validation(
                        "forward_floor is required with validation_mode='negative_rate_friendly'"
                            .to_string(),
                    )
                })?;
                if !forward_floor.is_finite() {
                    return Err(crate::Error::Validation(
                        "forward_floor must be finite".to_string(),
                    ));
                }
                Ok(Self::NegativeRateFriendly { forward_floor })
            }
            other => Err(crate::Error::Validation(format!(
                "unknown DiscountCurve validation_mode {other:?}; expected 'market_standard' or 'negative_rate_friendly'"
            ))),
        }
    }
}

/// Validate that discount factors are monotone (non-increasing) within tolerance.
///
/// Non-monotonic discount factors violate no-arbitrage conditions and will
/// produce incorrect pricing results.
pub(super) fn validate_monotonic_df(knots: &[f64], dfs: &[f64]) -> crate::Result<()> {
    if let Some((i, prev, curr)) = crate::math::interp::utils::find_monotone_violation(dfs, 1e-14) {
        return Err(crate::Error::Validation(format!(
            "Discount factors must be non-increasing: DF(t={:.4}) = {:.12} > DF(t={:.4}) = {:.12}",
            knots[i + 1],
            curr,
            knots[i],
            prev
        )));
    }
    Ok(())
}

/// Validate that implied forward rates are above a minimum threshold.
///
/// Forward rates are calculated as: f(t1, t2) = -ln(DF(t2)/DF(t1)) / (t2 - t1)
///
/// Excessively negative forward rates (below the specified floor) indicate
/// either data errors or unrealistic market conditions.
pub(super) fn validate_forward_rates(
    knots: &[f64],
    dfs: &[f64],
    min_rate: f64,
) -> crate::Result<()> {
    for (knot_pair, df_pair) in knots.windows(2).zip(dfs.windows(2)) {
        let dt = knot_pair[1] - knot_pair[0];
        if dt <= 0.0 {
            continue;
        }

        let fwd = -(df_pair[1] / df_pair[0]).ln() / dt;

        if fwd < min_rate {
            return Err(crate::Error::Validation(format!(
                "Forward rate {:.4}% (decimal: {:.6}) between t={:.4} and t={:.4} is below minimum {:.4}% (decimal: {:.6}). \
                 This may indicate a data error or create arbitrage opportunities.",
                fwd * 100.0, fwd, knot_pair[0], knot_pair[1], min_rate * 100.0, min_rate
            )));
        }
    }
    Ok(())
}
