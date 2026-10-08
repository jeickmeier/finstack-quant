//! Shared serial/parallel mapping over `finstack_quant_core::parallel`.

use crate::types::result::ExecutionPolicy;

/// Map `items` with the infallible `f`; see [`try_map_policy`].
pub(crate) fn map_policy<T, R, F>(policy: ExecutionPolicy, items: &[T], f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync + Send,
{
    let Ok(mapped) = try_map_policy(policy, items, |item| {
        Ok::<R, std::convert::Infallible>(f(item))
    });
    mapped
}

/// Fallible map of `items` with `f`, using Rayon only on native `Parallel`.
///
/// Both policies report the error of the lowest-index failing item, so a
/// parallel run names the same failure as a serial (or wasm32) run.
pub(crate) fn try_map_policy<T, R, E, F>(
    policy: ExecutionPolicy,
    items: &[T],
    f: F,
) -> Result<Vec<R>, E>
where
    T: Sync,
    R: Send,
    E: Send,
    F: Fn(&T) -> Result<R, E> + Sync + Send,
{
    match policy {
        ExecutionPolicy::Parallel => finstack_quant_core::parallel::try_map_ordered(items, f),
        ExecutionPolicy::Serial => items.iter().map(f).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_policy_reports_the_serial_first_error() {
        let items: Vec<usize> = (0..256).collect();
        let fails = |i: &usize| if i % 9 == 4 { Err(*i) } else { Ok(*i) };
        let serial = try_map_policy(ExecutionPolicy::Serial, &items, fails);
        assert_eq!(serial, Err(4));
        for _ in 0..30 {
            assert_eq!(
                try_map_policy(ExecutionPolicy::Parallel, &items, fails),
                serial
            );
        }
    }
}
