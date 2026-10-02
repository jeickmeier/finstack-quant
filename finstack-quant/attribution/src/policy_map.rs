//! Shared serial/parallel mapping so rayon stays off the wasm32 graph.

use crate::types::result::ExecutionPolicy;

/// Map `items` with `f`, using Rayon only on native `ExecutionPolicy::Parallel`.
pub(crate) fn map_policy<T, R, F>(policy: ExecutionPolicy, items: &[T], f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync + Send,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        match policy {
            ExecutionPolicy::Parallel => items.par_iter().map(f).collect(),
            ExecutionPolicy::Serial => items.iter().map(f).collect(),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = policy;
        items.iter().map(f).collect()
    }
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

/// Fallible zip-map of two slices, using Rayon only on native `Parallel`.
///
/// Pairs past the shorter slice are ignored. Both policies report the error of
/// the lowest-index failing pair.
pub(crate) fn try_map_policy_zip<A, B, R, E, F>(
    policy: ExecutionPolicy,
    left: &[A],
    right: &[B],
    f: F,
) -> Result<Vec<R>, E>
where
    A: Sync,
    B: Sync,
    R: Send,
    E: Send,
    F: Fn((&A, &B)) -> Result<R, E> + Sync + Send,
{
    match policy {
        ExecutionPolicy::Parallel => {
            finstack_quant_core::parallel::try_map_ordered(0..left.len().min(right.len()), |i| {
                f((&left[i], &right[i]))
            })
        }
        ExecutionPolicy::Serial => left.iter().zip(right.iter()).map(f).collect(),
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

    #[test]
    fn parallel_zip_reports_the_serial_first_error() {
        let left: Vec<usize> = (0..256).collect();
        let right: Vec<usize> = (0..300).collect();
        let fails = |(a, b): (&usize, &usize)| {
            if a % 11 == 6 {
                Err(*a)
            } else {
                Ok(a + b)
            }
        };
        let serial = try_map_policy_zip(ExecutionPolicy::Serial, &left, &right, fails);
        assert_eq!(serial, Err(6));
        for _ in 0..30 {
            assert_eq!(
                try_map_policy_zip(ExecutionPolicy::Parallel, &left, &right, fails),
                serial
            );
        }
        let ok = |(a, b): (&usize, &usize)| Ok::<_, ()>(a + b);
        assert_eq!(
            try_map_policy_zip(ExecutionPolicy::Parallel, &left, &right, ok),
            try_map_policy_zip(ExecutionPolicy::Serial, &left, &right, ok)
        );
    }
}
