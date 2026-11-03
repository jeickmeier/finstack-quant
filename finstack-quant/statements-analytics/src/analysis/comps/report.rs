//! Relative value report: the comps table, peer statistics, regression scatter
//! and composite rich/cheap score of one subject against its filtered peers, in
//! one result.

use super::multiples::compute_multiple;
use super::peer_set::{PeerFilter, PeerSet};
#[cfg(doc)]
use super::scoring::score_relative_value;
use super::scoring::{
    evaluate_dimension, is_finite_score, score_relative_value_unchecked,
    validate_dimension_weights, DimensionExclusion, MetricExtractor, RelativeValueResult,
    ScoreDirection, ScoringDimension,
};
use super::stats::{peer_stats, percentile_rank, z_score, PeerStats, RegressionResult};
use super::types::{CompanyId, CompanyMetrics, Multiple, PeriodBasis};
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// Request for [`relative_value_report`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RelativeValueReportConfig {
    /// Screen applied to the universe. Omitted, every other company is a peer.
    /// Any [`PeerFilter`] field may be omitted, meaning no constraint on it; an
    /// unknown field is rejected.
    #[serde(default)]
    pub filter: PeerFilter,
    /// Period basis the supplied metrics are stated on. It labels the report;
    /// the metrics are used as given.
    pub period_basis: PeriodBasis,
    /// Scoring dimensions, in the order the report returns them. May be empty
    /// when `multiples` is not.
    pub dimensions: Vec<ScoringDimension>,
    /// Columns of the comps table, in the order the report returns them. May be
    /// empty when `dimensions` is not.
    pub multiples: Vec<Multiple>,
}

/// One company's row in the comps table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RelativeValueCompany {
    /// Company identifier.
    pub id: CompanyId,
    /// Whether this row is the subject.
    pub is_subject: bool,
    /// One value per configured multiple, in config order; `None` when an input
    /// is missing, the denominator is not positive or the value is non-finite.
    pub values: Vec<Option<f64>>,
}

/// One column of the comps table, summarised over the peers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RelativeValueMultiple {
    /// The multiple.
    pub multiple: Multiple,
    /// Subject's value; `None` when it cannot be computed.
    pub subject_value: Option<f64>,
    /// Statistics of the peers' computable values; `None` when no peer has one.
    pub peer_stats: Option<PeerStats>,
    /// Share of those peer values at or below the subject's, from 0 to 1;
    /// `None` without a subject value or peer values.
    pub percentile: Option<f64>,
    /// Subject's z-score within those peer values; `None` without a subject
    /// value, with fewer than two peer values or when they do not differ.
    pub z_score: Option<f64>,
}

/// One company on a dimension's regression scatter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RelativeValuePoint {
    /// Company identifier.
    pub company_id: CompanyId,
    /// X metric.
    pub x: f64,
    /// Y metric.
    pub y: f64,
    /// `intercept + slope * x` of the dimension's regression; `None` without a
    /// regression.
    pub fitted_y: Option<f64>,
}

/// One configured scoring dimension, used by the composite or not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RelativeValueDimension {
    /// Dimension label.
    pub label: String,
    /// Configured weight, not normalized.
    pub weight: f64,
    /// Rich/cheap sign convention of the Y metric.
    pub direction: ScoreDirection,
    /// Y metric extractor.
    pub y: MetricExtractor,
    /// X metric extractor; `None` for a dimension scored on its peer
    /// distribution.
    pub x: Option<MetricExtractor>,
    /// Subject's Y metric; `None` when missing or non-finite.
    pub subject_y: Option<f64>,
    /// Subject's X metric; `None` without an X extractor or when missing or
    /// non-finite.
    pub subject_x: Option<f64>,
    /// Statistics of Y over every peer that has it, including peers missing X;
    /// `None` when no peer has it.
    pub peer_stats: Option<PeerStats>,
    /// Share of those peer Y values at or below the subject's, from 0 to 1.
    pub percentile: Option<f64>,
    /// Subject's raw z-score within those peer Y values, before the direction
    /// convention is applied.
    pub z_score: Option<f64>,
    /// Fit of Y on X over the peers that have both, evaluated at the subject:
    /// `fitted_value` is the subject's fitted Y and `residual` its actual less
    /// fitted, in Y units. `None` without an X extractor, without both subject
    /// metrics, or when it cannot be fitted. It can be present on an excluded
    /// dimension.
    pub regression: Option<RegressionResult>,
    /// What this dimension contributes to the composite, positive meaning cheap:
    /// the standardized regression residual, or the z-score without an X
    /// extractor, with the sign set by `direction`. `None` when excluded.
    pub signal: Option<f64>,
    /// Why the composite leaves this dimension out; `None` when it is used.
    pub excluded_reason: Option<DimensionExclusion>,
    /// Regression scatter: the subject first when it has both metrics, then
    /// every peer with both metrics in universe order. Empty without an X
    /// extractor.
    pub points: Vec<RelativeValuePoint>,
}

