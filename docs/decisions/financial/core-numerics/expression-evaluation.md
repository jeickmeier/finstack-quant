# CORE-NUMERICS-22: Checked scalar expressions, missing values and exact decimal quantization

- Decision status: **Accepted — complete scalar-expression package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Bounded rational/truth-table references, pinned dependency inspection and read-only compiler/kernel/caller reviews were completed on 2026-11-08; these do not certify production execution or financial formulas.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`expr/{ast,context,dag,eval,eval_functions}.rs` and scalar helpers in `math/mod.rs`: elementwise expressions over named f64 columns, including literals, arithmetic, comparison/logic, conditionals and twelve scalar functions. Period/window/cumulative/aggregate and capital-structure functions share AST vocabulary but require the statements evaluator and are rejected here.

## Accepted decision: checked scalar expressions, missing values and decimal quantization

Accepted on **2026-11-08** after whole-record review. Runtime,
consumer/host reconciliation and independent language/numerical/financial
qualification remain pending; observed behavior is retained below.

1. **Keep one deliberately scalar language boundary.** Core CompiledExpr
   supports elementwise columns, finite numeric literals, existing operators,
   conditionals and the twelve scalar functions below. Period/window/cumulative/
   aggregate/capital-structure functions remain statements-owned and are rejected
   here, including unreachable branches. Shared AST vocabulary and wire labels
   do not imply shared evaluator semantics, automatic routing or financial units.
   No new parser, evaluator registry, DSL backend or host binding is selected.

2. **Retain immutable flat DAG compilation and structural identity only.** Keep
   `CompiledExpr::new(ast)` as the sole checked owned-AST compiler, iterative
   left-to-right postorder planning and dense private dependency IDs. Deduplicate
   only equal operations plus ordered dependency IDs and finite literal bit
   patterns, preserving signed-zero distinctions. No commutative sorting,
   reassociation, constant folding, algebraic simplification or fused node math.
   Keep iterative owned rejection cleanup; compiled plans own no recursive AST,
   are immutable/nonserialized and cache no input values/results across calls.

3. **Validate complete syntax at compilation.** Reject nonfinite literals,
   unsupported nodes and every missing/excess scalar argument before execution.
   Unary functions require one argument; pow two; clamp three; round one or two.
   Validate both conditional branches and all static controls, even for empty
   evaluation. General Expr remains the shared data AST; compiler admission does
   not invent a JSON NaN literal or change the statements AST vocabulary.

4. **Bound admitted compilation without overclaiming raw-AST safety.** Retain
   root depth 0 with admitted depths 0..511, rejecting depth 512. Select an
   inclusive 100,000 visited AST-occurrence budget before deduplication, so huge
   repeated trees cannot evade it. Check edge/ID/stack/cache/layout arithmetic
   and use recoverable fallible reservations. Cleanup of a rejected owned tree
   still scales with its remaining size. Iterative compiler traversal does not
   make public Expr clone/debug/equality/serde/drop universally stack-safe;
   actual source ingress must be bounded before recursive cloning. No new public
   AST/state/budget framework or configurable resource hierarchy.

5. **Make row count and column alignment explicit.** Select
   `eval(ctx,cols,num_rows)->Result<Vec<f64>>`, deleting the inferred-row overload.
   SimpleContext names remain exact case-sensitive opaque strings in insertion
   order, including existing empty/whitespace/Unicode names; exact duplicates
   error, with no trim/case folding. Rename `index_of` to `get_index` without
   an alias. Require context cardinality equal to supplied column count and
   every supplied column length exactly num_rows. Only literals broadcast.
   No first-column length, padding, truncation or date/index alignment inference.
   Empty context/data with a literal and N rows returns N literal values.

6. **Distinguish missing data from invalid infinity input.** Runtime column
   NaN is the sole permitted missing sentinel. All supplied columns, including
   unused ones, must otherwise contain finite numbers; either infinity is an
   admission error with column/row context. Literals must be finite. Result
   values are finite or permitted missing NaN, never infinity. is_missing tests
   NaN only. No infinity-to-missing conversion, successful invalid calculation
   disguised as missing, NaN-payload wire promise or generic filling policy.

