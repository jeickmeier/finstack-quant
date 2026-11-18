# MARGIN-REGULATORY-XVA-006: Commodity name, tenor and delivery-location delta aggregation

- Decision status: **Accepted 2026-11-17 for checked commodity delta.**
- Implementation status: Existing behavior is described below. Qualified commodity and delivery-location identity, the commodity tenor grid and checked input and result paths remain pending.
- Validation status: Primary references and both parameter schedules checked. Independent examples covered shock and weight scales, pair coefficients and mixed-sign bucket 11 aggregation. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked commodity delta

1. **Require exact commodity and legal delivery-location identity.** Net
   contributions only for the same qualified commodity, complete legal
   delivery-location terms and maturity vertex. Location equality requires
   the same contractual delivery choices. Distinguish WTI from Brent,
   electricity by contractual delivery interval and production region, and
   freight by type, route and delivery week. Decompose commodity spreads
   into the individual underlying factor sensitivities.
   [Basel MAR21.13 and MAR21.83–84](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
2. **Admit all 11 prescribed commodity maturity vertices.** Use `0Y` for
   spot positions. Also admit `0.25Y`, `0.5Y`, `1Y`, `2Y`, `3Y`, `5Y`,
   `10Y`, `15Y`, `20Y` and `30Y`. Use current futures and forward marks
   and their contract maturities, including qualified electricity forward
   prices. The sensitivity producer documents the mapping to the prescribed
   grid using linear interpolation or a method consistent with its pricing
   model. The engine accepts the mapped vertices.
   [Basel MAR21.13 and MAR21.8 FAQ 2](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Use signed reporting-currency P&L for the prescribed relative shock.**
   Reprice at `1.01 × q` for qualified quote `q`, holding the other factors
   fixed. A negative quote becomes more negative. A zero quote produces an
   evaluated zero when the source confirms a valid reprice. Missing or
   unsupported reprices make the affected result unavailable. The producer
   owns physical-unit normalization and dated FX conversion.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR21.15–17 and MAR21.23](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [Published shock formula](https://www.bis.org/bcbs/publ/d457_faq.pdf).
4. **Require an evidenced Table 11 bucket and apply its weight once.**
   Multiply the prepared P&L by `[30, 35, 60, 80, 40, 45, 20, 35, 25, 35, 50]`
   for buckets 1–11. Bucket 11 requires a qualified other-commodity
   assignment. An electricity shock P&L of `+3` yields `+180` after weighting.
   [Basel MAR21.81–82](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Multiply the three pair coefficients before applying the scenario.**
   Each matching dimension contributes `1`. Different commodities use
   `[0.55, 0.95, 0.40, 0.80, 0.60, 0.65, 0.55, 0.45, 0.15, 0.40, 0.15]`
   by bucket. Different tenors contribute `0.99` and different delivery
   locations contribute `0.999`. Apply the scenario once to the product.
   A bucket 2 pair differing on all three axes has base correlation `0.9395595`.
   [Basel MAR21.83–84](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
6. **Use the ordinary quadratic for all buckets.** Bucket 11 applies the
   `0.15` distinct-commodity coefficient before tenor and location
   adjustments. Keep its `K²` in the cross-bucket root with zero
   cross-correlation. Other distinct bucket pairs use `0.20` before the
   scenario transformation. Apply the accepted conditional cap on signed
   bucket sums. Preserve exact factors, weights, coverage and scenario
   evidence through Rust, wire, Python and WASM.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) ·
   [Basel MAR21.83–85](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

Commodity delta uses explicit commodity name, bucket, tenor and delivery-location labels. It reuses the credit-style factorized correlation kernel with commodity-specific weights and correlations, rather than modelling a commodity price process.

## Algorithm and conventions

1. Multiply P&L per 1% price move by bucket 1–11 weights `[30,35,60,80,40,45,20,35,25,35,50]`.
2. Same commodity names correlate at one. Different names use bucket correlations `[0.55,0.95,0.40,0.80,0.60,0.65,0.55,0.45,0.15,0.40,0.15]`. Multiply by one for equal tenors or `0.99` for different tenors, and one for equal locations or `0.999` for different locations.
3. Apply scenario scaling to the product, then calculate bucket K and raw S. Commodity bucket 11 does **not** use the generic `other_bucket` sum-absolute rule; its within-bucket different-name correlation is `0.15`.
4. Inter-bucket gamma is `0.20` for two buckets from 1–10 and zero if either is 11. Apply scenario scaling and record 02's conditional capped-S formula.

## Selection and controls

The commodity branch delegates to `csr_bucketed_delta` with class `Commodity`. Validation requires a nonblank delivery-location label, a valid name and bucket, a finite amount and a tenor admitted by the shared GIRR validator. That validator rejects `0Y`. Bucket identity and delivered-price normalization come from the caller.

## Limitations and failure behavior

The shared kernel's fourth string means delivery location here, bond/CDS basis for credit; those meanings must not be interchanged by a generic feed. Delivery-tenor basis risk is binary equal/different, not term-distance dependent. No averaging of price risk across actual delivery dates, classification logic or physical-unit conversion occurs.

## Alternatives and tradeoffs

Contract-schedule mapping and conversion belong to the sensitivity producer. The accepted approach uses qualified, already-mapped inputs and validates factor identity and coverage when admitting them.

## Evidence and role

Source evidence covers the kernel, tenor admission and parameter schedules. Inspected tests include a two-coal/one-power example and a high-correlation scenario maximum. Independent arithmetic checked the three-axis correlation and its Low transform, bucket 11 with opposite-signed weighted amounts, and the reprice and risk-weight scales. Runtime verification remains pending.
[Runtime kernel](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs) ·
[Input validation](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs) ·
[Weights](../../../../finstack-quant/margin/src/regulatory/frtb/params/commodity.rs) ·
[Correlation rules](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs) ·
[Commodity charge tests](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) ·
[FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
