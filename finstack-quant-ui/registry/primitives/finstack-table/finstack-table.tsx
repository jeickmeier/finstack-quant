"use client";
import {
  tableFeatures,
  useTable,
  type ColumnDef,
  type RowData,
} from "@tanstack/react-table";
import type { ColumnPresentation } from "@/lib/finstack/format/columns";
/** Core-only table props. Row identity is supplied and independent of array positions. */
export interface FinstackTableProps<TData extends RowData> {
  data: TData[];
  columns: ColumnDef<{}, TData, any>[];
  getRowId: (row: TData) => string;
  caption: string;
  loading?: boolean;
  error?: string;
  emptyText?: string;
}
const features = tableFeatures({});
/** Shared unlinked table; native row/cell rendering with no sorting or aggregation features. */
export function FinstackTable<TData extends RowData>({
  data,
  columns,
  getRowId,
  caption,
  loading,
  error,
  emptyText = "No rows supplied",
}: FinstackTableProps<TData>) {
  const table = useTable({ features, data, columns, getRowId });
  const ids = data.map(getRowId);
  if (new Set(ids).size !== ids.length)
    throw new Error("Table row IDs must be unique");
  return (
    <div
      className="overflow-auto font-sans text-base text-foreground"
      aria-busy={loading}
    >
      <table className="w-full border-collapse text-sm">
        <caption className="text-left text-sm text-muted-foreground">
          {caption}
        </caption>
        <thead className="sticky top-0 bg-card">
          <tr>
            {table.getLeafHeaders().map((header) => (
              <th
                key={header.id}
                scope="col"
                className={`border-b border-border px-2 text-left font-medium ${(header.column.columnDef.meta as ColumnPresentation | undefined)?.className ?? ""}`}
              >
                <table.FlexRender header={header} />
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr
              key={row.id}
              className="h-[var(--row-height)] border-border [border-bottom-width:var(--table-rule-width)]"
            >
              {row.getAllCells().map((cell) => (
                <td
                  key={cell.id}
                  className={`px-2 ${(cell.column.columnDef.meta as ColumnPresentation | undefined)?.className ?? ""}`}
                >
                  <table.FlexRender cell={cell} />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {error ? (
        <p role="alert" className="text-error">
          {error}
        </p>
      ) : loading ? (
        <p role="status">Loading…</p>
      ) : data.length === 0 ? (
        <p role="status">{emptyText}</p>
      ) : null}
    </div>
  );
}
