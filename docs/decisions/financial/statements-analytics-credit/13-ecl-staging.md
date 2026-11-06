# STATEMENTS-ANALYTICS-CREDIT-13: SICR comparison, default backstops and probationary curing

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Staging is an ordered policy waterfall: immediate default evidence first, then significant-increase triggers, then cure-state rules. Current and origination PD snapshots remain distinct, including when rating labels match. The implementation's thresholds and cure durations are configurable policy choices.

## Scope and input domain

Exposure maturity, DPD, qualitative/default flags, current/origination labels, previous stage and consecutive-performing-period count; staging configuration; either direct matched-window PDs or current/origination term structures plus nonnegative finite exposure age. Measurement EAD/LGD/EIR are not validated or read by staging.

## Algorithm

1. Validate nonnegative finite PD-delta thresholds and maturity. Curve staging also validates age. DPD >= Stage-3 threshold immediately returns Stage 3 with that trigger. Otherwise, enabled bankruptcy/distressed-modification/cross-default/custom default evidence returns Stage 3. These early exits precede curve PD lookups.
2. Curve staging checks each supplied rating exists in its own source. If either exposure label is absent, skip the PD comparison; otherwise set h=min(remaining maturity,30). Origination comparison PD is `[F_orig(age+h)−F_orig(age)]/[1−F_orig(age)]`, requiring positive origination survival and monotone valid PDs; current PD is `F_current(h)`. Direct staging receives the two already matched conditional-window probabilities and validates both before entering the waterfall.
3. A current-minus-origination PD delta strictly above the absolute threshold adds a trigger. If relative comparison is enabled, compute current/origination and trigger strictly above the configured multiple. Both zero means no relative increase; positive current against zero origination, or any non-finite ratio, errors rather than silently treating the ratio as infinity.
4. With both rating labels and a positive notch threshold, locate each on the explicit ordered scale or standard migration scale. A deterioration of at least the threshold adds a trigger. Unknown labels skip this notch check. Enabled watchlist/forbearance/adverse/custom SICR flags and DPD >= Stage-2 threshold add further ordered triggers.
5. Any SICR triggers give Stage 2, except a prior Stage 3 remains Stage 3 until its Stage-3-to-2 performing count is satisfied. This triggered transition reports `cured=false` even when Stage 3 becomes Stage 2.
6. With no trigger, prior Stage 2 remains until its cure count then becomes Stage 1. Prior Stage 3 remains until the first count, then becomes Stage 2 until the sum of both cure counts, then Stage 1; the sum saturates on integer overflow. Successful no-trigger stage improvements set `cured=true`. The returned trigger is `NoTrigger` even when probation retains Stage 2/3.

## Selection and controls

Default policy: absolute PD delta .01, relative multiple 2, three rating notches, DPD thresholds 30/90, both qualitative switches true, cure counts 3/12. Count units are caller observations, not fixed months. Direct current/origination PDs stay separate even when labels match.

## Conventions

Absolute threshold is a probability-point decimal difference; relative threshold is a multiple of baseline PD. Rating labels are ordered best-to-worst; matching is exact. Forbearance is a SICR flag, distressed modification is default evidence. The 30-year cap applies to curve-derived SICR comparison, not Stage-2 allowance maturity.

## Limitations, failure and fallback

Configuration validation does not enforce DPD ordering, nonempty/unique custom scale labels or cure policy consistency; custom absolute PD threshold may exceed one although embedded policy validation is stricter. Unknown scale labels silently disable the notch test. Curve early-default paths can avoid missing/invalid PD-source detection, while direct PD staging validates supplied values before default classification. A generic `cured` flag does not uniformly mean any downward stage transition.

## Alternatives and tradeoffs

Conditional matched-window comparison avoids comparing unequal origination/current horizons; full-life or one-year PD comparisons would change SICR policy. Explicit cure-transition reasons could remove ambiguity. Rejecting unknown ratings or invalid threshold ordering is stricter but changes permissive input handling.

## Evidence

[staging.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/staging.rs) and [request.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/request.rs) own staging. Inline tests locate downgrade equality/custom/unknown scales, default versus SICR flags, Stage-3 probation, matched-age origination windows, missing snapshots and zero-baseline relative errors. [engine.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/engine.rs) locates unchanged-rating/origination-snapshot integration cases. [IFRS 9](../../../REFERENCES.md#ifrs-9-impairment) and [BCBS guidance](../../../REFERENCES.md#bcbs-2015-ecl-guidance) provide a named conceptual framework, not validation of these threshold calibrations.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
