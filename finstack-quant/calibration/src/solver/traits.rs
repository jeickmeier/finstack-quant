//! Shared runtime types and solver contracts for market calibration.
//!
use crate::report::ResidualUnits;
use finstack_quant_core::Result;

/// Result type for building time grid and initial guesses.
pub(crate) type TimeGridAndGuesses<Q> = (Vec<f64>, Vec<f64>, Vec<Q>);

/// Trait defining the specific physics for a bootstrapping process.
///
/// Implementations of this trait provide the domain-specific logic needed
/// to solve for individual knots sequentially. This includes mapping quotes
/// to times, building curves from partial knots, and calculating pricing
/// residuals.
pub(crate) trait BootstrapTarget {
    /// Type of input quote (e.g., [`RateQuote`](crate::quotes::rates::RateQuote)).
    type Quote;

    /// Type of the curve being built (e.g., [`DiscountCurve`](finstack_quant_core::market_data::DiscountCurve)).
    type Curve;

    /// Get the time (year fraction) for the knot corresponding to this quote.
    ///
    /// The bootstrapper requires increasing quote times and will sort inputs
    /// automatically.
    fn quote_time(&self, quote: &Self::Quote) -> Result<f64>;

    /// Build a temporary curve from a set of knots (lenient validation allowed).
    ///
    /// This is called repeatedly during the solver loop, so implementations may
    /// skip expensive checks (e.g. strict monotonicity) that the solver bounds or
    /// [`build_curve_final`](Self::build_curve_final) enforce.
    fn build_curve_for_solver(&self, knots: &[(f64, f64)]) -> Result<Self::Curve>;

    /// Build the final curve (strict validation).
    ///
    /// Called once after the last knot is solved to ensure the final term
    /// structure meets all requirements. Defaults to the solver build.
    fn build_curve_final(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        self.build_curve_for_solver(knots)
    }

    /// Calculate the pricing residual for a quote given the curve.
    ///
    /// Residuals may be expressed as model-minus-market price deltas **or**
    /// normalized PV per unit notional (e.g., PV of a par instrument). The solver
    /// only requires a signed scalar that crosses zero at the solution, so choose
    /// a consistent unit across all quotes for meaningful tolerances.
    fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64>;

    /// Units of the residuals returned by [`Self::calculate_residual`],
    /// stamped on the calibration report.
    fn residual_units(&self) -> ResidualUnits;

    /// Provide an initial guess for the solver for the next knot.
    ///
    /// Usually based on the previous knot or forward-flat extrapolation.
    fn initial_guess(&self, quote: &Self::Quote, previous_knots: &[(f64, f64)]) -> Result<f64>;

    /// Get scan points for root bracketing for the given quote.
    ///
    /// If empty, the bootstrapper uses its default geometric scan grid.
    fn scan_points(&self, _quote: &Self::Quote, _initial_guess: f64) -> Result<Vec<f64>> {
        Ok(Vec::new())
    }

    /// Market quote value reported next to the residual in per-quote
    /// diagnostics, in the quote's native units. Defaults to `None` for
    /// targets whose quotes carry no single scalar value.
    fn quote_value(&self, _quote: &Self::Quote) -> Option<f64> {
        None
    }

    /// Stable residual / diagnostics label for a quote in the report.
    ///
    /// Production targets return the quote's `QuoteId` so report residuals
    /// and per-quote diagnostics are keyed by the identifier the caller
    /// supplied in the envelope. Defaults to `quote_{idx:06}` (position in
    /// time-sorted order) for targets whose quotes carry no identifier.
    fn residual_key(&self, _quote: &Self::Quote, idx: usize) -> String {
        format!("quote_{idx:06}")
    }

    /// Whether the target can stop its scan at the nearest sign-changing bracket.
    ///
    /// This optimization is valid only when the residual is monotone in the
    /// knot currently being solved. Targets default to the exhaustive scan so
    /// non-monotone objectives retain closest-bracket selection across the
    /// entire caller-supplied grid.
    fn supports_nearest_first_bracketing(&self) -> bool {
        false
    }

    /// Optional: Validate the solved value before accepting it.
    ///
    /// Allows enforcing domain constraints (e.g. positive hazard rates)
    /// that are not captured by the residual formula.
    fn validate_knot(&self, _time: f64, _value: f64) -> Result<()> {
        Ok(())
    }

    /// Whether this target may accept a best-effort knot when the solver finds
    /// no sign-change bracket. The accepted knot's residual is still validated
    /// against the target tolerance, and the report flags it via the
    /// `approximate_knots` metadata.
    ///
    /// Defaults to `false`: a no-bracket knot is a hard failure.
    /// Targets whose objective is monotone and whose market quotes can
    /// legitimately sit outside the model-reachable range — notably
    /// base-correlation bootstrapping against tranche upfronts — override this
    /// to `true`.
    fn allow_approximate_knots(&self) -> bool {
        false
    }
}

