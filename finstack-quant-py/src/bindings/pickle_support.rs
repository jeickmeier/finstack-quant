//! Shared `__reduce__` helper for the JSON-backed pickle protocol.
//!
//! Every wrapper that exposes `to_json`/`from_json` reconstructs through that
//! same strict serde round-trip, so pickling never introduces a second state
//! format that could drift from the wire contract.

use pyo3::prelude::*;

/// Build a `__reduce__` result, refusing payloads that cannot be reconstructed.
///
/// `serde_json` writes `null` for `f64::NAN` and `f64::INFINITY`, and the
/// matching `from_json` then rejects `null` for a non-optional float. Left
/// unchecked, `pickle.dumps` succeeds and the corresponding `loads` fails — in
/// the *consumer*. Inside `multiprocessing.Pool` that is not even an exception:
/// it kills the pool's result-handler thread and the parent blocks forever.
/// Validating here keeps the failure on the producer side, where the traceback
/// names the offending object.
///
/// The verification round-trip runs only when the payload contains `null` at
/// all, so the common case costs one substring scan rather than a second
/// deserialization.
///
/// # Errors
///
/// Returns `ValueError` when the payload does not round-trip.
pub(crate) fn reduce_via_json<'py>(
    from_json: Bound<'py, PyAny>,
    payload: String,
) -> PyResult<(Bound<'py, PyAny>, (String,))> {
    if payload.contains("null")
        && let Err(err) = from_json.call1((&payload,))
    {
        return Err(crate::errors::value_error(format!(
            "cannot pickle this value: its JSON form does not round-trip ({err}). \
                 This usually means a non-finite float (NaN or infinity), which the \
                 JSON wire format cannot represent."
        )));
    }
    Ok((from_json, (payload,)))
}
