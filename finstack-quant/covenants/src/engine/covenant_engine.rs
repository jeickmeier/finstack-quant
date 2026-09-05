use super::helpers::{
    headroom_for, is_covenant_breached, spec_metric_names, springing_condition_met,
    InstrumentMutator, SpecEvaluation,
};
use super::types::{
    ConsequenceApplication, CovenantBreach, CovenantConsequence, CovenantSpec, CovenantWaiver,
    CovenantWindow,
};
use crate::metric::{CovenantMetricId, CovenantMetricSource};
use crate::CovenantReport;
use finstack_quant_core::dates::Date;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Covenant engine for evaluation and consequence application.
///
/// Only `specs` is required on the wire: `breach_history`, `windows` and
/// `waivers` default to empty, so `{"specs": [...]}` is a complete engine
/// document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantEngine {
    /// Active covenant specifications
    pub specs: Vec<CovenantSpec>,
    /// Historical breaches
    #[serde(default)]
    pub breach_history: Vec<CovenantBreach>,
    /// Covenant testing windows
    #[serde(default)]
    pub windows: Vec<CovenantWindow>,
    /// Active waivers and amendments
    #[serde(default)]
    pub waivers: Vec<CovenantWaiver>,
}

impl Default for CovenantEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CovenantEngine {
    /// Create a new covenant engine.
    pub fn new() -> Self {
        Self {
            specs: Vec::new(),
            breach_history: Vec::new(),
            windows: Vec::new(),
            waivers: Vec::new(),
        }
    }

