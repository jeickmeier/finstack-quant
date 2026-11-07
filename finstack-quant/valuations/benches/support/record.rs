//! Opt-in fixture/output records for the valuation performance investigation.

/// Emit one selected case's full inputs and preflight output for archival.
///
/// The closure is evaluated only when `FQ_BENCH_CAPTURE=1`. Ordinary timing and
/// memory runs therefore do not serialize fixtures or construct diagnostics.
/// The caller invokes this from its untimed selected-case validation callback.
///
/// # Arguments
/// - `case`: The complete Criterion case ID, including its group and parameters.
/// - `fields`: Builds a JSON object containing `fixture` inputs and `output`
///   values. Fixture data must include all model/configuration inputs needed to
///   reproduce the priced operation; monetary output retains its currency.
pub fn capture(case: &str, fields: impl FnOnce() -> serde_json::Value) {
    if std::env::var("FQ_BENCH_CAPTURE").as_deref() == Ok("1") {
        let record = serde_json::json!({"case": case, "data": fields()});
        eprintln!("FQ_BENCH_CAPTURE {record}");
    }
}
