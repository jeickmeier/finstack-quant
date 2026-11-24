//! Tree valuator for Bermudan swaption pricing.
//!
//! Implements backward induction with optimal exercise decisions at each
//! Bermudan exercise date using the Hull-White tree.
//!
//! # Algorithm
//!
//! At each tree node and time step:
//! 1. Compute continuation value (discounted expected value from child nodes)
//! 2. At exercise dates, compute the signed exercise value (S - K) × A × N for payer
//! 3. Take max(continuation, exercise) for optimal exercise decision
//!
//! # Usage
//!
//! ```text
//! use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
//! use finstack_quant_valuations_rates::swaption::{BermudanSwaption, PreparedHullWhiteModel, pricing::BermudanSwaptionTreeValuator};
//!
//! let swaption = BermudanSwaption::example().expect("example");
//! # let discount_curve: &finstack_quant_core::market_data::term_structures::DiscountCurve = todo!();
//! # let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
//!
//! // Create calibrated model
//! let ttm = swaption.time_to_maturity(as_of).unwrap();
//! let model = PreparedHullWhiteModel::prepare(
//!     HullWhiteCalibrationParams::default(),
//!     100,
//!     discount_curve,
//!     as_of,
//!     ttm,
//!     &swaption.exercise_times(as_of).unwrap(),
//! ).unwrap();
//!
//! // Create valuator and price
//! let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, discount_curve, as_of).unwrap();
//! let price = valuator.price().unwrap();
//! ```
use super::hw_cashflows::{HwExerciseNode, HwExerciseTerms, HwSwaptionCashflows};
use crate::swaption::BermudanSwaption;
use crate::swaption::PreparedHullWhiteModel;
use crate::swaption::types::BermudanType;
use finstack_quant_core::FxHashSet;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_models::rates::clock::{ModelDiscountCurve, model_time};
use finstack_quant_models::trees::kink_smoothing::smoothed_max;

