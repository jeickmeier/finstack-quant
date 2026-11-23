//! Canonical serde representations whose domain storage cannot describe its
//! JSON contract directly to `schemars`.

mod bounded_f64;
mod codec;
mod counts_wire;
mod date_wire;
mod decimal_wire;
mod non_finite;
#[cfg(test)]
mod tests;

pub use bounded_f64::{
    ClosedUnitIntervalF64Wire, CorrelationWire, NonNegativeF64Wire, OpenUnitIntervalF64Wire,
    PercentageQuantityWire, PositiveF64Wire, deserialize_correlation,
    deserialize_open_unit_interval_f64, deserialize_positive_f64, serialize_correlation,
    serialize_open_unit_interval_f64, serialize_positive_f64,
};
pub use codec::{SchemaVersion, serde_label, serde_parse, serde_parse_with_context, serde_tag};
pub use counts_wire::{count, counts, signed_count};
pub use date_wire::DateWire;
pub use decimal_wire::{DecimalWire, decimal, optional_decimal};
pub use non_finite::{
    FiniteOrNanWire, NanSentinel, NonFiniteF64Wire, NonFiniteFields, NonFiniteSentinel,
    non_finite_f64, non_finite_f64_seq,
};
