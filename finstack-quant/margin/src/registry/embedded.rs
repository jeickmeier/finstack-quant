//! Loading, merging, and wire-format support for margin registry data.
//!
use finstack_quant_core::{Error, Result};
use serde_json::Value;

const SCHEDULE_IM_PATH: &str = include_str!("../../data/margin/schedule_im.v1.json");
const COLLATERAL_SCHEDULES_PATH: &str =
    include_str!("../../data/margin/collateral_schedules.v1.json");
const DEFAULTS_PATH: &str = include_str!("../../data/margin/defaults.v1.json");
const CCP_PATH: &str = include_str!("../../data/margin/ccp_methodologies.v1.json");
const SIMM_PATH: &str = include_str!("../../data/margin/simm.v1.json");
const XVA_DEFAULTS_PATH: &str = include_str!("../../data/margin/xva_defaults.v1.json");

/// Load all embedded margin registry JSON blobs into a single root object.
pub(crate) fn load_embedded_root() -> Result<Value> {
    Ok(serde_json::json!({
        "schedule_im": parse_json(SCHEDULE_IM_PATH)?,
        "collateral_schedules": parse_json(COLLATERAL_SCHEDULES_PATH)?,
        "defaults": parse_json(DEFAULTS_PATH)?,
        "ccp": parse_json(CCP_PATH)?,
        "simm": parse_json(SIMM_PATH)?,
        "xva_defaults": parse_json(XVA_DEFAULTS_PATH)?,
    }))
}

fn parse_json(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(|e| {
        Error::Validation(format!(
            "Failed to parse embedded margin registry JSON: {e}"
        ))
    })
}