/// Why a [`RelativeValueReport`] has no composite score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CompositeExclusion {
    /// The config has no dimensions.
    NoDimensions,
    /// Every configured dimension is excluded; each says why.
    NoUsableDimension,
    /// Usable dimensions combine to a value too large to represent.
    NonFiniteScore,
}

/// Comps table, peer statistics, regression scatter and composite score of one
/// subject. Returned by [`relative_value_report`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RelativeValueReport {
    /// Subject company identifier.
    pub subject_id: CompanyId,
    /// Period basis from the config.
    pub period_basis: PeriodBasis,
    /// Number of peers, excluding the subject.
    pub peer_count: usize,
    /// Universe members the filter dropped, not counting the subject.
    pub excluded_count: usize,
    /// Comps table: the subject first, then the peers in universe order.
    pub companies: Vec<RelativeValueCompany>,
    /// One entry per configured multiple, in config order.
    pub multiples: Vec<RelativeValueMultiple>,
    /// One entry per configured dimension, in config order, including excluded
    /// ones.
    pub dimensions: Vec<RelativeValueDimension>,
    /// Result of [`score_relative_value`] over the same peers and dimensions;
    /// `None` when `composite_excluded_reason` is set.
    pub composite: Option<RelativeValueResult>,
    /// Why there is no composite; `None` when there is one.
    pub composite_excluded_reason: Option<CompositeExclusion>,
}

