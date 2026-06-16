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
- **Subset taken**: rows for dimensions d = 2 through d = 40 (the range
  embedded in `finstack-quant/core/src/math/random/sobol.rs`; dimension 1 is the
  conventional van der Corput sequence and has no table row).

Column semantics (matching the Joe & Kuo reference C++ code):

- `s`: degree of the primitive polynomial for the dimension.
- `a`: bit-encoding of the interior polynomial coefficients `a_1..a_{s-1}` of
  `x^s + a_1 x^{s-1} + ... + a_{s-1} x + 1` (bit `s-1-k` of `a` is `a_k`).
- `m_i`: odd initial direction integers; direction numbers are
  `v_i = m_i * 2^(32-i)` for `i <= s`, then the standard recurrence
  `v_i = v_{i-s} ^ (v_{i-s} >> s) ^ XOR_{k=1..s-1, a_k=1} v_{i-k}`.

Consumed by `finstack-quant/core/tests/sobol_golden.rs`.
