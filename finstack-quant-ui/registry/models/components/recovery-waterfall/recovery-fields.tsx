"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { shownValue } from "@/components/finstack/shared/primitives/native-display/native-display";

export type Density = "compact" | "comfortable";
/**
 * A returned number to four decimals, never "-0", as relative-value-table
 * shows its numbers; every digit stays in the cell's title.
 */
export const defaultValue = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 4,
  signDisplay: "negative",
}).format;
/** Rust's choices with their Rust descriptions. */
export const options = (
  values: readonly { value: string; description?: string }[],
) =>
  values.map(({ value, description }) => ({
    value,
    label: labelFor(value),
    description,
  }));
export type Row = Record<string, string> & { key: string };
export const row = (fields: Record<string, string>): Row => ({
  ...fields,
  key: crypto.randomUUID(),
});
/** Editable rows of field text; each row keeps a stable key across removals. */
export function useRows(initial: () => Row[]) {
  const [rows, setRows] = useState(initial);
  return {
    rows,
    set: (key: string, field: string, value: string) =>
      setRows((prior) =>
        prior.map((r) => (r.key === key ? { ...r, [field]: value } : r)),
      ),
    add: (fields: Record<string, string>) =>
      setRows((prior) => [...prior, row(fields)]),
    remove: (key: string) =>
      setRows((prior) => prior.filter((r) => r.key !== key)),
  };
}
interface Field {
  name: string;
  value: unknown;
}
/** Returned fields in the order given, each labelled from its Rust name. */
export function FieldTable({
  caption,
  rows,
  format,
  density,
}: {
  caption: string;
  rows: [string, unknown][];
  format: (value: number) => string;
  density: Density;
}) {
  const columns = useMemo<ColumnDef<{}, Field, any>[]>(
    () => [
      {
        id: "name",
        header: "Field",
        accessorFn: (field) => field.name,
        cell: ({ row }) => labelFor(row.original.name),
        meta: { rowHeader: true },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (field) => field.value,
        cell: ({ row }) => (
          <span title={String(row.original.value)}>
            {shownValue(row.original.value, format)}
          </span>
        ),
        meta: { className: "text-right finstack-numeric" },
      },
    ],
    [format],
  );
  return (
    <FinstackTable
      data={rows.map(([name, value]) => ({ name, value }))}
      columns={columns}
      getRowId={(field) => field.name}
      caption={caption}
      density={density}
      emptyState="No fields returned"
    />
  );
}
