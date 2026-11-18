# MARGIN-REGULATORY-XVA-012: Non-securitisation JTD, seniority-constrained netting and hedge benefit

- Decision status: **Accepted 2026-11-17 for checked non-securitisation default risk capital.**
- Implementation status: Existing behavior is described below. Prepared gross JTD, legal and lifecycle qualification, scoped capital coverage and checked result paths remain pending.
- Validation status: Primary references and all nine risk weights checked. Independent examples covered signed and zero-notional JTD, maturity boundaries, seniority-constrained matching and the hedge benefit. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked non-securitisation default risk capital

1. **Pass prepared gross JTD to aggregation.** Determine credit direction
   from loss or gain at default. For the notional-based route, use
   `raw = LGD × signed bond-equivalent notional + recognized P&L` on the
   prescribed product basis. Gross JTD is `max(raw, 0)` for long credit
   and `min(raw, 0)` for short credit. Keep notional and recognized P&L as
   distinct preparation inputs. A bought bond call worth `7` can have zero
   bond-equivalent notional and `+7` of gross JTD.
   [Basel MAR22.9–14](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/22/inforce/2023-01-01/published/2020-03-27).
2. **Qualify LGD separately from legal seniority.** Use `25%` for qualifying
   covered bonds, `75%` for senior debt, and `100%` for equity and non-senior
   debt. Use the full notional term for a qualified recovery-independent
   payoff. Require contractual or legal evidence for zero JTD from an unwind
   that extinguishes default exposure.
   [Basel MAR22.12–13 and MAR22.18 FAQ](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/22/inforce/2023-01-01/published/2020-03-27).
3. **Require the applicable exposure decomposition.** Use constituent legal
   obligors for traded non-securitisation credit and equity derivatives where
   look-through applies. For multi-name repricing, measure the loss from each
   name defaulting separately at zero recovery. Include the embedded equity
   option in convertible-bond JTD. Use the dedicated fund treatment when
   required. Route CTP and non-CTP securitisation exposures and their
   assigned hedges to their own calculations. Required but unqualified
   coverage makes full capital unavailable.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR22.2–8 and MAR22.11–14 FAQs](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/22/inforce/2023-01-01/published/2020-03-27).
4. **Establish that the exposure is live before scaling.** Multiply each
   gross JTD by `clamp(residual years, 0.25, 1.0)` before offsetting. Use the
   derivative contract's maturity. Elect three months or a horizon longer
   than one year for cash equity. Remove confirmed extinguished exposures
   from the live book. Require evidence of continuing exposure and a
   qualified maturity basis for matured but unsettled positions.
   [Basel MAR22.15–20](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/22/inforce/2023-01-01/published/2020-03-27).
5. **Offset only within the same effective obligor.** A short can offset
   a long of equal or higher seniority. Process the most junior longs first
   and consume the most senior eligible shorts first. Confirm legal rank
   and any qualifying guarantor substitution before matching. Retain
   remaining long and short amounts separately.
   [Basel MAR22.19–21](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/22/inforce/2023-01-01/published/2020-03-27) ·
   [Basel CRE22.71 and CRE22.73](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/22/inforce/2023-01-01/published/2020-11-26).
6. **Require evidenced sector and risk weight assignments.** Use corporate,
   sovereign, or local government and municipality buckets. The weights
   for AAA, AA, A, BBB, BB, B, CCC, unrated and defaulted are
   `0.5%, 2%, 3%, 6%, 15%, 30%, 50%, 15%, 100%` respectively. Require a
   consistent effective weight per obligor in the ordinary calculation.
   Mixed weights require a qualified rule for weighting residual exposures
   after offsetting. Internal rating mappings and public-sector zero
   weights require applicable approval and profile evidence.
   [Basel MAR22.7 and MAR22.22–24](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/22/inforce/2023-01-01/published/2020-03-27).
