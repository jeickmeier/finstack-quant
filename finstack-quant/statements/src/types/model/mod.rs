//! Financial model specification types.

pub(crate) mod capital_structure;
pub(crate) mod spec;
#[cfg(test)]
mod tests;

pub(crate) use capital_structure::validated_explicit_value_type;
