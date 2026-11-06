# CASHFLOWS-010 — Mortality conversions and prepayment assumptions

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped to conversion and prepayment specification kernels. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice, scope and units

Annual conditional rates convert to monthly rates by survival equivalence. Prepayment specifications return SMM for one seasoning month; they do not generate principal cashflows themselves. CPR, SMM and ABS speed are decimals; PSA speed is a multiplier (`1` means the base curve). `CashFlowBuilder::build` does not consume these models. Their application order relative to defaults, interest and scheduled amortization is owned by downstream asset engines and is not invented here.

## Algorithm inventory

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

## Validation and selection

Callers explicitly choose an enum variant; there is no default prepayment model. Constructors only package inputs. `validate` checks constant/post-lockout CPR and every vector entry, then evaluates months one and thirty to cover the speed-curve range. `smm` validates the selected branch/value but does not always validate unused future entries; for example a lockout call can return zero without checking its invalid future CPR. Whole-spec validation is therefore a separate obligation.

The per-month vector helper rejects nonfinite/negative selected values and clamps index selection to its last entry. Model validation additionally constrains annual probability vectors to `[0,1]`.

## Alternatives and evidence

Supported alternatives are constant, PSA, lockout, ABS and explicit annual-CPR vectors. Direct SMM vectors, refinancing incentives, burnout, stochastic prepayment and irregular-period mortality conversion are not provided by these kernels. No automatic fitting/calibration is present.

- [credit_rates.rs](../../../../finstack-quant/cashflows/src/builder/credit_rates.rs), [prepayment.rs](../../../../finstack-quant/cashflows/src/builder/specs/prepayment.rs), [vector lookup](../../../../finstack-quant/cashflows/src/builder/specs/mod.rs).
- Inspected [credit_models.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/credit_models.rs) PSA/SDA examples compare expected annual rates or call the same conversion helper; this combines direct curve-formula assertions and production-kernel/roundtrip evidence. Located ABS depletion formula in [audit_regressions.rs](../../../../finstack-quant/cashflows/tests/audit_regressions.rs) and checked/clamped boundary tests in credit_rates.
- [SIFMA formulas](../../../REFERENCES.md#sifma-mbs-standard-formulas) and [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) are repository pointers. The SIFMA registry entry specifically describes WAL; exact PSA/ABS section/page provenance was not verified. Names such as “industry standard” in tests are not adoption evidence.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
