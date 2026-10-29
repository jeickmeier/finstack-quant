"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  SimmCurvatureSensitivity,
  SimmSensitivitiesJson,
} from "finstack-quant-wasm";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

export interface SimmSensitivityTableProps {
  /** A SimmSensitivitiesJson document (canonical or host); null while none is available. */
  sensitivities: SimmSensitivitiesJson | null;
  /** Display text of a supplied amount; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
type TupleField = Exclude<
  keyof SimmSensitivitiesJson,
  "base_currency" | "curvature"
>;
/**
 * The wire fields of each SIMM risk class, in Rust's field order, with the
 * names of their tuple positions (from the Rust field docs). Curvature rows
 * carry their own risk_class and are listed under it.
 */
const riskClasses: readonly {
  riskClass: SimmCurvatureSensitivity["risk_class"];
  fields: readonly { field: TupleField; positions: readonly string[] }[];
}[] = [
  {
    riskClass: "interest_rate",
    fields: [
      { field: "ir_delta", positions: ["currency", "tenor", "amount"] },
      { field: "ir_vega", positions: ["currency", "tenor", "amount"] },
    ],
  },
  {
    riskClass: "credit_qualifying",
    fields: [
      {
        field: "credit_qualifying_delta",
        positions: ["sector", "name", "tenor", "amount"],
      },
      {
        field: "credit_qualifying_vega",
        positions: ["sector", "name", "tenor", "amount"],
      },
    ],
  },
  {
    riskClass: "credit_non_qualifying",
    fields: [
      {
        field: "credit_non_qualifying_delta",
        positions: ["name", "tenor", "amount"],
      },
      {
        field: "credit_non_qualifying_vega",
        positions: ["name", "tenor", "amount"],
      },
    ],
  },
  {
    riskClass: "equity",
    fields: [
      { field: "equity_delta", positions: ["underlier", "amount"] },
      { field: "equity_vega", positions: ["underlier", "amount"] },
    ],
  },
  {
    riskClass: "commodity",
    fields: [
      { field: "commodity_delta", positions: ["bucket", "amount"] },
      { field: "commodity_vega", positions: ["bucket", "amount"] },
    ],
  },
  {
    riskClass: "fx",
    fields: [
      { field: "fx_delta", positions: ["currency", "amount"] },
      { field: "fx_vega", positions: ["ccy1", "ccy2", "amount"] },
    ],
  },
];
type TupleRow = { index: number; values: readonly (string | number)[] };
function display(value: unknown, formatValue: (value: number) => string) {
  return typeof value === "number"
    ? formatValue(value)
    : formatRaw(value as string | null | undefined);
}
function tupleColumns(
  positions: readonly string[],
  formatValue: (value: number) => string,
): ColumnDef<{}, TupleRow, any>[] {
  return positions.map((position, j) => ({
    id: position,
    header: labelFor(position),
    accessorFn: (row: TupleRow) => row.values[j],
    cell: ({ getValue }: { getValue(): unknown }) =>
      display(getValue(), formatValue),
    meta: {
      className:
        j === positions.length - 1
          ? "text-right finstack-numeric"
          : "text-left",
    },
  }));
}
type CurvatureRow = SimmCurvatureSensitivity & { index: number };
const curvatureFields = [
  "factor",
  "bucket",
  "expiry_tenor",
  "risk_tenor",
  "volatility_weighted_vega",
] as const;
function curvatureColumns(
  formatValue: (value: number) => string,
): ColumnDef<{}, CurvatureRow, any>[] {
  return curvatureFields.map((field) => ({
    id: field,
    header: labelFor(field),
    accessorFn: (row: CurvatureRow) => row[field],
    cell: ({ getValue }: { getValue(): unknown }) =>
      display(getValue(), formatValue),
    meta: {
      className:
        field === "volatility_weighted_vega"
          ? "text-right finstack-numeric"
          : "text-left",
    },
  }));
}
/**
 * Presents a SIMM sensitivity set by risk class: each delta, vega and
 * curvature entry as supplied, in input order. Nothing is summed, netted or
 * converted; amounts are in the set's base currency.
 */
export function SimmSensitivityTable({
  sensitivities,
  formatValue = String,
  density = "compact",
}: SimmSensitivityTableProps) {
  if (!sensitivities)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No SIMM sensitivities
      </p>
    );
  const sections = riskClasses
    .map(({ riskClass, fields }) => ({
      riskClass,
      tables: fields.filter(({ field }) => sensitivities[field]?.length),
      // Input positions are kept as row ids; rows are filtered, never merged.
      curvature: (sensitivities.curvature ?? [])
        .map((row, index) => ({ ...row, index }))
        .filter((row) => row.risk_class === riskClass),
    }))
    .filter((section) => section.tables.length || section.curvature.length);
  return (
    <section
      aria-label="SIMM sensitivities"
      data-density={density}
      className="space-y-4 font-sans text-sm text-foreground"
    >
      <p className="text-muted-foreground">
        Amounts in base currency {sensitivities.base_currency}, as supplied.
      </p>
      {sections.length === 0 ? (
        <p role="status" className="text-muted-foreground">
          The set holds no sensitivities
        </p>
      ) : (
        sections.map((section) => (
          <section
            key={section.riskClass}
            aria-label={labelFor(section.riskClass)}
            className="min-w-0 space-y-2"
          >
            <h3 className="text-sm font-medium">
              {labelFor(section.riskClass)}
            </h3>
            {section.tables.map(({ field, positions }) => (
              <FinstackTable
                key={field}
                data={(
                  sensitivities[field] as readonly (string | number)[][]
                ).map((values, index) => ({ index, values }))}
                columns={tupleColumns(positions, formatValue)}
                getRowId={(row) => String(row.index)}
                caption={labelFor(field)}
                density={density}
              />
            ))}
            {section.curvature.length ? (
              <FinstackTable
                data={section.curvature}
                columns={curvatureColumns(formatValue)}
                getRowId={(row) => String(row.index)}
                caption={labelFor("curvature")}
                density={density}
              />
            ) : null}
          </section>
        ))
      )}
    </section>
  );
}
