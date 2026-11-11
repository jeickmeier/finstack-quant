# CORE-CONVENTIONS-012: Decimal monetary arithmetic, explicit boundary rounding and currency scales

- Decision status: **Accepted — complete monetary/currency package, 2026-11-07**.
- Implementation status: **Partly reconciled 2026-11-08**: items 1 (zero normalization in `Money::from_decimal`), 2 (the exact text grammar in `parse_decimal`, with the final value tested after the exponent) and 3 (exact `DecimalWire` restore and canonical normalized fixed-point output, exact Python `int`) landed. Item 4 landed 2026-11-09: `f64_to_decimal` (every `Money::new` and float adapter) admits exactly the shortest round-trip text and rejects any value outside the Decimal domain. By owner decision, pricer model outputs enter `Money` through the one named adapter `Money::from_model_value` (`f64_to_decimal_model_value`), which rounds the same text half-even to Decimal's 28-digit scale, so a sub-`1e-28` value becomes zero; it is not an input path. Items 6, 7, the ratio half of 5, and 11 landed 2026-11-09 (B3): Money sums and differences are exact when representable and otherwise round once, half-even, only below the currency's minor unit, erroring if a minor-unit digit would be lost (owner decision D7, amending #6's strict exact-or-error, which broke sums of full-precision bounded products); `checked_mul`/`checked_div` take a Decimal scalar, use the dependency's qualified greatest-scale half-even rounding and reject nonzero underflow, with `checked_mul_f64`/`checked_div_f64` as the named float adapters; `ratio` divides correctly rounded binary64 views without a Decimal quotient; `convert_at_rate` takes a strictly positive Decimal rate, requires exactly one for the same currency and validates before shortcuts, with `convert_at_rate_f64` as the float adapter. The `amount()` view (#5) is still `rust_decimal::as_f64`, which is not correctly rounded (about 10% of values are one ulp off). Items 8–10 landed 2026-11-10 (B4): `Money::round(scale, mode)` takes an explicit scale `0..=28` and mode with no default and no invalid-precision identity, and normalizes a rounded zero; `AwayFromZero`/`away_from_zero` is renamed `MidpointAwayFromZero`/`midpoint_away_from_zero` without an alias; `RoundingMode` has no default and the policy and stamped `RoundingContext` mode is optional (absent on the wire when unselected); `ingest_scale` returns only a configured override, so an absent override means no ingest rounding, and an override without a mode errors; `from_decimal_with_config` returns `Result` and float admission precedes configured rounding; formatting precision is capped at 28, the nearest-even display default is a declared presentation default, and a displayed zero has no sign; Python `Money.__round__` is removed in favour of `round(scale, mode)` (O10), with a WASM `round` twin. Items 5 (amount view) and 12–15 remain. Scoped to `decimal.rs`, `currency.rs`, generated currency metadata and `money/{types,rounding}.rs`.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent exact/rational/rounding/float-reference/currency checks passed; production Rust/host tests, backend accuracy and full ISO-source qualification were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

[Accepted ADR 0020](../conventions/0020-numerical-representation-and-rounding.md) owns the model/monetary arithmetic boundaries, precision-preserving unconfigured Money construction, explicit rounding and nonfinite monetary-input rejection. The complete accepted package below settles the remaining exact-input, bounded-arithmetic, conversion, rounding, currency-identity and data-authority choices. It does not reopen 0020 or [0044's financial perspectives](../conventions/0044-financial-perspectives-and-boundary-conversions.md), or certify caller compliance.

`Money` stores `rust_decimal::Decimal` plus a closed currency enum. Default float construction preserves representable internal precision rather than automatically rounding to minor units; configured ingest and display/output rounding are separate choices. The dependency supplies checked decimal arithmetic and rounding strategies. This supports amount conservation while acknowledging that model multipliers may originate as `f64`.

## Accepted decision: monetary precision, rounding and currency identity

Accepted on **2026-11-07** after whole-record review. Runtime/data reconciliation
and independent qualification remain pending. The stronger arithmetic/conversion
guarantees below are required targets awaiting implementation and qualification;
they are not inferred from a successful dependency `checked_*` call. The observed
source behavior remains separately recorded below.

1. **Keep one bounded Decimal amount plus an explicit currency.** Retain
   `rust_decimal::Decimal` storage, with signed coefficient magnitude at most
   `M = 2^96-1` and scale `0..=28`. This is not arbitrary precision or a fixed
   28-significant-digit guarantee. Negative amounts are valid; their economic
   perspective remains 0044's owner. Preserve sub-minor-unit precision until
   an explicitly selected financial rounding boundary. Normalize signed zero
   consistently; trailing-zero scale is not part of Money's economic identity.
   Equality, hashing, comparison and exact zero use Decimal value plus currency,
   not an `f64` view, display precision or money epsilon. Comparisons/addition/
   subtraction and Money ratios require equal currencies without implicit FX.

2. **Use one exact fixed/scientific text-admission rule.** After outer
   whitespace trimming, accept ASCII decimal syntax with optional leading
   sign, digits with an optional decimal point (at least one digit overall),
   and an optional `e`/`E` signed-integer exponent. Reject internal whitespace,
   separators/underscores, nondecimal bases, NaN/infinity and malformed input.
   The lexical grammar is `[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?`.
   The exponent must fit `i64`, even for zero; use checked digit/scale counters.
   Apply the exponent and remove redundant zeros before testing the final
   numerical value against the coefficient/scale domain. Do not require an
   intermediate mantissa spelling to fit independently. Accept exactly
   representable final values; reject any nonzero digit loss, out-of-range
   magnitude or scale. Lexically valid zero with a valid exponent is zero.
   Retain existing enclosing input-resource controls rather than a new
   financial magnitude cap or arbitrary decimal-specific text-length policy.

3. **Keep exact inputs exact through construction and restoration.**
   Rust Decimal/integer, Python integer/`decimal.Decimal`, decimal text and
   canonical wire amounts must not detour through binary64. Python integers
   are checked against Decimal's actual range, not first coerced to float.
   Route Money JSON/pickle and `DecimalWire` restoration through the same
   exact parser; reject monetary JSON numbers and malformed/unknown Money
   fields. The dependency's permissive string/scientific parser must not
   silently round wire input rejected by direct exact construction. Serialize
   exact canonical fixed-point decimal strings with normalized zero/trailing
   zeros and uppercase alphabetic currency identity. Display/output rounding
   never changes the wire amount or the stored value being serialized.

4. **Define a separate binary64-to-Decimal model boundary.** For a genuine
   finite `f64`, normalize signed zero, obtain Rust's shortest round-trip
   decimal text, then use the exact parser. The selected Decimal is exactly
   that decimal text's value, not the exact binary rational or a recovery of
   the caller's original intention. Thus `0.1 -> 0.1`, while the binary64
   expression `0.1+0.2 -> 0.30000000000000004`. Reject nonfinite inputs and
   any selected text outside the Decimal domain, including nonzero `1e-29`;
   do not trim significant digits or silently underflow to zero. All float
   constructors/scalar/rate adapters share this converter. Its input domain
   applies even when a combined mathematical product might fit: do not
   convert the Money amount to float to evade an unrepresentable factor.
   The [pinned 1.43.0 conversion documentation](https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html#method.from_f64_retain)
   distinguishes the dependency's float conversions; their presence does
   not establish this shortest-text contract.

5. **Give binary64 views and ratios explicit numerical budgets.** Keep
   exact Decimal amount accessors. A requested `amount()` model view must
   round the exact stored value once to binary64 nearest-even, within half
   a binary64 ULP; all valid stored amounts fit the finite normal binary64
   range except exact zero. This does not promise a lossless Decimal/float/
   Decimal round trip or that a rounded view can always re-enter Decimal's
   bounded domain. For same-currency Money/Money division, require a nonzero
   denominator and compute a dimensionless binary64 ratio without a bounded
   Decimal quotient intermediate. Nonzero Decimal ratios fit binary64's
   normal range. Require relative error at most `4*f64::EPSILON` against the
   exact admitted-Decimal ratio; a zero numerator returns normalized zero.
   Correctly rounded operand conversion followed by binary64 division is an
   eligible simple route, subject to independent qualification. These are
   required per-operation model-view targets, not a blanket pricing tolerance
   or proof that the current `Decimal::as_f64` path already meets them.

6. **Make Money addition/subtraction exact or fail.** After the same-currency
   check, return the exact mathematical Decimal sum/difference only when it
   is representable in the storage domain. Otherwise error, even if a
   dependency checked operation could return a rounded finite amount.
   For example, `1e28 + 0.01` must not silently discard the cent. Preserve
   exact cancellation, currency, negation and absolute value; Decimal's
   symmetric magnitude range makes the latter two infallible. This protects
   monetary aggregation without asserting associativity across computations
   that have already introduced approximate model or transform results.

7. **Specify bounded multiplication/division, separately from contract rounding.**
   Provide canonical Decimal-scalar monetary operations; explicit `f64`
   adapters first use point 4 and delegate. Scalar multiplication permits
   signed/zero factors; division requires a nonzero signed divisor. For
   exact admitted-Decimal result `z`, reject `|z| > M`. Otherwise choose
   the greatest scale `s` in `0..=28` whose integer coefficient obtained by
   rounding `z*10^s` once to nearest-even fits magnitude M, and return that
   Decimal value. Exact representable results remain exact. The absolute
   representation error is at most `0.5*10^-s`, measured at this computation
   scale before optional trailing-zero normalization. A nonzero true product/
   quotient rounded to zero is an underflow error; exact zero and exact
   cancellation remain valid. Overflow, division by zero and required
   conversion failures are errors, never panic/saturation/fallback amounts.
   This target applies to scalar transforms and cross-currency multiplication,
   relative to admitted operands; it does not extend through later float views.
   Use the maintained dependency where qualified, and reconcile rescaling paths
   that do not meet the target rather than claiming `checked` means exact.
   Contract owners must still qualify final required-unit rounding, especially
   near ties; a per-operation half-quantum budget is not exact conservation,
   an accumulated-chain error bound or permission for premature financial rounding.

8. **Require scale and mode for financial rounding.** Make the public
   Money rounding operation select an explicit decimal-place scale `0..=28`
   and an explicit mode. Remove None-to-currency-scale/Bankers and invalid-
   precision identity behavior; negative or above-28 scales error across
   constructors/config/wire/hosts. Retain these five mathematical choices:

   | Mode | Definition at the selected decimal quantum |
   | --- | --- |
   | Bankers | Nearest; exact midpoint goes to the even coefficient. |
   | MidpointAwayFromZero | Nearest; exact midpoint increases absolute magnitude. |
   | TowardZero | Truncate the fractional coefficient toward zero. |
   | Floor | Greatest quantized value no greater than the input. |
   | Ceil | Least quantized value no less than the input. |

   Rename ambiguous `AwayFromZero` / `away_from_zero` to
   `MidpointAwayFromZero` / `midpoint_away_from_zero`, without an alias;
   it is not directed-away rounding of every fraction. Remove a universal
   financial mode default. An explicit financial round may intentionally
   produce zero, unlike an unconfigured conversion/arithmetic underflow.
   Financial owners select the required stage and any intermediate rounding;
   this file does not choose one mode/stage for every payment, ledger or report.

9. **Preserve precision when ingest rounding is not configured.** Remove
   the implicit `max(6, currency minor units)` scale. A missing currency
   ingest override means no ingest rounding, even when a configuration object
   is supplied. A present validated scale applies only with an explicitly
   selected rounding mode; incomplete configured rounding errors instead of
   inferring Bankers. Reuse the existing configuration/rounding representation,
   with absence meaning unrequested, rather than a new policy framework.
   Apply configuration only after exact or declared float admission succeeds;
   it cannot rescue an unrepresentable input through hidden pre-rounding.
   Configured output scales are presentation controls, not Money equality,
   exact-zero or JSON quantization rules.

10. **Keep presentation defaults separate and bounded.** Human formatting
    may retain currency-minor-unit precision, nearest-even, currency prefix
    and no grouping as declared presentation defaults. Explicit formatting
    supports scales `0..=28` and an optional one-character grouping separator;
    replace the million-place padding allowance with the representation's
    scale ceiling. Formatting normalizes a displayed zero sign and does not
    mutate the stored amount. Do not feed display-rounded text back into a
    contractual calculation unless that rounding stage was explicitly required.
    Cash-increment rounding such as multiples of 0.05 is a separate owning
    contract, not inferred from ISO minor units or added as a generic facility here.

11. **Make already-resolved FX conversion lossless at its input boundary.**
    The canonical `convert_at_rate` accepts a strictly positive Decimal rate
    in target units per one source unit. An explicitly named float adapter
    uses point 4 before delegation; it must not force contractual rates or
    stored Money through float. Validate rate/domain before zero-amount or
    same-currency shortcuts. For equal source/target currency require rate
    exactly one, without a tolerance; reject a nonunit rate rather than
    silently ignoring it. For distinct currencies, use point 7's checked
    Decimal multiplication and change the currency tag without target-minor-
    unit rounding. Currency-tagged source precision, positive/negative amounts
    and zero remain supported. Market-rate authority, identity-provider lookup,
    inversion/triangulation and redenomination retain [013](013-fx-resolution-and-shocks.md)
    and their product owners; this is an explicitly supplied-rate operation.

12. **Use alphabetic currency identity and separate numeric metadata.**
    Retain a closed supported currency catalogue with distinct alphabetic
    code identities, including admitted historical balances. Decouple enum
    discriminants/equality/hash/wire identity from ISO numeric codes. The
    [official SIX amendment 176](https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/amendments/dl-currency-iso-amendment-176.pdf)
    assigns XCG the same numeric 532 as historical ANG; they must remain
    different currency identities. `numeric()` returns metadata rather than
    an enum cast. Retain numeric-to-currency lookup only for a unique match
    in the frozen supported catalogue; zero or multiple matches error and
    name the missing/ambiguous code. Never pick the current currency from
    today's date or reinterpret stored historical amounts. Text parsing
    retains ASCII-case normalization without whitespace trimming; canonical
    code output is uppercase and unknown alphabetic codes error.

13. **Scope currency metadata to its actual authority and purpose.** The
    audited 159-row CSV is a maintained selection, not a complete current-
    active ISO list. Preserve the stated supported deliverable-currency/
    selected-unit scope and historical identities; excluded precious-metal,
    no-currency/test or other non-admitted codes remain explicit exclusions.
    Pin source editions/amendments from the
    [SIX current/historical lists](https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html),
    extraction choices and per-code metadata. `decimals()` is an exhaustive
    maintained minor-unit exponent, not a generic missing-value fallback of
    two, current trading/settlement eligibility, a date-aware historical
    rounding rule, cash denomination or FX pip size. Products needing those
    meanings select their actual source/date/contract. Do not add an activity,
    legal-status or automatic redenomination engine to Money.

14. **Maintain currency data with reviewed, deterministic updates.**
    Validate unique alphabetic identities and consistent imported attributes;
    allow shared numeric metadata across distinct current/historical codes.
    Reject malformed/conflicting source data rather than deducing default
    scales or dropping historical rows. Pin edition, retrieval/hash,
    selection/exclusions and amendments; reviewed offline refreshes update
    maintained CSV, generated enum/metadata, affected schemas and fixtures
    together. Reconcile known omitted XCG and ZWG entries against their
    official amendments while preserving historical ANG/ZWL identities;
    [SIX amendment 177](https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/amendments/dl-currency-iso-amendment-177.pdf)
    separately defines ZWG. A generator-consistency pass is not ISO freshness
    or historical metadata qualification. Do not fetch live data during builds
    or restore stored values using a silently newer currency edition.

15. **Reconcile the canonical paths and numerical evidence together.**
    Implement the shared parser/wire/float conversion once in Rust and update
    Money, exact/float scalar and rate paths, configuration, currency identity,
    Python/WASM input/result conversion, JSON/pickle, schemas, parity and
    examples as a working slice. Reject Boolean/nonfinite numeric inputs and
    invalid scales/modes; Python integers remain exact, while JavaScript
    numbers represent the binary64 value actually received. An already-rounded
    unsafe JS integer cannot recover its intended digits; exact text is the
    existing route. Remove obsolete mode names/defaults/casts and permissive
    parsing paths without compatibility branches. Qualify the required
    per-operation targets against independent exact-integer/rational and
    rounding references before describing them as enforced. Keep contract-
    specific budgets/stages with their existing owners; add no arbitrary-
    precision Money type or parallel rounding/currency registry. Runtime,
    currency-data refresh and independent production qualification remain
    with the [existing follow-ups](../FOLLOW_UPS.md#core-conventions).

## Boundary examples for the accepted package

These independent decimal/rational examples express the proposal. They are
not executed production Rust/host tests, dependency-accuracy certification or
a refreshed ISO dataset.

| Request | Required result |
| --- | --- |
| Exact USD `0.001`, with no ingest override even if config is supplied | Preserve `0.001`; no automatic cent or six-place rounding. |
| Exact integer `9007199254740993` from Python/Rust/text | Preserve all digits; do not first convert through binary64. |
| Exact text `2000e-31` / `100000000000000000000000000000e-1` | `2e-28` / `1e28`, after final-value normalization. |
| Exact `2.5e-28`, including JSON/pickle restoration | Error: it cannot be represented exactly at scale at most 28; no silent `3e-28`. |
| `1_000`, embedded whitespace, NaN/infinity, malformed or out-of-i64 exponent | Input error under the single grammar. |
| Genuine binary64 `0.1` / expression `0.1+0.2` | Decimal `0.1` / `0.30000000000000004`. |
| Genuine binary64 `1e-29`; either signed zero | Conversion error for the nonzero unrepresentable selected value; canonical zero for both zero signs. |
| Same-currency Money `1e28 + 0.01` | Exactness error, not a rounded `1e28` balance. |
| Decimal Money `1 / 3` as a scalar transform | Nearest-even 28-place Decimal approximation; error at most `0.5e-28` against the exact admitted ratio. |
| Decimal Money `1e-28 * 0.1` | Underflow error; an explicit scale-2 financial round of admitted `1e-28` may separately return zero. |
| Exact result magnitude beyond `2^96-1`, even if rounding could bring it back | Overflow error, without boundary clipping. |
| Exact `2.5` / `-2.5` rounded to scale zero | Bankers `2/-2`; MidpointAway `3/-3`; TowardZero `2/-2`; Floor `2/-3`; Ceil `3/-2`. |
| MidpointAway at `1.1` / `-1.1`, scale zero | `1/-1`; the mode does not direct every fraction away from zero. |
| Missing mode for requested financial rounding, or scale -1/29 | Input/policy error; no default mode or invalid-precision identity. |
| Same-currency Money ratio `1e28 / 1e-28` | Finite model result near `1e56` within the selected relative budget, without Decimal-quotient overflow. |
| Currency mismatch, zero ratio/scalar divisor or invalid rate with zero Money | Error; zero amount does not bypass required currency/rate/divisor validation. |
| Same-currency resolved rate `1` / positive nonunit rate | Exact unchanged Money / input error. |
| Decimal USD `1.2345` converted at Decimal EUR-per-USD `0.8` | EUR `0.9876`, without automatic target-cent rounding. |
| Exact equal amounts with trailing-zero scales, or zero signs | Equal/hash-consistent for the same currency; canonical wire strings normalize the representation. |
| Historical ANG and XCG after the reviewed data update | Distinct alphabetic currencies sharing numeric 532; numeric-only lookup errors as ambiguous. |
| Unique numeric 840 / unknown numeric or alphabetic code | USD / explicit lookup error. Membership alone does not prove current eligibility. |

## Observed algorithm and selection

`parse_decimal` trims and parses exact decimal text. Scientific notation parses the mantissa exactly, subtracts exponent from scale, removes trailing zero coefficients only when needed to fit maximum scale, and multiplies the coefficient when exponent implies negative scale. It rejects inexact/unrepresentable scale or magnitude; a zero coefficient yields zero even with an extreme representable exponent.

`f64_to_decimal` rejects NaN/infinities and uses Decimal's checked conversion; finite values outside its magnitude domain error. `Money::new` normalizes signed zero and performs that conversion without minor-unit rounding. `from_decimal` is exact; integer tuple constructors preserve integral amounts. Configured float or decimal construction rounds to `cfg.ingest_scale(currency)` using the selected mode; the inspected config default is max(6,currency minor units), with validated per-currency overrides.

Add/subtract/compare and Money/Money division require identical currencies. Add/subtract/multiply/divide use checked Decimal operations; float multipliers/divisors are converted to Decimal first. Money/Money division returns `f64` after a decimal ratio; `amount()` similarly crosses to `f64`. Negation and absolute value retain currency. No operation implicitly performs FX conversion. `convert_at_rate` validates positive finite rate first, returns unchanged Money for same currency, otherwise multiplies Decimal amount by converted rate and changes currency without target-minor-unit rounding.

`round(None)` uses currency decimals and bankers rounding; negative digits error. Formatting defaults to currency decimals, bankers rounding, a currency prefix and no thousands separator; custom formatting has an optional one-character separator and precision cap 1,000,000. Rounding strategies are nearest-even; midpoint-away-from-zero (not all values away); truncate toward zero; floor; ceil. The internal `round_decimal` returns unchanged for negative precision, but the public Money round rejects it.

## Observed currency metadata and conventions

The generated table contains 159 currency codes and numeric IDs. Sixteen zero-decimal currencies are BIF/CLP/DJF/GNF/ISK/JPY/KMF/KRW/PYG/RWF/UGX/VND/VUV/XAF/XOF/XPF; BHD/IQD/JOD/KWD/LYD/OMR/TND have 3 decimals; CLF has 4; other supported currencies have 2. Code parsing ignores ASCII case but not whitespace. Unknown string/numeric codes error. The CSV includes historical currencies; this is a maintained static domain, not a current-active-currency filter or a cash-denomination rounding table.

## Observed limitations and alternatives

Decimal arithmetic can overflow and can round products/divisions to the dependency's finite precision; it is not arbitrary precision. Converting from an already-rounded `f64` cannot recover lost decimal digits. Display rounding does not mutate the stored amount. Same-currency conversion accepts a positive nonunit rate but returns the original amount. Exact-text input and configured rounding are supported alternatives; arbitrary precision or market cash-increment rounding are not implemented.

Pinned `rust_decimal` 1.43.0 also permits representation rounding during
add/sub alignment and nonzero product/conversion underflow; checked results
alone do not prove the accepted exact-sum/transform budgets. `f64_to_decimal`
uses `Decimal::try_from(f64)`, not the shortest-text algorithm claimed in some
Money documentation. The [Decimal wire adapter](../../../../finstack-quant/core/src/wire/decimal_wire.rs)
delegates string parsing to the dependency rather than the exact core parser;
Money/Money division first constructs a bounded Decimal quotient. Python
numeric construction converts integers to float. These are reconciliation
boundaries, not executed numerical-defect reproductions.

The generated currency enum uses numeric discriminants and the renderer rejects
duplicate numeric IDs. This cannot represent both ANG/XCG under the accepted
alphabetic-identity contract. `decimals()` actually uses an exhaustive generated
match; the prose claiming a missing-code two-place fallback is stale.

## Evidence

[Money](../../../../finstack-quant/core/src/money/types.rs), [rounding](../../../../finstack-quant/core/src/money/rounding.rs), [decimal parsing](../../../../finstack-quant/core/src/decimal.rs), [currency loader](../../../../finstack-quant/core/src/currency.rs), [currency CSV](../../../../finstack-quant/core/data/iso_4217.csv), [config defaults](../../../../finstack-quant/core/src/config.rs). Inline tests inspect exact decimal round trips, preserved precision, checked overflows, nonfinite rejection, hash/equality across scales and explicit rounding modes. [Money integration tests](../../../../finstack-quant/core/tests/money/rounding.rs) are arithmetic examples/invariants. Generated-reference `--check` passed; it verifies source consistency, not currency-status freshness. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) provides numerical background; central references do not establish the exact ISO snapshot or Decimal release rounding guarantees.

Implicit currency quantization in `Money::new` and acceptance of nonfinite monetary inputs are not open alternatives. No blanket precision certification follows from Decimal storage.

The 2026-11-07 review inspected locked `rust_decimal` 1.43.0 source/documentation
and SIX amendments 176/177. Independent exact-integer/rational, rounding,
binary64-reference and maintained-catalogue checks passed. Those references do
not establish that the current Rust backend meets the accepted numerical targets;
no runtime changes, production tests or maintained currency-data refresh were performed.

## Follow-up tracking

Implementation/data reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
