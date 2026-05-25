# Numerical Regression Review

Use this mode when a change may alter prices, sensitivities, calibration, risk, or solver behavior.

## What To Check

- Golden prices or risk numbers for representative instruments.
- Edge cases: expiry, zero or negative rates, zero vol, deep ITM/OTM, empty curves, missing fixings, and non-monotone pillars.
- Tolerance policy: absolute vs relative tolerance, units, currency scale, and basis-point interpretation.
- Solver stability: bracketing, convergence criteria, max iterations, low-vega behavior, fallback behavior, and error messages.
- Finite differences: bump size, central vs forward difference, unit of output, and reproducibility.
- Determinism: seeded RNG, stable ordering, parallel vs serial equivalence.
- Cross-language consistency: Rust, Python, and WASM results match within documented tolerance where exposed.

## Review Output Addendum

Add this section to the normal quant review when relevant:

```markdown
## Numerical Regression Notes
- Baseline checked:
- Tolerance policy:
- Edge cases covered:
- Missing regression tests:
- Suggested targeted tests:
```
