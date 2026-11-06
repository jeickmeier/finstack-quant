# CORE-CONVENTIONS-012: Decimal monetary arithmetic, explicit boundary rounding and currency scales

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `decimal.rs`, `currency.rs`, generated currency metadata and `money/{types,rounding}.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

[Accepted ADR 0020](../conventions/0020-numerical-representation-and-rounding.md) owns the model/monetary arithmetic boundaries, precision-preserving unconfigured Money construction, explicit rounding and nonfinite monetary-input rejection. This Draft owns the observed algorithms below and the choices not settled by that contract; it does not reopen those accepted requirements or certify caller compliance.

`Money` stores `rust_decimal::Decimal` plus a closed currency enum. Default float construction preserves representable internal precision rather than automatically rounding to minor units; configured ingest and display/output rounding are separate choices. The dependency supplies checked decimal arithmetic and rounding strategies. This supports amount conservation while acknowledging that model multipliers may originate as `f64`.

## Algorithm and selection

`parse_decimal` trims and parses exact decimal text. Scientific notation parses the mantissa exactly, subtracts exponent from scale, removes trailing zero coefficients only when needed to fit maximum scale, and multiplies the coefficient when exponent implies negative scale. It rejects inexact/unrepresentable scale or magnitude; a zero coefficient yields zero even with an extreme representable exponent.

`f64_to_decimal` rejects NaN/infinities and uses Decimal's checked conversion; finite values outside its magnitude domain error. `Money::new` normalizes signed zero and performs that conversion without minor-unit rounding. `from_decimal` is exact; integer tuple constructors preserve integral amounts. Configured float or decimal construction rounds to `cfg.ingest_scale(currency)` using the selected mode; the inspected config default is max(6,currency minor units), with validated per-currency overrides.

Add/subtract/compare and Money/Money division require identical currencies. Add/subtract/multiply/divide use checked Decimal operations; float multipliers/divisors are converted to Decimal first. Money/Money division returns `f64` after a decimal ratio; `amount()` similarly crosses to `f64`. Negation and absolute value retain currency. No operation implicitly performs FX conversion. `convert_at_rate` validates positive finite rate first, returns unchanged Money for same currency, otherwise multiplies Decimal amount by converted rate and changes currency without target-minor-unit rounding.

`round(None)` uses currency decimals and bankers rounding; negative digits error. Formatting defaults to currency decimals, bankers rounding, a currency prefix and no thousands separator; custom formatting has an optional one-character separator and precision cap 1,000,000. Rounding strategies are nearest-even; midpoint-away-from-zero (not all values away); truncate toward zero; floor; ceil. The internal `round_decimal` returns unchanged for negative precision, but the public Money round rejects it.

## Currency metadata and conventions

The generated table contains 159 currency codes and numeric IDs. Sixteen zero-decimal currencies are BIF/CLP/DJF/GNF/ISK/JPY/KMF/KRW/PYG/RWF/UGX/VND/VUV/XAF/XOF/XPF; BHD/IQD/JOD/KWD/LYD/OMR/TND have 3 decimals; CLF has 4; other supported currencies have 2. Code parsing ignores ASCII case but not whitespace. Unknown string/numeric codes error. The CSV includes historical currencies; this is a maintained static domain, not a current-active-currency filter or a cash-denomination rounding table.

## Limitations and alternatives

Decimal arithmetic can overflow and can round products/divisions to the dependency's finite precision; it is not arbitrary precision. Converting from an already-rounded `f64` cannot recover lost decimal digits. Display rounding does not mutate the stored amount. Same-currency conversion accepts a positive nonunit rate but returns the original amount. Exact-text input and configured rounding are supported alternatives; arbitrary precision or market cash-increment rounding are not implemented.

## Evidence

[Money](../../../../finstack-quant/core/src/money/types.rs), [rounding](../../../../finstack-quant/core/src/money/rounding.rs), [decimal parsing](../../../../finstack-quant/core/src/decimal.rs), [currency loader](../../../../finstack-quant/core/src/currency.rs), [currency CSV](../../../../finstack-quant/core/data/iso_4217.csv), [config defaults](../../../../finstack-quant/core/src/config.rs). Inline tests inspect exact decimal round trips, preserved precision, checked overflows, nonfinite rejection, hash/equality across scales and explicit rounding modes. [Money integration tests](../../../../finstack-quant/core/tests/money/rounding.rs) are arithmetic examples/invariants. Generated-reference `--check` passed; it verifies source consistency, not currency-status freshness. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) provides numerical background; central references do not establish the exact ISO snapshot or Decimal release rounding guarantees.

Implicit currency quantization in `Money::new` and acceptance of nonfinite monetary inputs are not open alternatives. No blanket precision certification follows from Decimal storage.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
