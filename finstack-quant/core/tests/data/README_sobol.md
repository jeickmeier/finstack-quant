# Sobol direction-number golden data

## `sobol_joe_kuo_d2_40.txt`

Subset (dimensions d = 2..40, plus the original header line) of the official
Joe & Kuo direction-number table `new-joe-kuo-6.21201`, kept verbatim in the
original whitespace-separated text format:

```
d  s  a  m_1 m_2 ... m_s
```

- **Source URL**: <https://web.maths.unsw.edu.au/~fkuo/sobol/new-joe-kuo-6.21201>
  (linked from <https://web.maths.unsw.edu.au/~fkuo/sobol/>)
- **Source file**: `new-joe-kuo-6.21201` (21201 dimensions; SHA-256 of the full
  downloaded file: `68eedd2a4e3b659b9695e7aff0f8ac68718bcf620730fc3d3a8c65df2a067441`)
- **Reference**: Joe, S., & Kuo, F. Y. (2008). "Constructing Sobol sequences
  with better two-dimensional projections." *SIAM J. Sci. Comput.*, 30(5),
  2635-2654.
- **Retrieved**: 2026-06-11
- **Subset taken**: rows for dimensions d = 2 through d = 40 — the range
  embedded in `finstack-quant/core/src/math/random/sobol.rs`, whose
  `MAX_SOBOL_DIMENSION` is 40. Dimension 1 is the conventional van der Corput
  sequence (`v_i = 2^(32-i)`) and has no table row, so the file is one header
  line plus 39 data rows.

Column semantics (matching the Joe & Kuo reference C++ code):

- `s`: degree of the primitive polynomial for the dimension.
- `a`: bit-encoding of the interior polynomial coefficients `a_1..a_{s-1}` of
  `x^s + a_1 x^{s-1} + ... + a_{s-1} x + 1` (bit `s-1-k` of `a` is `a_k`).
- `m_i`: odd initial direction integers; direction numbers are
  `v_i = m_i * 2^(32-i)` for `i <= s`, then the standard recurrence
  `v_i = v_{i-s} ^ (v_{i-s} >> s) ^ XOR_{k=1..s-1, a_k=1} v_{i-k}`.

`sobol.rs` uses the same encoding as the reference C++ code, so no re-encoding
step is needed when reading the table into the reference expansion.

## Use and verification

Consumed by `finstack-quant/core/tests/sobol_golden.rs`. The `(s, a, m)` table
embedded in `sobol.rs` is private, so both tests check it *transitively*, through
the direction numbers it expands to:

- `sobol_direction_numbers_match_joe_kuo_table` — for every dimension up to
  `MAX_SOBOL_DIMENSION`, all 32 direction numbers `v_0..v_31` match the expansion
  of this file. The library's point at index `n = 2^k` is exactly `v_k`, which is
  what makes the check exact on `s`, `a`, and every `m_i`.
- `sobol_first_16_points_match_reference` — the first 16 points for table
  dimensions d ∈ {1, 2, 5, 21, 30, 40} match points built by an independent
  reimplementation of the expansion and the direct (binary-expansion) Sobol
  construction.

Both compare with exact `f64` equality: with `scramble_seed = 0` the Owen
scramble is the identity and the `(x + 0.5) / 2^32` mapping is exact.

```bash
cargo nextest run -p finstack-quant-core --test core sobol_golden
```

Any edit to the embedded table must be re-validated against this file, not
against a hand-checked sample.
