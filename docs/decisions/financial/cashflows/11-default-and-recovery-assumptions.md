# CASHFLOWS-011 — Default curves, original-balance loss targets and recovery severity

- Decision status: **Accepted — complete default-target, capacity and recovery-authority package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to default/recovery assumptions and actual consumer/host handoffs.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact/high-precision examples and read-only primary-source/consumer reviews are reported below; these do not qualify an asset default/recovery engine.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

Default assumptions return a monthly default rate using either annual conditional CDR or original-balance cumulative targets. Recovery assumptions return a severity-derived or constant recovery rate and carry a recovery lag for downstream engines. These are deterministic assumption calculators, without hazard calibration, stochastic default simulation or event emission in this slice.

## Accepted decision: explicit default targets, strict capacity and one recovery authority

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[010's checked survival conversion and model-clock boundaries](10-prepayment-and-mortality.md#accepted-decision-checked-monthly-mortality-explicit-model-clocks-and-benchmark-boundaries).
The [SIFMA-hosted chapter SF](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf)
provides the SDA age-profile reference; this package does not implement its full
mortgage cashflow methodology or attribute bespoke target/allocation rules to it.
All assumptions remain binary64 model inputs, distinct from Money/principal
posting. No runtime changes are authorized here.

1. **Separate conditional rates from original-reference targets.** Keep
   explicit Constant, renamed SdaAge, annual-CDR Vector, CumulativeLoss and Timing
   choices within one DefaultModelSpec. Constant/SdaAge/Vector supply conditional
   monthly probabilities; CumulativeLoss/Timing supply targets against a fixed
   original reference balance. A cumulative default share above1 can be meaningful
   for a replenished target and is not a probability. Keep recovery selection
   separate and explicit; no model, original balance or recovery is inferred.

2. **Guard complete immutable input state.** Use fallible constructors and
   strict restoration into private validated default/recovery data. Check all
   future entries, ordering, parameter domains and reachable vector/year lengths
   before publication, including zero-target or unselected-month cases. Do not
   validate only selected entries or sample months1/30. Preserve frozen input
   order and one validation authority across Rust/serde/hosts, without repeated
   full-array validation on each safe monthly query or unchecked public enum/
   struct mutation. Local admission/query failures leave prior state unchanged.

3. **Distinguish applied months from the cumulative origin.** MDR and recovery
   default-vintage queries use month1..=u32::MAX; month0 errors even for a flat or
   zero model. Cumulative target lookup separately accepts endpoint0 as its
   genuine origin F(0)=0. Monthly vectors have1..=u32::MAX entries, first entry
   for month1, with stated hold-last tails. Actual loan/pool/reference clock
   mapping remains the owner’s responsibility; no payment-date, calendar-year,
   acquisition-date or missing-origination fallback is selected here.

4. **Use one explicit MDR interface and a distinct target view.** Replace
   mdr(month) and mdr_with_survival with mdr(month,available_fraction), where
   available_fraction is an explicit Option<f64>. Conditional variants require
   None; target variants require Some(S). Reject missing or inapplicable context
   instead of ignoring it. Remove the inferred1-F(previous) denominator and old
   specialized method without aliases. Rename cumulative_default_fraction to
   get_target(month): return Result<Option<f64>>, Some cumulative original-
   reference target for target variants, None for conditional variants, after
   guarded admission/query validation. Returned cumulative views are not inputs
   to the monthly rate calculation.

5. **Keep constant and vector CDR units explicit.** Constant cdr and every
   Vector annual_cdr entry are finite annual decimal CDR in[0,1], converted by010.
   Rename monthly_cdr to annual_cdr; preserve vector order and hold the last
   annual quote beyond its length. No direct-MDR reinterpretation, interpolation,
   default-zero selection or annualization by asset payment frequency.

6. **Name and specify the SDA age-profile component.** Rename Sda/sda to
   SdaAge/sda_age. For applied loan month t, the unscaled annual CDR is t/5000
   for1..30,3/500 for31..60,(2340-19*t)/200000 for61..120, then3/10000. Require
   finite speed>=0 and EXACT represented-input3*speed<=500 at construction,
   ensuring every profile month remains a probability. This is a disclosed
   project domain restriction; the cited SDA section does not prescribe PSA's
   annual cap. Do not clamp excessive speed or admit it merely because the
   requested tail month happens to have a lower rate.

7. **Preserve SDA survival and disclose the missing product context.** Form
   the scaled profile with exact input/benchmark arithmetic and stable positive
   annual survival through the private monthly conversion, as in010's PSA path;
   do not first restore an annual CDR and lose its near-one complement. Speed0
   gives positive0 after validation. Source SDA also has a final liquidation-
   window shutdown and collateral applicability conditions; an age-only query
   cannot determine them. Actual maturity/liquidation/collateral owners must
   select and qualify those rules or report unsupported full-method use.

8. **Define cumulative-loss conversion literally.** Keep
   cumulative_net_loss_pct as finite nonnegative nondecreasing percentages of
   the fixed reference balance, with1.5 meaning1.5%, not decimal1.5. Require
   nonempty input and finite reference severity s in(0,1]. The cumulative gross
   default target is F(t)=L(t)/(100*s), F(0)=0, with the last L held afterward.
   Permit F>1 without a universal original-balance cap. Monthly demand derives
   directly from adjacent supplied loss values, not rounded F values. This is
   a default-cohort net-loss TARGET converted using reference severity, not a
   calendar realized-loss/recovery-cash timeline or an empirically calibrated
   rating-agency model.

9. **Make Timing's normalization an explicit model definition.** Retain total
   cumulative_default_rate T in[0,1], but replace annual_pct with annual_weights:
   nonempty finite nonnegative relative weights with strictly positive EXACT
   represented-input total W. Define each annual share as w_y/W, irrespective
   of the numeric scale of the supplied weights. This renamed operator replaces
   the100±1e-6 gate; it is not automatic repair of percentage data. Zero-weight
   years are valid, with no sorting or inferred missing years. Require at most
   floor(u32::MAX/12) complete annual buckets and checked indexing/horizon math.
   W need not itself fit binary64 merely to compute a representable normalized
   result; no ordinary overflowing sum or separately rounded normalized weights.

10. **Compute target increments from original inputs.** For CumulativeLoss,
    use exact Δ=(L_t-L_previous)/(100*s), with initial L_previous=0 and tail Δ=0.
    For Timing, applied year y=(t-1)/12 has Δ=T*w_y/(12*W), or0 after the table.
    Its cumulative target is T times the completed/fractional year weights
    divided by W, holding T after12*N; F(0)=0. These are twelve uniform monthly
    slices per supplied model year, not fiscal/calendar dates. Retain full exact
    transient differences/products/normalization through the requested result;
    do not floor decreasing deltas, subtract separately rounded cumulative
    queries or require those queries/intermediate Δ to fit binary64 first.

11. **Require actual eligible capacity against the same fixed reference.**
    Target conversion requires finite S>=0, representing actual eligible default-
    exposure balance divided by the positive fixed original reference used for
    L/T. S may exceed1 after par build. The asset owner determines eligibility
    and event phase and supplies that consistent ratio; do not reconstruct the
    reference from current balance plus tallies, infer S from curve survival or
    use S=1 for a missing/empty baseline. Such ratios remain model inputs, not a
    universal performing/delinquent/defaulted balance classification.
    Capacity admission is against this represented model ratio: prior ratio
    rounding can make an exact cash-capacity boundary unavailable. Such an error
    does not certify insufficient actual Money collateral; do not repair it by
    silently increasing S or clipping demand.

12. **Select strict capacity feasibility, without silent target shortfall.**
    Compare exact monthly Δ with the supplied S before division. If Δ>S,
    including positive Δ with S=0, return an infeasible-target error. If both
    are0, return positive0 as an explicit no-op; otherwise return Δ/S, with
    Δ=0 yielding0 and Δ=S yielding1. Remove the EPSILON empty-stock gate,
    min/max clamps and inferred-survival shortcut. No target backlog, catch-up,
    clipping, rescheduling or realized-default ledger is introduced. A product
    choosing another capacity operator must own that policy explicitly.

13. **Check only the requested mathematical output boundary.** Evaluate
    target capacity comparison and complete ratio scale-safely from admitted
    inputs, using existing exact/checked numerical authority. A subnormal Δ can
    still yield a valid MDR after division by a small S. Conversely a requested
    get_target may independently error for overflow or nonzero-to-zero loss
    without invalidating a different representable monthly query. Require finite
    returned nonnegative target views and MDR in[0,1], preserving010's zero-loss
    and upper-rounding qualifications. Rounded MDR times a separately rounded
    exposure need not exactly reproduce target dollars; financial owners use
    their accepted Money authority rather than claiming exact cash attainment.

14. **Give recovery one exclusive primary authority.** Keep RecoveryModelSpec
    with explicit Constant{rate,lag_months} or Vector{severity_by_month,lag_months}
    choices and guarded constant/vector constructors. Rate and every severity
    are finite decimals in[0,1]; vector nonempty and held last. Remove the dormant
    base-plus-override fallback, with_lag/with_severity_vector construction chain,
    public mutable fields and infallible recovery_rate. Provide checked
    get_rate(default_month), get_severity(default_month) and get_lag_months from
    the same validated owner, without clamps, invalid-entry substitutes or old
    aliases. Explicit0/1 are valid; no recovery value is defaulted.

15. **Preserve primary recovery/severity before complement rounding.** Constant
    retains the supplied recovery; Vector retains the supplied severity. A
    secondary binary64 complement is a checked approximate view and may round
    to0/1 under the stated numerical profile, not a replacement primary input.
    Never recover a tiny original severity as1-roundedRecovery, or a tiny
    original recovery as1-roundedSeverity. Retain the selected primary in owning
    calculation provenance. Actual loss/recovery Money calculations first use
    core012's fallible model-factor admission, which may reject an otherwise
    valid binary64 primary; do not bypass it with an exact binary-rational factor.
    Use the admitted primary/complement authority and owned rounding/allocation
    rules. This layer does not promise exact conservation between two separately
    rounded binary64 views or define a new settlement cash split.

16. **Lock recovery to economic default vintage; keep dates with the owner.**
    Select vector severity using the positive month of DEFAULT and retain that
    source/vintage with the default cohort. Do not reselect it at recovery cash
    arrival. lag_months is required unsigned calendar-month metadata in0..=u32::MAX;
    zero is valid. Actual default date, checked core007 date bridge, range/BDC/
    payment-cadence/settlement phase and terminal-tail treatment remain product
    owned. Overflow cannot become zero lag. Do not fabricate a default date by
    subtracting lag from a desired receipt date; month clamping is not invertible.

17. **Keep reference severity and actual recoveries independent and truthful.**
    CumulativeLoss's s converts its quoted loss target to gross-default demand;
    it does not automatically override RecoveryModelSpec or derive a constant
    recovery from it. If actual recovery assumptions differ, realized net loss
    can differ from that target. Owners claiming net-loss attainment must
    reconcile the definitions explicitly. No implied recovery-cost/advance,
    correlation, borrower behavior, default hazard calibration or discounting
    model is supplied here; reporting recovery remains separately015-owned.

18. **Reconcile actual allocation and queue consumers.** Update structured-
    credit default sources for explicit model context, actual fixed reference,
    direct monthly demand and strict capacity. Period aggregation must use its
    owner’s evolving exposure, not repeatedly compound target rates calculated
    from one unchanged S and claim target preservation. Reconcile pool baseline/
    par-build constraints, delinquency/eligible balance, stochastic extra caps,
    default-vintage recovery queries and lag conversion/queue errors. Existing
    payment-date default stamping, terminal draining and workout backdating are
    observed product choices, not adopted by this package. Failures must
    propagate without publishing partial model state or error-to-zero flows.

19. **Reconcile existing host/wire/parity surfaces together.** Python factories
    already validate; preserve that while guarding Rust constructors/restoration
    and WASM evaluation. Update renamed model fields/variants/methods, required
    explicit None/null versus Some capacity, recovery choices, lag/getters and
    strict JSON/schema/stubs/facade/examples in one coordinated slice. Validating
    constructors need WASM twins over existing plain values, not literal parity
    exclusions. Remove obsolete paths/defaults without compatibility aliases,
    new host classes, registries or state frameworks.

20. **Qualify formula, applicability and provenance separately.** Independently
    cover SDA landmarks/global speed/near-one survival and missing shutdown,
    complete model validation, monthly/cumulative origins and tails, normalized
    timing conservation/large sums, tiny direct increments and par build, strict
    zero/thin/infeasible capacity, recovery primary complements/vintages and
    actual lag/date errors across hosts. Retain exact input authorities, selected
    operator, reference/eligible-exposure association and verified source sections.
    Mathematical references support these definitions, not complete contractual
    cashflows, calibrated fit or runtime/host/product qualification.

## Independent bounded reference checks

On2026-11-09 independent inline Python Fraction/Decimal calculations, without
importing production functions, checked:

- SDA age-profile annual rates at1/30/60/90/120/121 are0.0002/0.006/0.006/
  0.00315/0.0003/0.0003. At the admitted binary64 speed nearest below500/3,
  peak annual survival is1/17592186044416000 and MDR about0.9557183954479313;
  prematurely rounding CDR gives a materially different answer.
- Loss inputs100+2^-46 then100+2^-45 percent, severity1, produce distinct
  month2 target Δ=1/7036874417766400 although both rounded cumulative fractions
  coincide. At reference par2^53, that increment represents1.28. With eta=2^-1074,
  loss eta percent and severity eta instead give target0.01, without first
  losing loss/100 to zero.
- S=2^-60 and Δ=2^-61 give MDR0.5; an EPSILON cutoff would return0. S=0 with
  Δ>0 and any Δ>S are infeasible under the selected rule. S=0,Δ=0 is the
  explicit no-op. S=1.5 supports original-reference target increments above the
  unreplenished survival profile; no1-F(previous) denominator is inferred.
- Timing weights[1,3] and[25,75] define the same normalized two-year allocation.
  Lifetime T0.24 gives monthly demands0.005 in year1 and0.015 in year2 and
  terminal F0.24. Weights[MAX,MAX] still define equal shares despite an
  overflowing ordinary sum. Weights[eta,1], T1, S=eta give month1 MDR near1/12
  even though its original-reference increment cannot be restored to binary64.
- Primary recovery input1e-17 gives a recovery view rounded1; core012-compatible
  admitted Decimal severity1e-17 and Money par1e17 retain loss1. A10% loss target with reference severity50% implies
  gross defaults20%; actual recovery80% would instead realize4% net loss.
- Calendar clamping is not invertible:31March2025 minus1M is28February, whose
  plus1M is28March. Backdating a default by lag therefore cannot generally
  establish the intended economic vintage/receipt date.

The exact-fraction identities above are model references, not a bypass of
core012 admission or a claim that every intermediate factor is valid Money data.
For example, exact target Δ=1/12 exceeds supplied S=binary64(1/12), so the strict
model check rejects that boundary even when actual eligible Money1/reference12
would fit exactly. This is a disclosed ratio-representation limit.

These bounded checks support the accepted distinctions; they are not executed
production allocation/queue tests, complete asset cashflows or vendor comparisons.

## Observed default algorithms

1. **Constant/Vector:** read annual decimal CDR and convert using [CASHFLOWS-010](10-prepayment-and-mortality.md). Vector month zero/one selects its first entry and tail months hold the last value. Checked conversion requires finite CDR in `[0,1]`.
2. **SDA:** base annual CDR is `0.006×m/30` for `m≤30`; `0.006` through month 60; linear decline `0.006−(m−60)/60×(0.006−0.0003)` through month 120; `0.0003` thereafter. Multiply by nonnegative finite speed. Derived annual CDR over one errors. Month zero yields zero.
3. **CumulativeLoss:** supplied monthly cumulative **net loss percentages of original principal**, not decimals, imply cumulative defaults `F(m)=loss_pct(m)/100/severity`, with `F(0)=0`. Severity must be finite in `(0,1]`; the curve must be nondecreasing and nonnegative under whole-spec validation. `F` may exceed one, intentionally allowing par build above original balance. Tail cumulative targets hold constant.
4. **Timing:** total cumulative default rate is a decimal in `[0,1]`; annual timing entries are nonnegative percentages summing to 100 within `1e-6`. Each year's default share accrues linearly over its twelve months. `F(m)=clamp(total_default_rate×Σ annual_pct[y]/100×clamp(m−12y,0,12)/12,0,1)`.
5. For either original-balance model, use `m=max(seasoning_month,1)`, `ΔF=max(F(m)−F(m−1),0)`. Conditional MDR is `clamp(ΔF/S,0,1)`, where default `S=1−F(m−1)` or explicit `mdr_with_survival` receives actual surviving balance/original balance. If `S≤f64::EPSILON`, return zero. Explicit `S` must be finite nonnegative but may exceed one. Capping at one limits defaults to available collateral.

`cumulative_default_fraction` validates the complete target model before returning `Some(F)` for original-balance variants; conditional variants return `None` without full validation. `mdr` directly evaluates the requested branch and does not call full `validate`: decreasing loss targets can be locally floored to zero or malformed future timing can escape a single local request. Callers needing a whole-curve guarantee must validate first.

## Observed recovery algorithm and controls

`RecoveryModelSpec {rate,recovery_lag,severity_vector}` has no automatic product default. Without a nonempty vector, `recovery_rate(month)=clamp(rate,0,1)`. With a vector it selects the same first-entry/tail-hold month convention and returns `clamp(1-severity,0,1)`. If selected vector lookup fails, the infallible method substitutes severity `1-rate`, silently reverting to base recovery. `validate` instead requires finite base rate in `[0,1]`, nonempty optional vector and all severities in `[0,1]`. Thus validation and direct evaluation have materially different invalid-input behavior; NaN base rate can remain NaN through the direct clamp path.

`recovery_lag` is an unsigned month count stored for consumers. This file does not calculate a recovery cash date, discounted recovery value or outstanding-default balance. Recovery estimation in reporting PV is separately specified in [CASHFLOWS-015](15-credit-adjusted-pv.md).

## Rationale, alternatives and limitations

Source comments identify SDA's ramp/plateau/decline and explain survival-aware original-balance default allocation. No approval evidence for these collateral assumptions was located. Conditional CDR and cumulative-loss/timing models are implemented alternatives, with different behavior after prepayment, amortization or par build. Hazard-curve calibration, empirical severity calibration and loss given default correlated with default timing are not supplied here.

## Evidence

- [default.rs](../../../../finstack-quant/cashflows/src/builder/specs/default.rs), [recovery.rs](../../../../finstack-quant/cashflows/src/builder/specs/recovery.rs), [shared vector validation](../../../../finstack-quant/cashflows/src/builder/specs/mod.rs).
- Inspected default unit test `cumulative_loss_preserves_defaults_after_par_build` explicitly expects cumulative defaults 1.4 for 70% net loss at 50% severity, with available-balance clipping. Inspected SDA landmark expectations in [credit_models.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/credit_models.rs); several derive the annual rate via the production inverse conversion. Located whole-curve validation and nonfinite-implied-default tests in default source.
- [SIFMA-hosted chapter SF](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf), sectionC.4, printedSF20–22, supplies the SDA profile and final liquidation-window condition; C.5 states collateral applicability. Relevant pages are dated02/01/99 without asserting a uniform PDF edition. These clauses do not certify this library's original-reference target, capacity or recovery-input operators. The existing [reference entry](../../../REFERENCES.md#sifma-mbs-standard-formulas) identifies the precise support. [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) remains a general pointer, not verified page-level authority for bespoke loss/timing assumptions.
- [Structured-credit default source](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs) supplies inferred/current original-balance context and aggregates monthly rates. [Pool reference balance](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool.rs) reconstructs/restricts original balance in ways requiring par-build reconciliation. [Recovery pool flows](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs), [instrument flows](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs) and [queue release](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/utils/simulation.rs) own workout stamping, lag handling and release behavior. These are static consumer findings, not executed pricing reproductions.
- Existing [Python default](../../../../finstack-quant-py/src/bindings/cashflows/builder/specs/default.rs) and [recovery](../../../../finstack-quant-py/src/bindings/cashflows/builder/specs/recovery.rs) factories/restored inputs validate. [WASM specs](../../../../finstack-quant-wasm/src/api/cashflows/specs.rs) and the [parity ledger](../../../../finstack-quant-py/parity_contract.toml) expose the corresponding operations and constructor exclusions. No production tests executed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