/// Tree valuator for Bermudan swaption pricing.
///
/// Uses a calibrated Hull-White tree to price Bermudan swaptions via
/// backward induction with optimal exercise decisions.
pub struct BermudanSwaptionTreeValuator<'a> {
    /// Reference to the Bermudan swaption
    swaption: &'a BermudanSwaption,
    /// Reference to the calibrated Hull-White model
    model: &'a PreparedHullWhiteModel,
    discount_curve: ModelDiscountCurve<'a>,
    exercise_steps: FxHashSet<usize>,
    exercise_cashflows: finstack_quant_core::FxHashMap<usize, HwSwaptionCashflows>,
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
        model: &'a PreparedHullWhiteModel,
        discount_curve: &'a DiscountCurve,
        as_of: Date,
    ) -> Result<Self> {
        if swaption.get_forward_curve_id() != swaption.get_discount_curve_id() {
            return Err(finstack_quant_core::Error::Validation(
                super::bermudan::single_curve_only_message("Bermudan tree"),
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

        model.validate_reuse(discount_curve, as_of, swaption.time_to_maturity(as_of)?)?;
        let tree = model.tree();
        let exercise_times = swaption.exercise_times(as_of)?;
        let exercise_steps: FxHashSet<usize> = exercise_times
            .iter()
            .map(|&t| tree.step_at_time(t))
            .collect::<Result<_>>()?;
        let mut exercise_cashflows = finstack_quant_core::FxHashMap::default();
        for &date in &swaption.exercise_schedule.exercise_dates {
            let time = model_time(as_of, date);
            if !exercise_times.iter().any(|&t| (t - time).abs() < 1e-10) {
                continue;
            }
            let step = tree.step_at_time(time)?;
            let flows = HwSwaptionCashflows::new(
                &swaption.underlying_fixed_leg,
                &swaption.underlying_float_leg,
                date,
                as_of,
                swaption.settlement,
                crate::swaption::CashSettlementMethod::default(),
            )?;
            model.validate_reuse(discount_curve, as_of, flows.horizon())?;
            exercise_cashflows.insert(step, flows);
        }
        let strike = swaption.strike_f64()?;

        Ok(Self {
            swaption,
            model,
            discount_curve: ModelDiscountCurve::new(discount_curve, as_of)?,
            exercise_steps,
            exercise_cashflows,
            strike,
        })
    }

    fn tree(&self) -> &finstack_quant_models::trees::hull_white_tree::HullWhiteTree {
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
        self.tree()
            .backward_induction(&self.terminal_values(n), |step, values| {
                if self.exercise_steps.contains(&step) {
                    smoothed_max(values, &self.exercise_values(step, values.len()));
                }
            })
    }

    /// Exercise values across one tree level, ordered by state.
    fn exercise_values(&self, step: usize, num_nodes: usize) -> Vec<f64> {
        (0..num_nodes)
            .map(|node_idx| self.exercise_value(step, node_idx))
            .collect()
    }

    /// Option values at the final step: the kink-smoothed positive part of
    /// the exercise value when that step is an exercise date, zero otherwise.
    fn terminal_values(&self, final_step: usize) -> Vec<f64> {
        let num_nodes = self.tree().num_nodes(final_step);
        let mut terminal = vec![0.0; num_nodes];
        if self.exercise_steps.contains(&final_step) {
            smoothed_max(&mut terminal, &self.exercise_values(final_step, num_nodes));
        }
        terminal
    }

    /// Compute exercise value at a node.
    ///
    /// For a payer swaption: (S - K) × A × N, for a receiver (K - S) × A × N.
    /// The value is signed; the level-wide exercise decision floors it.
    fn exercise_value(&self, step: usize, node_idx: usize) -> f64 {
        self.exercise_cashflows.get(&step).map_or(0.0, |flows| {
            flows.value(
                HwExerciseNode {
                    tree: self.tree(),
                    step,
                    index: node_idx,
                    discount: &self.discount_curve,
                },
                HwExerciseTerms {
                    strike: self.strike,
                    notional: self.swaption.notional.amount(),
                    option_type: self.swaption.option_type,
                },
            )
        })
    }

    /// Compute exercise probabilities at each exercise date.
    ///
    /// Returns a vector of (time, probability) pairs representing the
    /// risk-neutral probability of exercise at each date.
    ///
    /// Uses a forward pass tracking survival probabilities through the tree,
    /// deducting mass at nodes where optimal exercise occurs.
    ///
    /// # Errors
    ///
    /// Propagates tree backward-induction validation failures.
    pub fn exercise_probabilities(&self) -> Result<Vec<(f64, f64)>> {
        let n = self.tree().num_steps();

        // 1. Backward pass: record the continuation value at every exercise
        //    step before the exercise decision is applied to it.
        let mut exercise_cont_values: finstack_quant_core::FxHashMap<usize, Vec<f64>> =
            finstack_quant_core::FxHashMap::default();
        self.tree()
            .backward_induction(&self.terminal_values(n), |step, values| {
                if self.exercise_steps.contains(&step) {
                    exercise_cont_values.insert(step, values.to_vec());
                    smoothed_max(values, &self.exercise_values(step, values.len()));
                }
            })?;

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

        Ok(exercise_probs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swaption::{BermudanSchedule, BermudanSwaption, PreparedHullWhiteModel};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
    use finstack_quant_models::types::OptionType;
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
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("Valid curve")
    }

    fn test_bermudan_swaption() -> BermudanSwaption {
        let swap_start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
        let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

        BermudanSwaption::new(crate::swaption::BermudanSwaptionParams {
            id: "TEST-BERM".into(),
            option_type: OptionType::Call,
            notional: Money::from((10_000_000_i64, Currency::USD)),
            strike: 0.03,
            underlying_start_date: swap_start,
            underlying_maturity: swap_end,
            exercise_schedule: BermudanSchedule::co_terminal(
                first_exercise,
                swap_end,
                Tenor::semi_annual(),
            )
            .expect("valid Bermudan schedule"),
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            vol_surface_id: "USD-VOL".into(),
        })
        .expect("valid Bermudan swaption")
    }

    #[test]
    fn test_valuator_creation() {
        let curve = test_discount_curve();
        let swaption = test_bermudan_swaption();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");
        let model = PreparedHullWhiteModel::prepare(
            HullWhiteCalibrationParams::new(0.03, 0.01).expect("valid HW params"),
            50,
            &curve,
            as_of,
            ttm,
            &swaption.exercise_times(as_of).expect("exercise times"),
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
        let model = PreparedHullWhiteModel::prepare(
            HullWhiteCalibrationParams::new(0.03, 0.01).expect("valid HW params"),
            50,
            &curve,
            as_of,
            ttm,
            &swaption.exercise_times(as_of).expect("exercise times"),
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
        let model = PreparedHullWhiteModel::prepare(
            HullWhiteCalibrationParams::new(0.03, 0.01).expect("valid HW params"),
            50,
            &curve,
            as_of,
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