/// Trait defining the specific physics for a global optimization process.
///
/// Implementations of this trait provide the logic needed for simultaneous
/// fitting of multiple knots. This is used for multi-curve calibration
/// or sparse data scenarios where sequential bootstrapping is insufficient.
pub(crate) trait GlobalSolveTarget {
    /// Type of input quote.
    type Quote;

    /// Type of the curve being built.
    type Curve;

    /// Build the time grid and initial guesses for the optimization.
    ///
    /// Returns `(times, initial_params, active_quotes)`. The length of `times`
    /// determines the dimensionality of the problem.
    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<TimeGridAndGuesses<Self::Quote>>;

    /// Build a curve from parameters (e.g., zero rates).
    ///
    /// Called on every solver iteration, so lenient validation is allowed.
    fn build_curve_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve>;

    /// Build the final curve returned to callers (strict validation).
    ///
    /// Default implementation delegates to `build_curve_from_params`.
    fn build_curve_final_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        self.build_curve_from_params(times, params)
    }

    /// Market quote value reported next to the residual in per-quote
    /// diagnostics, in the quote's native units. Defaults to `None` for
    /// targets whose quotes carry no single scalar value.
    fn quote_value(&self, _quote: &Self::Quote) -> Option<f64> {
        None
    }

    /// Provide a stable residual key for reporting.
    ///
    /// Defaults to `GLOBAL-{idx:06}` if not overridden.
    fn residual_key(&self, _quote: &Self::Quote, idx: usize) -> String {
        format!("GLOBAL-{:06}", idx)
    }

    /// Provide per-quote residual weights (for weighted least squares).
    ///
    /// Higher weights increase the penalty for residuals on specific quotes.
    /// Default implementation fills weights with 1.0.
    fn residual_weights(&self, _quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
        weights_out.fill(1.0);
        Ok(())
    }

    /// Calculate residuals for all quotes given the curve.
    ///
    /// Residual units should match the `calculate_residual` contract (price delta
    /// or normalized PV) so that tolerance/reporting can be interpreted
    /// consistently across instruments. Populates the `residuals` slice.
    fn calculate_residuals(
        &self,
        curve: &Self::Curve,
        quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> Result<()>;

    /// Units of the residuals written by [`Self::calculate_residuals`],
    /// stamped on the calibration report.
    fn residual_units(&self) -> ResidualUnits;

    /// Compute the Jacobian matrix of residuals with respect to curve parameters.
    ///
    /// This method can be overridden by targets that provide an optimized Jacobian
    /// calculation (e.g., exploiting sparsity structure or using efficient finite
    /// differences). The default implementation returns an error, causing the solver
    /// to fall back to generic finite differences.
    ///
    /// # Arguments
    /// * `params` - The current parameter vector (e.g. zero rates).
    /// * `times` - The knot times corresponding to parameters.
    /// * `quotes` - The active calibration quotes corresponding to residuals.
    /// * `jacobian` - Output matrix (rows=residuals, cols=params).
    fn jacobian(
        &self,
        _params: &[f64],
        _times: &[f64],
        _quotes: &[Self::Quote],
        _jacobian: &mut [Vec<f64>],
    ) -> Result<()> {
        Err(finstack_quant_core::Error::Calibration {
            message: "Efficient Jacobian not implemented for this target".to_string(),
            category: "efficient_jacobian".to_string(),
        })
    }

    /// Returns true if this target provides an efficient Jacobian implementation.
    ///
    /// Targets returning `true` have a custom [`jacobian`](Self::jacobian) method
    /// that exploits problem structure (e.g., sparsity, locality) for faster
    /// computation than generic finite differences. The actual implementation
    /// may still use finite differences internally, but optimized for the
    /// specific calibration target's structure.
    fn supports_efficient_jacobian(&self) -> bool {
        false
    }

    /// Optional lower bounds for parameters.
    ///
    /// If provided, the solver will clamp parameters to these bounds after
    /// each Levenberg-Marquardt step (projected LM). The length must match
    /// the parameter vector length.
    fn lower_bounds(&self) -> Option<Vec<f64>> {
        None
    }

    /// Optional upper bounds for parameters.
    ///
    /// If provided, the solver will clamp parameters to these bounds after
    /// each Levenberg-Marquardt step (projected LM). The length must match
    /// the parameter vector length.
    fn upper_bounds(&self) -> Option<Vec<f64>> {
        None
    }
}
