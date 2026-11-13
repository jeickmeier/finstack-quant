import type { StatementResult } from "finstack-quant-wasm";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import type { StatementResultWire } from "@/lib/finstack/generated/types/statement_result";
import statementSchema from "@/lib/finstack/generated/schemas/statement_result.json";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import { formatRawMoney } from "@/lib/finstack/format/format";
import { groupDecimal } from "@/lib/finstack/format/format";
import Big from "big.js";

const resultCodec = createWireCodec(statementSchema);
/** The native evaluation layer that produced a cell (`Value > Forecast > Formula`, after a `where` mask). */
export type StatementCellSource = NonNullable<
  StatementResultWire["node_sources"]
>[string][string];
export type StatementPeriod =
  FinancialModelSpecWire["periods"]["periods"][number];
export interface StatementCell {
  periodId: string;
  present: boolean;
  text: string;
  /** The unmodified native number or sentinel; monetary values retain their Money wire object. */
  value: unknown;
  /** The layer Rust recorded in `node_sources`; absent for results without it (for example a rollup). */
  source?: StatementCellSource;
}
/** Explicit presentation scaling only; the returned value and exact text remain untouched. */
export function displayStatementCell(
  cell: StatementCell | undefined,
  mode: "exact" | "millions",
): string {
  if (!cell?.present) return "—";
  if (mode === "exact") return cell.text;
  const value = cell.value;
  if (
    value &&
    typeof value === "object" &&
    "amount" in value &&
    "currency" in value
  ) {
    const money = value as { amount: string; currency: string };
    // finstack-arith: display-only millions scaling; the native value and exact text stay untouched
    return `${money.currency} ${groupDecimal(new Big(money.amount).div("1000000").toFixed(1))}m`;
  }
  if (typeof value === "number" && Number.isFinite(value))
    return new Intl.NumberFormat("en-US", { maximumFractionDigits: 3 }).format(
      value,
    );
  return cell.text;
}
/** Accounting-style presentation; native values and edit inputs remain in base units. */
export function reportStatementCell(
  cell: StatementCell | undefined,
  mode: "exact" | "millions",
): string {
  if (!cell?.present) return "—";
  const value = cell.value;
  if (value && typeof value === "object" && "amount" in value) {
    try {
      // finstack-arith: parses the supplied amount for display
      const amount = new Big((value as { amount: string }).amount);
      // finstack-arith: display-only millions scaling; the native value and exact text stay untouched
      const scaled = mode === "millions" ? amount.div("1000000") : amount;
      const text = groupDecimal(
        // finstack-arith: accounting display of the supplied sign; the native value stays untouched
        mode === "millions" ? scaled.abs().toFixed(1) : scaled.abs().toString(),
      );
      return scaled.lt(0) ? `(${text})` : text;
    } catch {
      return cell.text;
    }
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    const text = new Intl.NumberFormat("en-US", {
      maximumFractionDigits: 3,
      // finstack-arith: accounting display of the supplied sign; the native value stays untouched
    }).format(Math.abs(value));
    return value < 0 ? `(${text})` : text;
  }
  return cell.text;
}
export interface StatementRow {
  nodeId: string;
  name: string;
  valueType: string;
  cells: Record<string, StatementCell>;
}

/** Verify the complete structured facade result against the generated Rust output schema. */
export function adaptStatementResult(
  value: StatementResult,
): StatementResultWire {
  return resultCodec.fromHost(value) as StatementResultWire;
}

/** Project only returned values into the model's supplied period order. */
export function statementRows(
  model: FinancialModelSpecWire,
  result: StatementResultWire,
): StatementRow[] {
  const ids = new Set([
    ...Object.keys(model.nodes),
    ...Object.keys(result.nodes),
  ]);
  return [...ids].map((nodeId) => {
    const kind = result.node_value_types?.[nodeId];
    const valueType =
      kind?.type === "monetary"
        ? `Monetary · ${kind.currency}`
        : (kind?.type ?? "Unspecified");
    const cells = Object.fromEntries(
      model.periods.periods.map((period) => {
        const monetary = result.monetary_nodes?.[nodeId]?.[period.id];
        const numeric = result.nodes[nodeId]?.[period.id];
        const present = monetary !== undefined || numeric !== undefined;
        const source = result.node_sources?.[nodeId]?.[period.id];
        return [
          period.id,
          {
            periodId: period.id,
            present,
            ...(source === undefined ? {} : { source }),
            value: monetary ?? numeric,
            text: monetary
              ? formatRawMoney(monetary)
              : numeric === undefined
                ? "—"
                : String(numeric),
          },
        ];
      }),
    );
    return {
      nodeId,
      name: model.nodes[nodeId]?.name ?? nodeId,
      valueType,
      cells,
    };
  });
}

export const statementCellKey = (nodeId: string, periodId: string) =>
  JSON.stringify([nodeId, periodId]);
