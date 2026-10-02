//! Order-preserving fallible maps that behave the same on native and wasm32.
//!
//! Collecting a Rayon iterator straight into `Result<Vec<_>, E>` returns
//! whichever error a worker happens to hit first, so when several items fail
//! the reported error changes from run to run and differs from the serial
//! (wasm32) path, which always stops at the first failing item. The helpers
//! here always report the **lowest-index** failure: native builds evaluate
//! every item on the Rayon pool, collect the per-item results in input order
//! and then pick the first error serially; wasm32 builds (no thread pool) run
//! the same map serially.
//!
//! # Determinism
//!
//! On success the output vector is in input order on every target. On failure
//! the error is the one produced by the lowest-index failing item on every
//! target and every thread count. The native arm gives up Rayon's early exit on
//! error: every item is evaluated before the first error is chosen.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::parallel::try_map_ordered;
//!
//! let squares = try_map_ordered(vec![1_u32, 2, 3], |x| Ok::<_, String>(x * x));
//! assert_eq!(squares, Ok(vec![1, 4, 9]));
//!
//! // Items 3, 11, 19, ... fail; the reported error is always item 3's.
//! let result = try_map_ordered(0..64_usize, |i| if i % 8 == 3 { Err(i) } else { Ok(i) });
//! assert_eq!(result, Err(3));
//! ```

/// Map `items` with the fallible `f` and return the outputs in input order.
///
/// On native targets the items run on the Rayon pool; every item is evaluated
/// and the error of the lowest-index failing item is returned, which is the
/// same error a serial loop returns.
///
/// # Arguments
///
/// * `items` - The inputs to map: a slice, a `&Vec<T>`, an owned `Vec<T>` or
///   an index range. It must be an indexed source, so the item order is the
///   output order and defines which failure is "first".
/// * `f` - Fallible per-item computation. It is called once per item,
///   possibly concurrently from several threads on native targets, so it must
///   not depend on call order.
///
/// # Returns
///
/// The outputs of `f`, one per item, in input order.
///
/// # Errors
///
/// Returns the error produced by the lowest-index item for which `f` fails.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::parallel::try_map_ordered;
///
/// let names = ["a", "b", "c"];
/// let upper = try_map_ordered(&names[..], |s| Ok::<_, ()>(s.to_uppercase()));
/// assert_eq!(upper, Ok(vec!["A".to_string(), "B".to_string(), "C".to_string()]));
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub fn try_map_ordered<I, R, E, F>(items: I, f: F) -> Result<Vec<R>, E>
where
    I: rayon::iter::IntoParallelIterator,
    I::Iter: rayon::iter::IndexedParallelIterator,
    R: Send,
    E: Send,
    F: Fn(I::Item) -> Result<R, E> + Sync + Send,
{
    use rayon::iter::ParallelIterator;
    let results: Vec<Result<R, E>> = items.into_par_iter().map(f).collect();
    results.into_iter().collect()
}

/// Map `items` with the fallible `f` and return the outputs in input order.
///
/// wasm32 has no Rayon thread pool, so the items run serially and the map
/// stops at the first failing item, which is the same error the native
/// parallel arm reports.
///
/// # Arguments
///
/// * `items` - The inputs to map: a slice, a `&Vec<T>`, an owned `Vec<T>` or
///   an index range. The item order is the output order and defines which
///   failure is "first".
/// * `f` - Fallible per-item computation, called once per item in order until
///   the first failure.
///
/// # Returns
///
/// The outputs of `f`, one per item, in input order.
///
/// # Errors
///
/// Returns the error produced by the lowest-index item for which `f` fails.
#[cfg(target_arch = "wasm32")]
pub fn try_map_ordered<I, R, E, F>(items: I, f: F) -> Result<Vec<R>, E>
where
    I: IntoIterator,
    F: Fn(I::Item) -> Result<R, E>,
{
    items.into_iter().map(f).collect()
}

#[cfg(test)]
mod tests {
    use super::try_map_ordered;

    #[test]
    fn returns_outputs_in_input_order() {
        let items: Vec<u64> = (0..1_000).collect();
        let out = try_map_ordered(&items, |x| Ok::<_, ()>(x * 2));
        assert_eq!(out, Ok(items.iter().map(|x| x * 2).collect::<Vec<_>>()));
    }

    #[test]
    fn several_failures_always_report_the_serial_first_error() {
        let fails = |i: usize| {
            if i % 7 == 5 {
                Err(format!("item {i}"))
            } else {
                Ok(i)
            }
        };
        let serial: Result<Vec<usize>, String> = (0..512).map(fails).collect();
        assert_eq!(serial, Err("item 5".to_string()));
        for _ in 0..50 {
            assert_eq!(try_map_ordered(0..512_usize, fails), serial);
        }
    }

    #[test]
    fn owned_items_are_moved_into_the_map() {
        let items = vec![String::from("x"), String::from("y")];
        let out = try_map_ordered(items, |s| Ok::<_, ()>(s + "!"));
        assert_eq!(out, Ok(vec!["x!".to_string(), "y!".to_string()]));
    }
}
