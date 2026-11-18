# MARGIN-REGULATORY-XVA-013: Gross-notional residual-risk add-on

- Decision status: **Accepted 2026-11-17 for checked residual risk add-on.**
- Implementation status: Existing behavior is described below. Qualified eligibility, exemptions, transaction identity and gross-notional evidence, and checked aggregation and result paths remain pending.
- Validation status: Primary references, both weights and the exemption scope checked. Independent arithmetic covered unmatched gross aggregation and overflow with finite inputs. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked residual risk add-on

1. **Require an explicit eligibility assessment.** Record the applicable
   profile, live position, underlying and reason for the assessment.
   Classify an underlying as exotic when its exposure falls outside the
   standardised delta, vega, curvature and default-risk treatments.
   Examples include weather and future realised volatility as a swap
   underlying.
   [Basel MAR23.2–3](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/23/inforce/2023-01-01/published/2020-03-27).
2. **Use the prescribed other-residual-risk triggers.** Admit a trade subject
   to vega or curvature when its payoff cannot be perfectly replicated by
   a finite linear combination of vanilla options on a prescribed single
   underlying. CTP membership is a separate trigger, with an exception for
   recognised eligible CTP hedges. Carry applicable RRAO requirements for
   maturityless options and the specified strike or barrier cases.
   [Basel MAR23.4–5](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/23/inforce/2023-01-01/published/2020-03-27) ·
   [Basel MAR21.26](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Apply the specific scope rules.** Cheapest-to-deliver risk, smile risk,
   the specified plain-vanilla multi-underlying correlation risk and
   incidental dividend risk are insufficient on their own. Index and
   multi-underlying products follow the same eligibility tests. Use the
   dedicated mandate-based calculation for funds subject to the unrated
   other-sector equity treatment, with exposure at the maximum extent
   the mandate permits.
   [Basel MAR23.6](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/23/inforce/2023-01-01/published/2020-03-27).
4. **Require evidence for each exemption.** Exclude both trades in a verified
   exact third-party back-to-back match. Under the Basel profile, listing
   or central-clearing eligibility exempts other residual risk.
   Exotic-underlying trades remain chargeable under that listing and
   clearing rule. An in-scope unmatched total return swap retains its full
   prescribed gross notional.
   [Basel MAR23.7 and FAQs](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/23/inforce/2023-01-01/published/2020-03-27).
5. **Use a validated gross amount per live trade.** Require nonnegative
   gross notional in the reporting currency, with a qualified product-specific
   basis and dated FX evidence. Use transaction identity to reject duplicate
   contributions and charge distinct trades on the same instrument
   individually. Require lifecycle evidence for expired, terminated or
   unsettled trades.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR23.8](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/23/inforce/2023-01-01/published/2020-03-27).
6. **Apply one rate per chargeable trade.** Use `1.0%` for exotic underlyings
   and `0.1%` for other residual risks. In the elected profile, a trade
   qualifying under both definitions receives `1.0%` once. Sum the gross
   charges for unmatched trades. Two opposing unmatched trades with
   `1 million` each in chargeable other-residual-risk notional contribute
   `2,000` together.
   [Basel MAR23.8](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/23/inforce/2023-01-01/published/2020-03-27).
7. **Add the checked total once to full capital.** Preserve eligibility,
   exemption evidence, gross amounts, weights and charges through Rust,
   wire, Python and WASM. Report zero only with established coverage.
   Missing required evidence makes full capital unavailable. RRAO sits
   outside the correlation scenarios, and each other capital component
   follows its own scope rules.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) ·
   [Basel MAR23.1 and MAR23.8](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/23/inforce/2023-01-01/published/2020-03-27).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

RRAO adds a fixed gross-notional charge from caller-classified positions. It does not infer residual-risk eligibility or exemption from instrument features, and it is independent of correlation scenarios.

## Algorithm and conventions

For each supplied position choose `0.01` when `is_exotic=true`, otherwise `0.001`. Sum `abs(notional)×weight`. The fold starts from positive zero so an empty vector returns `+0.0`. There is no netting across long/short positions, repeated instrument IDs or different risk flags.

## Selection and controls

The full engine validates nonblank instrument IDs and finite signed notionals, then always computes RRAO even when the SBA risk-class list is restricted. DRC/RRAO vectors append records instead of accumulating keyed risk. Negative notional is accepted and charged by magnitude.

## Limitations and failure behavior

The Boolean selects the rate for inputs the caller supplies. The current path relies on the caller for eligibility, exemptions, notional preparation and currency conversion. It charges repeated entries as supplied. Large finite inputs can overflow the total.

## Alternatives and tradeoffs

The accepted boundary requires qualification evidence and one gross contribution per chargeable transaction. Eligibility and exempt matches are established before checked aggregation.

## Evidence and role

The inspected examples exercise both rates. Independent arithmetic checked the two rates, gross aggregation for unmatched trades and overflow from finite inputs under the current arithmetic. Runtime and regulatory qualification remain pending.
[Runtime and weights](../../../../finstack-quant/margin/src/regulatory/frtb/rrao.rs) ·
[Input collection and validation](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs) ·
[Engine examples](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) ·
[FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
