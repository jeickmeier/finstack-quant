//! Runtime docstrings compiled from the `.pyi` stubs.
//!
//! The stubs under `finstack_quant/` are the single source of every Python
//! docstring. `build.rs` extracts each one to
//! `$OUT_DIR/pydoc/<module>/<qualified name>.txt`; [`pydoc!`] includes that
//! file, so `help()` shows the stub text and a key with no stub docstring is a
//! compile error.

/// Expand to the stub docstring of one Python object, as a `&'static str`.
///
/// # Arguments
///
/// * `$module` - Dotted module path of the stub file holding the docstring:
///   the public module (`"finstack_quant.valuations.market"`) or the private
///   fragment mirroring the owner's Rust file
///   (`"finstack_quant.valuations.instruments._typed_rates"`).
/// * `$key` - Qualified name inside that stub: `"Class"`, `"Class.member"`,
///   `"function"`, or `"__module__"` for the module docstring. Macro-generated
///   owners may pass a `concat!(..)` of literals instead of one literal.
///
/// Use it as `#[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry")]`
/// on a `#[pyclass]`, `#[pymethods]` method, `#[getter]` or `#[pyfunction]`,
/// and as the value of a module's `__doc__`. Do not also write `///` docs on
/// the same item: PyO3 would concatenate both.
macro_rules! pydoc {
    ($module:literal, $($key:tt)+) => {
        include_str!(concat!(env!("OUT_DIR"), "/pydoc/", $module, "/", $($key)+, ".txt"))
    };
}
pub(crate) use pydoc;
