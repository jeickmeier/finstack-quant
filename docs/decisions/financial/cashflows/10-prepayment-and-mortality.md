# CASHFLOWS-010 — Mortality conversions and prepayment assumptions

- Decision status: **Accepted — complete checked mortality and prepayment-assumption package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to shared mortality conversions, prepayment assumptions and actual consumer/host handoffs.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded high-precision/exact examples and read-only primary-source/consumer reviews are reported below; they do not qualify a complete asset cashflow model.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice, scope and units

Annual conditional rates convert to monthly rates by survival equivalence. Prepayment specifications return SMM for one seasoning month; they do not generate principal cashflows themselves. CPR, SMM and ABS speed are decimals; PSA speed is a multiplier (`1` means the base curve). `CashFlowBuilder::build` does not consume these models. Their application order relative to defaults, interest and scheduled amortization is owned by downstream asset engines and is not invented here.

## Accepted decision: checked monthly mortality, explicit model clocks and benchmark boundaries

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. These are binary64
model assumptions and scalar probabilities under accepted core numeric/model
boundaries, not Money calculations or an asset ledger. Keep one canonical
PrepaymentModelSpec and the existing checked scalar conversion/ABS interfaces.
The primary [SIFMA-hosted chapter SF](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf),
sections B.2 and B.4, supplies benchmark definitions; the explicit API domain,
tail, error and terminal-extension choices below remain library decisions.
No runtime changes are authorized here.

1. **Keep units and the twelve-step conversion meaning explicit.** Annual
   CPR/CDR and monthly SMM/MDR are conditional probabilities represented as
   finite binary64 decimals in[0,1]. PSA speed is a nonnegative finite multiplier,
   with1 meaning100% PSA. ABS speed is a monthly decimal benchmark fraction,
   not an annual CPR or PSA multiplier. Twelve equivalent monthly survival
   steps define annual/monthly conversion; no day count, payment frequency,
   intensity interpretation, observation-frequency inference or irregular-period
   exponent is supplied by these scalar functions.

2. **Use stable checked survival-equivalent conversion.** Retain
   annual_to_monthly(a)=-expm1(log1p(-a)/12) and
   monthly_to_annual(m)=-expm1(12*log1p(-m)). Validate the whole scalar input
   before endpoint shortcuts, return exact positive0 for either signed zero
   and exact1 for1, and handle endpoints directly. Remove the explicit1-a
   subtraction before log; check required intermediates and final results.
   Negative, above-one and nonfinite inputs error without clamping or NaN output.

3. **State the finite-precision result domain honestly.** Successful results
   are finite in[0,1]. For positive mathematical probabilities, a lost nonzero
   intermediate or zero-underflow output errors instead of becoming0. An
   interior probability can round to upper endpoint1, especially monthly→annual;
   permit that finite approximation without treating it as exact certainty.
   Do not force an interior result down to nextDown(1), promise exact inverse
   round trips, correctly rounded transcendental results or a portable ULP/
   relative-error bound. Some extremely small otherwise representable cases may
   error when the chosen kernel cannot resolve them. Independently qualify
   claimed accuracy across the supported numerical domain before runtime signoff.

4. **Remove permissive and redundant public conversion paths.** Delete
   annual_to_monthly_clamped, its infinity repairs and NaN fallback. Remove
   public unchecked psa_cpr; retain one private checked PSA calculation behind
   the validated model. PAC/support consumers needing SMM should use that model,
   not an annual CPR helper followed by permissive conversion. Keep the real
   exposed abs_to_smm helper, sharing the exact same ABS authority as the model.
   No compatibility aliases or replacement generic repair helper.

5. **Admit the complete immutable model before use.** Keep explicit model
   selection without a default. Fallible Rust constructors and strict restoration
   validate every active parameter, including post-lockout CPR and ALL vector
   entries, before publishing private immutable model data. Raw wire shapes may
   be private ingress values, not unchecked public calculation owners. Reuse one
   validation authority across constructors, serde and hosts. Do not rely on a
   caller remembering validate(), sample months1/30 as validation, or rescan an
   immutable complete vector on each monthly query. Guard query arguments before
   zero, lockout, tail or depletion shortcuts.

6. **Select one-based applied months throughout.** smm(month) and abs_to_smm
   use integer month in1..=u32::MAX; reject0 even for Constant, zero speed or a
   locked-out query. Month1 is the first applied monthly risk/accrual interval
   after the chosen origin, not a point-in-time age0, a coupon ordinal or a
   rolled payment date. Remove ABS/vector month0 aliases and PSA's special0
   result. Actual product owners map loan age, origination and pool seasoning to
   this index, including relevant benchmark pool exceptions; no universal+1,
   acquisition-date fallback or new date engine is introduced here.