/// Compare one subject with its filtered peers: comps table, peer statistics,
/// regression scatter and composite rich/cheap score in one result.
///
/// Composes [`PeerSet::from_universe`], [`compute_multiple`], [`peer_stats`],
/// [`percentile_rank`], [`z_score`], the per-dimension evaluation of
/// [`score_relative_value`] and that function itself, so a used dimension and
/// the composite are exactly what `score_relative_value` returns on the same
/// inputs. Where that function fails because no dimension is usable, this one
/// still returns the table, with `composite` set to `None`.
///
/// Monetary metrics carry no currency, so every company's must already be in
/// one currency: normalization is the caller's. Rows are reported as supplied,
/// so a duplicated peer id gives two rows.
///
/// # Arguments
///
/// * `subject` - Company being evaluated. Its metrics are used as given and it
///   need not be in `universe`.
/// * `universe` - Candidate peers in the order the report returns them. A member
///   with the subject's id is never a peer and does not count as excluded.
/// * `config` - Filter, period basis, scoring dimensions (weights finite and
///   non-negative; zero disables a dimension) and the multiples to tabulate.
///
/// # Returns
///
/// The report: the subject first in `companies`, multiples and dimensions in
/// config order, and each dimension's exclusion reason when it has one.
///
/// # Errors
///
/// Returns a validation error when `config.dimensions` and `config.multiples`
/// are both empty, or a dimension weight is negative or non-finite.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_statements_analytics::analysis::{
///     relative_value_report, CompanyMetrics, MetricExtractor, Multiple, PeerFilter, PeriodBasis,
///     RelativeValueReportConfig, ScoreDirection, ScoringDimension,
/// };
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let company = |id: &str, leverage: f64, oas_bp: f64| {
///     let mut metrics = CompanyMetrics::new(id);
///     metrics.leverage = Some(leverage);
///     metrics.oas_bp = Some(oas_bp);
///     metrics
/// };
/// let universe = [
///     company("P1", 1.0, 100.0),
///     company("P2", 2.0, 200.0),
///     company("P3", 3.0, 310.0),
///     company("P4", 4.0, 380.0),
/// ];
/// let report = relative_value_report(
///     &company("SUBJECT", 2.5, 300.0),
///     &universe,
///     &RelativeValueReportConfig {
///         filter: PeerFilter::default(),
///         period_basis: PeriodBasis::Ltm,
///         dimensions: vec![ScoringDimension {
///             label: "Spread vs Leverage".to_string(),
///             y_extractor: MetricExtractor::Named("oas_bp".to_string()),
///             x_extractor: Some(MetricExtractor::Named("leverage".to_string())),
///             weight: 1.0,
///             direction: ScoreDirection::HigherIsCheap,
///         }],
///         multiples: vec![Multiple::SpreadPerTurn],
///     },
/// )?;
/// assert_eq!(report.companies[0].values, vec![Some(120.0)]); // 300 / 2.5
/// let p1 = &report.dimensions[0].points[1];
/// assert!((p1.fitted_y.unwrap() - 105.0).abs() < 1e-9); // 10 + 95 * 1
/// assert!(report.composite.is_some_and(|composite| composite.composite_score > 0.0));
/// # Ok(())
/// # }
/// ```
pub fn relative_value_report(
    subject: &CompanyMetrics,
    universe: &[CompanyMetrics],
    config: &RelativeValueReportConfig,
) -> Result<RelativeValueReport> {
    if config.dimensions.is_empty() && config.multiples.is_empty() {
        return Err(Error::Validation(
            "a relative value report needs at least one scoring dimension or multiple".into(),
        ));
    }
    validate_dimension_weights(&config.dimensions)?;

    let peer_set = PeerSet::from_universe(
        subject.clone(),
        universe,
        &config.filter,
        config.period_basis.clone(),
    );
    let excluded_count = universe
        .iter()
        .filter(|company| company.id != subject.id && !config.filter.accepts(company))
        .count();

    let companies = std::iter::once(subject)
        .chain(&peer_set.peers)
        .enumerate()
        .map(|(index, company)| RelativeValueCompany {
            id: company.id.clone(),
            is_subject: index == 0,
            values: config
                .multiples
                .iter()
                .map(|&multiple| compute_multiple(company, multiple))
                .collect(),
        })
        .collect();
    let multiples = config
        .multiples
        .iter()
        .map(|&multiple| multiple_summary(&peer_set, multiple))
        .collect();
    let dimensions: Vec<_> = config
        .dimensions
        .iter()
        .map(|dim| dimension_row(&peer_set, dim))
        .collect();

    let (composite, composite_excluded_reason) = if config.dimensions.is_empty() {
        (None, Some(CompositeExclusion::NoDimensions))
    } else if dimensions.iter().all(|dim| dim.excluded_reason.is_some()) {
        (None, Some(CompositeExclusion::NoUsableDimension))
    } else {
        // Only a non-finite composite becomes a reason; any other score error
        // is returned.
        let result = score_relative_value_unchecked(&peer_set, &config.dimensions)?;
        if is_finite_score(&result) {
            (Some(result), None)
        } else {
            (None, Some(CompositeExclusion::NonFiniteScore))
        }
    };

    Ok(RelativeValueReport {
        subject_id: subject.id.clone(),
        period_basis: config.period_basis.clone(),
        peer_count: peer_set.peer_count(),
        excluded_count,
        companies,
        multiples,
        dimensions,
        composite,
        composite_excluded_reason,
    })
}

/// Summarise one multiple over the peers and place the subject within them.
fn multiple_summary(peer_set: &PeerSet, multiple: Multiple) -> RelativeValueMultiple {
    let subject_value = compute_multiple(&peer_set.subject, multiple);
    let peer_values: Vec<f64> = peer_set
        .peers
        .iter()
        .filter_map(|peer| compute_multiple(peer, multiple))
        .collect();
    RelativeValueMultiple {
        multiple,
        subject_value,
        peer_stats: peer_stats(&peer_values),
        percentile: subject_value.and_then(|value| percentile_rank(&peer_values, value)),
        z_score: subject_value.and_then(|value| z_score(&peer_values, value)),
    }
}

/// Report one dimension from its shared evaluation, adding the scatter points.
fn dimension_row(peer_set: &PeerSet, dim: &ScoringDimension) -> RelativeValueDimension {
    let evaluation = evaluate_dimension(peer_set, dim);
    let fitted = |x: f64| {
        evaluation
            .regression
            .as_ref()
            .map(|reg| reg.intercept + reg.slope * x)
            .filter(|value| value.is_finite())
    };
    let subject_point = evaluation.subject_x.zip(evaluation.subject_y);
    let points = subject_point
        .map(|(x, y)| (&peer_set.subject.id, x, y))
        .into_iter()
        .chain(evaluation.peer_pairs.iter().copied())
        .map(|(id, x, y)| RelativeValuePoint {
            company_id: id.clone(),
            x,
            y,
            fitted_y: fitted(x),
        })
        .collect();
    RelativeValueDimension {
        label: dim.label.clone(),
        weight: dim.weight,
        direction: dim.direction,
        y: dim.y_extractor.clone(),
        x: dim.x_extractor.clone(),
        subject_y: evaluation.subject_y,
        subject_x: evaluation.subject_x,
        peer_stats: peer_stats(&evaluation.peer_y),
        percentile: evaluation.percentile,
        z_score: evaluation.z_score,
        regression: evaluation.regression.clone(),
        signal: evaluation.signal,
        excluded_reason: evaluation.excluded_reason,
        points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::comps::scoring::score_relative_value;
    use finstack_quant_core::types::Attributes;

    fn company(id: &str, leverage: Option<f64>, oas_bp: Option<f64>) -> CompanyMetrics {
        let mut metrics = CompanyMetrics::new(id);
        metrics.attributes = Attributes::default().with_meta("gics_sector", "Energy");
        metrics.leverage = leverage;
        metrics.oas_bp = oas_bp;
        metrics
    }

    fn with_ev(mut metrics: CompanyMetrics, ev: f64, ebitda: Option<f64>) -> CompanyMetrics {
        metrics.enterprise_value = Some(ev);
        metrics.ebitda = ebitda;
        metrics
    }

    fn subject() -> CompanyMetrics {
        with_ev(company("SUBJ", Some(2.5), Some(300.0)), 1000.0, Some(100.0))
    }

    /// P1-P4 regress exactly as in `scoring_composite_is_standardized_residual`.
    /// P5 has OAS and EV/EBITDA but no leverage. OUT is another sector. The
    /// subject's own row is in the universe.
    fn universe() -> Vec<CompanyMetrics> {
        let mut out = company("OUT", Some(1.0), Some(9_999.0));
        out.attributes = Attributes::default().with_meta("gics_sector", "Tech");
        vec![
            with_ev(company("P1", Some(1.0), Some(100.0)), 800.0, Some(100.0)),
            out,
            with_ev(company("P2", Some(2.0), Some(200.0)), 900.0, Some(100.0)),
            subject(),
            with_ev(company("P3", Some(3.0), Some(310.0)), 1200.0, Some(100.0)),
            with_ev(company("P4", Some(4.0), Some(380.0)), 500.0, None),
            with_ev(company("P5", None, Some(250.0)), 1100.0, Some(100.0)),
        ]
    }

    fn energy() -> PeerFilter {
        PeerFilter {
            gics_sectors: vec!["Energy".to_string()],
            ..Default::default()
        }
    }

    fn named(name: &str) -> MetricExtractor {
        MetricExtractor::Named(name.to_string())
    }

    fn spread_on_leverage(weight: f64) -> ScoringDimension {
        ScoringDimension {
            label: "Spread vs Leverage".to_string(),
            y_extractor: named("oas_bp"),
            x_extractor: Some(named("leverage")),
            weight,
            direction: ScoreDirection::HigherIsCheap,
        }
    }

    fn spread_level(weight: f64) -> ScoringDimension {
        ScoringDimension {
            label: "Spread Level".to_string(),
            y_extractor: named("oas_bp"),
            x_extractor: None,
            weight,
            direction: ScoreDirection::HigherIsRich,
        }
    }

    fn config(
        dimensions: Vec<ScoringDimension>,
        multiples: Vec<Multiple>,
    ) -> RelativeValueReportConfig {
        RelativeValueReportConfig {
            filter: energy(),
            period_basis: PeriodBasis::Ltm,
            dimensions,
            multiples,
        }
    }

    fn report(dimensions: Vec<ScoringDimension>) -> RelativeValueReport {
        relative_value_report(&subject(), &universe(), &config(dimensions, vec![]))
            .expect("valid request")
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "{actual} is not {expected}"
        );
    }

    #[test]
    fn table_holds_one_value_per_multiple_with_the_subject_first() {
        let report = relative_value_report(
            &subject(),
            &universe(),
            &config(
                vec![],
                vec![Multiple::SpreadPerTurn, Multiple::EvEbitda, Multiple::Pe],
            ),
        )
        .expect("valid request");

        assert_eq!(report.subject_id, CompanyId::new("SUBJ"));
        assert_eq!(report.period_basis, PeriodBasis::Ltm);
        assert_eq!(report.peer_count, 5);
        // OUT only: the subject's own universe row is not counted.
        assert_eq!(report.excluded_count, 1);

        let ids: Vec<_> = report.companies.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["SUBJ", "P1", "P2", "P3", "P4", "P5"]);
        let subjects: Vec<_> = report.companies.iter().map(|c| c.is_subject).collect();
        assert_eq!(subjects, [true, false, false, false, false, false]);

        // Columns in config order: OAS / leverage, EV / EBITDA, P/E (no inputs).
        assert_eq!(report.companies[0].values, [Some(120.0), Some(10.0), None]);
        assert_eq!(report.companies[1].values, [Some(100.0), Some(8.0), None]);
        // P4 has no EBITDA, P5 no leverage: only those cells are empty.
        assert_eq!(report.companies[4].values, [Some(95.0), None, None]);
        assert_eq!(report.companies[5].values, [None, Some(11.0), None]);

        // EV/EBITDA: peers 8, 9, 12, 11. Mean 10; deviations -2, -1, 2, 1;
        // sample variance 10/3. The subject's 10 is above 8 and 9: rank 2/4; z 0.
        let ev_ebitda = &report.multiples[1];
        assert_eq!(ev_ebitda.multiple, Multiple::EvEbitda);
        assert_eq!(ev_ebitda.subject_value, Some(10.0));
        let stats = ev_ebitda.peer_stats.as_ref().expect("four peer values");
        assert_eq!(stats.count, 4);
        close(stats.mean, 10.0);
        close(stats.median, 10.0);
        close(stats.std_dev, (10.0_f64 / 3.0).sqrt());
        assert_eq!(ev_ebitda.percentile, Some(0.5));
        close(ev_ebitda.z_score.expect("peers differ"), 0.0);

        // No company has the P/E inputs.
        let pe = &report.multiples[2];
        assert_eq!(pe.multiple, Multiple::Pe);
        assert_eq!(pe.subject_value, None);
        assert_eq!(pe.peer_stats, None);
        assert_eq!(pe.percentile, None);
        assert_eq!(pe.z_score, None);

        assert_eq!(report.dimensions, []);
        assert_eq!(report.composite, None);
        assert_eq!(
            report.composite_excluded_reason,
            Some(CompositeExclusion::NoDimensions)
        );
    }

    #[test]
    fn regression_dimension_matches_the_hand_calculation() {
        let report = report(vec![spread_on_leverage(1.0)]);
        let dim = &report.dimensions[0];

        assert_eq!(dim.label, "Spread vs Leverage");
        assert_eq!(dim.weight, 1.0);
        assert_eq!(dim.direction, ScoreDirection::HigherIsCheap);
        assert_eq!(dim.y, named("oas_bp"));
        assert_eq!(dim.x, Some(named("leverage")));
        assert_eq!(dim.subject_y, Some(300.0));
        assert_eq!(dim.subject_x, Some(2.5));

        // Y over all five peers, P5 included: 100, 200, 310, 380, 250.
        // Mean 248. Squared deviations 21904 + 2304 + 3844 + 17424 + 4 = 45480;
        // sample variance 11370. Three of five are at or below 300.
        let stats = dim.peer_stats.as_ref().expect("five peer values");
        assert_eq!(stats.count, 5);
        close(stats.mean, 248.0);
        close(stats.median, 250.0);
        close(stats.std_dev, 11_370.0_f64.sqrt());
        assert_eq!(stats.min, 100.0);
        assert_eq!(stats.max, 380.0);
        assert_eq!(dim.percentile, Some(0.6));
        close(
            dim.z_score.expect("peers differ"),
            52.0 / 11_370.0_f64.sqrt(),
        );

        // The fit uses the four pairs. Means x 2.5, y 247.5; Sxx 5; Sxy 475;
        // Syy 45475. Slope 95, intercept 10, R^2 = 475^2 / (5 * 45475).
        // Fitted at 2.5: 247.5. Residual: 300 - 247.5 = 52.5.
        let regression = dim.regression.as_ref().expect("four pairs");
        close(regression.slope, 95.0);
        close(regression.intercept, 10.0);
        close(regression.r_squared, 225_625.0 / 227_375.0);
        close(regression.fitted_value, 247.5);
        close(regression.residual, 52.5);
        assert_eq!(regression.n, 4);

        // Peer residuals -5, 0, 15, -10: squares sum 350, sample variance 350/3.
        close(dim.signal.expect("used"), 52.5 / (350.0_f64 / 3.0).sqrt());
        assert_eq!(dim.excluded_reason, None);

        // Subject first, then peers with both metrics; P5 has no point.
        let expected = [
            ("SUBJ", 2.5, 300.0, 247.5),
            ("P1", 1.0, 100.0, 105.0),
            ("P2", 2.0, 200.0, 200.0),
            ("P3", 3.0, 310.0, 295.0),
            ("P4", 4.0, 380.0, 390.0),
        ];
        assert_eq!(dim.points.len(), expected.len());
        for (point, (id, x, y, fitted)) in dim.points.iter().zip(expected) {
            assert_eq!(point.company_id.as_str(), id);
            assert_eq!(point.x, x);
            assert_eq!(point.y, y);
            close(point.fitted_y.expect("regression"), fitted);
        }
    }

    #[test]
    fn used_dimensions_agree_exactly_with_the_score() {
        let dimensions = vec![
            spread_on_leverage(0.7),
            spread_on_leverage(0.0),
            spread_level(0.3),
            ScoringDimension {
                label: "Nobody has it".to_string(),
                y_extractor: MetricExtractor::Custom("missing".to_string()),
                x_extractor: None,
                weight: 5.0,
                direction: ScoreDirection::HigherIsCheap,
            },
        ];
        let report = relative_value_report(
            &subject(),
            &universe(),
            &config(dimensions.clone(), vec![Multiple::EvEbitda]),
        )
        .expect("valid request");
        let peer_set = PeerSet::from_universe(subject(), &universe(), &energy(), PeriodBasis::Ltm);
        let score = score_relative_value(&peer_set, &dimensions).expect("usable dimensions");

        assert_eq!(report.composite.as_ref(), Some(&score));
        assert_eq!(report.composite_excluded_reason, None);
        assert_eq!(report.dimensions.len(), 4);

        let used: Vec<_> = report
            .dimensions
            .iter()
            .filter(|dim| dim.excluded_reason.is_none())
            .collect();
        assert_eq!(used.len(), score.dimensions.len());
        for (row, scored) in used.iter().zip(&score.dimensions) {
            assert_eq!(row.label, scored.label);
            assert_eq!(row.percentile, Some(scored.percentile));
            assert_eq!(row.z_score, Some(scored.z_score));
            assert_eq!(
                row.regression.as_ref().map(|reg| reg.residual),
                scored.regression_residual
            );
            assert_eq!(
                row.regression.as_ref().map(|reg| reg.r_squared),
                scored.r_squared
            );
            assert_eq!(row.weight, scored.weight);
        }

        // The composite is the weighted mean of the reported signals.
        let signals: Vec<f64> = used.iter().map(|dim| dim.signal.expect("used")).collect();
        close(
            score.composite_score,
            (0.7 * signals[0] + 0.3 * signals[1]) / (0.7 + 0.3),
        );
        // Spread level is rich when high: its signal is the negated z-score.
        close(signals[1], -used[1].z_score.expect("peers differ"));
    }

    #[test]
    fn peer_missing_one_metric_drops_only_its_pair() {
        let report = report(vec![spread_on_leverage(1.0)]);
        let dim = &report.dimensions[0];
        assert!(!dim.points.iter().any(|p| p.company_id.as_str() == "P5"));
        assert_eq!(dim.regression.as_ref().expect("fit").n, 4);
        // P5 still counts in the Y statistics and has a row in the table.
        assert_eq!(dim.peer_stats.as_ref().expect("stats").count, 5);
        assert!(report.companies.iter().any(|c| c.id.as_str() == "P5"));
    }

    fn exclusion_of(
        subject: CompanyMetrics,
        peers: Vec<CompanyMetrics>,
        dim: ScoringDimension,
    ) -> RelativeValueDimension {
        let mut config = config(vec![dim], vec![]);
        config.filter = PeerFilter::default();
        relative_value_report(&subject, &peers, &config)
            .expect("valid request")
            .dimensions
            .remove(0)
    }

    #[test]
    fn every_exclusion_reason_is_reported_with_no_signal() {
        let subject = || company("SUBJ", Some(2.5), Some(300.0));
        let fitted_peers = || {
            vec![
                company("P1", Some(1.0), Some(100.0)),
                company("P2", Some(2.0), Some(200.0)),
                company("P3", Some(3.0), Some(310.0)),
            ]
        };

        let zero = exclusion_of(subject(), fitted_peers(), spread_on_leverage(0.0));
        assert_eq!(zero.excluded_reason, Some(DimensionExclusion::ZeroWeight));
        assert_eq!(zero.signal, None);
        // Its figures are still there to draw.
        assert!(zero.regression.is_some());
        assert_eq!(zero.points.len(), 4);

        let no_subject = exclusion_of(
            company("SUBJ", Some(2.5), None),
            fitted_peers(),
            spread_on_leverage(1.0),
        );
        assert_eq!(
            no_subject.excluded_reason,
            Some(DimensionExclusion::NoSubjectValue)
        );
        assert_eq!(no_subject.signal, None);
        assert_eq!(no_subject.percentile, None);
        assert_eq!(no_subject.regression, None);
        // The peers are still plotted, without a line, and the subject is not.
        assert_eq!(no_subject.points.len(), 3);
        assert!(no_subject.points.iter().all(|p| p.fitted_y.is_none()));

        let no_peers = exclusion_of(
            subject(),
            vec![company("P1", Some(1.0), None)],
            spread_level(1.0),
        );
        assert_eq!(
            no_peers.excluded_reason,
            Some(DimensionExclusion::NoPeerValues)
        );
        assert_eq!(no_peers.peer_stats, None);

        for peers in [
            vec![company("P1", None, Some(100.0))],
            vec![
                company("P1", None, Some(100.0)),
                company("P2", None, Some(100.0)),
            ],
        ] {
            let flat = exclusion_of(subject(), peers, spread_level(1.0));
            assert_eq!(flat.excluded_reason, Some(DimensionExclusion::NoDispersion));
            assert_eq!(flat.signal, None);
            assert!(flat.percentile.is_some());
            assert!(flat.peer_stats.is_some());
        }

        let no_x = exclusion_of(
            company("SUBJ", None, Some(300.0)),
            fitted_peers(),
            spread_on_leverage(1.0),
        );
        assert_eq!(no_x.excluded_reason, Some(DimensionExclusion::NoSubjectX));
        assert_eq!(no_x.subject_x, None);
        // It never falls back to the z-score it has.
        assert!(no_x.z_score.is_some());
        assert_eq!(no_x.signal, None);

        let too_few_pairs = exclusion_of(
            subject(),
            vec![
                company("P1", Some(1.0), Some(100.0)),
                company("P2", Some(2.0), Some(200.0)),
            ],
            spread_on_leverage(1.0),
        );
        assert_eq!(
            too_few_pairs.excluded_reason,
            Some(DimensionExclusion::RegressionUnusable)
        );
        assert_eq!(too_few_pairs.regression, None);
        assert!(too_few_pairs.points.iter().all(|p| p.fitted_y.is_none()));

        let flat_x = exclusion_of(
            subject(),
            vec![
                company("P1", Some(2.0), Some(100.0)),
                company("P2", Some(2.0), Some(200.0)),
                company("P3", Some(2.0), Some(310.0)),
            ],
            spread_on_leverage(1.0),
        );
        assert_eq!(
            flat_x.excluded_reason,
            Some(DimensionExclusion::RegressionUnusable)
        );

        // Peers exactly on y = 100x: the line is reported, the residual cannot
        // be standardized.
        let on_the_line = exclusion_of(
            subject(),
            vec![
                company("P1", Some(1.0), Some(100.0)),
                company("P2", Some(2.0), Some(200.0)),
                company("P3", Some(3.0), Some(300.0)),
            ],
            spread_on_leverage(1.0),
        );
        assert_eq!(
            on_the_line.excluded_reason,
            Some(DimensionExclusion::NoResidualDispersion)
        );
        assert_eq!(on_the_line.signal, None);
        close(on_the_line.regression.expect("fit").slope, 100.0);
    }

    #[test]
    fn composite_is_none_while_the_table_still_has_values() {
        let request = config(vec![spread_on_leverage(0.0)], vec![Multiple::EvEbitda]);
        let all_zero =
            relative_value_report(&subject(), &universe(), &request).expect("valid request");
        assert_eq!(all_zero.composite, None);
        assert_eq!(
            all_zero.composite_excluded_reason,
            Some(CompositeExclusion::NoUsableDimension)
        );
        assert_eq!(all_zero.companies[0].values, [Some(10.0)]);
        assert_eq!(all_zero.multiples[0].percentile, Some(0.5));

        let unusable = report(vec![ScoringDimension {
            label: "Nobody has it".to_string(),
            y_extractor: MetricExtractor::Custom("missing".to_string()),
            x_extractor: None,
            weight: 1.0,
            direction: ScoreDirection::HigherIsCheap,
        }]);
        assert_eq!(unusable.composite, None);
        assert_eq!(
            unusable.composite_excluded_reason,
            Some(CompositeExclusion::NoUsableDimension)
        );
        assert_eq!(unusable.companies.len(), 6);
    }

    #[test]
    fn composite_too_large_to_represent_leaves_the_table() {
        // Each z-score is about 1.4e308; their weighted sum overflows.
        let huge = company("SUBJ", None, Some(1e308));
        let peers = [
            company("P1", None, Some(1.0)),
            company("P2", None, Some(2.0)),
        ];
        let mut request = config(
            vec![spread_level(1.0), spread_level(1.0)],
            vec![Multiple::SpreadPerTurn],
        );
        request.filter = PeerFilter::default();
        let report = relative_value_report(&huge, &peers, &request).expect("valid request");
        assert_eq!(report.composite, None);
        assert_eq!(
            report.composite_excluded_reason,
            Some(CompositeExclusion::NonFiniteScore)
        );
        assert!(report
            .dimensions
            .iter()
            .all(|d| d.excluded_reason.is_none()));
    }

    #[test]
    fn invalid_requests_are_errors() {
        let empty = config(vec![], vec![]);
        assert!(relative_value_report(&subject(), &universe(), &empty).is_err());
        for weight in [-1.0, f64::NAN, f64::INFINITY] {
            let request = config(vec![spread_level(weight)], vec![Multiple::EvEbitda]);
            assert!(relative_value_report(&subject(), &universe(), &request).is_err());
        }
    }

    #[test]
    fn request_is_strict_and_filter_defaults_to_empty() {
        let request: RelativeValueReportConfig = serde_json::from_value(serde_json::json!({
            "period_basis": {"custom": "FY2025E"},
            "dimensions": [],
            "multiples": ["ev_ebitda"],
        }))
        .expect("filter may be omitted");
        let report =
            relative_value_report(&subject(), &universe(), &request).expect("valid request");
        // Every member but the subject is a peer, the other sector included.
        assert_eq!(report.peer_count, 6);
        assert_eq!(report.excluded_count, 0);
        assert_eq!(
            report.period_basis,
            PeriodBasis::Custom("FY2025E".to_string())
        );

        let error = serde_json::from_value::<RelativeValueReportConfig>(serde_json::json!({
            "period_basis": "ltm",
            "dimensions": [],
            "multiples": [],
            "guess": true,
        }))
        .expect_err("unknown field");
        assert!(error.to_string().contains("guess"));

        // A partial filter screens on the fields it names only.
        let partial: RelativeValueReportConfig = serde_json::from_value(serde_json::json!({
            "filter": {"gics_sectors": ["Energy"]},
            "period_basis": "ltm",
            "dimensions": [],
            "multiples": ["ev_ebitda"],
        }))
        .expect("a partial filter deserializes");
        let report =
            relative_value_report(&subject(), &universe(), &partial).expect("valid request");
        assert_eq!(report.peer_count, 5);
        assert_eq!(report.excluded_count, 1);

        let error = serde_json::from_value::<RelativeValueReportConfig>(serde_json::json!({
            "filter": {"rating": ["BB"]},
            "period_basis": "ltm",
            "dimensions": [],
            "multiples": ["ev_ebitda"],
        }))
        .expect_err("misspelt filter field");
        assert!(error.to_string().contains("rating"), "{error}");
    }

    #[test]
    fn report_round_trips_through_json() {
        let report = report(vec![spread_on_leverage(1.0), spread_level(1.0)]);
        let json = serde_json::to_string(&report).expect("serialize");
        let back: RelativeValueReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, report);
        assert_eq!(
            serde_json::to_value(DimensionExclusion::NoResidualDispersion).expect("serialize"),
            "no_residual_dispersion"
        );
        assert_eq!(
            serde_json::to_value(CompositeExclusion::NoUsableDimension).expect("serialize"),
            "no_usable_dimension"
        );
    }
}
