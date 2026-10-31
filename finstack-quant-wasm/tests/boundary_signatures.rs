//! Exported bindings take host values as `JsValue` and convert them through
//! `utils::input`.
//!
//! wasm-bindgen's glue for primitive parameters checks nothing at runtime: a
//! `&str`/`String` parameter traps the instance (and leaks) on a non-string,
//! integers go through `ToInt32`/`ToBigInt64` wrapping, `bool` through
//! truthiness, `f64` through `ToNumber` (`null` becomes `0`, `"5"` becomes
//! `5`), `&[f64]`/`Vec<f64>` through typed-array glue (a string becomes
//! `NaN`s, `{}` becomes `[]`), and `Vec<String>` splits strings into
//! characters or accepts `{}` as empty. This test scans every `#[wasm_bindgen]` export under
//! `src/api` and rejects those parameter types, so a new binding cannot
//! reintroduce the silent coercions.

// `dts_contract.rs` shares the scanner and reads different `Export` fields.
#[allow(dead_code)]
#[path = "export_scan.rs"]
mod export_scan;

/// Parameter types whose wasm-bindgen glue coerces or traps.
const FORBIDDEN: &[&str] = &[
    "&str",
    "String",
    "Option<String>",
    "Option<&str>",
    "bool",
    "Option<bool>",
    "usize",
    "u8",
    "u16",
    "u32",
    "u64",
    "i8",
    "i16",
    "i32",
    "i64",
    "isize",
    "Option<usize>",
    "Option<u8>",
    "Option<u16>",
    "Option<u32>",
    "Option<u64>",
    "Option<i8>",
    "Option<i16>",
    "Option<i32>",
    "Option<i64>",
    "Vec<String>",
    "Option<Vec<String>>",
    "f32",
    "f64",
    "Option<f32>",
    "Option<f64>",
    "&[f32]",
    "&[f64]",
    "&mut [f64]",
    "Vec<f32>",
    "Vec<f64>",
    "Box<[f64]>",
    "Option<&[f64]>",
    "Option<Vec<f64>>",
    "Option<Box<[f64]>>",
    "Vec<Vec<f64>>",
];

/// Reasoned exceptions as `(file, function, parameter)`; keep empty.
const ALLOWED: &[(&str, &str, &str)] = &[];

#[test]
fn exported_bindings_take_host_values_as_js_value() {
    let exports = export_scan::scan_exports();
    let mut offenders = Vec::new();
    for export in &exports {
        for (param, ty) in &export.params {
            if FORBIDDEN.contains(&ty.as_str())
                && !ALLOWED.contains(&(
                    export.file.as_str(),
                    export.rust_name.as_str(),
                    param.as_str(),
                ))
            {
                offenders.push(format!(
                    "{}: {}({param}: {ty})",
                    export.file, export.rust_name
                ));
            }
        }
    }
    assert!(
        exports.len() > 500,
        "scanner found only {} exports",
        exports.len()
    );
    assert!(
        offenders.is_empty(),
        "take these as JsValue / Option<JsValue> and convert with utils::input:\n{}",
        offenders.join("\n")
    );
}
