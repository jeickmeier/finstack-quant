//! Master scale mapping: continuous scores/PDs to discrete rating grades.
//!
//! A master scale defines a set of rating grades ordered from best to worst,
//! each with a PD upper boundary and a central (representative) PD. Any
//! continuous PD can be mapped to the corresponding grade.

use serde::{Deserialize, Serialize};

use crate::credit::scoring::ScoringResult;

use super::error::PdCalibrationError;

/// A master scale mapping continuous PDs to discrete rating grades.
///
/// Each grade has an upper bound, a label, and an associated central PD.
/// Grades are ordered from best (lowest PD) to worst (highest PD).
/// A PD is mapped to the first grade whose **inclusive** `upper_pd` it does
/// not exceed (`pd <= upper_pd`).
///
/// # Examples
///
/// ```
/// use finstack_quant_core::credit::pd::MasterScale;
///
/// let scale = MasterScale::sp_assumptions_v1().unwrap();
/// let result = scale.map_pd(0.0015).unwrap();
/// assert_eq!(result.grade, "BBB");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "MasterScaleWire")]
pub struct MasterScale {
    grades: Vec<MasterScaleGrade>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MasterScaleWire {
    grades: Vec<MasterScaleGrade>,
}

impl TryFrom<MasterScaleWire> for MasterScale {
    type Error = PdCalibrationError;

    fn try_from(wire: MasterScaleWire) -> Result<Self, Self::Error> {
        MasterScale::new(wire.grades)
    }
}

/// A single grade in a master scale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterScaleGrade {
    /// Grade label (e.g., "AAA", "Aaa", "1", etc.).
    pub label: String,
    /// Upper PD boundary for this grade (**inclusive**).
    ///
    /// A PD <= this value maps to this grade (checked in order), so a PD
    /// exactly on the boundary maps to the better grade.
    pub upper_pd: f64,
    /// Central (representative) PD for the grade.
    ///
    /// Typically the geometric mean of the grade's PD range.
    pub central_pd: f64,
}

/// Result of mapping a PD to a master scale grade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterScaleResult {
    /// The assigned rating grade label.
    pub grade: String,
    /// The central PD for the assigned grade.
    pub central_pd: f64,
    /// The input PD that was mapped.
    pub input_pd: f64,
    /// Index of the grade in the scale (0 = best).
    pub grade_index: usize,
}

impl MasterScale {
    /// Construct a custom master scale.
    ///
    /// Grades must be ordered by `upper_pd` (ascending). Each `upper_pd`
    /// must be in (0, 1] and each `central_pd` must be in (0, 1).
    ///
    /// # Errors
    ///
    /// - [`PdCalibrationError::EmptyInput`] if grades are empty.
    /// - [`PdCalibrationError::GradesNotSorted`] if grades are not in ascending order.
    /// - [`PdCalibrationError::ValueOutOfRange`] if any PD value is invalid.
    pub fn new(grades: Vec<MasterScaleGrade>) -> Result<Self, PdCalibrationError> {
        if grades.is_empty() {
            return Err(PdCalibrationError::EmptyInput);
        }

        // Validate PD values
        for g in &grades {
            if g.upper_pd <= 0.0 || g.upper_pd > 1.0 || !g.upper_pd.is_finite() {
                return Err(PdCalibrationError::ValueOutOfRange {
                    value: g.upper_pd,
                    min: 0.0,
                    max: 1.0,
                });
            }
            if g.central_pd <= 0.0 || g.central_pd >= 1.0 || !g.central_pd.is_finite() {
                return Err(PdCalibrationError::ValueOutOfRange {
                    value: g.central_pd,
                    min: 0.0,
                    max: 1.0,
                });
            }
        }

        // Validate ascending order
        for i in 1..grades.len() {
            if grades[i].upper_pd <= grades[i - 1].upper_pd {
                return Err(PdCalibrationError::GradesNotSorted);
            }
        }

        Ok(Self { grades })
    }

    /// Map a PD to the corresponding grade.
    ///
    /// Returns the first grade whose inclusive `upper_pd >= input_pd`.
    /// If `input_pd` exceeds all grades, returns the worst (last) grade.
    ///
    /// # Errors
    ///
    /// Returns [`PdCalibrationError::NonFiniteValue`] if `pd` is NaN or
    /// infinite (NaN previously fell through
    /// every comparison and silently mapped to the worst grade).
    pub fn map_pd(&self, pd: f64) -> Result<MasterScaleResult, PdCalibrationError> {
        if !pd.is_finite() {
            return Err(PdCalibrationError::NonFiniteValue { value: pd });
        }
        for (i, grade) in self.grades.iter().enumerate() {
            if pd <= grade.upper_pd {
                return Ok(MasterScaleResult {
                    grade: grade.label.clone(),
                    central_pd: grade.central_pd,
                    input_pd: pd,
                    grade_index: i,
                });
            }
        }

        // PD exceeds all grades: return worst
        let last = self.grades.len() - 1;
        Ok(MasterScaleResult {
            grade: self.grades[last].label.clone(),
            central_pd: self.grades[last].central_pd,
            input_pd: pd,
            grade_index: last,
        })
    }