7. **Choose missing-first numeric operations and explicit finite failures.**
   For ordinary arithmetic/functions, any missing operand produces missing
   before that operation's numeric-domain checks. With complete finite operands,
   invalid domains and newly nonfinite results are whole-call computation errors
   with row/node context; do not synthesize NaN or a repaired value. Division and
   remainder by either signed zero error. `%` is truncating remainder, not
   Euclidean modulo. Retain ordinary binary64 subnormal/rounded-underflow results,
   including signed zero; they do not prove economic zero or model adequacy.
   Evaluate each ordered node as written; no exact-real rescue across nodes.

8. **Make comparisons and logic consistent with missingness.** Finite numeric
   comparisons are exact, with +0 and -0 equal and no tolerance. Any missing
   comparison operand produces missing, including equality/inequality. Finite
   zero is false and any finite nonzero is true. Use strong three-valued logic:
   false AND missing is false; true OR missing is true; otherwise an unknown
   operand remains unknown unless the other operand decides the result. NOT
   missing is missing. Known logical/comparison outputs are numeric 0 or 1.
   This adopts the truth tables, not SQL syntax, evaluation order or compatibility.

9. **Retain eager evaluation and disclose its numerical consequence.** Evaluate
   every admitted DAG dependency once per call in deterministic topological/node
   order, with rows in input order; AND/OR and conditionals do not short-circuit
   numeric work. A finite conditional selects then iff nonzero, otherwise else;
   a missing condition produces missing. An invalid finite calculation in an
   unselected branch still fails the call. Thus `if(x>0,ln(x),0)` is NOT a guard
   for x<=0 under this profile; `false AND (1/0)` also errors. Static binding/
   shape/control errors are unconditional. No row-mask/lazy executor is added.

10. **Pin all scalar function domains and zero details.** Missing-first handling
    precedes ordinary function identities: pow(missing,0) remains missing.
    ln/log10 require positive inputs; sqrt requires nonnegative input and retains
    the ordinary signed-zero result. Pow permits negative bases only for exactly
    integral finite exponents, selects numeric `0^0=1`, and rejects zero raised
    to a negative exponent. Exp/pow overflow and finite-input NaN error; ordinary
    underflow follows decision 7. abs(-0)=+0; sign of either zero is +0, otherwise
    sign returns -1/+1. Floor/ceil keep their mathematical integer direction.
    Clamp uses finite ordered inclusive bounds and preserves an in-range input,
    including its zero sign. is_missing is the sole missing-inspecting builtin.

11. **Make round digits a static language control.** round(x) retains default
    digits 0. A supplied second argument must be a direct finite exactly integral
    i32 literal, including allowed negative values; reject constant columns,
    computed constant subexpressions and tolerance-based integer coercion.
    Validate once at compile time, before zero-row evaluation. No per-row digits,
    bit-identical column scan or malformed-control-to-NaN result. This is a
    generic numeric-language convention, not an inferred currency scale.

12. **Define rounding on the exact represented binary64 input.** Keep one
    `round_half_away(x,digits)->Result<f64>` helper, finite x only. Let q=10^-digits
    be an exact decimal quantum, choose the nearest integer to exact x/q with
    ties away from zero, multiply by q exactly, then restore ONCE to nearest
    binary64 with ties to even. Preserve the input sign for zero results.
    Error iff the final nearest-binary64 restoration is nonfinite: an exact
    decimal target slightly above MAX may still restore to finite MAX. This does not reinterpret
    the user's original decimal text: actual binary64 2.675 rounds to 2.67 at
    two digits. No full Excel compatibility, Money/Decimal or exact-DAG claim.

13. **Reuse bounded existing rational primitives, not float scaling.** Core
    already depends on num-bigint/num-rational/num-traits. Use exact from_float,
    positive decimal powers, the existing rational half-away integer round and
    checked rational-to-binary64 restoration, explicitly rejecting infinity even
    when the conversion returns Some. Full i32 digits are bounded: digits>=324
    return finite x unchanged; digits<=-309 return signed zero; only -308..323
    need rational work. Apply tails before negating/powering extreme i32 controls.
    No overflowing/underflowing powi scale, return-x-on-overflow fallback, Decimal
    conversion, string conversion, new dependency or new/custom arbitrary-precision engine.

