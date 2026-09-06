use super::helpers::{
    headroom_for, is_covenant_breached, spec_metric_name, springing_condition_met, validate_metric,
    validated_ratio_value, InstrumentMutator, SpecEvaluation,
};
use super::types::{
    ConsequenceApplication, CovenantBreach, CovenantConsequence, CovenantScope, CovenantSpec,
    CovenantWaiver, CovenantWindow,
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
        for (i, waiver) in self.waivers.iter().enumerate() {
            for other in &self.waivers[i + 1..] {
                if waiver.covenant_id == other.covenant_id
                    && waiver.effective_date <= other.expiry_date.unwrap_or(Date::MAX)
                    && other.effective_date <= waiver.expiry_date.unwrap_or(Date::MAX)
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "overlapping waivers for covenant '{}'",
                        waiver.covenant_id
                    )));
                }
            }
        }
        for specs in std::iter::once(&self.specs).chain(self.windows.iter().map(|w| &w.covenants)) {
            let mut keys = BTreeSet::new();
            for spec in specs {
                if !keys.insert(&spec.covenant.label) {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "duplicate covenant instance key '{}'",
                        spec.covenant.label
                    )));
                }
            }
        }
        let mut episodes = BTreeSet::new();
        for breach in &self.breach_history {
            if !episodes.insert((&breach.covenant_id, breach.breach_date))
                || breach.covenant_id.trim().is_empty()
                || breach.cure_deadline.is_some_and(|d| d < breach.breach_date)
                || !breach
                    .consequences
                    .starts_with(&breach.applied_consequences)
            {
                return Err(finstack_quant_core::Error::Validation(
                    "invalid breach identity, cure deadline, or consequence progress".into(),
                ));
            }
            for consequence in &breach.consequences {
                consequence.validate()?;
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
    /// This evaluates both maintenance and incurrence specifications as snapshot
    /// compliance/capacity checks; it does not record an executed incurrence action. At
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
    /// * `scope` - `Maintenance` for scheduled compliance, or `Incurrence` only
    ///   when testing a completed action with its pro forma metrics. A failed
    ///   hypothetical capacity check must not be recorded as an actual breach.
    pub fn evaluate_and_track(
        &mut self,
        context: &dyn CovenantMetricSource,
        test_date: Date,
        scope: CovenantScope,
    ) -> finstack_quant_core::Result<IndexMap<String, CovenantReport>> {
        self.validate()?;
        let specs: Vec<_> = self
            .applicable_specs(test_date)
            .into_iter()
            .filter(|spec| spec.covenant.scope == scope)
            .collect();
        let reports = self.evaluate_specs(&specs, context, test_date)?;
        let mut new_breaches = Vec::new();
        for spec in specs {
            let cid = spec.covenant.instance_key();
            let report = &reports[&cid];
            if report.passed || self.find_active_breach(&cid, test_date).is_some() {
                continue;
            }
            let cure_deadline = spec
                .covenant
                .cure_period_days
                .map(|days| {
                    test_date
                        .checked_add(time::Duration::days(i64::from(days)))
                        .ok_or_else(|| {
                            finstack_quant_core::Error::Validation(
                                "cure deadline exceeds supported date range".into(),
                            )
                        })
                })
                .transpose()?;
            new_breaches.push(CovenantBreach {
                covenant_id: cid,
                covenant_type: report.covenant_type.clone(),
                breach_date: test_date,
                actual_value: report.actual_value,
                threshold: report.threshold,
                cure_deadline,
                is_cured: false,
                consequences: spec.covenant.consequences.clone(),
                applied_consequences: Vec::new(),
            });
        }
        for (cid, report) in &reports {
            if !report.passed || !report.actual_value.is_some_and(f64::is_finite) {
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
                    breach.is_cured = true;
                }
            }
        }
        self.breach_history.extend(new_breaches);

        Ok(reports)
    }

    /// Apply outstanding consequences from the current historical breach state.
    ///
    /// Supplied snapshots identify historical episodes by covenant id and breach
    /// date. Eligibility and progress come from current history. Each episode
    /// retains its original consequence order independently of later amendments.
    /// Successfully completed actions form a prefix; retries resume at the first
    /// outstanding action. Cured or future breaches and unexpired cure periods
    /// are skipped.
    ///
    /// Each action is applied to a clone and committed only after success. Earlier
    /// successful actions remain committed if a later action fails. Mutators must
    /// only change their own state, with no external side effects.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Cloneable target whose state is committed after each
    ///   successful default, rate, sweep, distribution, collateral, or maturity action.
    /// * `breaches` - Snapshots identifying existing historical episodes; stale
    ///   flags and consequence lists do not override current history.
    /// * `as_of` - Eligibility and execution date; actions cannot precede the
    ///   breach or execute on or before its inclusive cure deadline.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` for an unknown historical episode, validation errors
    /// for malformed history or terms, and the target's error for an unsupported
    /// or failed action. A failed action remains outstanding and changes no target state.
    pub fn apply_consequences<T>(
        &mut self,
        instrument: &mut T,
        breaches: &[CovenantBreach],
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<ConsequenceApplication>>
    where
        T: InstrumentMutator + Clone,
    {
        self.validate()?;
        let indices = breaches
            .iter()
            .map(|breach| {
                self.breach_history
                    .iter()
                    .position(|historical| {
                        historical.covenant_id == breach.covenant_id
                            && historical.breach_date == breach.breach_date
                    })
                    .ok_or_else(|| {
                        finstack_quant_core::Error::from(
                            finstack_quant_core::InputError::NotFound {
                                id: format!(
                                    "covenant_breach:{}:{}",
                                    breach.covenant_id, breach.breach_date
                                ),
                            },
                        )
                    })
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        let mut applications = Vec::new();
        for index in indices {
            let breach = &self.breach_history[index];
            if breach.is_cured
                || as_of < breach.breach_date
                || breach
                    .cure_deadline
                    .is_some_and(|deadline| as_of <= deadline)
            {
                continue;
            }
            loop {
                let next = {
                    let breach = &self.breach_history[index];
                    breach
                        .consequences
                        .get(breach.applied_consequences.len())
                        .cloned()
                };
                let Some(consequence) = next else {
                    break;
                };
                let mut candidate = instrument.clone();
                let application =
                    self.apply_single_consequence(&mut candidate, &consequence, as_of)?;
                *instrument = candidate;
                self.breach_history[index]
                    .applied_consequences
                    .push(consequence);
                applications.push(application);
            }
        }

        Ok(applications)
    }

    pub(crate) fn applicable_specs(&self, test_date: Date) -> Vec<&CovenantSpec> {
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
            validate_metric(condition.metric_id.as_str(), condition_value)?;
            if !springing_condition_met(condition_value, condition.test) {
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

        let threshold = self
            .effective_threshold(spec, test_date)
            .unwrap_or(base_threshold);

        let Some(metric_name) = spec_metric_name(spec) else {
            return Err(finstack_quant_core::Error::Validation(format!(
                "covenant '{}' has a threshold but no metric name",
                spec.covenant.description(),
            )));
        };
        let raw_value = context.get_metric(&CovenantMetricId::from(metric_name))?;
        validate_metric(metric_name, raw_value)?;
        let denominator = match &spec.denominator_metric_id {
            Some(id) => {
                let value = context.get_metric(id)?;
                validate_metric(id.as_str(), value)?;
                Some(value)
            }
            None => None,
        };
        let metric_value = validated_ratio_value(spec, raw_value, denominator);

        let detail = metric_value.is_nan().then(||
            "Non-positive earnings denominator or invalid gross leverage ratio; not meaningful, treated as breach".to_string());
        let passed = !is_covenant_breached(covenant_type, metric_value, threshold);

        let raw_headroom = headroom_for(covenant_type.bound_kind(), metric_value, threshold);
        let headroom = raw_headroom.is_finite().then_some(raw_headroom);

        Ok(SpecEvaluation {
            passed,
            actual_value: metric_value.is_finite().then_some(metric_value),
            threshold: Some(threshold),
            headroom,
            detail,
        })
    }

    pub(crate) fn effective_threshold(&self, spec: &CovenantSpec, date: Date) -> Option<f64> {
        self.active_waiver(&spec.covenant.label, date)
            .and_then(|w| w.amended_threshold)
            .or_else(|| {
                spec.threshold_schedule
                    .as_ref()
                    .and_then(|s| s.threshold_for(date))
            })
            .or_else(|| spec.covenant.covenant_type.threshold_value())
    }

    pub(crate) fn active_waiver(&self, covenant_id: &str, as_of: Date) -> Option<&CovenantWaiver> {
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
            CovenantConsequence::RequireCollateral { description } => {
                instrument.require_collateral(description, as_of)?;
                Ok(ConsequenceApplication {
                    consequence_type: "require_collateral".to_string(),
                    applied_date: as_of,
                    details: description.clone(),
                })
            }
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