    /// Map a [`ScoringResult`] to a grade, using the result's `implied_pd`.
    ///
    /// # Errors
    ///
    /// Returns [`PdCalibrationError::MissingImpliedPd`] when the scoring model
    /// has no native or explicitly calibrated probability, or
    /// [`PdCalibrationError::NonFiniteValue`] when it is non-finite.
    pub fn map_score(
        &self,
        result: &ScoringResult,
    ) -> Result<MasterScaleResult, PdCalibrationError> {
        self.map_pd(
            result
                .implied_pd
                .ok_or(PdCalibrationError::MissingImpliedPd)?,
        )
    }

    /// Version 1 library PD-band assumptions using S&P-style labels.
    ///
    /// These bands are Finstack Quant library assumptions. They are not
    /// presented as S&P empirical default rates or as an agency calibration.
    ///
    /// | Grade | Upper PD  | Central PD |
    /// |-------|-----------|------------|
    /// | AAA   | 0.0001    | 0.00004    |
    /// | AA    | 0.0005    | 0.0002     |
    /// | A     | 0.001     | 0.0007     |
    /// | BBB   | 0.005     | 0.002      |
    /// | BB    | 0.02      | 0.01       |
    /// | B     | 0.07      | 0.04       |
    /// | CCC   | 0.25      | 0.12       |
    /// | CC/C  | 1.0       | 0.40       |
    ///
    /// The selected scale ID comes from the embedded credit-assumptions
    /// registry's default PD master-scale setting. The labels resemble S&P
    /// notation solely as a reporting convention; neither boundaries nor
    /// central PDs should be presented as agency-published statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded credit registry cannot be loaded, its
    /// configured default PD master-scale ID is absent, or the registry grades
    /// violate [`MasterScale::new`] invariants. This indicates an invalid
    /// package/configuration, not an inability to map a particular PD.
    pub fn sp_assumptions_v1() -> crate::Result<Self> {
        Self::from_registry_id(
            crate::credit::registry::embedded_registry()?.default_pd_master_scale_id(),
        )
    }

    /// Legacy compatibility name for [`Self::sp_assumptions_v1`].
    ///
    /// Despite the historical method name, the returned bands are versioned
    /// Finstack Quant assumptions, not sourced S&P empirical default rates.
    ///
    /// # Errors
    ///
    /// Propagates all registry-loading and grade-validation errors from
    /// [`sp_assumptions_v1`](Self::sp_assumptions_v1).
    pub fn sp_empirical() -> crate::Result<Self> {
        Self::sp_assumptions_v1()
    }

    /// Version 1 library PD-band assumptions using Moody's-style labels.
    ///
    /// These bands are Finstack Quant library assumptions. They are not
    /// presented as Moody's empirical default rates or as an agency calibration.
    ///
    /// | Grade | Upper PD  | Central PD |
    /// |-------|-----------|------------|
    /// | Aaa   | 0.0001    | 0.00003    |
    /// | Aa    | 0.0005    | 0.0002     |
    /// | A     | 0.001     | 0.0007     |
    /// | Baa   | 0.005     | 0.002      |
    /// | Ba    | 0.02      | 0.01       |
    /// | B     | 0.08      | 0.04       |
    /// | Caa   | 0.25      | 0.13       |
    /// | Ca/C  | 1.0       | 0.45       |
    ///
    /// These are Finstack Quant library assumptions using Moody's-style
    /// notation, not Moody's historical default-rate observations or a rating
    /// agency calibration. Select this scale only when that reporting-label
    /// convention is appropriate for the downstream scorecard.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded credit registry cannot be loaded, the
    /// `moodys_assumptions_v1` entry is absent, or its grades violate
    /// [`MasterScale::new`] invariants.
    pub fn moodys_assumptions_v1() -> crate::Result<Self> {
        Self::from_registry_id("moodys_assumptions_v1")
    }

    /// Legacy compatibility name for [`Self::moodys_assumptions_v1`].
    ///
    /// Despite the historical method name, the returned bands are versioned
    /// Finstack Quant assumptions, not sourced Moody's empirical default rates.
    ///
    /// # Errors
    ///
    /// Propagates all registry-loading and grade-validation errors from
    /// [`moodys_assumptions_v1`](Self::moodys_assumptions_v1).
    pub fn moodys_empirical() -> crate::Result<Self> {
        Self::moodys_assumptions_v1()
    }

    /// Load a PD master scale from the credit assumptions registry.
    ///
    /// Deprecated registry aliases remain readable for backward-compatible
    /// configuration loading and emit a warning. New configurations should
    /// use `sp_assumptions_v1` or `moodys_assumptions_v1`. The returned scale
    /// retains grades in best-to-worst order and maps PD boundaries
    /// inclusively, exactly as [`map_pd`](Self::map_pd) documents.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry cannot load, `id` is unknown
    /// after compatibility-alias resolution, or the selected grades fail the
    /// non-empty, finite, in-range, strictly increasing-boundary invariants of
    /// [`MasterScale::new`].
    pub fn from_registry_id(id: &str) -> crate::Result<Self> {
        let grades = crate::credit::registry::embedded_registry()?.pd_master_scale_grades(id)?;
        Self::new(grades).map_err(|err| {
            crate::Error::Validation(format!("invalid PD master scale '{id}': {err}"))
        })
    }

    /// Number of grades in the scale.
    #[must_use]
    pub fn n_grades(&self) -> usize {
        self.grades.len()
    }

    /// All grades in order (best to worst).
    #[must_use]
    pub fn grades(&self) -> &[MasterScaleGrade] {
        &self.grades
    }
}
