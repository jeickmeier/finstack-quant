//! Tree valuator for Bermudan swaption pricing.
//!
//! Implements backward induction with optimal exercise decisions at each
//! Bermudan exercise date using the Hull-White tree.
//!
//! # Algorithm
//!
//! At each tree node and time step:
//! 1. Compute continuation value (discounted expected value from child nodes)
//! 2. At exercise dates, compute exercise value = max(0, (S - K) × A × N) for payer
//! 3. Take max(continuation, exercise) for optimal exercise decision
//!
//! # Usage
//!
//! ```text
//! use finstack_quant_valuations::instruments::rates::swaption::{BermudanSwaption, CalibratedHullWhiteModel, HullWhiteParams, pricing::BermudanSwaptionTreeValuator};
//!
//! let swaption = BermudanSwaption::example();
//! # let discount_curve: &dyn finstack_quant_core::market_data::traits::Discounting = todo!();
//! # let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
//!
//! // Create calibrated model
//! let ttm = swaption.time_to_maturity(as_of).unwrap();
//! let model = CalibratedHullWhiteModel::calibrate(
//!     HullWhiteParams::default(),
//!     100,
//!     discount_curve,
//!     ttm,
//! ).unwrap();
//!
//! // Create valuator and price
//! let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, discount_curve, as_of).unwrap();
//! let price = valuator.price().unwrap();
//! ```
#[allow(dead_code)] // Public API items may be used by external bindings or tests
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::rates::swaption::types::BermudanType;
use crate::instruments::rates::swaption::BermudanSwaption;
use crate::instruments::rates::swaption::CalibratedHullWhiteModel;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::HashSet;
use finstack_quant_core::Result;

/// Tree valuator for Bermudan swaption pricing.
///
/// Uses a calibrated Hull-White tree to price Bermudan swaptions via
/// backward induction with optimal exercise decisions.
pub struct BermudanSwaptionTreeValuator<'a> {
    /// Reference to the Bermudan swaption
    swaption: &'a BermudanSwaption,
    /// Reference to the calibrated Hull-White model
    model: &'a CalibratedHullWhiteModel,
    /// Reference to the discount curve
    discount_curve: &'a dyn Discounting,
    /// Valuation date.
    ///
    /// Stored for potential future use in:
    /// - Theta calculations (time decay from as_of to as_of + 1 day)
    /// - Exercise boundary reporting with actual dates
    /// - Diagnostic output including the valuation date
    ///
    /// Currently the valuator uses pre-computed year fractions for all calculations,
    /// so this field is not actively used in pricing. It is retained for API
    /// consistency and future extensibility.
    _as_of: Date,
    /// Exercise step indices (mapped from exercise dates)
    exercise_steps: HashSet<usize>,
    /// Swap payment times (year fractions from as_of)
    payment_times: Vec<f64>,
    /// Accrual fractions for each payment period
    accrual_fractions: Vec<f64>,
    /// Swap start time (year fraction)
    swap_start_time: f64,
    /// Swap end time (year fraction)
    swap_end_time: f64,
    /// Strike rate as f64, validated at construction
    strike: f64,
}

