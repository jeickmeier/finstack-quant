"use client";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { TableEnvelope } from "@/components/finstack/shared/table/table-envelope/table-envelope";
import { UndefinedValue } from "@/components/finstack/shared/primitives/native-display/native-display";

/** The two StatementResult table layouts: a column per node, or a row per node and period. */
export type StatementTableLayout = "wide" | "long";
const layouts = (["wide", "long"] as const).map((value) => ({
  value,
  label: labelFor(value),
}));
/**
 * One RFC 4180 field. Rust holds a cell with no value as NaN, which a worker
 * delivers as NaN and JSON stores as null: both are an empty field. Anything
 * else is its plain `String` form, with no rounding.
 */
function csvField(cell: unknown): string {
  if (cell == null || Number.isNaN(cell)) return "";
  const text = String(cell);
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}
/**
 * The cells of a native table as RFC 4180 CSV: a header of the native column
 * names, then one CRLF-terminated record per row, in native order.
 *
 * @param table - Native TableEnvelope, written as supplied.
 * @returns The CSV text.
 */
export function statementTableCsv(table: TableEnvelopeWire): string {
  const columns = table.columns;
  const header = columns.map((column) => csvField(column.name)).join(",");
  const records = Array.from({ length: table.row_count }, (_, row) =>
    columns
      .map((column) =>
        csvField((column.data.values as readonly unknown[])[row]),
      )
      .join(","),
  );
  return [header, ...records].map((record) => `${record}\r\n`).join("");
}
/** Saves the text as a file the way JsonViewer saves JSON. */
function download(text: string, name: string) {
  const url = URL.createObjectURL(
    new Blob([text], { type: "text/csv;charset=utf-8" }),
  );
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export interface StatementExportProps {
  /** The layout the table holds. */
  layout: StatementTableLayout;
  /** Called with the layout the user picks, for the host to fetch that table. */
  onLayoutChange(layout: StatementTableLayout): void;
  /** The native statementResultToTableWide or statementResultToTableLong result; null while none is available. */
  table: TableEnvelopeWire | null;
  loading?: boolean;
  /** The native error of the request, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}
/**
 * A statement result as a table in either layout, and a CSV download of exactly
 * those cells. Rows, columns and digits are Rust's; nothing is computed here.
 */
export function StatementExport({
  layout,
  onLayoutChange,
  table,
  loading,
  error,
  density = "compact",
}: StatementExportProps) {
  return (
    <section
      aria-label="Statement export"
      data-density={density}
      className="@container min-w-0 max-w-full space-y-4 font-sans text-sm text-foreground"
    >
      <div className="flex flex-wrap items-end gap-4">
        <EnumField
          label="Layout"
          layout="radio"
          value={layout}
          onValueChange={(value) =>
            onLayoutChange(value as StatementTableLayout)
          }
          options={layouts}
        />
        <Button
          type="button"
          variant="outline"
          disabled={!table || !!error || loading}
          onClick={() =>
            download(statementTableCsv(table!), `statement-${layout}.csv`)
          }
        >
          Download CSV
        </Button>
      </div>
      {error ? (
        <p role="alert" className="text-error">
          {error}
        </p>
      ) : loading ? (
        <p role="status" className="text-muted-foreground">
          Building table…
        </p>
      ) : !table ? (
        <p role="status" className="text-muted-foreground">
          No table
        </p>
      ) : (
        <TableEnvelope
          envelope={table}
          caption={`Statement result, ${layout} layout`}
          density={density}
          missingCell={
            <UndefinedValue reason="Rust returned no value for this cell" />
          }
        />
      )}
    </section>
  );
}
