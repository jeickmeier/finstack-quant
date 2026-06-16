//! Loader for option conventions embedded in JSON registries.

use crate::market::conventions::defs::OptionConventions;
use crate::market::conventions::ids::OptionConventionId;
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;

/// Load the Option conventions from the embedded JSON registry.
pub(crate) fn load_registry() -> Result<HashMap<OptionConventionId, OptionConventions>, Error> {
    let json = include_str!("../../../../data/conventions/option_conventions.json");
    super::json::parse_and_rekey(
        json,
        "Option",
        OptionConventionId::new,
        |rec: &OptionConventions| Ok(rec.clone()),
    )
}
