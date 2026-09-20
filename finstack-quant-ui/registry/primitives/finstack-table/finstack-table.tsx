"use client";
import {
  tableFeatures,
  rowSelectionFeature,
  cellSelectionFeature,
  useTable,
  type ColumnDef,
  type RowData,
} from "@tanstack/react-table";
import type { ReactNode, MouseEvent, KeyboardEvent } from "react";
import { tableSelection, type TableLinkProps } from "./selection";
import type { ColumnPresentation } from "@/lib/finstack/format/columns";
export interface CellActivation<TData, TValue = unknown> {
  row: TData;
  rowId: string;
  columnId: string;
  value: TValue;
}
/** Table data and totals are supplied canonical values, never computed here. */
export interface FinstackTableProps<
  TData extends RowData,
  TValue = unknown,
> extends TableLinkProps<TData> {
  data: TData[];
  columns: ColumnDef<{}, TData, any>[];
  getRowId: (row: TData) => string;
  caption: string;
  loading?: boolean;
  error?: string;
  emptyState?: ReactNode;
  density?: "compact" | "comfortable";
  /** Already formatted canonical totals, addressed by explicit column ID. */
  totals?: Readonly<Record<string, ReactNode>>;
  /** Notifications only. The native change adapter is the sole selection writer. */
  onRowActivate?: (row: TData) => void;
  onCellActivate?: (cell: CellActivation<TData, TValue>) => void;
}
const coreFeatures = tableFeatures({});
const linkedFeatures = tableFeatures({
  rowSelectionFeature,
  cellSelectionFeature,
});
const nestedControl = (event: MouseEvent | KeyboardEvent) => {
  const target = event.target;
  if (!(target instanceof Element)) return false;
  const control = target.closest(
    'button,a[href],input,select,textarea,summary,[contenteditable]:not([contenteditable="false"]),[role="button"],[role="checkbox"],[role="combobox"],[role="switch"],[role="radio"],[role="slider"],[role="spinbutton"],[role="tab"],[data-table-action]',
  );
  return control !== null && control !== event.currentTarget;
};
const keyActivation = (event: KeyboardEvent, activate: () => void) => {
  if (
    event.target !== event.currentTarget ||
    !["Enter", " "].includes(event.key)
  )
    return;
  event.preventDefault();
  event.stopPropagation();
  activate();
};
function columnIds<TData extends RowData>(
  columns: readonly ColumnDef<{}, TData, any>[],
): string[] {
  return columns.flatMap((column) => {
    if ("columns" in column && column.columns) return columnIds(column.columns);
    if (!column.id)
      throw new Error("Linked table columns require explicit stable IDs");
    return [column.id];
  });
}
/** Shared core table with optional native, single-target row/cell selection. */
export function FinstackTable<TData extends RowData, TValue = unknown>(
  props: FinstackTableProps<TData, TValue>,
) {
  // Native feature registration belongs to an instance and cannot change in place.
  return <TableBody key={props.link ? "linked" : "core"} {...props} />;
}
function TableBody<TData extends RowData, TValue>({
  data,
  columns,
  getRowId,
  caption,
  loading,
  error,
  emptyState = "No rows supplied",
  density,
  totals,
  link,
  getRowKey,
  getCellKey,
  getActiveCell,
  onRowActivate,
  onCellActivate,
}: FinstackTableProps<TData, TValue>) {
  const features: Partial<typeof linkedFeatures> = link
    ? linkedFeatures
    : coreFeatures;
  const explicitColumns = link ? columnIds(columns) : [];
  if (new Set(explicitColumns).size !== explicitColumns.length)
    throw new Error("Table column IDs must be unique");
  const selection = tableSelection(data, getRowId, explicitColumns, {
    link,
    getRowKey,
    getCellKey,
    getActiveCell,
  });
  const table = useTable<Partial<typeof linkedFeatures>, TData>({
    features,
    data,
    columns,
    getRowId,
    ...(link
      ? {
          ...selection,
          enableMultiRowSelection: false,
          enableSubRowSelection: false,
          enableRowRangeSelection: false,
          enableCellRangeSelection: false,
          enableMultiCellRangeSelection: false,
          enableCellSelectionDrag: false,
          autoResetCellSelection: false,
          enableRowSelection: (row: { original: TData }) =>
            getRowKey?.(row.original) != null,
          enableCellSelection: (cell: {
            row: { original: TData };
            column: { id: string };
          }) => getCellKey?.(cell.row.original, cell.column.id) != null,
        }
      : {}),
  });
  return (
    <div
      className="overflow-auto font-sans text-base text-foreground"
      aria-busy={loading}
      data-density={density}
    >
      <table
        className="w-full border-collapse text-sm"
        role={link ? "grid" : undefined}
        aria-multiselectable={link ? false : undefined}
      >
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
              className="h-[var(--row-height)] border-border [border-bottom-width:var(--table-rule-width)] data-[linked=true]:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
              data-linked={
                link &&
                link.selectedKey !== null &&
                getRowKey?.(row.original) === link.selectedKey
              }
              aria-selected={link ? row.getIsSelected() : undefined}
              tabIndex={
                getRowKey?.(row.original) != null || onRowActivate
                  ? 0
                  : undefined
              }
              onClick={(event) => {
                if (nestedControl(event)) return;
                if (link && row.getCanSelect()) row.toggleSelected(true);
                onRowActivate?.(row.original);
              }}
              onKeyDown={(event) =>
                keyActivation(event, () => {
                  if (link && row.getCanSelect()) row.toggleSelected(true);
                  onRowActivate?.(row.original);
                })
              }
            >
              {row.getAllCells().map((cell) => (
                <td
                  key={cell.id}
                  className={`px-2 data-[linked=true]:bg-accent data-[linked=true]:outline data-[linked=true]:outline-1 data-[linked=true]:outline-primary focus-visible:outline-2 focus-visible:outline-ring ${(cell.column.columnDef.meta as ColumnPresentation | undefined)?.className ?? ""}`}
                  data-linked={
                    link &&
                    link.selectedKey !== null &&
                    getCellKey?.(row.original, cell.column.id) ===
                      link.selectedKey
                  }
                  aria-selected={link ? cell.getIsSelected() : undefined}
                  tabIndex={
                    getCellKey?.(row.original, cell.column.id) != null
                      ? 0
                      : undefined
                  }
                  onClick={(event) => {
                    if (
                      nestedControl(event) ||
                      getCellKey?.(row.original, cell.column.id) == null
                    )
                      return;
                    event.stopPropagation();
                    if (link) table.setFocusedCell(row.id, cell.column.id);
                    onCellActivate?.({
                      row: row.original,
                      rowId: row.id,
                      columnId: cell.column.id,
                      value: cell.getValue() as TValue,
                    });
                  }}
                  onKeyDown={(event) => {
                    if (getCellKey?.(row.original, cell.column.id) == null)
                      return;
                    keyActivation(event, () => {
                      if (link) table.setFocusedCell(row.id, cell.column.id);
                      onCellActivate?.({
                        row: row.original,
                        rowId: row.id,
                        columnId: cell.column.id,
                        value: cell.getValue() as TValue,
                      });
                    });
                  }}
                >
                  <table.FlexRender cell={cell} />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
        {totals && (
          <tfoot>
            <tr>
              {table.getAllLeafColumns().map((column) => (
                <td
                  key={column.id}
                  className={`border-t border-border px-2 font-medium ${(column.columnDef.meta as ColumnPresentation | undefined)?.className ?? ""}`}
                >
                  {totals[column.id]}
                </td>
              ))}
            </tr>
          </tfoot>
        )}
      </table>
      {error ? (
        <p role="alert" className="text-error">
          {error}
        </p>
      ) : loading ? (
        <p role="status">Loading…</p>
      ) : data.length === 0 ? (
        <p role="status">{emptyState}</p>
      ) : null}
    </div>
  );
}
