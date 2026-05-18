use finstack_core::Result;

/// Result type for building time grid and initial guesses.
pub(crate) type TimeGridAndGuesses<Q> = (Vec<f64>, Vec<f64>, Vec<Q>);

/// Trait defining the specific physics for a bootstrapping process.
///
/// Implementations of this trait provide the domain-specific logic needed
/// to solve for individual knots sequentially. This includes mapping quotes
/// to times, building curves from partial knots, and calculating pricing
/// residuals.
pub(crate) trait BootstrapTarget {
    /// Type of input quote (e.g., [`RateQuote`](crate::market::quotes::rates::RateQuote)).
    type Quote;

    /// Type of the curve being built (e.g., [`DiscountCurve`](finstack_core::market_data::DiscountCurve)).
    type Curve;

    /// Get the time (year fraction) for the knot corresponding to this quote.
    ///
    /// The bootstrapper requires increasing quote times and will sort inputs
    /// automatically.
    fn quote_time(&self, quote: &Self::Quote) -> Result<f64>;

    /// Build a temporary curve from a set of knots.
    ///
    /// This is called repeatedly during the solver loop.
    fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve>;

    /// Build a temporary curve for the solver (fast path, lenient validation).
    ///
    /// Overriding this can improve performance by skipping expensive checks
    /// (e.g. strict monotonicity) during optimization if they are enforced
    /// by the solver bounds or the final build step.
    fn build_curve_for_solver(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        self.build_curve(knots)
    }

    /// Build the final curve (strict validation).
    ///
    /// Called once after a knot is successfully solved to ensure the final
    /// term structure meets all requirements.
    fn build_curve_final(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        self.build_curve(knots)
    }

    /// Calculate the pricing residual for a quote given the curve.
    ///
    /// Residuals may be expressed as model-minus-market price deltas **or**
    /// normalized PV per unit notional (e.g., PV of a par instrument). The solver
    /// only requires a signed scalar that crosses zero at the solution, so choose
    /// a consistent unit across all quotes for meaningful tolerances.
    fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64>;

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
    /// Defaults to `false`: a no-bracket knot is a hard failure (audit item 10).
    /// Targets whose objective is monotone and whose market quotes can
    /// legitimately sit outside the model-reachable range — notably
    /// base-correlation bootstrapping against tranche upfronts — override this
    /// to `true` (the audit's sanctioned "explicit caller opt-in").
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
    fn build_curve_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve>;

    /// Build a curve for solver iterations (lenient validation allowed).
    ///
    /// Default implementation delegates to `build_curve_from_params`.
    fn build_curve_for_solver_from_params(
        &self,
        times: &[f64],
        params: &[f64],
    ) -> Result<Self::Curve> {
        self.build_curve_from_params(times, params)
    }

    /// Build the final curve returned to callers (strict validation).
    ///
    /// Default implementation delegates to `build_curve_from_params`.
    fn build_curve_final_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        self.build_curve_from_params(times, params)
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
    fn residual_weights(&self, quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
        if quotes.len() != weights_out.len() {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Global solve requires weights.len() == quotes.len(); got {} vs {}.",
                    weights_out.len(),
                    quotes.len()
                ),
                category: "global_solve".to_string(),
            });
        }
        for weight in weights_out.iter_mut() {
            *weight = 1.0;
        }
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
        Err(finstack_core::Error::Calibration {
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
