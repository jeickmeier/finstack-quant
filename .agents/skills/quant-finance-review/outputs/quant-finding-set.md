# Findings

### Major
- `finstack/valuations/src/instruments/fixed_income/bond.rs`: z-spread risk is reported without stating whether the bump is per bp of spread or absolute rate units. This can create downstream P&L attribution errors because users may compare it to curve DV01. Define the unit in the metric key/docstring and add a regression test for a 1 bp bump.

### Moderate
- `finstack/core/src/math/volatility/implied.rs`: implied-vol solving handles low-vega cases but lacks a golden regression for near-intrinsic short-expiry options. Add a tolerance-anchored test that verifies the solver fails loudly or returns the expected boundary behavior.

## Open Questions or Assumptions
- Did not verify Bloomberg/QuantLib parity for the cited examples.

## Brief Summary
The implementation shape is plausible, but the review is not complete until units and low-vega edge cases are pinned by regression tests.

## Quant Notes
- Use `references/numerical-regression.md` for tolerance and edge-case policy.
- Use the relevant market-standard file before changing day-count, settlement, or curve-role behavior.
