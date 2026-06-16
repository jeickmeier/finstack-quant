//! Deterministic seed derivation for Monte Carlo pricing.
//!
//! Provides deterministic seed generation based on instrument ID and scenario
//! name. This ensures that MC-priced instruments produce identical results
//! across multiple runs when computing greeks via finite differences.
//!
//! # Usage
//!
//! ```ignore
//! use finstack_quant_core::types::InstrumentId;
//! use finstack_quant_monte_carlo::seed;
//!
//! let instrument_id = InstrumentId::from("BOND-001");
//!
//! // Base pricing scenario.
//! let base_seed = seed::derive_seed(&instrument_id, "base");
//!
//! // Finite-difference Greeks: every bump leg must share ONE seed scenario
//! // so common random numbers cancel the Monte Carlo noise. Deriving
//! // different seeds per leg (e.g. "delta_up" vs "delta_down") destroys the
//! // CRN coupling and inflates the Greek's variance by O(σ²/h²).
//! let greeks_seed = seed::derive_seed(&instrument_id, "greeks_crn");
//! // ... use `greeks_seed` for the base, up, AND down valuations.
//! # let _ = (base_seed, greeks_seed);
//! ```

use finstack_quant_core::types::InstrumentId;

const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV1A_64_PRIME: u64 = 0x100000001b3;

fn fnv1a_extend(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV1A_64_PRIME);
    }
    hash
}

/// Derive a deterministic seed from an instrument ID and scenario name.
///
/// The seed is computed using a stable FNV-1a hash of the instrument ID string
/// and scenario name. We avoid `DefaultHasher` here because its algorithm is an
/// implementation detail of the Rust standard library, so changing toolchains
/// could silently change Monte Carlo seeds.
///
/// This ensures that:
/// - Same instrument + same scenario → same seed
/// - Different instruments → different seeds (high probability)
/// - Different scenarios → different seeds
///
/// # Arguments
/// * `instrument_id` - The instrument identifier
/// * `scenario` - Scenario name (e.g., "base", "delta_up", "vega_down")
///
/// # Returns
/// A deterministic u64 seed
pub fn derive_seed(instrument_id: &InstrumentId, scenario: &str) -> u64 {
    let hash = fnv1a_extend(FNV1A_64_OFFSET_BASIS, instrument_id.as_ref().as_bytes());
    let hash = fnv1a_extend(hash, &[0xff]);
    fnv1a_extend(hash, scenario.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::types::InstrumentId;

    #[test]
    fn test_seed_determinism() {
        let id = InstrumentId::from("TEST-001");

        // Same inputs should produce same seed
        let seed1 = derive_seed(&id, "base");
        let seed2 = derive_seed(&id, "base");
        assert_eq!(seed1, seed2);
        assert_eq!(seed1, 0xf4f7d33fea0c2ce9);

        // Different scenarios should produce different seeds
        let seed_base = derive_seed(&id, "base");
        let seed_delta_up = derive_seed(&id, "delta_up");
        assert_ne!(seed_base, seed_delta_up);
    }

    #[test]
    fn test_seed_different_instruments() {
        let id1 = InstrumentId::from("TEST-003");
        let id2 = InstrumentId::from("TEST-004");

        let seed1 = derive_seed(&id1, "base");
        let seed2 = derive_seed(&id2, "base");

        // Different instruments should produce different seeds (with high probability)
        assert_ne!(seed1, seed2);
    }
}