14. **Replace the NaN-named clamp helper with a checked scalar.** Select
    `clamp(x,lo,hi)->Result<f64>` over finite arguments and lo<=hi, removing
    clamp_or_nan and its alias path. Use comparisons/selection so an in-range
    zero keeps its sign. Standalone bad arguments are input errors. The core DSL
    first applies its missing rule; complete finite inverted bounds are an
    expression computation failure. For example clamp(missing,2,1) is missing,
    but clamp(0,2,1) errors. Prior errors computing dependencies still propagate.

15. **Preflight zero rows/resources and publish results atomically.** Resolve
    all referenced names, exact counts/shapes and source-value admission before
    output/arena work, even when num_rows=0 or a branch would be unselected.
    Zero rows then return an empty vector without executing numeric nodes or
    secretly constant-folding them. Check node-count times rows, offsets, bytes,
    layout and recoverable allocation failures; preserve input columns/context.
    On any evaluation failure discard private partial results and return no
    partial output. Keep one arena segment per unique DAG node and no cross-call
    result cache. No configurable memory manager, scalar-host API or diagnostics
    hierarchy; admission does not promise huge valid calls are cheap.

16. **Reconcile actual scalar and shared-helper consumers.** Composite quantity
    expressions use explicit one-row evaluation, exact shapes and propagated
    failures; keep their own finite/nontrivial quantity/state gates. Bound their
    existing expression ingress before recursive clones, without claiming the
    current compile guard already does so. Propagate checked round/clamp through
    statements dispatch, bond-future conversion-factor rounding and waterfall
    payment rounding. Propagate helper failures; only the explicitly permitted
    missing-operand handling below may return NaN. No error-to-NaN adapter,
    infinity wrapper or silently changed financial rounding point/unit.

17. **Preserve shared AST/host ownership without conflating evaluators.** Core
    CompiledExpr/SimpleContext remain Rust-only private-state APIs with no serde;
    remove SimpleContext's stale deserialization claim. Keep existing strict
    shared AST wire shapes/labels. Statements compile to that AST and use a
    separate period-aware evaluator. Adopt a narrow shared round/clamp admission
    rule there: reject any evaluated infinite operand, then propagate permitted
    NaN operands as missing, otherwise use the checked finite helper and propagate
    failures. Thus clamp(NaN,2,1) remains missing, while finite inverted bounds
    error; clamp(NaN,infinity,1) errors. Round's existing statements-owned
    period/control evaluation still supplies its i32 digits; this package does
    not extend core's literal-only rule or alter every statements integer control.
    Other statements lazy/comparison/missing policies remain separately reviewed,
    not automatically adopted here or forced onto core. Reconcile shared-helper
    signature/error and AST documentation at both owners. Existing host parse_and_compile docs
    should describe the shared AST accurately, not claim scalar-DAG execution.
    No new direct binding, persisted plan, wire carrier, routing or backend.

18. **Keep financial interpretation and qualification explicit.** Core columns
    are already aligned unitless f64 data; no currency scale, day count,
    annualization, dependency availability, dimensional analysis or financial
    formula correctness is inferred. Accepted Money rounding remains separate.
    Owners retain statements language/period policies, composite quantity rules,
    contractual bond/waterfall rounding and price/risk acceptance. Truth tables,
    exact rational examples and inspected dependency code support definitions;
    production execution/accuracy/resource and financial qualification remain
    open. This review changes no runtime, tests, fixtures, dependencies,
    generators, schemas or hosts.

## Selected scalar arities

| Scalar functions | Required arguments |
| --- | --- |
| abs, sign, floor, ceil, ln, exp, log10, sqrt, is_missing | Exactly 1 |
| pow | Exactly 2 |
| clamp | Exactly 3 |
| round | 1 or 2; optional second argument is a direct integral i32 literal |

## Missing truth tables for the accepted package

