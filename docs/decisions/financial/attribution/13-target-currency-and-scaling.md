# ATTRIBUTION-013: Currency translation, quantity scaling and residual qualification

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

A native-currency attribution translates factor amounts at closing FX and reports opening-principal FX translation separately. Quantity scaling acts on every signed Money leaf. These transformations preserve existing decomposition algebra; they do not recalculate underlying sensitivities or market moves.

## Algorithm

1. Translation is a no-op when target equals headline currency. Otherwise work on a clone so failure leaves the original unchanged. Compute `FX_translation = convert(PV0,T1 FX) - convert(PV0,T0 FX)` and replace the translation bucket.
2. Let native cash addback=`total - MTM`, with missing MTM treated as total. Reconstruct closing native PV=`PV0 + MTM`; reporting MTM is its T1 conversion minus PV0's T0 conversion. Reporting total adds cash addback converted at T1 FX. If both endpoint fields exist, write their date-matched conversions.
3. Convert all signed factor amounts, detail amounts, waterfall PV/step amounts and sensitivity Money values at T1 FX, including embedded pricing `fx_pnl`; convert unsigned adder magnitude separately. Market moves and dimensionless diagnostics are unchanged. Reset the residual placeholder to target currency and recompute it. Cash addback uses closing FX, not each economic cash date, despite the generic `CashflowDate` policy enum stamp.
4. Scaling requires a finite factor and also mutates a clone. Multiply headline/MTM/endpoints/translation/residual, all signed factors and detail Money by that factor; multiply `adder_magnitude` by its absolute value. This preserves negative-position signs and nonnegative gross diagnostics. Scaling preserves tolerance metadata and residual percentage; it does not recompute residual.
5. `accumulate` adds only aggregate amounts, optional headlines/endpoints when both sides provide them, invalid flags, count and notes. It does not merge detailed maps or economic metadata. Unlike scale/translate, accumulation mutates sequentially and can leave a partial update on a later currency/overflow error.
6. Residual validates currencies in every Money leaf, then subtracts carry, all market/model/cross factors and reporting FX translation from total. Currency/subtraction failure flags the result invalid; tolerance qualification then refuses success. Percentage is signed `100*amount/total` and absent when total is effectively zero under the stored rounding context. Tolerance succeeds when absolute residual is at most the larger of absolute threshold and `abs(total)*pct_threshold/100`; effectively-zero total uses absolute threshold only.

## Limits and alternatives

Translation assumes the supplied PV0 corresponds to this attribution and replaces existing `fx_translation_pnl`. A reachable chained-conversion discrepancy exists in [portfolio attribution](../../../../finstack-quant/portfolio/src/attribution.rs): `attribute_composite_primitives` first translates each native leaf to composite reporting currency and accumulates its FX bucket; `aggregate_position_attributions` translates that aggregate again when portfolio base currency differs. The second transform retains the first translation in headline MTM/total but overwrites its factor bucket, which is excluded from the shared factor Money walk. Ignoring rounding, recalculated residual therefore acquires the previous FX-translation bucket multiplied by the second closing FX rate. This is confirmed from source/algebra and independently reviewed, not runtime-tested here. Existing foreign-leaf composite coverage uses the same composite and portfolio currency and does not exercise the second nontrivial translation.

Waterfall audit PVs all convert at T1 FX, while headline endpoints use date-matched FX; consumers must not confuse these roles. Tolerances are currency-amount thresholds and remain numerically unchanged by scaling/translation. Alternatives are revaluation in reporting currency, cash-date cash translation and explicitly composable transformations; none is selected automatically.

The rationale is explicit economic separation between pricing FX exposure and translation of opening capital, with atomic failure for transformation. This is observed/inferred rationale, not adoption. Located tests cover coupon cash preservation, full detail conversion, residual reconciliation and atomic scaling overflow; these are algebraic/contract evidence, not an external FX attribution standard.

## Source and evidence

- [target_currency.rs](../../../../finstack-quant/attribution/src/target_currency.rs) — owning implementation.
- [result.rs](../../../../finstack-quant/attribution/src/result.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