7. **Retain Constant as an explicit annual-CPR assumption.** Require finite
   cpr in[0,1] at admission and return its checked annual_to_monthly conversion
   for each valid applied month. A literal0 is a valid no-prepayment assumption;
   it does not bypass invalid query or model inputs. No implicit model is chosen
   when an assumption is absent.

8. **Adopt PSA's published CPR cap as part of its formula.** For finite
   speed>=0 and positive applied loan month t, use annual
   CPR=min(speed*min(t,30)/500,1), with the same monthly survival equivalence. This deliberately
   selects the benchmark's100% annual cap, rather than rejecting all sufficiently
   high speeds or repairing an invalid scalar probability. Check the cap against
   the EXACT represented speed without first overflowing an uncapped product.
   Below the cap, retain a stable positive annual survival/log-survival through
   the private conversion kernel; do not first round a derived CPR to1 and then
   call annual_to_monthly(1). Use stable log1p near zero mortality and a checked
   surviving-fraction calculation near the cap, or error if required positive
   information cannot be resolved. No premature rounded benchmark/rate stage
   may move the cap or erase that surviving fraction. Speed0
   returns positive0 after validation. Lost positive uncapped results error under
   point3. Public month0 is excluded even though the source describes separate
   pool AGE/MONTH mapping conventions.

9. **Name the simple lockout model without a CMBS contract claim.** Rename
   CmbsLockout to Lockout and cmbs_with_lockout to lockout, without aliases.
   Validate finite post_lockout_cpr in[0,1] even for a locked-out query. Months
   1..=L return0; month L+1 onward uses its checked conversion. L=0 locks no
   valid month. L=u32::MAX locks every addressable query without overflowing L+1.
   This is a probability assumption, not defeasance, yield maintenance,
   prepayment penalties or legal permission to repay a CMBS loan.

10. **Describe ABS as a loan-age/count benchmark.** For speed p in[0,1] and
    positive underlying loan month t, the reference formula uses
    q=1-p*(t-1) and SMM=p/q before terminal depletion. Its original-loan-count
    interpretation is not a promise of constant original-pool-dollar cash
    prepayments when scheduled amortization/defaults/heterogeneity intervene.
    Do not replace underlying loan age with pool age or infer absent loan
    seasoning from acquisition. Collateral suitability and actual balances/
    payment/application order remain product-owned.

11. **Make ABS terminal selection exact and remove early depletion repair.**
    After full validation p=0 returns positive0. For p>0, select terminal1 when
    the EXACT represented-input product t*p>=1, continuing1 for later queries
    as an explicit library terminal-rate extension. Do not use a rounded
    product, ceil(1/p),1e-9 padding or a tolerance. A bounded dyadic mantissa×u32
    comparison fits existing integer arithmetic; no public precision mode is
    needed. Otherwise q>p and q>=1/t, so use stable checked denominator evaluation
    (such as fused1-p*(t-1)) and checked division p/q without epsilon floors or
    blanket clamp. Check the actual result under point3. The terminal extension
    is not a probability conditional on an actually empty population or proof
    that an asset's cash balance has reached0.

12. **Keep annual CPR vectors explicit and frozen.** Retain Vector/vector,
    renaming monthly_cpr to annual_cpr. Require1..=u32::MAX entries, every finite
    value in[0,1], and caller-supplied order. Entry0 applies to month1; choose
    min(t-1,length-1) and deliberately hold the last annual CPR thereafter, then
    use checked conversion. No sorting, interpolation, missing-entry defaults,
    auto-extension of payload data or direct-SMM reinterpretation. Constant and
    vector assumptions may agree numerically without losing model identity.

13. **Return model probabilities without applying principal.** Keep assumption
    values in binary64; do not force them through Money/Decimal or represent
    numerical rounding as financial rounding. This layer neither schedules cash
    nor chooses prepayment/default/amortization/interest order, risk exposure,
    recovery, scheduled-balance denominator, borrower incentive, burnout, shocks
    or a complete pooling model. Applying ABS SMM to an owner's selected balance
    must not be described as a guarantee about original-balance dollars.

14. **Respect the shared default/recovery ownership boundary.** The two scalar
    survival conversions also serve CDR/MDR; their strict numerical contract is
    shared.011's default curves, SDA/cumulative targets, recovery, month origins,
    tail and application policies remain unreviewed. The current private vector
    helper also serves011, so do not silently adopt its models' indexing or
    selection by changing a shared helper under010. Reconcile actual selected
    owners together; this accepted package does not approve the whole011 package.

