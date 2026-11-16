# PORTFOLIO-ATTRIBUTION-004: Modified Dietz, geometric linking and money-weighted return

- ID: PORTFOLIO-ATTRIBUTION-004
- Decision status: Accepted 2026-11-16 for checked portfolio performance returns.
- Implementation status: Observed for the stated owning source scope. Runtime, caller and host reconciliation with the accepted contract are pending.
- Validation status: Published Dietz examples and independent date, loss, linking, netting and money-weighted calculations were checked. Runtime, host and numerical-range qualification of the accepted contract are pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked portfolio performance returns

1. Use one dated request for Dietz and portfolio money-weighted return. Require nonnegative opening and closing end-of-day NAVs, a strictly positive date span, one reporting currency, exact `Money` amounts and a consistent valuation and fee basis. Require an explicit list of external flows in `(start, end]`, with contributions positive and withdrawals negative. The opening mark includes start-date activity and the closing mark includes final-day flows. Admit distinct same-date flows. The caller supplies complete classified flows and any upstream FX conversion. [Financial perspectives and boundary conversions](../conventions/0044-financial-perspectives-and-boundary-conversions.md).
2. For Modified Dietz let `D` be the actual calendar days from start to end and `k_j` the actual days from each flow to end. Compute `D*(E-B-sum C_j)/(D*B+sum k_j*C_j)`. Final-day flows receive zero denominator weight. Evaluate numerator and denominator exactly with widened decimal coefficients, require strictly positive weighted capital, and convert only the final ratio to binary64. Reject exact returns below `-1`. Admit an exact `-1` as terminal only with zero closing NAV. A partial loss that rounds to `-1` at the linkable return boundary gives a range error. Modified Dietz approximates time-weighted performance when flow-date valuations are unavailable. [GIPS Handbook for Firms, provision 2.A.24](https://www.gipsstandards.org/standards/gips-standards-for-firms/gips-standards-handbook-for-firms/).
3. Geometrically link one compatible finite simple total return per adjacent interval of an ordered actual valuation-date grid. Require a nonempty, contiguous sequence and check it fully before evaluating returns. Accept exact `-1` only in the final interval. Use checked compensated `log1p` accumulation and `expm1` for cumulative return. Keep explicit terminal-loss status because strictly positive remaining wealth can round to displayed `-1`. Evaluate without intermediate wealth products and qualify mixed-sign cancellation near zero. [Accepted analytics wealth rules](../analytics/02-wealth-and-annualization.md).
4. Report cumulative linked return for every admitted window. Expose the annualized figure only from the first calendar anniversary, using the core original-anchor clamping rule, and calculate it directly from log growth with ACT/365F. March 1, 2023 to February 29, 2024 spans 365 days and precedes the first anniversary. Short-window mathematical CAGR remains an analytics operation. [GIPS Handbook for Firms, provision 2.A.12](https://www.gipsstandards.org/standards/gips-standards-for-firms/gips-standards-handbook-for-firms/). [Accepted core date arithmetic](../core-conventions/007-tenor-date-arithmetic.md).
5. Build the investor stream from negative opening NAV, negated external flows and positive closing NAV. Net amounts exactly by date before converting each net amount to binary64. Delegate to core XIRR for ACT/365F, one-sign-change eligibility, solving and residual checks, preserving its errors. A zero closing NAV with no positive receipt gives no supported root. For a sub-year window report the holding-period equivalent `expm1(log1p(r)*actual_days/365)` from the annual-effective core root. From the first anniversary report the annual-effective rate with its basis. [Accepted core XIRR](../core-conventions/017-irr.md).
6. Use the same checked Rust inputs, result basis, terminal status and typed errors across Python, WASM and the UI. Reconcile schemas, declarations and caller examples. Move bond and LBO examples that require raw investor-stream XIRR to the canonical core operation. Use the published Dietz example accurately and label the repository's distinct fixture as an independent example.

Independent checks reproduce `15/98` (approximately `15.3061%`) for the handbook's single-period example and `4259/27250` (approximately `15.6294%`) after revaluing and linking its two subperiods. A full final-day withdrawal can yield zero Dietz return. Zero closing NAV with no flows gives an exact terminal `-1` for positive opening NAV.

## Observed choice and conventions

These helpers distinguish an approximate flow-adjusted period return, a geometric link of caller-supplied subperiod returns and an investor-perspective XIRR. They do not infer external cashflows from positions or choose valuation dates. Returns are decimals, not percentages; `horizon_years` is supplied in 365-day years.

## Algorithms and controls

**Modified Dietz.** Given beginning and ending market values `B,E`, contributions `C_j` and remaining-period fractions `a_j`, calculate

`R = (E-B-sum C_j)/(B+sum a_j*C_j)`.

Positive `C` is capital contributed to the portfolio. Validate finite values and `a_j` in `[0,1]`, require a finite strictly positive denominator and a finite result. Flows exactly at period start use fraction one; end flows use zero. The wrapper accepts the fractions directly, so date consistency is caller-owned. This approximates a time-weighted return when intraperiod valuation is unavailable.

**Linked return.** Validate finite horizon and finite subperiod returns strictly greater than `-1`. Multiply `G=product(1+r_j)` and require finite positive growth. Return cumulative `G-1`. Only when horizon is at least one year return `G^(1/horizon)-1` as annualized; for shorter, zero or negative finite horizons the annualized field equals cumulative and the flag is false. An empty return sequence produces zero. The algorithm uses direct multiplication, not log accumulation, and rejects underflow to zero or overflow.

**Money-weighted return.** Convert dated investor flows to core XIRR inputs. The audit recorded `xirr(...,Act365F,None)`. The current wrapper calls `xirr(&flows)`, whose clock is fixed ACT/365F. Contributions are negative and withdrawals and terminal proceeds positive. Core owns date netting, one-sign-change eligibility, root bracketing and residual checks. [XIRR](../core-conventions/017-irr.md).

## Alternatives and evidence

Exact subperiod valuation and geometric linking avoid Dietz timing approximation but require more observations. Modified IRR and multiple-root selection are not provided by this XIRR wrapper. Log accumulation could extend linking's numerical range, but current direct-product failure semantics must be recorded accurately.

[Source and inline formula examples](../../../../finstack-quant/portfolio/src/performance.rs) include a stylized Dietz fixture labelled as a GIPS example, no-flow identity, annualization thresholds and XIRR roundtrip. The handbook inputs differ from the stylized fixture. No production tests ran for this review.

## Follow-up tracking

Implementation and independent validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
