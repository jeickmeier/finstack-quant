"use client";
import { numericEdit } from "@/lib/finstack/schema.mjs";
import { FinstackTable } from "../finstack-table/finstack-table";
import { DecimalInput } from "../decimal-input/decimal-input";
import type { ColumnDef } from "@tanstack/react-table";
export type KnotEdit = [number | string, number | string];
interface KnotRow {
  id: string;
  point: KnotEdit;
}
/** Editable supplied knots using the shared table. Row IDs are supplied separately from coordinates. */
export function KnotTable({
  value,
  rowIds,
  onValueChange,
  xLabel = "x",
  yLabel = "value",
  min,
  sorted,
  error,
}: {
  value: KnotEdit[];
  rowIds: readonly string[];
  onValueChange: (value: KnotEdit[]) => void;
  xLabel?: string;
  yLabel?: string;
  min?: number;
  sorted?: boolean;
  error?: string;
}) {
  if (rowIds.length !== value.length)
    throw new Error("Each knot requires a stable row ID");
  const data = value.map((point, i) => ({ id: rowIds[i], point }));
  const columns: ColumnDef<{}, KnotRow, any>[] = [xLabel, yLabel].map(
    (label, index) => ({
      id: index === 0 ? "x" : "y",
      header: label,
      accessorFn: (row) => row.point[index],
      meta: { className: "text-right finstack-numeric" },
      cell: (info) => (
        <DecimalInput
          label={`${label} ${info.row.id}`}
          value={String(info.getValue())}
          min={index === 0 ? min : undefined}
          onValueChange={(text) => {
            const next = numericEdit(text);
            onValueChange(
              data.map((row) =>
                row.id === info.row.id
                  ? index === 0
                    ? [next, row.point[1]]
                    : [row.point[0], next]
                  : row.point,
              ),
            );
          }}
        />
      ),
    }),
  );
  return (
    <div data-sorted={sorted}>
      <FinstackTable
        data={data}
        columns={columns}
        getRowId={(row) => row.id}
        caption={`${xLabel} / ${yLabel}`}
        error={error}
      />
    </div>
  );
}