    /// Validate the engine configuration before evaluation or JSON canonicalization.
    ///
    /// This checks every top-level and window-specific specification, verifies
    /// that testing windows are ordered, non-overlapping, and unique, and
    /// checks waiver date ranges and amended thresholds. It does not query
    /// metrics or evaluate a covenant; call this when accepting a package from
    /// a user, a file, or a binding before relying on its state.
    ///
    /// # Errors
    ///
    /// Returns a validation error when a contained specification is invalid, a
    /// window starts after it ends, two windows overlap or duplicate one
    /// another, a waiver expires before it takes effect, or an amended waiver
    /// threshold is non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        for spec in &self.specs {
            spec.validate()?;
        }
        for window in &self.windows {
            if window.start > window.end {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "covenant window start {} must be on or before end {}",
                    window.start, window.end
                )));
            }
            for spec in &window.covenants {
                spec.validate()?;
            }
        }
        for left_index in 0..self.windows.len() {
            for right_index in (left_index + 1)..self.windows.len() {
                let left = &self.windows[left_index];
                let right = &self.windows[right_index];
                if left.start <= right.end && left.end >= right.start {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "covenant windows must not overlap: [{}, {}] overlaps [{}, {}]",
                        left.start, left.end, right.start, right.end
                    )));
                }
            }
        }
        let mut seen_windows = BTreeSet::new();
        for window in &self.windows {
            let key = (window.start, window.end);
            if !seen_windows.insert(key) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "duplicate covenant window [{}, {}]",
                    window.start, window.end
                )));
            }
        }
        for waiver in &self.waivers {
            if waiver
                .expiry_date
                .is_some_and(|expiry| expiry < waiver.effective_date)
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "waiver '{}' expiry date must be on or after effective date",
                    waiver.covenant_id
                )));
            }
            if waiver
                .amended_threshold
                .is_some_and(|value| !value.is_finite())
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "waiver '{}' amended_threshold must be finite",
                    waiver.covenant_id
                )));
            }
        }
        Ok(())
    }

    /// Add a covenant specification.
    pub fn add_spec(&mut self, spec: CovenantSpec) -> &mut Self {
        self.specs.push(spec);
        self
    }

    /// Add a covenant window.
    ///
    /// Window overlap is validated by [`validate`](Self::validate) before
    /// evaluation and JSON canonicalization.
    pub fn add_window(&mut self, window: CovenantWindow) -> &mut Self {
        self.windows.push(window);
        self
    }

    /// Record a covenant waiver or amendment.
    pub fn add_waiver(&mut self, waiver: CovenantWaiver) -> &mut Self {
        self.waivers.push(waiver);
        self
    }

    /// Evaluate every applicable covenant against current metrics.
    ///
    /// This evaluates both maintenance and incurrence specifications. At
    /// `test_date`, a matching covenant window replaces
    /// the engine's top-level specification set. Results are keyed by stable
    /// covenant instance key, preserving separate labels for same-type tests.
    ///
    /// Inactive covenants, unmet springing conditions, and full waivers produce
    /// passing reports with explanatory details. An amended waiver instead
    /// changes the threshold used by the applicable evaluation.
    ///
    /// # Errors
    ///
    /// Returns an error when the engine configuration is invalid, applicable
    /// specifications have duplicate instance keys, the metric source cannot
    /// provide a required input, or a covenant cannot compute its test value
    /// from the supplied metrics.
    ///
    /// # Arguments
    ///
    /// * `context` - Metric source providing the operating values referenced by
    ///   each applicable specification. Keys are covenant metric identifiers
    ///   such as `debt_to_ebitda`; ratio values are in turns (`4.5` means 4.5x).
    /// * `test_date` - Date that selects the active window, waivers, threshold
    ///   schedule, and cure-period state.
    pub fn evaluate(
        &self,
        context: &dyn CovenantMetricSource,
        test_date: Date,
    ) -> finstack_quant_core::Result<IndexMap<String, CovenantReport>> {
        self.validate()?;
        let applicable_specs = self.applicable_specs(test_date);
        self.evaluate_specs(&applicable_specs, context, test_date)
    }

    fn evaluate_specs(
        &self,
        specs: &[&CovenantSpec],
        context: &dyn CovenantMetricSource,
        test_date: Date,
    ) -> finstack_quant_core::Result<IndexMap<String, CovenantReport>> {
        tracing::debug!(spec_count = specs.len(), %test_date, "evaluating covenants");

        let mut seen = BTreeSet::new();
        for spec in specs {
            let key = spec.covenant.instance_key();
            if !seen.insert(key.clone()) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "duplicate covenant instance key '{key}': covenants sharing a type must \
                     carry distinct labels",
                )));
            }
        }

        let mut reports = IndexMap::new();

        for spec in specs {
            let cid = spec.covenant.instance_key();
            let cid = cid.as_str();
            let description = spec.covenant.description();

            if !spec.covenant.is_active {
                reports.insert(
                    cid.to_string(),
                    CovenantReport::passed(&description)
                        .with_covenant_id(cid)
                        .with_details("Covenant inactive"),
                );
                continue;
            }

            if let Some(waiver) = self.active_waiver(cid, test_date) {
                if waiver.amended_threshold.is_none() {
                    tracing::info!(covenant_id = cid, %test_date, "covenant waived by lender agreement");
                    reports.insert(
                        cid.to_string(),
                        CovenantReport::passed(&description)
                            .with_covenant_id(cid)
                            .with_details("Waived by lender agreement"),
                    );
                    continue;
                }
            }

            let evaluation = self.evaluate_spec(spec, context, test_date)?;

            let mut report = if evaluation.passed {
                CovenantReport::passed(&description)
            } else {
                CovenantReport::failed(&description)
            };
            report = report.with_covenant_id(cid);

            if let Some(value) = evaluation.actual_value {
                report = report.with_actual(value);
            }
            if let Some(thresh) = evaluation.threshold {
                report = report.with_threshold(thresh);
            }
            if let Some(hr) = evaluation.headroom {
                report = report.with_headroom(hr);
            }

            if !evaluation.passed {
                tracing::warn!(
                    covenant_id = cid,
                    actual = evaluation.actual_value,
                    threshold = evaluation.threshold,
                    %test_date,
                    "covenant breach detected",
                );
                if let Some(breach) = self.find_active_breach(cid, test_date) {
                    if breach.cure_deadline.is_some_and(|d| test_date <= d) {
                        report = report.with_details("In cure period");
                    }
                }
            }

            if let Some(detail) = evaluation.detail {
                report = report.with_details(&detail);
            }

            reports.insert(cid.to_string(), report);
        }

        Ok(reports)
    }

    /// Evaluate covenants and update the engine's breach history.
    ///
    /// Combines [`evaluate`](Self::evaluate) with breach tracking: any failing
    /// covenant that doesn't already have an active (uncured) breach record
    /// gets a new [`CovenantBreach`] entry in `breach_history`. A later passing
    /// report cures the newest active breach only when its cure deadline has
    /// not elapsed. Repeated failures for the same still-active breach do not
    /// create duplicate records. This method does not apply consequences; pass
    /// the tracked breaches to [`apply_consequences`](Self::apply_consequences)
    /// after their cure periods have elapsed.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`evaluate`](Self::evaluate). On error, no
    /// breach-history update occurs because evaluation completes before the
    /// mutation phase begins.
    ///
    /// # Arguments
    ///
    /// * `context` - Metric source providing the operating values referenced by
    ///   each applicable specification.
    /// * `test_date` - Date that selects the active window, waivers, threshold
    ///   schedule, and cure-period state, and that stamps new breach records.
    pub fn evaluate_and_track(
        &mut self,
        context: &dyn CovenantMetricSource,
        test_date: Date,
    ) -> finstack_quant_core::Result<IndexMap<String, CovenantReport>> {
        let reports = self.evaluate(context, test_date)?;

        for (cid, report) in &reports {
            if !report.passed {
                continue;
            }
            if let Some(breach) = self
                .breach_history
                .iter_mut()
                .filter(|b| b.covenant_id == *cid && !b.is_cured && b.breach_date <= test_date)
                .max_by_key(|b| b.breach_date)
            {
                if breach
                    .cure_deadline
                    .is_some_and(|deadline| test_date <= deadline)
                {
                    tracing::info!(
                        covenant_id = %cid,
                        breach_date = %breach.breach_date,
                        %test_date,
                        "marking covenant breach cured by metric recovery",
                    );
                    breach.is_cured = true;
                }
            }
        }

        for (cid, report) in &reports {
            if report.passed {
                continue;
            }

            let description = report.covenant_type.clone();

            let already_tracked = self
                .breach_history
                .iter()
                .any(|b| b.covenant_id == *cid && !b.is_cured && b.breach_date <= test_date);
            if already_tracked {
                continue;
            }

            let spec = self
                .specs
                .iter()
                .find(|s| s.covenant.instance_key() == *cid);

            let cure_deadline = spec.and_then(|s| {
                s.covenant
                    .cure_period_days
                    .map(|d| test_date + time::Duration::days(d as i64))
            });

            tracing::warn!(
                covenant_id = %cid,
                actual = report.actual_value,
                threshold = report.threshold,
                %test_date,
                "recording new covenant breach",
            );

            self.breach_history.push(CovenantBreach {
                covenant_id: cid.clone(),
                covenant_type: description,
                breach_date: test_date,
                actual_value: report.actual_value,
                threshold: report.threshold,
                cure_deadline,
                is_cured: false,
                applied_consequences: Vec::new(),
            });
        }

        Ok(reports)
    }

    /// Apply eligible consequences for the supplied breach records.
    ///
    /// Consequences that have already been applied (recorded in `breach_history`)
    /// are skipped to prevent double-application. Cured breaches and breaches
    /// still inside their cure period are also skipped. Each successful
    /// application is returned and recorded against the matching historical
    /// breach, making repeated calls idempotent for that breach date and
    /// covenant instance.
    ///
    /// `breaches` should normally be drawn from [`breach_history`](Self::breach_history)
    /// after [`evaluate_and_track`](Self::evaluate_and_track). A supplied
    /// breach must identify an existing specification by its instance key.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::NotFound`] if an eligible
    /// breach has no matching covenant specification. It also propagates errors
    /// from the [`InstrumentMutator`] while applying a configured consequence;
    /// callers should treat a returned error as a potentially partial mutation
    /// and reconcile the instrument before retrying.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Target implementing [`InstrumentMutator`]; each eligible
    ///   consequence mutates it in place (rate increase, cash sweep, default
    ///   flag, distribution block, or maturity acceleration).
    /// * `breaches` - Breach records to consider, typically from
    ///   [`Self::breach_history`] after [`Self::evaluate_and_track`].
    /// * `as_of` - Date used to decide whether a cure period has elapsed and to
    ///   stamp each application.
    pub fn apply_consequences<T>(
        &mut self,
        instrument: &mut T,
        breaches: &[CovenantBreach],
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<ConsequenceApplication>>
    where
        T: InstrumentMutator,
    {
        let mut applications = Vec::new();

        for breach in breaches {
            if breach.is_cured {
                continue;
            }
            if let Some(deadline) = breach.cure_deadline {
                if as_of <= deadline {
                    continue;
                }
            }

            let already_applied = self.breach_history.iter().any(|b| {
                b.covenant_id == breach.covenant_id
                    && b.breach_date == breach.breach_date
                    && !b.applied_consequences.is_empty()
            });
            if already_applied {
                tracing::debug!(
                    covenant_id = %breach.covenant_id,
                    breach_date = %breach.breach_date,
                    "skipping consequence application — already applied",
                );
                continue;
            }

            let spec = self
                .specs
                .iter()
                .find(|s| s.covenant.instance_key() == breach.covenant_id)
                .ok_or(finstack_quant_core::InputError::NotFound {
                    id: format!("covenant_spec:{}", breach.covenant_id),
                })?;

            for consequence in &spec.covenant.consequences {
                let application = self.apply_single_consequence(instrument, consequence, as_of)?;
                tracing::info!(
                    covenant_id = %breach.covenant_id,
                    consequence = %application.consequence_type,
                    %as_of,
                    "applied covenant consequence",
                );
                applications.push(application);

                if let Some(historical_breach) = self.breach_history.iter_mut().find(|b| {
                    b.covenant_id == breach.covenant_id && b.breach_date == breach.breach_date
                }) {
                    historical_breach
                        .applied_consequences
                        .push(consequence.clone());
                }
            }
        }

        Ok(applications)
    }

    fn applicable_specs(&self, test_date: Date) -> Vec<&CovenantSpec> {
        for window in &self.windows {
            if test_date >= window.start && test_date <= window.end {
                return window.covenants.iter().collect();
            }
        }
        self.specs.iter().collect()
    }

    fn evaluate_spec(
        &self,
        spec: &CovenantSpec,
        context: &dyn CovenantMetricSource,
        test_date: Date,
    ) -> finstack_quant_core::Result<SpecEvaluation> {
        if let Some(condition) = &spec.covenant.springing_condition {
            let condition_value = context.get_metric(&condition.metric_id)?;
            if !springing_condition_met(
                condition.metric_id.as_str(),
                condition_value,
                condition.test,
            ) {
                tracing::debug!(
                    metric = condition.metric_id.as_str(),
                    value = condition_value,
                    "springing condition not met — covenant inactive",
                );
                return Ok(SpecEvaluation {
                    passed: true,
                    actual_value: None,
                    threshold: None,
                    headroom: None,
                    detail: Some("Springing condition not met".to_string()),
                });
            }
        }

        let covenant_type = &spec.covenant.covenant_type;

        let Some(base_threshold) = covenant_type.threshold_value() else {
            return Ok(SpecEvaluation {
                passed: true,
                actual_value: None,
                threshold: None,
                headroom: None,
                detail: None,
            });
        };

        let covenant_cid = spec.covenant.instance_key();
        let threshold = self
            .active_waiver(&covenant_cid, test_date)
            .and_then(|w| w.amended_threshold)
            .or_else(|| {
                spec.threshold_schedule
                    .as_ref()
                    .and_then(|s| s.threshold_for(test_date))
            })
            .unwrap_or(base_threshold);

        let Some(metric_name) = spec_metric_names(spec).into_iter().next() else {
            return Err(finstack_quant_core::Error::Validation(format!(
                "covenant '{}' has a threshold but no metric name",
                spec.covenant.description(),
            )));
        };
        let metric_value = context.get_metric(&CovenantMetricId::from(metric_name))?;

        let detail = (covenant_type.is_ratio_max() && metric_value < 0.0).then(|| {
            "Negative ratio value (negative denominator) — not meaningful, treated as breach"
                .to_string()
        });
        let passed = !is_covenant_breached(covenant_type, metric_value, threshold);

        let headroom = Some(headroom_for(
            covenant_type.bound_kind(),
            metric_value,
            threshold,
        ));

        Ok(SpecEvaluation {
            passed,
            actual_value: Some(metric_value),
            threshold: Some(threshold),
            headroom,
            detail,
        })
    }

    fn active_waiver(&self, covenant_id: &str, as_of: Date) -> Option<&CovenantWaiver> {
        self.waivers.iter().find(|w| {
            w.covenant_id == covenant_id
                && w.effective_date <= as_of
                && w.expiry_date.is_none_or(|exp| as_of <= exp)
        })
    }

    fn find_active_breach(&self, cid: &str, as_of: Date) -> Option<&CovenantBreach> {
        self.breach_history
            .iter()
            .filter(|b| b.covenant_id == cid && !b.is_cured)
            .filter(|b| b.breach_date <= as_of)
            .max_by_key(|b| b.breach_date)
    }

    fn apply_single_consequence<T>(
        &self,
        instrument: &mut T,
        consequence: &CovenantConsequence,
        as_of: Date,
    ) -> finstack_quant_core::Result<ConsequenceApplication>
    where
        T: InstrumentMutator,
    {
        match consequence {
            CovenantConsequence::Default => {
                instrument.set_default_status(true, as_of)?;
                Ok(ConsequenceApplication {
                    consequence_type: "default".to_string(),
                    applied_date: as_of,
                    details: "Loan in default".to_string(),
                })
            }
            CovenantConsequence::RateIncrease { bp_increase } => {
                instrument.increase_rate(*bp_increase / 10000.0)?;
                Ok(ConsequenceApplication {
                    consequence_type: "rate_increase".to_string(),
                    applied_date: as_of,
                    details: format!("Rate increased by {} bp", bp_increase),
                })
            }
            CovenantConsequence::CashSweep { sweep_percentage } => {
                instrument.set_cash_sweep(*sweep_percentage)?;
                Ok(ConsequenceApplication {
                    consequence_type: "cash_sweep".to_string(),
                    applied_date: as_of,
                    details: format!("{}% cash sweep activated", sweep_percentage * 100.0),
                })
            }
            CovenantConsequence::BlockDistributions => {
                instrument.set_distribution_block(true)?;
                Ok(ConsequenceApplication {
                    consequence_type: "block_distributions".to_string(),
                    applied_date: as_of,
                    details: "Distributions blocked".to_string(),
                })
            }
            CovenantConsequence::RequireCollateral { description } => Ok(ConsequenceApplication {
                consequence_type: "require_collateral".to_string(),
                applied_date: as_of,
                details: description.clone(),
            }),
            CovenantConsequence::AccelerateMaturity { new_maturity } => {
                instrument.set_maturity(*new_maturity)?;
                Ok(ConsequenceApplication {
                    consequence_type: "accelerate_maturity".to_string(),
                    applied_date: as_of,
                    details: format!("Maturity accelerated to {}", new_maturity),
                })
            }
        }
    }
}
