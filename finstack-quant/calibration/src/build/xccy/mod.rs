//! Builders for cross-currency swap instruments from market quotes.

mod builder;
#[cfg(test)]
mod tests;

pub(crate) use builder::build_xccy_instrument;
