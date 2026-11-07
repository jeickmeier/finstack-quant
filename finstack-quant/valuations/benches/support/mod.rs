//! Checked execution shared by valuation benchmarks.

use criterion::Bencher;
use std::hint::black_box;

/// Validate the selected operation outside its timing loop, then measure it.
///
/// Call this inside Criterion's selected-case closure so unselected cases do
/// not execute their preflight. `operation` must require success on every call
/// (for example with `expect`), including the measured calls. Structural and
/// numerical output checks belong in `validate`, outside the measured loop.
///
/// # Arguments
/// - `bencher`: Criterion's runner for the selected case; only `iter` is timed.
/// - `operation`: The same successful pricing operation for preflight and timing.
/// - `validate`: Untimed assertions on the selected operation's successful output.
pub fn bench<T>(
    bencher: &mut Bencher<'_>,
    mut operation: impl FnMut() -> T,
    validate: impl FnOnce(&T),
) {
    let output = operation();
    validate(&output);
    drop(output);
    bencher.iter(|| black_box(operation()));
}
