import type { StatementResult } from "finstack-quant-wasm";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import type { StatementResultWire } from "@/lib/finstack/generated/types/statement_result";
import statementSchema from "@/lib/finstack/generated/schemas/statement_result.json";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import { formatRawMoney } from "@/lib/finstack/format/format";
import { groupDecimal } from "@/lib/finstack/format/format";
import Big from "big.js";

const resultCodec = createWireCodec(statementSchema);
type StatementResultCell = StatementResultWire["cells"][string][string];
/** The native value source selected for a cell (`Value > Forecast > Formula`). */
export type StatementCellSource = NonNullable<StatementResultCell["source"]>;
/** Whether a cell has a value, and if not, the native kind of absence. */
export type StatementCellStatus = StatementResultCell["status"];
export type StatementPeriod =
  FinancialModelSpecWire["periods"]["periods"][number];
export interface StatementCell {
  periodId: string;
  present: boolean;
  text: string;
  /** The unmodified native number or sentinel; monetary values retain their Money wire object. */
  value: unknown;
  /** The source Rust recorded on the cell; absent when none was selected (a masked or missing cell, or a rollup). */
  source?: StatementCellSource;
  /** The native cell status; absent when the result has no cell for the period. */
  status?: StatementCellStatus;
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
    ...Object.keys(result.cells),
  ]);
  return [...ids].map((nodeId) => {
    const kind = result.node_value_types?.[nodeId];
    const valueType =
      kind?.type === "monetary"
        ? `Monetary · ${kind.currency}`
        : (kind?.type ?? "Unspecified");
    const cells = Object.fromEntries(
      model.periods.periods.map((period) => {
        const native = result.cells[nodeId]?.[period.id];
        const value = native?.value ?? undefined;
        const monetary =
          value !== undefined && typeof value === "object" ? value : undefined;
        const numeric = typeof value === "number" ? value : undefined;
        // A cell without a value (missing, masked, unavailable) is not present.
        const present = value !== undefined;
        const source = native?.source ?? undefined;
        return [
          period.id,
          {
            periodId: period.id,
            present,
            ...(source === undefined ? {} : { source }),
            ...(native === undefined ? {} : { status: native.status }),
            value,
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
