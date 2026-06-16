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
/// let scale = MasterScale::sp_empirical().unwrap();
/// let result = scale.map_pd(0.0015).unwrap();
/// assert_eq!(result.grade, "BBB");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterScale {
    grades: Vec<MasterScaleGrade>,
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
    /// Returns [`PdCalibrationError::NonFiniteValue`] if the result's
    /// `implied_pd` is NaN or infinite.
    pub fn map_score(
        &self,
        result: &ScoringResult,
    ) -> Result<MasterScaleResult, PdCalibrationError> {
        self.map_pd(result.implied_pd)
    }

    /// S&P empirical PD master scale.
    ///
    /// Grade boundaries based on S&P Global Ratings 1981-2023 one-year
    /// corporate default rate study. Central PDs are geometric means of
    /// each grade's historical default rate range.
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
    pub fn sp_empirical() -> crate::Result<Self> {
        Self::from_registry_id(
            crate::credit::registry::embedded_registry()?.default_pd_master_scale_id(),
        )
    }

    /// Moody's empirical PD master scale.
    ///
    /// Grade boundaries based on Moody's Investors Service 1983-2023
    /// annual default study. Uses Moody's alphanumeric notation.
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
    pub fn moodys_empirical() -> crate::Result<Self> {
        Self::from_registry_id("moodys_empirical")
    }

    /// Load a PD master scale from the credit assumptions registry.
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
