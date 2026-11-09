# CORE-CONVENTIONS-018: Decimal rates, percent/basis-point scaling and integer spread arithmetic

- Decision status: **Accepted — complete Rate/Bps package, 2026-11-07**.
- Implementation status: **Reconciled 2026-11-08** for `types/rates.rs`, the Python/WASM Rate surface and the term-loan covenant step-up consumer (item 11: finite nonnegative fractional-bp increases through checked Decimal, no whole-bp rounding). Former discrepancies have been removed from the follow-up register.
- Validation status: **Evidence inspected; independent conversion/arithmetic examples passed; production qualification pending**. Exact unit/text grammar, numeric-versus-text rounding, normal/subnormal/MAX, integer and underflow examples passed. Linked Rust/host tests were located or inspected, not run; these checks do not certify current parser, arithmetic or host behavior.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

This record owns Rate/Bps representation, explicit unit conversion, text admission,
arithmetic, display and primitive wire semantics. [Accepted 0020](../conventions/0020-numerical-representation-and-rounding.md)
permits binary64 model inputs; [012](012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
owns separate Decimal Money admission/rounding. [016](016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting)
and [017](017-irr.md#accepted-decision-irr-and-xirr-timing-eligibility-and-numerical-acceptance)
retain their operation-specific compounding/rate domains. A unit type alone does
not select a coupon basis, curve, risk-bump role or product eligibility.

## Accepted decision: Rate and Bps units, admission and precision

Accepted on **2026-11-07** after whole-record review. Runtime implementation
and independent production/host qualification remain pending;
observed source behavior is retained separately below.

1. **Keep Rate as a finite signed binary64 decimal model value.** Decimal here
   names the unit, not Decimal storage: `0.05=5%=500bp`, `1bp=0.0001` decimal
   `=0.01` percentage point. Normalize signed zero at every construction, parse,
   arithmetic, negation and restore boundary. Equality/order/zero predicates use
   the stored finite value, not an epsilon or a displayed quote. Negative values,
   including below -1, are allowed at this primitive layer; each financial
   operation owns stricter domains. No day-count, compounding, currency, curve,
   rate-versus-spread or absolute-versus-relative bump is inferred from magnitude.

2. **Keep Bps as exact signed whole i32 basis points.** Preserve integer
   construction/extraction and range `[-2147483648,2147483647]`. `Bps::try_new(f64)`
   requires a finite exactly integral value in that range, without epsilon,
   truncation, nearest rounding or saturation; signed zero becomes integer zero.
   Fractional bp belongs in Rate, not a new decimal-Bps type or widened alias.
   Integer Bps-to-decimal/percent/Rate views may be binary64 approximations;
   their integer input is exact and their bounded quotients are always finite.
   Do not imply that every instrument's spread must therefore be whole bp.

3. **Use one explicit numeric unit-scaling rule.** Genuine f64 constructor
   inputs already denote binary64 values. `Rate::from_decimal(r)` admits finite
   r unchanged apart from zero normalization; `from_percent(p)` performs one
   binary64 `p/100`; broaden canonical `from_bp(b)` to finite fractional f64
   input and one binary64 `b/10000`, returning Result. `as_decimal` returns the
   stored value; `as_percent` and canonical `as_bp` return checked f64 views
   from `r*100`/`r*10000`. Reject nonfinite inputs/results and a nonzero scaling
   result rounded to zero; permit nonzero subnormals of either sign. Remove
   `scale_decimal`, its range-dependent Decimal assistance and fallback, and
   any midpoint/integer snapping. Bps conversions use exact i32-to-f64 conversion
   then their stated division, with no implicit unlabelled Bps-to-float conversion.

4. **Remove Rate's implicit whole-bp quantization.** Delete the old rounded,
   saturating integer `Rate::as_bp` behavior; rename its fractional
   `as_bp_f64` view to the sole `as_bp -> Result<f64>`. `Bps::as_bp -> i32`
   remains exact extraction. Add no generic `round_bp`, rounding-mode default or
   compatibility alias merely to preserve obsolete behavior. A caller asking
   for Bps must supply an exact whole quote or select its own explicit qualified
   financial rounding stage/mode before checked integer admission. Unit views
   need not round-trip bit-exactly: even an integer-bp Rate view can differ from
   that integer by floating error, so strict Bps admission may reject it. Keep
   the original Bps when its exact integer identity matters.

5. **Give decimal quote text its own single-conversion boundary.** After
   outer whitespace trimming, accept numeric grammar
   `[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?`
   with an optional case-insensitive `%`, `bp` or `bps` suffix. A bare number is
   decimal Rate, not guessed percent. Permit whitespace only between the numeric
   token and its suffix, not inside the token. Reject underscores, grouping,
   nondecimal bases, nonfinite words, missing digits and repeated/mixed units.
   The explicit exponent must fit i64, including for lexical zero. Reuse 012's
   lexical rules where useful, not its bounded Decimal representation. Apply
   the unit's decimal exponent shift (0/-2/-4) in checked widened arithmetic to
   the full significand/exponent before one final standard f64 parse. Do not
   first require the unscaled quote/mantissa to fit f64 or Decimal. Thus
   `1e310bp` may become finite `1e306`. No BigInt/new parser framework or
   dependency is required for preserving digits and shifting an exponent.

6. **Make conversion accuracy and range admission explicit.** Text conversion
   targets the nearest binary64 value, ties-to-even, of the exact decimal quote
   after unit scaling: at most half an output ULP for normal results and
   `eta/2` absolute error for subnormals, where `eta=2^-1074`. These are real-number
   bounds; eta/2 is not itself representable in f64. At MAX, use its finite binade
   spacing when defining ULP. Admit the correctly rounded result only if finite;
   a decimal slightly above MAX that rounds to MAX is permitted, unlike 012's
   exact Decimal-magnitude admission. Reject lexical nonzero that rounds to zero;
   a valid lexical zero remains normalized zero after exponent validation.
   Numeric scaling uses ordinary nearest-even binary64 operations, so textual
   `1.1%` and genuine f64 `from_percent(1.1)` can differ by rounding. Preserve
   these distinct inputs rather than claim recovered decimal intention, exact
   binary-rational text identity, bit-exact unit round trips or ledger accuracy.
   Independently qualify these per-conversion targets; no financial price-error
   guarantee follows from a valid Rate.

7. **Validate scalar operands and checked Rate arithmetic explicitly.** Add/sub
   operate on validated Rates; multiplication/division take finite dimensionless
   scalars and validate them before zero/identity shortcuts. Reject both signs
   of zero divisor. Require finite results, rejecting nonzero multiply/divide
   results rounded to zero while admitting genuine zero inputs/cancellation and
   signed nonzero subnormals. No overflow clipping, infinite-divisor-to-zero,
   epsilon zero, Decimal detour or fallback precision path. Negation/absolute
   value remain infallible on finite stored values, with zero normalized.
   Binary64 arithmetic does not promise associativity or unchanged admission
   under reordering; operation-specific economic floors/caps remain owner-owned.

8. **Make integer Bps arithmetic exact or fail.** Retain checked integer
   addition/subtraction, multiplication by i32, absolute value and negation.
   `checked_div(i32)` requires nonzero divisor, representable quotient and zero
   remainder; fractional results error instead of truncating toward zero.
   `MIN/-1`, negation/abs(MIN) and other overflow error without a panic or wrap.
   Exact negative quotients are valid. Use explicit Rate conversion/arithmetic
   when a fractional-bp result is intended, rather than another truncating method.

9. **Make default display truthful without financial rounding.** Default Rate
   Display is shortest round-trip decimal-unit text, no percent scaling or fixed
   four-place formatting; its default output parses back to the same normalized
   stored value. Bps Display remains `Nbp`. Display is not canonical wire or a
   financial rounding stage. Percent/bp presentation first requests the checked
   view, then the owner selects formatting/precision; huge valid Rates cannot
   silently format as `inf%` or panic because a fallible percent getter was used.
   Do not preserve original quote spelling/unit as part of Rate identity.

10. **Keep primitive wire units and host admission unambiguous.** Serialize
    Rate as its finite decimal-unit binary64 number and Bps as its exact i32 bp
    number; restore through the same validated constructors and normalize zero.
    Unit-suffixed quote strings use the explicit text parser, not a second wire
    representation or an inferred numeric magnitude. Numeric host/deserializer
    inputs are already supplied numeric values: validation cannot recover text
    digits or nonzero intent lost by an earlier float/JSON conversion. Strict
    lexical-underflow rules apply when the quote parser receives the original
    text, not to a pre-rounded zero passed as f64. Canonical finite stored Rate
    round trips preserve its model value. Reject booleans, nonfinite values and
    fractional/out-of-range Bps in host constructors; never use unchecked casts
    or null/default substitution. Reuse existing error mapping and private types,
    with no new rate registry, wrapper hierarchy or global rounding configuration.

11. **Keep financial roles and Money boundaries with their owners.** A 25bp
    absolute rate change is 0.0025 decimal; a 1% relative increase on 3% is a
    different operation, giving about3.03%, not4%. Neither Rate nor Bps chooses
    that role. Whole-bp bond fields, covenant steps, index/spread admissibility,
    pricing risk-bump units and compounding conversions remain product policies.
    Remove hidden rounding/saturation in consumers or obtain explicit owner
    selection for a genuine financial rounding boundary; do not mechanically
    substitute a new quantizer. In particular reconcile the term-loan covenant
    step-up's current whole-bp rounding and unchecked cast at its owner. A finite
    Rate such as 1e-29 may be valid here and fail 012's narrower Decimal monetary
    factor admission; do not relax that boundary or convert Money to float.

12. **Propagate the complete canonical surface together.** Reconcile Rust
    signatures/re-exports, helpers/parsing/display/serde, Python/WASM constructor
    and accessor types/errors, stubs/facade, schemas/parity, examples/tests/benches
    and every actual rate/bump/product consumer in one working slice. Rate
    `from_bp` now accepts fractional numeric quotes, Rate `as_bp` returns a
    fallible fractional view, percent views are fallible and Bps division is exact.
    Remove `_f64` aliases, saturating/rounded extraction, Decimal fallback and
    stale precision/round-trip claims without compatibility paths or migrations.
    Source inspection or convention acceptance does not execute those changes
    or independently qualify financial products and conversion boundaries.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Rate decimal0.05 / percent5 / bp500 | About the same model rate; units are explicit, not inferred by magnitude. |
| Bare text5 / text5% | Decimal5 (500%) / decimal0.05. |
| Numeric from_bp62.5; Bps.try_new62.5 | Fractional Rate allowed / exact-whole admission error. |
| Rate as_bp on62.5bp | Fractional view about62.5, never rounded63. |
| Rate as_percent/as_bp on huge finite Rate | Result overflow error, not infinity or i32 saturation. |
| Decimal Rate -2 / negative bp | Primitive-valid; downstream operation owns any economic lower bound. |
| ±0 across constructors/negation/restore | One normalized zero, with exact zero/equality semantics. |
| Numeric percent or bp smallest subnormal divided to0 | Nonzero-underflow error; signed nonzero subnormal results remain valid. |
| Text 1e310bp / bare1e310 | Finite about1e306 / nonfinite-result error. |
| Text .5%, 5., +1e2 BP | Valid explicit quote grammar and units. |
| Text 1e-10000% / 0e-10000% | Nonzero-underflow error / valid zero. |
| Zero quote with malformed/out-of-i64 exponent | Grammar/domain error, not an early zero shortcut. |
| Text NaN, inf, 1_000bp, 1e 2%, 1bps% | Error, no permissive parser fallback. |
| Text1.1% versus from_percent(f64 1.1) | May differ by an ULP; distinct declared conversion boundaries. |
| Bps3→Rate→fractional bp view→strict Bps admission | No universal whole-integer round-trip guarantee; retain original Bps or explicitly own rounding. |
| Rate0 divided/multiplied by infinity or NaN | Nonfinite-scalar error before zero shortcut. |
| Rate divided by ±0; finite multiplication overflow | Error, no infinity/zero replacement. |
| Nonzero Rate product/quotient rounds0 | Error; genuine zero/cancellation remains valid. |
| Bps3/2 and Bps-3/2 | Fractional-result error, not ±1. |
| Bps-4/2 and Bps MIN/MIN | Exact -2bp and1bp. |
| Bps MAX+1, MIN-1, MIN/-1, abs/neg MIN | Checked overflow error without saturation/wrap/panic. |
| Default Rate display for0.05 or a tiny nonzero model rate | Decimal shortest text preserves value; no fixed-place percent zero. |
| Wire Rate0.05; Bps500 | Distinct decimal-rate and integer-bp numeric units; unit strings are a separate parser. |
| Numeric decoder already supplied0 after external underflow | Valid zero model input; original lexical intent cannot be reconstructed. |
| Finite Rate1e-29 passed to a Money factor adapter | Rate valid; 012 monetary admission may reject it without float escape. |
| Absolute25bp versus relative1% on3% | Decimal+0.0025 versus multiplicative growth to about0.0303; owner selects role. |
| Covenant step-up at a half bp | No generic silent round-up; product owner must settle exact eligibility/rounding. |

## Observed algorithm and selection

`Rate` stores a finite binary64 decimal and normalizes signed zero at every constructor, parse, arithmetic result, negation, absolute value and serde restore. `from_decimal` stores the value unchanged; `from_percent(p)` and `from_bp(b)` (finite fractional `f64`) perform one binary64 `p/100` or `b/10000` and return `Result`. A nonzero input whose scaled result rounds to zero is a validation error; nonzero subnormals are valid. `as_percent` and `as_bp` return checked `Result<f64>` views from `r*100` and `r*10000`; there is no rounded or saturating whole-bp extraction and no `_f64` alias.

Text parsing trims outer whitespace, then accepts `[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?` with an optional case-insensitive `%`, `bp` or `bps` suffix, allowing whitespace only before the suffix. A bare number is a decimal Rate. The exponent must fit `i64`, the unit shift (0/-2/-4) is applied to the exact significand/exponent, and one standard binary64 parse produces the nearest-even result, so `1e310bp` is finite `1e306` while `1e310` is a non-finite error. Default Display is shortest round-trip decimal-unit text.

Checked add/sub admit finite results, including exact cancellation to zero. Multiplication and division validate the scalar as finite before any zero shortcut, reject zero divisors of either sign, and reject nonzero results that round to zero. `Bps::try_new` requires a finite whole number in `i32` range; checked Bps add/sub/mul/abs/neg reject overflow, and `checked_div` additionally rejects a nonzero remainder (`3bp/2` errors). `Bps::as_rate` is the infallible exact-integer `bp/10000` view.

## Observed conventions, limits and alternatives

One bp = 0.0001 decimal = 0.01 percent. No day-count, compounding or curve role attaches to Rate; callers must carry those separately. Unit views need not round-trip bit-exactly (`Rate::from_percent(1.1).as_bp()` is `110.00000000000001`); keep the original `Bps` when integer identity matters. Python and WASM reject booleans for numeric rate inputs and raise or throw on overflowing views.

## Evidence

[Python primitive types](../../../../finstack-quant-py/src/bindings/core/types.rs),
[WASM primitive types](../../../../finstack-quant-wasm/src/api/core/types.rs)
and [term-loan covenant steps](../../../../finstack-quant/valuations/fixed-income/src/term_loan/types.rs)
locate actual surface/product reconciliation boundaries. Current host Rate exposes
both rounded i32 as_bp and fractional as_bp_f64, and term-loan increase_rate rounds
to whole bp then casts without this primitive's checked admission. Search found no
financial-product callers of Rate's integer as_bp quantizer or Bps integer
division beyond binding wrappers/tests; this does
not prove absence of external clients or adopt the covenant's financial rule.

[Rust's f64 string conversion](https://doc.rust-lang.org/std/primitive.f64.html#impl-FromStr-for-f64)
returns the nearest representable value under primitive rounding rules, supporting
the selected single-final-conversion boundary. Its broader NaN/infinity grammar is
not this Rate quote grammar. [Rust numeric casts](https://doc.rust-lang.org/reference/expressions/operator-expr.html#numeric-cast)
explain why the current float-to-i32 cast saturates rather than supplying financial
validation. These are language semantics, not currency, rate-role or financial
rounding standards; the package's unit/domain/quantization choices require their
accepted scope here and independent implementation qualification.

[Implementation and inline tests](../../../../finstack-quant/core/src/types/rates.rs): `fractional_bp_accessors_scale_without_binary_noise`, `from_percent_keeps_tiny_magnitudes_ordering_and_signed_zero`, checked i32 minimum handling, parsing and serde nonfinite rejection. The inspected tests validate ordinary scaling and edge domains; they do not establish an approved quantization rule for financial risk bumps. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) gives numerical background.

The source algorithms above remain observed behavior, not proof of the accepted precision/domain contract. No runtime source change is executed by this review.

The 2026-11-07 whole-record review passed documentation links/anchors,
status/count and whitespace checks. Independent exact/rational references checked
unit conversion and final quote range, nearest-even normal/subnormal/MAX cases,
strict whole-bp arithmetic/admission, numeric-versus-text distinctions and relative
versus absolute shifts. Those examples qualify the chosen boundaries, not the
current Rust implementation, codecs, host parity or instrument rounding policy.
No runtime source/tests/fixtures were changed and no production Rust/host test
or build pass was run.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