15. **Unify existing Rust and host admission and errors.** Python factories
    already call validate(); preserve that safeguard while moving admission
    into canonical guarded Rust construction/restoration. Reconcile WASM wire
    evaluation so unselected invalid future data cannot survive. Update actual
    constructors, month/annual_cpr names, Lockout wire values, strict JSON/schema,
    exports, stubs, facade/parity and examples together without old-name aliases.
    Existing scalar conversion/ABS host twins remain. Validating model
    constructors require corresponding WASM operations over the existing plain
    values; remove constructor literal parity exclusions because these operations
    validate rather than merely package data. Removing Rust-only psa_cpr/clamped
    helpers does not itself require replacement host functions or new classes. No partial
    model publication, warning repair or error-to-zero transport.

16. **Reconcile actual product clocks and error paths explicitly.** Update
    MBS, structured-credit, PAC/support and credit-pool callers for guarded models,
    one-based queries and removal of generic clamping. Preserve factor-correlated
    prepayment's model/conversion errors instead of substituting zero. Actual
    days/30.4375 seasoning, payment-date ageing, irregular-period fallbacks,
    stochastic shock saturation and additional50% monthly caps are separate
    owner choices, not approved by these kernels. An owner must adopt/qualify
    its required mapping/model or report unsupported behavior; no mechanical+1,
    date snapping or hidden compatibility fallback.

17. **Keep mathematical endpoints separate from market qualification.**
    Independent references must cover exact0/1, negative zero, tiny/subnormal
    cancellation, rounded upper endpoints and near-one conversion behavior;
    PSA months1/29/30/31, cap thresholds and extreme finite speeds; lockout
    boundaries and complete future-parameter validation; ABS exact near-depletion
    comparisons including u32 limits; vector tails/all-entry validation and
    cross-host parity. A round trip or a comparison with the same production
    helper does not independently qualify either formula or applicability.

18. **Freeze provenance and scope of adoption.** Retain the selected model,
    exact input binary64 values, positive applied-month meaning and primary
    benchmark version/sections with owning configuration/docs. PSA saturation,
    vector hold-last, lockout and ABS terminal extension must remain visible
    assumptions, without automatic collateral classification or calibration.
    This package selects definitions and failure boundaries; production execution,
    numerical accuracy, contractual fit and full asset-model qualification remain
    separate evidence requirements.

## Independent bounded reference checks

On2026-11-09 independent inline Python Decimal/Fraction calculations, without
importing production functions, checked:

- Annual a=2^-54 has monthly equivalent about4.625929269271486e-18; forming1-a
  in binary64 loses that input before log. Stable high-precision survival
  references distinguish this from an intended0. For eta=2^-1074, a=6eta has
  true monthly value just above eta/2; a basic log/divide kernel can fail to
  resolve that subnormal tie. This accepted package permits an honest error rather than
  false0 and makes no correctly-rounded promise.
- Monthly31/32 has exact annual equivalent1-2^-60, whose nearest binary64 is1;
  that approximation does not establish exact certainty or an exact round trip.
- PSA speed1 gives annual CPR0.002 at month1 and0.06 at30/31; speed2 at15 gives
  0.06. The selected annual cap makes speed20 at30 equal1, not an invalid-model
  substitute. At month11 and actual binary64 speed45.45454545454545, exact
  pre-cap annual survival is1/35184372088832000; monthly mortality is about
  0.9582037309389736, despite its derived CPR rounding to1 in binary64. The
  private kernel must preserve that survival rather than manufacture capped1.
  Constant and vector references use the same twelve-step survival identity
  while preserving distinct model choices.
- At ABS month2, p=1/2-2^-32 gives SMM=(2^31-1)/(2^31+1)<1, while the observed
  padded depletion comparison chooses1. At t=2^32-1 and p=2^-32+2^-64, exact
  t*p=1-2^-64<1 even though the binary64 product rounds to1; the correct ratio
  is (2^32+1)/(2^32+2). Depletion therefore cannot use only rounded t*p.
- With100 identical original loans and ABS speed0.1, ten loans prepay in each
  of the first two reference months. Post-scheduled per-loan balances90 then80
  give cash900 then800, rather than constant original-pool-dollar cash1000.
  This illustration does not select an asset engine's application order.
- Under the accepted admission/domain rules, locked-out invalid future CPR and
  a vector with an invalid unselected entry must fail complete admission;
  month0 must fail all model/helper queries, while
  vector month1 and later hold-last selection retain their explicit meaning.

These are bounded mathematical/semantic checks, not executed production
state/host tests, calibration or complete contractual cashflow comparisons.

## Observed algorithm inventory

