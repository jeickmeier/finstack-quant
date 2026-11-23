//! Formula-literal formatting helper shared by statement templates.

/// Format an `f64` for embedding in a generated formula at full
/// (shortest-roundtrip) precision, ensuring the literal still looks like a
/// float (e.g. `5` → `"5.0"`).
pub(crate) fn fmt_f64(value: f64) -> String {
    let s = format!("{}", value);
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{s}.0")
    }
}