impl<'a> BermudanSwaptionTreeValuator<'a> {
    /// Create a new tree valuator for a Bermudan swaption.
    ///
    /// # Arguments
    ///
    /// * `swaption` - The Bermudan swaption to price
    /// * `model` - Calibrated Hull-White model
    /// * `discount_curve` - Discount curve for pricing
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// A valuator ready to compute prices via `price()` method.
    pub fn new(
        swaption: &'a BermudanSwaption,
        model: &'a CalibratedHullWhiteModel,
        discount_curve: &'a dyn Discounting,
        as_of: Date,
    ) -> Result<Self> {
        if swaption.get_forward_curve_id() != swaption.get_discount_curve_id() {
            return Err(finstack_quant_core::Error::Validation(
                "Bermudan tree pricing is currently single-curve only. \
                 Set forward_curve_id equal to discount_curve_id or use a multi-curve-capable engine."
                    .into(),
            ));
        }

        // Non-co-terminal Bermudans require per-exercise-date swap end dates,
        // which this valuator does not yet support. Reject early with a clear
        // message rather than silently pricing incorrectly.
        if swaption.bermudan_type == BermudanType::NonCoTerminal {
            return Err(finstack_quant_core::Error::Validation(
                "Non-co-terminal Bermudan swaptions are not yet supported by the tree valuator. \
                 Each exercise date would require a different swap end date, which is not \
                 implemented. Use BermudanType::CoTerminal instead."
                    .into(),
            ));
        }

        let tree = model.tree();
        // Get exercise times and map to tree steps
        let exercise_times = swaption.exercise_times(as_of)?;
        let exercise_steps: HashSet<usize> = exercise_times
            .iter()
            .map(|&t| tree.time_to_step(t))
            .collect();

        // Build swap schedule
        let (_payment_dates, accrual_fractions) = swaption.build_swap_schedule(as_of)?;
        let payment_times = swaption.payment_times(as_of)?;

        let ctx = finstack_quant_core::dates::DayCountContext::default();
        let swap_start_time =
            swaption
                .get_day_count()
                .year_fraction(as_of, swaption.get_swap_start(), ctx)?;
        let swap_end_time =
            swaption
                .get_day_count()
                .year_fraction(as_of, swaption.get_swap_end(), ctx)?;
        let strike = swaption.strike_f64()?;

        Ok(Self {
            swaption,
            model,
            discount_curve,
            _as_of: as_of,
            exercise_steps,
            payment_times,
            accrual_fractions,
            swap_start_time,
            swap_end_time,
            strike,
        })
    }

    fn tree(&self) -> &crate::models::trees::HullWhiteTree {
        self.model.tree().as_ref()
    }

    /// Price the Bermudan swaption using backward induction.
    ///
    /// # Returns
    ///
    /// Present value of the Bermudan swaption at valuation date.
    ///
    /// # Errors
    ///
    /// Propagates tree backward-induction validation failures.
    pub fn price(&self) -> Result<f64> {
        let n = self.tree().num_steps();

        // Terminal values: exercise value at last step if it's an exercise date
        let terminal: Vec<f64> = (0..self.tree().num_nodes(n))
            .map(|j| {
                if self.exercise_steps.contains(&n) {
                    self.exercise_value(n, j).max(0.0)
                } else {
                    0.0
                }
            })
            .collect();

        // Backward induction with exercise decisions
        self.tree()
            .backward_induction(&terminal, |step, node_idx, continuation| {
                if self.exercise_steps.contains(&step) {
                    let exercise = self.exercise_value(step, node_idx);
                    continuation.max(exercise)
                } else {
                    continuation
                }
            })
    }

    /// Compute exercise value at a node.
    ///
    /// For a payer swaption: max(0, (S - K) × A × N)
    /// For a receiver swaption: max(0, (K - S) × A × N)
    fn exercise_value(&self, step: usize, node_idx: usize) -> f64 {
        let t = self.tree().time_at_step(step);

        // OPTIMIZATION: Find start index without allocating
        // payment_times is sorted by construction (from swaption schedule)
        let start_idx = self.payment_times.partition_point(|&pt| pt <= t);

        if start_idx >= self.payment_times.len() {
            return 0.0;
        }

        // Use slices instead of allocating new vectors
        let remaining_payment_times = &self.payment_times[start_idx..];
        let remaining_accruals = &self.accrual_fractions[start_idx..];

        // Compute forward swap rate at this node
        let swap_start = self.swap_start_time.max(t); // Swap starts at exercise time
        let swap_rate = self.tree().forward_swap_rate(
            step,
            node_idx,
            swap_start,
            self.swap_end_time,
            remaining_payment_times,
            remaining_accruals,
            self.discount_curve,
        );

        // Compute annuity at this node
        let annuity = self.tree().annuity(
            step,
            node_idx,
            remaining_payment_times,
            remaining_accruals,
            self.discount_curve,
        );

        let notional = self.swaption.notional.amount();

        let intrinsic = match self.swaption.option_type {
            OptionType::Call => (swap_rate - self.strike).max(0.0),
            OptionType::Put => (self.strike - swap_rate).max(0.0),
        };

        intrinsic * annuity * notional
    }

