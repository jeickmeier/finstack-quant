# MARGIN-COLLATERAL-IM-012 — Schedule initial margin from notional and maturity

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: the schedule calculator applies a deterministic rate grid, not a sensitivity or loss model. Proposed rationale: a transparent, reproducible notional-based alternative where the caller has selected the relevant schedule.

## Input domain and conventions

Finite monetary notional (sign is discarded), one of six built-in classes or `custom_[a-z0-9_]+`, and finite nonnegative maturity in years. Registered default is interest_rate,5 years, MPOR 10. Maturity is supplied; standalone calculation does not derive it from dates.

## Ordered algorithm

1. Validate maturity; select short if years<2, medium if 2≤years<5, long if years≥5. Thus exact 2 and 5 enter the higher buckets.
2. Lookup `(asset_class,bucket)` or use fallback default_rate0.15. Validate the selected rate is finite and within[0,1].
3. Compute IM=`abs(notional)*rate` in the notional currency and return one class-labelled component, as-of date and MPOR metadata.
4. Embedded short/medium/long rates are IR 1%/2%/4%, credit 2%/5%/10%, equity/commodity/other 15% throughout and FX 6% throughout.
5. Trait calculation requires `Marginable::im_exposure_base`; absence errors. It uses the calculator's default class/maturity and does not substitute current MTM for notional or automatically infer an instrument class.

## Selection, controls and limitations

Configured constructors use schedule `bcbs_iosco`; explicit ID constructors use embedded lookup. Builder maturity validates; public fields can be mutated directly. Registry validates boundaries increasing and positive and default maturity within[0,100], but runtime `rate` only validates maturity and selected rate. MPOR is a label in this standalone calculator; portfolio rescaling is owned by the portfolio inventory.

Registry parsing accepts schedule rates above one before consumer rejection and does not reject zero schedule MPOR; its admitted domain differs from the calculator’s.

## Alternatives and tradeoffs

Sensitivity-based margin captures hedges more directly; full stressed loss models need substantially more inputs. A fail-closed missing-cell policy would be stricter than the current 15% fallback. Explicit instrument classification avoids broad default interest-rate treatment.

## Source and validation evidence

Inline tests inspect selected grid cells, boundary behavior, absence of explicit exposure base and single-trade formulas. `audit_regressions.rs` checks invalid maturity at entry points. The data README records historical Appendix-A provenance, not an independently verified current regulatory mandate.

Implementation and data: [src/calculators/im/schedule.rs](../../../../finstack-quant/margin/src/calculators/im/schedule.rs), [src/calculators/im/mod.rs](../../../../finstack-quant/margin/src/calculators/im/mod.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [data/margin/schedule_im.v1.json](../../../../finstack-quant/margin/data/margin/schedule_im.v1.json).

Existing evidence: [src/calculators/im/schedule.rs](../../../../finstack-quant/margin/src/calculators/im/schedule.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
