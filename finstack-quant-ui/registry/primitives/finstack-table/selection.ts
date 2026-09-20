import type {
  CellSelectionState,
  RowSelectionState,
  Updater,
} from "@tanstack/react-table";
import type { LinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";

export interface CellAddress {
  rowId: string;
  columnId: string;
}
export interface TableLinkProps<TData> {
  link?: LinkedSelection;
  /** Semantic correspondence, independent of the native render ID. Null means unmapped. */
  getRowKey?: (row: TData) => string | null;
  getCellKey?: (row: TData, columnId: string) => string | null;
  /** Required to create a native active cell. Many-to-one matches remain decorative. */
  getActiveCell?: (key: string) => CellAddress | null;
}
const resolve = <T>(updater: Updater<T>, current: T): T =>
  typeof updater === "function" ? (updater as (old: T) => T)(current) : updater;

/** Derive native state without retaining a second copy of the accepted key. */
export function tableSelection<TData>(
  data: readonly TData[],
  getRowId: (row: TData) => string,
  columnIds: readonly string[],
  props: TableLinkProps<TData>,
) {
  const { link, getRowKey, getCellKey, getActiveCell } = props;
  const rows = new Map(data.map((row) => [getRowId(row), row]));
  if (rows.size !== data.length)
    throw new Error("Table row IDs must be unique");
  const selected = link?.selectedKey ?? null;
  const rowSelection: RowSelectionState = Object.fromEntries(
    [...rows]
      .filter(([, row]) => selected !== null && getRowKey?.(row) === selected)
      .map(([id]) => [id, true]),
  );
  const cellKey = (address: CellAddress) => {
    if (!rows.has(address.rowId) || !columnIds.includes(address.columnId))
      return null;
    return getCellKey?.(rows.get(address.rowId)!, address.columnId) ?? null;
  };
  const address = selected === null ? null : getActiveCell?.(selected);
  const cellSelection: CellSelectionState =
    address && cellKey(address) === selected
      ? [
          {
            anchorRowId: address.rowId,
            focusRowId: address.rowId,
            anchorColumnId: address.columnId,
            focusColumnId: address.columnId,
          },
        ]
      : [];
  return {
    state: { rowSelection, cellSelection },
    onRowSelectionChange(updater: Updater<RowSelectionState>) {
      const next = resolve(updater, rowSelection);
      const keys = new Set(
        Object.entries(next)
          .filter(([, selected]) => selected)
          .flatMap(([id]) => (rows.has(id) ? [getRowKey?.(rows.get(id)!)] : []))
          .filter((key): key is string => key != null),
      );
      if (keys.size > 1)
        throw new Error("Linked tables accept one semantic target");
      link?.select(keys.values().next().value ?? null);
    },
    onCellSelectionChange(updater: Updater<CellSelectionState>) {
      const next = resolve(updater, cellSelection);
      if (next.length > 1)
        throw new Error("Linked tables do not accept cell ranges");
      const range = next[0];
      if (
        range &&
        (range.operation === "exclude" ||
          range.anchorRowId !== range.focusRowId ||
          range.anchorColumnId !== range.focusColumnId)
      )
        throw new Error("Linked tables do not accept cell ranges");
      link?.select(
        range
          ? cellKey({
              rowId: range.anchorRowId,
              columnId: range.anchorColumnId,
            })
          : null,
      );
    },
  };
}
