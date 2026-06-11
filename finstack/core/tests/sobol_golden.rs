//! Golden test: Sobol direction numbers vs the published Joe & Kuo (2008) table.
//!
//! Validates `finstack_core::math::random::sobol::SobolRng` against the
//! official `new-joe-kuo-6.21201` direction-number table (dimensions 2-40),
//! stored verbatim at `tests/data/sobol_joe_kuo_d2_40.txt` (provenance in
//! `tests/data/README_sobol.md`).
//!
//! # Encoding mapping (table -> library)
//!
//! The library (`initialize_direction_numbers`) uses the same encoding as the
//! Joe & Kuo reference C++ code, so no re-encoding is needed:
//!
//! - `s` is the primitive-polynomial degree.
//! - `a` packs the interior coefficients `a_1..a_{s-1}` of
//!   `x^s + a_1 x^{s-1} + ... + a_{s-1} x + 1`; bit `s-1-k` of `a` is `a_k`
//!   (leading `x^s` and trailing `1` are implicit, not stored).
//! - Direction numbers: `v_i = m_i << (32 - i)` for `i = 1..=s` (1-based),
//!   then `v_i = v_{i-s} ^ (v_{i-s} >> s) ^ XOR_{k=1..s-1, a_k=1} v_{i-k}`.
//!
//! The embedded `(s, a, m)` table in `sobol.rs` is private, so parameters are
//! verified transitively: the reference expansion below derives all 32
//! direction numbers per dimension from the table file, and the library's
//! output at index `n = 2^k` exposes exactly `v_k` (binary-expansion
//! construction). Equality for every `k in 0..32` and every dimension is an
//! exact check of `s`, `a`, and all `m_i`.
//!
//! # Point construction and ordering
//!
//! The library generates point `n` directly from the binary expansion of `n`
//! (`x_n = XOR of v_j over set bits j of n`), not via the Gray-code recurrence
//! `x_{n+1} = x_n ^ v_{ctz(n+1)}`. Both produce the same point set; Gray-code
//! order is a permutation (`gray-code point m == direct point gray(m)`). The
//! reference here uses the direct construction to match the library's order.
//!
//! # Dimension indexing
//!
//! Library dimension index 0 is the van der Corput / table d=1 dimension
//! (`v_k = 2^(31-k)`, no table row); library index `i` corresponds to table
//! dimension `d = i + 1`.
//!
//! # u01 mapping
//!
//! With `scramble_seed = 0` the Owen scramble is the identity and the library
//! maps integer state `x` to `(x + 0.5) / 2^32`. Both sides of that mapping
//! are exact in f64 (33-bit numerator, power-of-two divisor), so all
//! comparisons below use exact `f64` equality.

use finstack_core::math::random::sobol::{SobolRng, MAX_SOBOL_DIMENSION};
use std::path::Path;

/// One parsed row of the Joe & Kuo table.
struct JoeKuoRow {
    d: usize,
    s: usize,
    a: u32,
    m: Vec<u32>,
}

/// Parse `tests/data/sobol_joe_kuo_d2_40.txt` (header line + rows d=2..40).
fn load_joe_kuo_rows() -> Vec<JoeKuoRow> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sobol_joe_kuo_d2_40.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let fields: Vec<u64> = line
            .split_whitespace()
            .map(|t| t.parse().expect("integer field"))
            .collect();
        if fields.is_empty() {
            continue;
        }
        let (d, s) = (fields[0] as usize, fields[1] as usize);
        assert_eq!(fields.len(), 3 + s, "row d={d}: expected {s} m-values");
        rows.push(JoeKuoRow {
            d,
            s,
            a: fields[2] as u32,
            m: fields[3..].iter().map(|&x| x as u32).collect(),
        });
    }
    assert_eq!(rows.len(), 39, "expected rows for d=2..40");
    assert_eq!(rows.first().expect("non-empty").d, 2);
    assert_eq!(rows.last().expect("non-empty").d, 40);
    rows
}

