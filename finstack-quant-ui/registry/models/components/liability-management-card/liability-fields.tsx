"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { shownValue } from "@/components/finstack/shared/primitives/native-display/native-display";

export type Density = "compact" | "comfortable";
/** The supplied numbers as field text. */
function fieldText(terms: object) {
  return Object.fromEntries(
    Object.entries(terms).flatMap(([key, value]) =>
      typeof value === "number" ? [[key, String(value)]] : [],
    ),
  );
}
/** Decimal fields over the supplied terms' numbers, kept as typed text. */
export function useFields(terms: object) {
  const [values, setValues] = useState<Record<string, string>>(() =>
    fieldText(terms),
  );
  const field = (key: string, label: string, help: string) => (
    <DecimalInput
      key={key}
      label={label}
      help={help}
      value={values[key] ?? ""}
      onValueChange={(value) =>
        setValues((prior) => ({ ...prior, [key]: value }))
      }
    />
  );
  return { values, field };
}
interface Field {
  name: string;
  value: unknown;
}
/** Returned fields in the order given, each labelled from its Rust name. */
export function AnalysisTable({
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
        accessorFn: (row) => row.name,
        cell: ({ row }) => labelFor(row.original.name),
        meta: { rowHeader: true },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (row) => row.value,
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
      getRowId={(row) => row.name}
      caption={caption}
      density={density}
      emptyState="No fields returned"
    />
  );
}