    /// Get exercise boundary information.
    ///
    /// Returns a vector of (time, critical_rate) pairs where the holder
    /// would be indifferent between exercising and continuing.
    pub fn exercise_boundary(&self) -> Vec<(f64, Option<f64>)> {
        let _n = self.tree().num_steps();
        let mut boundary = Vec::new();

        // Work backward through exercise dates
        let mut sorted_steps: Vec<usize> = self.exercise_steps.iter().copied().collect();
        sorted_steps.sort();

        for &step in &sorted_steps {
            let t = self.tree().time_at_step(step);

            // Find the critical rate at this step
            // This is the rate at which exercise value equals continuation value
            // For simplicity, we return the rate at the central node
            let central_node = self.tree().num_nodes(step) / 2;
            let rate = self.tree().rate_at_node(step, central_node);

            // Note: A full implementation would solve for the exact critical rate
            // by finding where exercise_value = continuation_value
            boundary.push((t, Some(rate)));
        }

        boundary
    }

    /// Compute exercise probabilities at each exercise date.
    ///
    /// Returns a vector of (time, probability) pairs representing the
    /// risk-neutral probability of exercise at each date.
    ///
    /// Uses a forward pass tracking survival probabilities through the tree,
    /// deducting mass at nodes where optimal exercise occurs.
    pub fn exercise_probabilities(&self) -> Vec<(f64, f64)> {
        let n = self.tree().num_steps();
        let comp = self.tree().config().compounding;

        // 1. Backward pass: compute continuation values at each node
        let mut cont_values: Vec<f64> = (0..self.tree().num_nodes(n))
            .map(|j| {
                if self.exercise_steps.contains(&n) {
                    self.exercise_value(n, j).max(0.0)
                } else {
                    0.0
                }
            })
            .collect();

        // Store continuation values at exercise steps for forward pass comparison
        let mut exercise_cont_values: finstack_quant_core::HashMap<usize, Vec<f64>> =
            finstack_quant_core::HashMap::default();

        for step in (0..n).rev() {
            let num_curr = self.tree().num_nodes(step);
            let dt = self.tree().dt_at_step(step);

            // Compute continuation values first (before exercise decision)
            let continuations: Vec<f64> = (0..num_curr)
                .map(|j| {
                    let r_j = self.tree().rate_at_node(step, j);
                    let (p_up, p_mid, p_down) = self.tree().probabilities(step, j);
                    let center = self.tree().branch_center(step, j);
                    let expected = p_up * cont_values[center + 1]
                        + p_mid * cont_values[center]
                        + p_down * cont_values[center - 1];
                    expected * comp.df(r_j, dt)
                })
                .collect();

            if self.exercise_steps.contains(&step) {
                exercise_cont_values.insert(step, continuations.clone());
            }

            // Apply exercise decision
            cont_values = continuations
                .into_iter()
                .enumerate()
                .map(|(j, cont)| {
                    if self.exercise_steps.contains(&step) {
                        cont.max(self.exercise_value(step, j))
                    } else {
                        cont
                    }
                })
                .collect();
        }

        // 2. Forward pass: track survival probabilities
        let mut survival = vec![1.0]; // Start at root
        let mut exercise_probs = Vec::new();
        let mut sorted_steps: Vec<usize> = self.exercise_steps.iter().copied().collect();
        sorted_steps.sort();

        for step in 0..=n {
            let num_curr = self.tree().num_nodes(step);

            // Pad survival if needed
            while survival.len() < num_curr {
                survival.push(0.0);
            }

            if self.exercise_steps.contains(&step) {
                let cont_vals = exercise_cont_values.get(&step);
                let mut step_prob = 0.0;

                for (j, surv) in survival.iter_mut().enumerate().take(num_curr) {
                    if *surv < 1e-15 {
                        continue;
                    }
                    let exercise_val = self.exercise_value(step, j);
                    let cont_val = cont_vals.and_then(|v| v.get(j)).copied().unwrap_or(0.0);
                    if exercise_val >= cont_val && exercise_val > 0.0 {
                        step_prob += *surv;
                        *surv = 0.0; // Exercised
                    }
                }
                exercise_probs.push((self.tree().time_at_step(step), step_prob));
            }

            if step < n {
                let num_next = self.tree().num_nodes(step + 1);
                let mut next_survival = vec![0.0; num_next];

                for (j, &surv) in survival.iter().enumerate().take(num_curr) {
                    if surv < 1e-15 {
                        continue;
                    }
                    let (p_up, p_mid, p_down) = self.tree().probabilities(step, j);
                    let center = self.tree().branch_center(step, j);

                    next_survival[center + 1] += surv * p_up;
                    next_survival[center] += surv * p_mid;
                    next_survival[center - 1] += surv * p_down;
                }
                survival = next_survival;
            }
        }

        exercise_probs
    }
}

