import type { AccessorFnColumnDef, RowData } from "@tanstack/react-table";
import type { MoneyValue } from "finstack-quant-wasm";
import {
  formatRawMoney,
  formatRate,
  formatSigned,
  type RatePresentation,
} from "./format";

/** Formatting metadata read by the shared table; it adds no native table feature. */
export interface ColumnPresentation {
  className: string;
  unit?: string;
}
/** Core-only Table v9 column with explicit presentation metadata. */
export type FormattedColumn<
  TData extends RowData,
  TValue,
> = AccessorFnColumnDef<{}, TData, TValue> & {
  meta: ColumnPresentation;
};
/** Money column retaining currency on every row, including mixed-currency data. `displayText` only retrieves caller-precomputed text; no formatter runs during render. */
export function moneyColumn<TData extends RowData>(
  id: string,
  accessorFn: (row: TData) => MoneyValue,
  displayText?: (row: TData) => string | undefined,
): FormattedColumn<TData, MoneyValue> {
  return {
    id,
    header: id,
    accessorFn,
    meta: { className: "text-right finstack-numeric" },
    cell: (info) =>
      displayText?.(info.row.original) ?? formatRawMoney(info.getValue()),
  };
}
/** Source-backed rate presentation; absent metadata leaves values raw with unavailable units. */
export function rateColumn<TData extends RowData>(
  id: string,
  accessorFn: (row: TData) => string | number,
  presentation?: RatePresentation,
): FormattedColumn<TData, string | number> {
  return {
    id,
    header: id,
    accessorFn,
    meta: {
      className: "text-right finstack-numeric",
      unit: presentation?.display ?? "Unit unavailable",
    },
    cell: (info) => {
      const value = formatRate(info.getValue(), presentation);
      return value.unit ? `${value.text} ${value.unit}` : value.text;
    },
  };
}
/** ISO date column; epoch inputs must first pass through the native date adapter. */
export function dateColumn<TData extends RowData>(
  id: string,
  accessorFn: (row: TData) => string,
): FormattedColumn<TData, string> {
  return {
    id,
    header: id,
    accessorFn,
    meta: { className: "text-left finstack-numeric" },
    cell: (info) => info.getValue(),
  };
}
/** Identifier column with no interpretation or normalization of the supplied ID. */
export function idColumn<TData extends RowData>(
  id: string,
  accessorFn: (row: TData) => string,
): FormattedColumn<TData, string> {
  return {
    id,
    header: id,
    accessorFn,
    meta: { className: "text-left font-mono" },
    cell: (info) => info.getValue(),
  };
}
/** Supplied signed-value column; computes no difference, total or status colour. */
export function signedColumn<TData extends RowData>(
  id: string,
  accessorFn: (row: TData) => string | number | bigint,
): FormattedColumn<TData, string | number | bigint> {
  return {
    id,
    header: id,
    accessorFn,
    meta: { className: "text-right finstack-numeric" },
    cell: (info) => formatSigned(info.getValue()),
  };
}