M denotes a permitted runtime NaN, not a nonfinite literal or invalid computation.

| A | B | A AND B | A OR B |
| --- | --- | --- | --- |
| false | false | false | false |
| false | true | false | true |
| false | M | false | M |
| true | true | true | true |
| true | M | M | true |
| M | M | M | M |

AND/OR have the same value for exchanged operands. Eager numeric errors are
still evaluated; this table does not select short-circuit execution. NOT M is M.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Literal-only expression, no columns, num_rows=3 | Three literal values. |
| Unequal column lengths, context count mismatch or missing referenced name | Whole-call input/binding error, including zero rows. |
| Infinity in an unused supplied column / nonfinite AST literal | Admission / compile error. |
| M==0, M!=0, NOT M, pow(M,0) | M in every case. |
| is_missing(M) / is_missing(finite) | 1 / 0; invalid computations still error before inspection. |
| M/0 / finite 1/0 | M / whole-call computation error. |
| if(x>0,ln(x),0), x=0 | Error despite selected else branch: eager profile. |
| round(x, column_digits), extra unary args or malformed literal digits | Compile error, independent of row count/data constancy. |
| Exact binary .125 / -.125, digits=2 | Binary64 restorations of .13 / -.13. |
| Actual binary64 2.675, digits=2 | 2.67; no decimal-string reinterpretation or false scaled tie. |
| x=7*2^50+1, digits=1 | x unchanged, not x+1 from finite scaling drift. |
| x=eta=2^-1074, digits=323 / 324 | +0 / eta. Negative input preserves negative zero where applicable. |
| Finite x, digits<=-309 | Signed zero; no scale-underflow NaN. |
| MAX, digits=-308 | Computation/range error because exact rounded target is 2e308. |
| MAX, digits=-1 | Exact decimal target MAX+2 restores to finite MAX; no premature exact-target range refusal. |
| clamp(-0,-1,1) / sign(-0) | -0 / +0. |
| exp(-1000) / exp(1000), demanded with finite input | Ordinary rounded underflow 0 / whole-call range error. |
| Scalar AST uses a period/window/CS function in an unselected branch | Compile error; no statements routing. |

## Observed choice and rationale

Compile an iterative postorder DAG, deduplicate structurally identical nodes, and evaluate each once per call. This avoids repeated subtree work and recursive compile/drop on rejected input. The DAG key preserves operand order and literal f64 bit patterns; it performs no algebraic simplification.

## Observed algorithm

1. Visit the AST left-to-right using explicit Visit/Finish stacks. Reject depth≥512 and unsupported nodes before descending. A rejected owned tree is drained iteratively. Deduplicate by operation plus dependency node IDs; literal keys use bits, distinguishing signed zeros/NaN payloads.
2. Context construction rejects duplicate names and records exact case-sensitive insertion positions. On evaluation set row count to first supplied column length (zero if none), allocate one row-length arena segment per DAG node, and evaluate all dependencies in topological order.
3. Named-column lookup errors if absent from context or data. Short columns are NaN-padded; long columns truncate. Literals broadcast. Arithmetic uses f64, except division by either signed zero returns NaN. Comparisons use exact IEEE comparisons and return 0/1. Logic regards every nonzero as true, so NaN is true for `and`/`or` and false for `not`.
4. Both conditional branches are evaluated before selection. A NaN condition produces NaN; otherwise nonzero chooses then. Thus missing columns/errors in an unselected branch still fail the full evaluation.
5. Scalar abs/sign/pow/floor/ceil/log/exp/sqrt use elementwise kernels. `is_missing` is 1 for any nonfinite value. Clamp propagates any NaN and inverted bounds as NaN. Missing function arguments generate NaN output; no general arity check is performed at compilation.
6. Round defaults digits to zero. A supplied digits series must be bit-identical throughout or errors; a nonfinite, nonintegral or out-of-i32-range constant generates NaN. `round_half_away` scales by 10^digits, rounds ties away from zero, unscales, and returns original x if scaling overflows. Nonfinite x passes through.

## Observed selection, controls and conventions

