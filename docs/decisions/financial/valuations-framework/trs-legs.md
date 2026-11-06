# VALUATIONS-FRAMEWORK-06 — Total-return swap leg lifecycle, financing, and par spread

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope, and rationale

`TrsEngine` shares period iteration and discounting across equity and fixed-income TRS. The underlying-specific return model remains in its product crate. The common adapter pays each period's total return and financing at the scheduled, potentially lagged payment date; completed accrual remains outstanding until payment. The same financing projection supplies PV, annuity and par spread so their algebra reconciles.

## Algorithm and conventions

Build accrual periods through the cashflow scheduler using contractual frequency, stub, roll/calendar rules; resolve payment as adjusted accrual end plus business-day lag. Exclude payments on or before `as_of`, including same-day cash. Pass signed start/end year fractions and `initial_level.unwrap_or(1)` to the underlying return model. Reject nonfinite returns; otherwise total-return cash is `N × period_return`, discounted with the total-return leg's date-based discount ratio and accumulated with Neumaier compensation. No clipping of extreme finite returns occurs.

Financing simple rates require exact period-start fixing when start precedes `as_of`. On `as_of`, use an exact published fixing if available, otherwise project `forward.rate_between_dates(start,end,τ)`. Future starts project. For overnight compounding, adjust the accrual endpoints, delegate to [overnight projection](overnight-projection.md), and use its actual accrual fraction. An inverted/empty adjusted overnight interval yields zero rate/accrual and factor one. `SimpleAverage` financing is rejected. The schedule's business-day convention is used when accrual dates are adjusted; otherwise the overnight adapter explicitly uses Modified Following.

Financing cash is `N(P−1)+N s τ`, where simple `P=1+rτ` and `s=spread_bp/10000`. Discount at the financing payment date and accumulate. Exported projected fixings include simple future resets or overnight observations so horizon rolls can hold those rates. The financing annuity is `A=N Σ DF(payment)τ` on exactly the same unpaid periods; `|A|<1e−12` fails. `PV_finance=PV_float+sA`, and par spread is `10000(PV_total_return−PV_float)/A`, independent of swap side. Nonfinite par spread fails. Callers apply receiver/payer sign.

## Controls, limitations, and alternatives

`FinancingLegSpec::new` infers compounding from registered index conventions, falling back to simple on an unknown ID or registry error; later validation rejects simple for a known overnight index. Explicit `with_compounding` overrides construction. Missing calendars fail validation; the internal overnight path recognizes explicit weekends-only and can resolve an empty calendar by currency. The start-level default one is appropriate only where the product model defines that normalization; it is not a historical-level estimate.

Constructor inference is not the serde default: omitted JSON compounding becomes Simple and subsequently fails validation for a known overnight index. Such wire input must explicitly choose the compounded method. The empty-calendar currency fallback is an internal overnight-helper branch; the public engine first constructs its period schedule, which strictly resolves the supplied calendar, so this is not a reachable rescue for an invalid public `TrsScheduleSpec`.

The par-spread helper accepts a `Money` total-return PV but uses its numeric amount with the separately supplied notional; it does not check currency agreement itself. Product-level validation must establish that contract. The financing-annuity Rustdoc says periods ending on/before as-of are excluded, whereas implementation excludes by payment date. The implementation is internally consistent with lagged payments.

Unimplemented alternatives here include payment-date cash inclusion, compounding the financing spread, and pathwise stochastic financing/underlying correlation. None is silently selected by this helper.

## Evidence

- [trs.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/trs.rs): complete engine after the test-only helper, financing projection, PV, annuity and par spread; inline tests are formula/reconciliation evidence, not an independent market benchmark.
- [trs_common.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/trs_common.rs) and [legs.rs](../../../../finstack-quant/valuations/core/src/instruments/legs.rs): schedule and compounding/default validation.
- [Hull](../../../REFERENCES.md#hull-options-futures) supplies general carry/discounting context. A product-specific source supporting each TRS fixing and payment convention is not established here.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
