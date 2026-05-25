//! One-shot helper to dump a real `PnlAttribution` JSON, used to bootstrap
//! Python DataFrame tests. Run with:
//!
//! ```text
//! cargo test -p finstack-attribution --test attribution \
//!     attribution::dump_baseline::dump_pnl_attribution_baseline -- --nocapture --ignored
//! ```
//!
//! Capture the printed `BASELINE_PNL_ATTRIBUTION_JSON` value into the Python
//! fixture under `finstack-py/tests/fixtures/attribution_baseline.json`.

use finstack_attribution::{AttributionMethod, PnlAttribution};
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use time::macros::date;

#[test]
#[ignore = "fixture dump — run on demand with --ignored --nocapture"]
fn dump_pnl_attribution_baseline() {
    let attr = PnlAttribution::new(
        Money::new(1000.0, Currency::USD),
        "FIXTURE-BOND",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );
    let json = serde_json::to_string_pretty(&attr).expect("serialize baseline");
    println!("BASELINE_PNL_ATTRIBUTION_JSON_START");
    println!("{json}");
    println!("BASELINE_PNL_ATTRIBUTION_JSON_END");
}