The scalar compiler is explicitly selected; it does not route unsupported nodes automatically. No monetary scale, percentage conversion, date alignment or unit checking occurs. Inputs are already-aligned columns. Function labels/serde names are centralized in the AST macro.

## Observed limitations, failure and fallback

Comparison/logical missing-data semantics differ from conditional and arithmetic propagation. Empty columns produce empty result even for literals. Extreme negative digits can underflow the decimal scale to zero and produce NaN after division. Arena element multiplication is checked, but there is no configurable memory budget. Eager branch evaluation is material for error behavior.

## Observed alternatives and tradeoffs

The statements evaluator supplies separate period-aware behavior. Lazy conditional evaluation, three-valued logic, strict column lengths and dimensional analysis are not supported options in this compiler. DAG reuse reduces computation at the cost of retaining one full result column for each unique node.

## Evidence

- [AST](../../../../finstack-quant/core/src/expr/ast.rs),
  [context](../../../../finstack-quant/core/src/expr/context.rs),
  [DAG](../../../../finstack-quant/core/src/expr/dag.rs),
  [evaluator](../../../../finstack-quant/core/src/expr/eval.rs),
  [function kernels](../../../../finstack-quant/core/src/expr/eval_functions.rs)
  and [shared scalar helpers](../../../../finstack-quant/core/src/math/mod.rs)
  establish observed scope/behavior. Located dedup/depth, tail-padding,
  division-by-zero, round-control and function cases were inspected/located,
  **not executed**. Old IEEE/NaN/padding assertions do not qualify this accepted contract.
- [Expression tests](../../../../finstack-quant/core/tests/expr/eval.rs) and
  [function tests](../../../../finstack-quant/core/tests/expr/functions.rs) are
  language-contract evidence, not independent financial formula qualification.
  Native Expr recursive clone/drop/serde are outside the iterative compiler's
  present safety claim; actual composite ingress currently clones before compile.
- [Three-valued comparison/logic references](../../../REFERENCES.md#expression-three-valued-logic)
  corroborate only the selected truth tables.
  [Pinned exact-rational rounding primitives](../../../REFERENCES.md#binary64-decimal-quantization)
  identify from_float, half-away integer round and ties-even restoration in
  num-rational 0.4.2. Existing [core dependencies](../../../../finstack-quant/core/Cargo.toml)
  and [lockfile](../../../../Cargo.lock) already supply these libraries; both
  cached/published source were inspected, not executed as a rounding oracle.
- Independent **2026-11-08** exact-binary-rational/logic references distinguish
  actual binary input from decimal text, verify tie/signed-zero/extreme-digit
  targets, finite scaling drift and final rounded overflow, and cover all nine
  input pairs for each three-valued boolean operation. These bounded references
  do not run production compilation/evaluation or certify dependency/arena/host
  behavior. The digit-tail identities follow from the finite binary64 range and
  minimum spacing, not sampling alone.
- Actual reconciliation includes composite
  [spec](../../../../finstack-quant/valuations/src/instruments/composite/spec.rs)
  and [instrument](../../../../finstack-quant/valuations/src/instruments/composite/instrument.rs),
  [statements dispatch](../../../../finstack-quant/statements/src/evaluator/formula_dispatch.rs)
  and [separate evaluator](../../../../finstack-quant/statements/src/evaluator/formula.rs),
  [bond-future rounding](../../../../finstack-quant/valuations/fixed-income/src/bond_future/pricer.rs)
  and [waterfall rounding](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/waterfall.rs).
  Existing [Python statements DSL](../../../../finstack-quant-py/src/bindings/statements/dsl.rs)
  and [WASM statements API](../../../../finstack-quant-wasm/src/api/statements/mod.rs)
  expose shared-AST compilation, not this core scalar engine. [ADR 0009](../conventions/0009-missing-and-invalid-data.md)
  permits operation-specific policies; [representation/rounding authority](../conventions/0020-numerical-representation-and-rounding.md)
  keeps financial Money quantization separate from generic binary-input rounding.

This record chooses language behavior; it does not validate a financial equation,
all transcendental accuracy, every raw-AST ingress or current statements parity.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
