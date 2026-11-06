# CORE-CONVENTIONS-018: Decimal rates, percent/basis-point scaling and integer spread arithmetic

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `types/rates.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

`Rate` stores a finite `f64` in decimal units (0.05 = 5%); `Bps` stores signed integer basis points. Rates permit negative values and impose no economic floor/cap beyond finiteness. The distinction prevents ambiguity between rate levels and integer bump amounts, but integer quantization is explicitly lossy.

## Algorithm and selection

Rate constructors accept decimal, percent or integer bp; parsing also accepts decimal, `%`, `bp` or `bps` text after outer trim and case normalization, including fractional text bp. Percent/bp float scaling first parses the shortest decimal representation of the f64 into Decimal and shifts scale by two/four digits, converting back to f64. If exact Decimal parsing/scaling is unavailable, it falls back to direct float multiplication/division; signed zero directly follows float scaling. This limits visible binary scaling artifacts without making Rate a Decimal amount.

`as_bp_f64` preserves fractional basis points through that scaling helper. `as_bp` instead uses `round(rate*10000) as i32`: ties follow f64 round away from zero, and the cast saturates if out of i32 range. `as_percent`/`as_bp_f64` do not return Results and can overflow to infinity for huge finite Rate values through the float fallback. Display uses four decimals in percent units.

Rate add/subtract/multiply/divide validate the resulting Rate as finite; division explicitly rejects zero. The scalar operand itself is not separately finite-validated in these arithmetic methods, so division by positive infinity yields a finite zero Rate. Bps `try_new(f64)` requires a finite whole number in i32 range; it does not round fractional bp. Bps add/subtract/multiply/divide/abs/neg use checked integer operations; integer division truncates toward zero and rejects zero/overflow. Conversion to Rate divides integer bp by 10,000; percent divides by 100.

## Conventions, limits and alternatives

One bp = 0.0001 decimal = 0.01 percent. No day-count, compounding or curve role attaches to Rate; callers must carry those separately. Checked Rate arithmetic prevents nonfinite stored results but does not guarantee finite presentation conversions. Integer Bps is suitable only where truncation/whole-point bumps are intended; fractional Rate is a supported alternative.

## Evidence

[Implementation and inline tests](../../../../finstack-quant/core/src/types/rates.rs): `fractional_bp_accessors_scale_without_binary_noise`, `from_percent_keeps_tiny_magnitudes_ordering_and_signed_zero`, checked i32 minimum handling, parsing and serde nonfinite rejection. The inspected tests validate ordinary scaling and edge domains; they do not establish an approved quantization rule for financial risk bumps. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) gives numerical background.

These are observed behaviors, not adopted precision guarantees; no source change is authorized by this record.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
