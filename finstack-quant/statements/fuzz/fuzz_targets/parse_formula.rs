//! Fuzz target for the surrounding parser or validation boundary.
//!
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = finstack_quant_statements::dsl::parse_formula(data);
    let _ = finstack_quant_statements::dsl::parse_and_compile(data);
});
