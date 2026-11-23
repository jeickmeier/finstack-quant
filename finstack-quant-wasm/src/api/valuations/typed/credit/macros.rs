//! Shared macro for CDS-family upfront payment setters.

/// The `upfront(paymentDate, amount)` setter shared by the CDS-family builders.
macro_rules! upfront_setter {
    ($bty:ident, $bjs:literal) => {
        dated_money_setter!(
            $bty,
            $bjs,
            /// Set the upfront payment exchanged at inception.
            /// @param paymentDate - Upfront payment date as an ISO-8601 string.
            /// @param amount - Upfront amount; positive when the protection buyer pays.
            /// @returns The builder, for chaining.
            /// @throws Error - Throws with kind `validation` if `paymentDate` is malformed or the builder was already consumed by `build()`, and kind `invalid_type` if `paymentDate` is not a string.
            upfront
        );
    };
}
