# MODELS-ANALYTIC-22: Price-equivalent ATM volatility convention conversion

**Decision status:** Accepted (complete package adopted 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and actual caller paths inspected. Independent scalar and high-precision examples support the proposed distinctions. Production and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked price-equivalent ATM volatility conversion

All 23 decisions were accepted on **2026-11-10**. Implementation reconciliation
and independent complete-path qualification remain open.

1. **Convert the original at-the-forward option.** Set the same
   original K=F on both sides and use one positive expiry on the
   same model clock. Match the undiscounted unit forward premium.
   Calls and puts have the same price at this strike. A common
   positive discount or annuity cancels. The conversion returns
   target annual sigma in its requested convention.

2. **Require explicit supported conventions.** Use the existing
   Normal, Lognormal and ShiftedLognormal tags for source and target.
   Normal sigma has the original forward's units per square root
   model year. Relative Black sigma is decimal per square root
   model year. A shift is an explicit signed value in forward
   units. Preserve a shifted tag with literal zero displacement.

3. **Validate the complete request first.** Require finite F,
   finite T>0, finite nonnegative source sigma and finite shifts.
   Require the explicit validated acceptance controls below.
   A zero expiry premium does not identify volatility, so zero
   expiry errors for every request, including identity. Negative
   or nonfinite expiry errors before any shortcut.

4. **Apply the checked source and target forward domains.** Normal
   permits finite signed F. For relative Black use A=F in the
   unshifted law or mathematical A=F+s in the shifted law.
   Require A in (0, binary64 MAX] under record 02. Preserve the
   exact represented-input sum through all comparisons and
   pricing. Both domains apply before zero or identity returns.

5. **Keep only exact zero and law identities.** A literal zero
   source sigma returns canonical zero after full admission. A
   valid identical law returns the source sigma unchanged. Two
   Black-family laws are identical here when their effective A
   values agree exactly, including unshifted and explicitly
   shifted zero. The target tag remains explicit. Remove the
   near-equal-shift shortcut.

6. **Use the analytic ATM price laws.** With `h = sqrt(T)`,
   Normal has `P = sigma_N h / sqrt(2 pi)`.
   Black and shifted Black have `P = A erf(u)` with
   `u = sigma_B h/(2 sqrt(2))`. Keep the original unit-premium
   meaning of record 02. The normal law is linear in sigma,
   and a finite positive Black sigma gives 0<P<A.

7. **Keep the premium in scale until needed.** Preserve the
   complete price or complementary cap gap using checked scaled
   products and signed differences. Intermediate premium and
   standard deviation need not separately fit binary64 when
   the final volatility and its acceptance are resolvable.
   The requested output is volatility. Record 02 continues to
   define the separate scalar premium API.

8. **Handle small Black price arguments in scale.** Use
   `P = A sigma_B h/sqrt(2 pi) × R(u)`, where
   `R(u) = sqrt(pi) erf(u)/(2u)` tends to one. For u≤2⁻²⁷
   the factor-one approximation omits at most u²/3 in the
   normalized factor, by integrating the bound on exp(−x²).
   Use checked erf and complete products elsewhere. Preserve
   the positive modeled price through small-argument scaling.

9. **Invert a Normal target directly.** Compute
   `sigma_N = sqrt(2 pi) P / h` with complete scaling.
   Use the actual source premium. The small-variance linear
   conversion between Normal and Black is a limit of the
   ATM law.

10. **Require the strict cap for a Black target.** Let A_t be
    the target effective level. For a positive source quote,
    require a resolved positive gap `D = A_t - P`.
    A resolved P≥A_t has no finite target inverse. Use the
    mathematical cap, with no bound slack from premium
    tolerances. An unresolved sign is a precision error.

11. **Preserve the cap gap from a Black source.** Compute
    `D = (A_t - A_s) + A_s erfc(u_s)`, using the exact
    shift difference for the first term at fixed original F
    and the scaled or logarithmic complementary tail for
    the second. For the unshifted law use s=0 in the
    difference. Retain cancellation and sign information.
    A rounded source premium at its apparent cap cannot
    decide this comparison.

12. **Preserve the cap gap from a Normal source.** Compare the
    complete scaled source premium with A_t. Near the cap,
    use a stable signed difference, for example from
    `2 pi A_t² - sigma_N² T` with a checked enclosure for pi
    when needed to resolve the sign, followed by factorization.
    A rounded division equaling
    the cap cannot decide the original sign.

13. **Use the smaller probability for Black inversion.** If
    P≤D, use `u_t = erf_inv(P/A_t)`. If P>D, use
    `u_t = erfc_inv(D/A_t)`. Restore
    `sigma_t = 2 sqrt(2) u_t/h` in scale. Use the existing
    pinned statrs inverse primitives through a checked
    private model path. Remove default Brent, guesses,
    volatility floors and retry pricing.

14. **Preserve a small lower-half result.** For r=P/A_t≤2⁻²⁷,
    use the complete leading expression
    `sqrt(2 pi) P/(A_t h)`. The normalized inverse factor
    differs from one by at most exp(r²)−1 in this range.
    Apply the same final premium replay as for the direct
    inverse. This permits a positive output even when the
    intermediate r would round to zero.

15. **Refuse an unresolved inverse range.** In the upper
    branch require a usable strictly positive represented
    complementary probability. Propagate range or precision
    failure when the probability is unavailable, its gap
    is unresolved, the inverse fails or the final positive
    sigma cannot be represented. Never nudge an endpoint or
    replace positive sigma by zero. This branch does not
    extend to an unrepresentable upper-tail probability, even
    when the mathematical target volatility would be finite.

16. **Require two explicit premium controls.** Require a
    finite nonnegative absolute tolerance in original unit
    premium and a finite relative tolerance strictly between
    zero and one. The relative reference is P for a Normal
    target and min(P,D) for a Black target. Controls are
    required even for exact identity and zero requests.

17. **Replay the actual represented target sigma.** Price the
    returned candidate under the selected target convention
    at the same original F and T with the checked ATM law.
    If the smaller target component is the cap gap, compare
    the target's complementary premium to the required gap.
    Otherwise compare positive premiums. Keep the same
    reference component throughout the comparison.

18. **Require both replay gates.** Require the absolute
    difference to be at most the absolute premium tolerance.
    Comparing the cap gaps gives the same absolute premium
    difference. Also require relative error in the selected
    positive reference component within the declared
    tolerance. A scaled log-ratio evaluation can compare
    log(actual/reference) to the inclusive log1p bounds.
    The gates use checked evaluated values. Resolve any
    approximation uncertainty that could change either verdict,
    or report a precision error. A missed gate also errors.

19. **Keep the result and failure contract simple.** Return one
    checked annual sigma. Use input/domain errors for invalid
    controls, quote domains or an impossible finite Black
    target. Propagate a computation or precision error for
    numerical range, inversion or replay failure. Preserve
    the applicable source/target context through host error
    mapping.

20. **Keep strike, clock and product decisions with their owners.**
    This is a literal K=F conversion. FX delta-neutral-straddle
    ATM remains governed by record 17. A source owner first
    resolves a genuine K=F quote if conversion is requested.
    Retain actual volatility clock agreement and market
    selection. General strike inversion remains in record 03.
    Product pricing does not infer conversion from a quote tag.

21. **Reconcile the actual existing surfaces.** Use one Rust
    implementation from Python and WASM, with the explicit
    controls, existing convention wire tags and checked errors.
    Reconcile stubs, declarations, schemas, facade, parity,
    the existing examples and the short-rate tree example
    paths. No new valuation or UI conversion path is selected.

22. **Qualify the meaning and limits.** The result matches a
    single ATM unit premium within the two gates. Near a cap,
    the complementary gate preserves information that the
    rounded premium can hide. Cross-convention round trips,
    other strikes, other expiry clocks and smile risk have
    separate meanings and precision limits.

23. **Require independent numerical and host evidence.** Cover
    literal T and zero boundaries, Normal signed F, signed and
    nearly equal shifts, same-law identity, nonrepresentable
    intermediate premiums, small and near-cap inverses,
    infeasible and unresolved caps and actual rounded-sigma
    replay. Qualify existing Python, WASM and documentation
    paths independently. See the
    [ATM conversion references](../../../REFERENCES.md#checked-price-equivalent-atm-volatility-conversion).

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| F=0.05, T=5, Black sigma 0.75 | Unit premium is about 0.02991321815 and equivalent Normal sigma is about 0.03353266499. The linear scale gives 0.0375. |
| F=−2⁻¹⁰, T=1, source shift 2⁻¹⁰+2⁻⁵⁰ and target shift 2⁻¹⁰+2⁻⁴², source sigma 1 | Shift difference is below 1e−12. The effective levels differ by 256 times and the old identity returns 256 times the source premium. The matching target sigma is about 0.003749418. |
| F=T=1, source Black sigma 20, target shift ±1e−20 | The displayed source premium and both displayed target scales can round to 1. Positive shift gives target sigma about 18.671767. Negative shift puts the target cap below the source premium. |
| F=1, T=1e−200, source Normal sigma 1e−300 | The unit premium is about 3.989422804e−401. The corresponding Black sigma is representable and approximately 1e−300. |
| F=0.05, T=1, source Normal sigma 0.2 | Unit premium is about 0.07978845608, above the unshifted Black cap of 0.05. |
| F=T=1, source Normal sigma is the binary64 value of sqrt(2 pi) and then its next larger neighbor | Ordinary division by the same rounded constant gives 1 in the first case. High-precision comparisons put the first price below the cap and the next one above it. |
| F=2¹⁰⁰⁰, T=1, source Black sigma 150 and target shift 2⁻¹⁰⁷⁴ | The positive complementary probability is approximately 2⁻²⁰⁷⁴. It rounds to zero in binary64, so the selected upper inverse returns a range error. |

These are independent formula and range checks. Production conversion
and host qualification remain pending.

## Observed implementation and evidence

### Scope and input domain

Positive finite volatility and finite forward, Normal/Lognormal/ShiftedLognormal conventions. Lognormal effective forwards must be positive; shifts finite.

### Choice and rationale

Price equivalence at K=F is solved with core Brent rather than applying σN≈FσLN as the answer. That approximation only initializes the solver.

### Algorithm

Validate vol, negative time, forward and shifts; exactly zero time returns input vol before convention-domain checks. Identical conventions (or shifts within1e−12) return unchanged. Price an ATM call under source model; solve target ATM call−source price=0 with σ floor1e−16. Seed scales vol by F or F+shift, using ratio for changing displacements; bad guess falls back to max(|vol|+1e−6,1e−6). Use default Brent settings and return finite positive solution; solver failure becomes VolatilityConversionFailed with residual at guess.

### Selection and controls

Explicit source/target convention; no market-data lookup or OTM conversion. Core Brent owns bracketing/tolerance controls (not caller-configurable here). Same-convention shortcut bypasses numerical pricing.

### Conventions

Normal annual volatility uses forward units/√year; lognormal is dimensionless/√year. Shared ATM price is undiscounted; common annuity would cancel. Shift unit matches forward.

### Limitations, failure and fallback

Time validation only checks time<0, not finiteness: NaN/infinite time can pass same-convention shortcut. Zero time returns unchanged even when cross-convention units differ, because price alone cannot identify vol. No final independent residual threshold is applied beyond the solver’s success contract.

### Alternatives and tradeoffs

First-order scale conversion is cheaper but not price-equivalent at larger variance; explicit Black/normal IV solvers exist elsewhere but this API chooses Brent.

### Implementation and validation evidence

[convert.rs](../../../../finstack-quant/models/src/volatility/convert.rs). Inline round-trip and representative-ATM-price tests compare the same pricing kernels, with negative-forward/insufficient-shift rejection. References: [Brent](../../../REFERENCES.md#brent-1973), [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900).

The Python and WASM functions delegate directly to the Rust converter.
The short-rate tree documentation illustrates explicit caller conversion.
FX delta-neutral straddle uses its separately defined strike under the
accepted FX smile record. See the
[Python binding](../../../../finstack-quant-py/src/bindings/models/volatility.rs),
[WASM binding](../../../../finstack-quant-wasm/src/api/models/volatility.rs),
[short-rate tree documentation](../../../../finstack-quant/models/src/trees/short_rate_tree/mod.rs)
and [FX smile record](fx-delta-smiles.md#accepted-decision-for-checked-fx-delta-smiles-and-strike-materialization).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
