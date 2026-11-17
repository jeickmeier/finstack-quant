//! Versioned random streams for stochastic forecasts.
//!
//! Every stochastic forecast draws from the shared PCG64 generator on a
//! stream derived from one key: the key version, the method's exact seed, the
//! node, and the evaluation mode with its run seed and path where the run is
//! a Monte Carlo path. The key is hashed as canonical JSON, so the mapping
//! from a recorded key to a stream is replayable, and a change to it is a
//! change of [`STREAM_VERSION`].

use crate::error::{Error, Result};
use finstack_quant_core::math::random::Pcg64Rng;
use serde::Serialize;

/// Version of the mapping from a stream key to a generator stream.
pub(crate) const STREAM_VERSION: u32 = 1;

/// One Monte Carlo path of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathId {
    /// Seed of the Monte Carlo run.
    pub run_seed: u64,
    /// Zero-based index of the path in the run.
    pub path: u64,
}

/// The identity of one node's random stream in one run or path.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StreamKey<'a> {
    /// The forecast method's exact configured seed.
    pub seed: u64,
    /// The forecast node.
    pub node_id: &'a str,
    /// The Monte Carlo path; `None` for a single run.
    pub path: Option<PathId>,
}

/// Canonical hashed form of a [`StreamKey`]. Integers are decimal text so
/// the hash input is exact above 2^53.
#[derive(Serialize)]
struct KeyDocument<'a> {
    domain: &'static str,
    version: u32,
    seed: String,
    node_id: &'a str,
    mode: &'static str,
    run_seed: Option<String>,
    path: Option<String>,
}

impl StreamKey<'_> {
    /// The PCG64 starting state and stream selector of this key.
    pub(crate) fn derive(&self) -> Result<(u64, u64)> {
        let document = KeyDocument {
            domain: "finstack.statements.forecast_stream",
            version: STREAM_VERSION,
            seed: self.seed.to_string(),
            node_id: self.node_id,
            mode: if self.path.is_some() {
                "monte_carlo_path"
            } else {
                "single_run"
            },
            run_seed: self.path.map(|path| path.run_seed.to_string()),
            path: self.path.map(|path| path.path.to_string()),
        };
        let hash = finstack_quant_core::canonical::content_hash(&document)
            .map_err(|error| Error::forecast(format!("cannot derive a random stream: {error}")))?;
        let digits = hash.trim_start_matches("sha256:");
        let word = |range: std::ops::Range<usize>| {
            digits
                .get(range)
                .and_then(|digits| u64::from_str_radix(digits, 16).ok())
                .ok_or_else(|| Error::forecast("cannot derive a random stream: malformed hash"))
        };
        Ok((word(0..16)?, word(16..32)?))
    }

    /// The generator positioned at the start of this key's stream.
    pub(crate) fn rng(&self) -> Result<Pcg64Rng> {
        let (state, stream) = self.derive()?;
        Ok(Pcg64Rng::new_with_stream(state, stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::math::random::RandomNumberGenerator;

    fn key(seed: u64, node_id: &str, path: Option<PathId>) -> StreamKey<'_> {
        StreamKey {
            seed,
            node_id,
            path,
        }
    }

    #[test]
    fn every_key_component_selects_a_different_stream() {
        let path = |run_seed, path| Some(PathId { run_seed, path });
        let keys = [
            key(42, "a", None),
            key(43, "a", None),
            key(42, "b", None),
            key(42, "a", path(0, 0)),
            key(42, "a", path(1, 0)),
            key(42, "a", path(0, 1)),
            // Seeds that differ only above 2^53.
            key(9_007_199_254_740_992, "a", None),
            key(9_007_199_254_740_993, "a", None),
        ];
        let streams: Vec<_> = keys.iter().map(|key| key.derive().unwrap()).collect();
        for (index, stream) in streams.iter().enumerate() {
            assert!(!streams[..index].contains(stream), "{:?}", keys[index]);
        }
    }

    /// A recorded state and stream replay the draws without the key.
    #[test]
    fn a_recorded_stream_replays_the_draws() {
        let key = key(7, "revenue", None);
        let (state, stream) = key.derive().unwrap();
        let mut original = key.rng().unwrap();
        let mut replayed = Pcg64Rng::new_with_stream(state, stream);
        for _ in 0..8 {
            assert_eq!(
                original.normal(0.0, 1.0).to_bits(),
                replayed.normal(0.0, 1.0).to_bits()
            );
        }
    }

    /// The version 1 mapping is pinned: changing it needs a new version.
    #[test]
    fn version_one_mapping_is_pinned() {
        assert_eq!(STREAM_VERSION, 1);
        let derived = key(42, "revenue", None).derive().unwrap();
        assert_eq!(derived, PINNED);
        // The draws of the stream are part of the version, not only its key.
        let mut rng = key(42, "revenue", None).rng().unwrap();
        let normals: Vec<u64> = (0..3).map(|_| rng.normal(0.0, 1.0).to_bits()).collect();
        let indices: Vec<usize> = (0..6).map(|_| rng.index(3)).collect();
        assert_eq!(
            (normals, indices),
            (PINNED_NORMALS.to_vec(), PINNED_INDICES.to_vec())
        );
    }

    const PINNED_NORMALS: [u64; 3] = [
        4_608_841_250_242_571_330,
        13_830_133_336_873_686_510,
        13_809_749_431_602_484_183,
    ];
    const PINNED_INDICES: [usize; 6] = [0, 0, 0, 0, 1, 1];

    const PINNED: (u64, u64) = (3_751_829_398_131_865_565, 15_411_871_087_006_207_807);
}
