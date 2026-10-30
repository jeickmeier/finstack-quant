"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type { ReactNode } from "react";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import { formatRaw, groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

type EnvelopeColumn = TableEnvelopeWire["columns"][number];
/** Position of a row in the envelope's shared row count. */
interface EnvelopeRow {
  index: number;
}
/** One envelope cell; int64 columns arrive as `bigint` through the host codec. */
type EnvelopeCell = string | number | bigint | null | undefined;
const numericTypes = new Set([
  "float64",
  "nullable_float64",
  "u_int32",
  "nullable_u_int32",
  "int64",
  "nullable_int64",
]);
/** Exact supplied digits with grouping; null as an em dash; no rounding or unit. */
function formatCell(value: EnvelopeCell, numeric: boolean): string {
  if (value == null || !numeric) return formatRaw(value);
  if (typeof value === "number" && !Number.isFinite(value))
    return String(value);
  return groupDecimal(String(value));
}
function envelopeColumns(
  columns: readonly EnvelopeColumn[],
): ColumnDef<{}, EnvelopeRow, any>[] {
  return columns.map((column) => {
    const numeric = numericTypes.has(column.data.type);
    const values = column.data.values as readonly EnvelopeCell[];
    return {
      id: column.name,
      header: labelFor(column.name),
      accessorFn: (row: EnvelopeRow) => values[row.index],
      meta: {
        className: numeric ? "text-right finstack-numeric" : "text-left",
      },
      cell: (context) =>
        formatCell(context.getValue() as EnvelopeCell, numeric),
    };
  });
}
export interface TableEnvelopeProps {
  /** Native `TableEnvelope` as a Rust table function returned it; shown as is. */
  envelope: TableEnvelopeWire;
  caption: string;
  captionVisibility?: "visible" | "sr-only";
  density?: "compact" | "comfortable";
  loading?: boolean;
  error?: string;
  emptyState?: ReactNode;
}
/**
 * Presents a native column-oriented `TableEnvelope`: one row per envelope row
 * and one column per envelope column, both in native order, headers through
 * `labelFor`, numeric columns right-aligned with their exact digits and null
 * as an em dash. It sorts, groups, totals and converts nothing.
 */
export function TableEnvelope({
  envelope,
  caption,
  captionVisibility,
  density,
  loading,
  error,
  emptyState,
}: TableEnvelopeProps) {
  const rows = Array.from(
    { length: envelope.row_count },
    (_, index): EnvelopeRow => ({ index }),
  );
  return (
    <FinstackTable
      data={rows}
      columns={envelopeColumns(envelope.columns)}
      getRowId={(row) => String(row.index)}
      caption={caption}
      captionVisibility={captionVisibility}
      density={density}
      loading={loading}
      error={error}
      emptyState={emptyState}
    />
  );
}
