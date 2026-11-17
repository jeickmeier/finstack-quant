//! Validation helpers shared by rolled/conditioned term structures.

/// Reject a transformation whose evaluation disagrees with the stored pillars.
///
/// `actual` evaluates the transformation at a pillar time and `what` names it
/// in the error. A non-finite value (or, with `require_positive`, a
/// non-positive one), or a mismatch beyond `1e-12` relative tolerance, is a
/// validation error.
pub(crate) fn validate_pillars(
    points: impl IntoIterator<Item = (f64, f64)>,
    require_positive: bool,
    what: &str,
    actual: impl Fn(f64) -> f64,
) -> crate::Result<()> {
    for (time, value) in points {
        let actual = actual(time);
        if !actual.is_finite()
            || (require_positive && actual <= 0.0)
            || (actual - value).abs() > 1e-12 * actual.abs().max(value.abs()).max(1.0)
        {
            return Err(crate::Error::Validation(format!(
                "{what} disagrees with its pillar samples"
            )));
        }
    }
    Ok(())
}
