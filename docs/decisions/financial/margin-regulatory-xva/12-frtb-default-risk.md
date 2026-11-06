# MARGIN-REGULATORY-XVA-012: Non-securitisation JTD, seniority-constrained netting and hedge benefit

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

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

No securitisation DRC, look-through, recognition of contractual hedges or automatic cash-equity horizon election is present. Maturity zero is accepted and floored to a quarter-year; settled/matured positions are not removed automatically. Zero-notional/nonzero-P&L cases are ignored. Same-issuer rating consistency is enforced but rating assignment and P&L sign are external.

## Alternatives and tradeoffs

Unrestricted issuer netting can overstate hedge recognition for senior shorts against junior longs. A richer capital taxonomy can support securitisations but must be a separately specified branch; CSR securitisation SBA support alone does not supply it.

## Evidence and role

Runtime and fixed tables: [src/regulatory/frtb/drc.rs](../../../../finstack-quant/margin/src/regulatory/frtb/drc.rs); position validation: [src/regulatory/frtb/types.rs](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs). Inspected [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) checks a junior long/senior short restriction and the quarter-year maturity floor; [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates single-long and per-sector hedge-benefit tests. The tests are authored formula examples, not an external portfolio certification. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