7. **Calculate hedge benefit from unweighted residuals.** In each bucket
   let `L` be the long sum and `H` the short magnitude. Use
   `HBR = L / (L + H)` for a positive denominator and zero for an empty
   bucket. With weighted residuals `WL` and `WH`, calculate
   `max(WL − HBR × WH, 0)`. A BBB long of `100` and a BB short of `50`
   from distinct obligors in the same bucket give a charge of `1`.
   Sum the three bucket charges.
   [Basel MAR22.23–26](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/22/inforce/2023-01-01/published/2020-03-27).
8. **Use checked arithmetic and preserve calculation evidence.** Carry
   gross JTD, maturity scaling, permitted offsets, residuals, applied
   weights, hedge benefit and coverage through Rust, wire, Python and
   WASM. Add DRC once to full capital outside the correlation scenarios.
   Selected SBA diagnostics report their selected SBA charges.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

DRC is a separate non-securitisation default-risk calculation, outside correlation scenarios. A position supplies signed notional under the field name `jtd_amount`, P&L adjustment, residual maturity, issuer, sector, seniority and rating; the engine applies LGD itself.

## Algorithm and conventions

1. Validate nonblank issuer, rating 1–9, finite nonnegative maturity, finite signed notional/P&L, and equity seniority only in the Corporate sector. One issuer must have one sector/rating across all positions.
2. LGD by rank is CoveredBond `0.25`, SeniorUnsecured `0.75`, Subordinated `1.00`, Equity `1.00`; ranks increase from senior to junior. Form `raw=LGD×signed_notional+PnL` and maturity factor `clamp(maturity_years,0.25,1)`.
3. Positive notional contributes `max(raw,0)×factor` to long JTD; negative notional contributes `max(−raw,0)×factor` to short JTD magnitude. Zero notional contributes nothing even if P&L is nonzero.
4. Within an issuer, consume long ranks from junior to senior. A short at rank j can offset a long at rank i only when `j>=i`: equally senior or more junior shorts may offset that long. Within each long rank consume eligible short ranks in increasing order. This preserves seniority restrictions instead of simply summing all signed JTD.
5. Aggregate residual long L and short magnitude H by sector, plus rating-weighted sums WL and WH. Rating weights for buckets 1–9 are `[0.005,0.02,0.03,0.06,0.15,0.30,0.50,0.15,1.00]`; bucket 8 is unrated, 9 defaulted.
6. `hedge_benefit=L/(L+H)` when denominator positive, otherwise zero. Sector charge is `max(WL−hedge_benefit×WH,0)`. Sum Sovereign, Corporate and LocalGovernment charges without cross-sector offset.

## Selection and controls

DRC always runs on the supplied position vector even for an SBA risk-class subset. Empty vectors produce zero. Position order is retained in input, but obligors are grouped in a BTreeMap before constrained offsetting. Negative notionals are allowed; sign comes from the original notional, not adjusted raw JTD.

## Limitations and failure behavior

The current path covers non-securitisation DRC and assumes the positions already reflect any required look-through. The caller supplies legal, cash-equity horizon and lifecycle treatment. A zero maturity is floored to a quarter-year, and the engine skips zero-notional positions even when P&L is present. The engine validates one rating and sector per issuer, and relies on the caller for classification and P&L sign.

## Alternatives and tradeoffs

The accepted boundary preserves the legal seniority constraint and lets the preparation owner establish signed JTD for each supported product basis. The separate default risk rules for securitisations require their own qualified coverage.

## Evidence and role

Inspected examples cover seniority restrictions, the quarter-year floor and bucket hedge benefit. Independent arithmetic checked the Table 2 weights, signed and zero-notional JTD, both maturity boundaries and a mixed-rating hedge. An exhaustive comparison on 6,561 small four-rank cases checked that the eligible matching order attains the maximum offset under one effective weight per obligor. Runtime verification remains pending.
[Runtime and tables](../../../../finstack-quant/margin/src/regulatory/frtb/drc.rs) ·
[Input validation](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs) ·
[Audit regressions](../../../../finstack-quant/margin/tests/audit_regressions.rs) ·
[Engine examples](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) ·
[FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