| Kernel/model | Implemented formula and boundaries |
|---|---|
| Annual to monthly | `m=1-(1-a)^(1/12)`, implemented as `-expm1(ln(1-a)/12)`; exact zero returns positive zero. Require finite `a∈[0,1]`. Used for both CPR/SMM and CDR/MDR. |
| Monthly to annual | `a=1-(1-m)^12`, implemented as `-expm1(12×ln1p(-m))`; exact zero returns positive zero. Require finite `m∈[0,1]`. |
| Clamped conversion | Clamp annual input to `[0,1]`, then checked conversion; negative/positive infinity become zero/one, NaN remains NaN. This is an explicit permissive helper, not the default checked API. |
| Constant CPR | Convert the supplied CPR each month. |
| PSA | `CPR(m)=speed×0.06×min(m/30,1)`; month zero gives zero. Validate nonnegative finite multiplier and error if derived CPR exceeds one. Raw `psa_cpr` itself does not validate multiplier. |
| CMBS lockout | SMM zero while `seasoning_months <= lockout_months`; convert post-lockout CPR afterward. The first active month is `lockout+1`. |
| ABS | With `k=max(month,1)-1`, remaining original fraction `q=1-speed×k`; return one when `q <= speed×(1+1e-9)`, otherwise `clamp(speed/q,0,1)`. Speed zero returns zero. Month zero aliases month one. |
| Vector | Read annual CPR from the selected monthly entry, then convert. Month zero and one both select entry zero; beyond the vector hold the final entry. Empty vector errors. |

The source comment identifies precision retention for small mortality rates as the rationale for `expm1`/`ln1p`. The annual-to-monthly implementation still forms `1-a` before `ln`; extremely tiny nonzero `a` can round away. No audited lower accuracy bound or new runtime experiment establishes the safe range.

## Observed validation and selection

Callers explicitly choose an enum variant; there is no default prepayment model. Constructors only package inputs. `validate` checks constant/post-lockout CPR and every vector entry, then evaluates months one and thirty to cover the speed-curve range. `smm` validates the selected branch/value but does not always validate unused future entries; for example a lockout call can return zero without checking its invalid future CPR. Whole-spec validation is therefore a separate obligation.

The per-month vector helper rejects nonfinite/negative selected values and clamps index selection to its last entry. Model validation additionally constrains annual probability vectors to `[0,1]`.

## Alternatives and evidence

Supported alternatives are constant, PSA, lockout, ABS and explicit annual-CPR vectors. Direct SMM vectors, refinancing incentives, burnout, stochastic prepayment and irregular-period mortality conversion are not provided by these kernels. No automatic fitting/calibration is present.

- [credit_rates.rs](../../../../finstack-quant/cashflows/src/builder/credit_rates.rs), [prepayment.rs](../../../../finstack-quant/cashflows/src/builder/specs/prepayment.rs), [vector lookup](../../../../finstack-quant/cashflows/src/builder/specs/mod.rs).
- Inspected [credit_models.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/credit_models.rs) PSA/SDA examples compare expected annual rates or call the same conversion helper; this combines direct curve-formula assertions and production-kernel/roundtrip evidence. Located ABS depletion formula in [audit_regressions.rs](../../../../finstack-quant/cashflows/tests/audit_regressions.rs) and checked/clamped boundary tests in credit_rates.
- [SIFMA-hosted chapter SF](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf), Bond Market Association formula pages dated02/01/99: sectionB.2(a)–(c), printedSF5–7, covers survival conversion, PSA's cap and AGE/MONTH distinction; sectionB.4, printedSF13–14, covers ABS and its interpretation limits. The PDF also contains later page updates; this is not a whole-edition date assertion. The accepted positive-month domain and post-depletion extension are separately stated library choices. The existing [reference entry](../../../REFERENCES.md#sifma-mbs-standard-formulas) now identifies this verified source; no2010-edition equivalence is inferred. [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) remains a general pointer, not independently checked page-level support.
- [Credit pool models](../../../../finstack-quant/models/src/credit/pool) and [PAC/support](../../../../finstack-quant/valuations/structured-credit/src/cmo/pac_support.rs) use the permissive helper; factor-correlated prepayment currently substitutes0 for selected model/conversion failures. [MBS types](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/types.rs) and structured-credit flows have owner-specific seasoning clocks. These are static consumer findings, not executed pricing reproductions.
- [Python prepayment binding](../../../../finstack-quant-py/src/bindings/cashflows/builder/specs/prepayment.rs) already validates factories/restored inputs. [WASM specs](../../../../finstack-quant-wasm/src/api/cashflows/specs.rs) parse and evaluate the raw model, exposing the branch-validation discrepancy. The [parity ledger](../../../../finstack-quant-py/parity_contract.toml) records exposed scalar conversion/ABS twins and Rust-only psa_cpr/clamped helpers. No production tests executed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