/// Standard Joe & Kuo direction-number expansion (independent reimplementation).
///
/// Returns the 32 direction numbers `v_0..v_31` (0-based; `v_k` scales `2^-(k+1)`),
/// each stored as a 32-bit integer `v * 2^32`.
fn expand_direction_numbers(s: usize, a: u32, m: &[u32]) -> [u32; 32] {
    let mut v = [0u32; 32];
    for (i, &mi) in m.iter().enumerate().take(s) {
        assert!(mi % 2 == 1 && mi < (1 << (i + 1)), "m_i constraints");
        v[i] = mi << (31 - i);
    }
    for i in s..32 {
        let mut vi = v[i - s] ^ (v[i - s] >> s);
        for k in 1..s {
            if (a >> (s - 1 - k)) & 1 == 1 {
                vi ^= v[i - k];
            }
        }
        v[i] = vi;
    }
    v
}

/// Full reference table: index 0 is the van der Corput (d=1) dimension.
fn reference_direction_numbers() -> Vec<[u32; 32]> {
    let mut all = Vec::with_capacity(MAX_SOBOL_DIMENSION);
    let mut vdc = [0u32; 32];
    for (k, slot) in vdc.iter_mut().enumerate() {
        *slot = 1u32 << (31 - k);
    }
    all.push(vdc);
    for row in load_joe_kuo_rows() {
        assert_eq!(row.d, all.len() + 1, "table rows must be contiguous");
        all.push(expand_direction_numbers(row.s, row.a, &row.m));
    }
    all
}

/// Direct (binary-expansion) Sobol integer state for point `n`, dimension `dims`.
fn reference_point_int(v: &[u32; 32], n: u64) -> u32 {
    let mut x = 0u32;
    let mut n = n;
    let mut bit = 0usize;
    while n > 0 {
        if n & 1 == 1 {
            x ^= v[bit];
        }
        n >>= 1;
        bit += 1;
    }
    x
}

/// Exact u01 value the library emits for integer state `x` (unscrambled).
fn to_u01(x: u32) -> f64 {
    (f64::from(x) + 0.5) / 4_294_967_296.0
}

/// (a) Parameter check: every direction number `v_k` (k=0..31) of every
/// dimension (1..=40) must match the Joe & Kuo table expansion. The library's
/// point at index `n = 2^k` is exactly `v_k`, so this transitively verifies
/// the embedded `s`, `a`, and all `m_i` for dimensions 2..40.
#[test]
fn sobol_direction_numbers_match_joe_kuo_table() {
    let reference = reference_direction_numbers();
    assert_eq!(reference.len(), MAX_SOBOL_DIMENSION);
    let mut sobol = SobolRng::try_new(MAX_SOBOL_DIMENSION, 0).expect("valid dimension");

    for k in 0..32usize {
        sobol.reset();
        sobol.skip(1u64 << k);
        let point = sobol.next_point();
        for (dim_idx, v) in reference.iter().enumerate() {
            let expected = to_u01(v[k]);
            assert_eq!(
                point[dim_idx],
                expected,
                "direction number v_{k} mismatch for table dimension d={} \
                 (library index {dim_idx}): library u01 {}, Joe-Kuo table u01 {expected} \
                 (integer v_k = {})",
                dim_idx + 1,
                point[dim_idx],
                v[k],
            );
        }
    }
}

/// (b) Reference-point check: first 16 integer states for table dimensions
/// d in {1, 2, 5, 21, 30, 40} (library indices d-1), generated by an
/// independent reimplementation of the Joe & Kuo expansion + direct Sobol
/// construction, must equal the library output exactly (via the
/// `(x + 0.5) / 2^32` mapping, exact in f64).
#[test]
fn sobol_first_16_points_match_reference() {
    let reference = reference_direction_numbers();
    let mut sobol = SobolRng::try_new(MAX_SOBOL_DIMENSION, 0).expect("valid dimension");

    let check_dims: [usize; 6] = [1, 2, 5, 21, 30, 40]; // table d
    for n in 0..16u64 {
        let point = sobol.next_point();
        for &d in &check_dims {
            let dim_idx = d - 1; // library index
            let x = reference_point_int(&reference[dim_idx], n);
            let expected = to_u01(x);
            assert_eq!(
                point[dim_idx], expected,
                "point n={n}, table dimension d={d}: library u01 {} != reference u01 \
                 {expected} (reference integer state {x})",
                point[dim_idx],
            );
        }
    }
}