/// Result of Bermudan swaption pricing with additional analytics.
#[derive(Debug, Clone)]
#[allow(dead_code)] // public API result struct
pub(crate) struct BermudanSwaptionPriceResult {
    /// Present value
    pub(crate) pv: f64,
    /// Exercise boundary (time, critical_rate)
    pub(crate) exercise_boundary: Vec<(f64, Option<f64>)>,
    /// Exercise probabilities (time, probability)
    pub(crate) exercise_probabilities: Vec<(f64, f64)>,
    /// European swaption value (first exercise only, for comparison)
    pub(crate) european_value: Option<f64>,
    /// Bermudan premium (Bermudan - European)
    pub(crate) bermudan_premium: Option<f64>,
}

#[allow(dead_code)] // public API constructor
impl BermudanSwaptionPriceResult {
    /// Create a new price result.
    pub(crate) fn new(
        pv: f64,
        exercise_boundary: Vec<(f64, Option<f64>)>,
        exercise_probabilities: Vec<(f64, f64)>,
        european_value: Option<f64>,
    ) -> Self {
        let bermudan_premium = european_value.map(|euro| pv - euro);
        Self {
            pv,
            exercise_boundary,
            exercise_probabilities,
            european_value,
            bermudan_premium,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::swaption::{
        BermudanSchedule, BermudanSwaption, CalibratedHullWhiteModel, HullWhiteParams,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use time::Month;

    fn test_discount_curve() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
            .knots([
                (0.0, 1.0),
                (0.5, 0.985),
                (1.0, 0.97),
                (2.0, 0.94),
                (5.0, 0.85),
                (10.0, 0.70),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Valid curve")
    }

    fn test_bermudan_swaption() -> BermudanSwaption {
        let swap_start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
        let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

        BermudanSwaption::new_payer(
            "TEST-BERM",
            Money::new(10_000_000.0, Currency::USD),
            0.03,
            swap_start,
            swap_end,
            BermudanSchedule::co_terminal(first_exercise, swap_end, Tenor::semi_annual())
                .expect("valid Bermudan schedule"),
            "USD-OIS",
            "USD-OIS",
            "USD-VOL",
        )
        .expect("valid Bermudan swaption")
    }

    #[test]
    fn test_valuator_creation() {
        let curve = test_discount_curve();
        let swaption = test_bermudan_swaption();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");
        let model = CalibratedHullWhiteModel::calibrate(
            HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
            50,
            &curve,
            ttm,
        )
        .expect("Calibration should succeed");

        let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of);
        assert!(valuator.is_ok(), "Valuator creation should succeed");
    }

    #[test]
    fn test_bermudan_price_positive() {
        let curve = test_discount_curve();
        let swaption = test_bermudan_swaption();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");
        let model = CalibratedHullWhiteModel::calibrate(
            HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
            50,
            &curve,
            ttm,
        )
        .expect("Calibration should succeed");

        let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of)
            .expect("Valuator creation should succeed");

        let price = valuator.price().expect("pricing should succeed");

        // Price should be non-negative (it's an option)
        assert!(
            price >= 0.0,
            "Bermudan swaption price should be non-negative"
        );
    }

    #[test]
    fn exercise_dates_land_exactly_on_tree_grid() {
        let curve = test_discount_curve();
        let swaption = test_bermudan_swaption();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");
        let exercise_times = swaption.exercise_times(as_of).expect("exercise times");
        let model = CalibratedHullWhiteModel::calibrate_with_times(
            HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
            50,
            &curve,
            ttm,
            &exercise_times,
        )
        .expect("Calibration should succeed");

        // Every exercise time must coincide exactly with a tree grid point.
        let tree = model.tree();
        for &t in &exercise_times {
            let step = tree
                .step_at_time(t)
                .expect("exercise time must land on the grid");
            assert!(
                (tree.time_at_step(step) - t).abs() <= 1e-9,
                "exercise time {t} should land exactly on grid point {}",
                tree.time_at_step(step)
            );
        }

        // The valuator must price on the exercise-aligned grid.
        let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of)
            .expect("Valuator creation should succeed");
        let price = valuator.price().expect("pricing should succeed");
        assert!(price >= 0.0);
    }
}
