"use client";
import { useMemo, useState } from "react";
import type { StatementResultJson } from "finstack-quant-wasm";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import {
  adaptStatementResult,
  reportStatementCell,
  statementCellKey,
  statementRows,
} from "./projection";

type Edit = { nodeId: string; periodId: string; draft: string };

/** A report-style, period-aligned statement with optional inline edits of explicit values. */
export function StatementGrid({
  model,
  result,
  link,
  nodeIds,
  title = "Statement results",
  showRaw = true,
  displayMode = "exact",
  showType = true,
  activePeriod,
  onEditValue,
}: {
  model: FinancialModelSpecWire;
  result: StatementResultJson;
  link?: LinkedSelection;
  /** Supplied line IDs in display order. Omit to show every returned line. */
  nodeIds?: string[];
  title?: string;
  showRaw?: boolean;
  /** Millions affect presentation only. Edits always use exact model units. */
  displayMode?: "exact" | "millions";
  showType?: boolean;
  activePeriod?: string | null;
  /** Return an error message on rejection, or null when WASM accepts the edit. */
  onEditValue?: (
    nodeId: string,
    periodId: string,
    value: string | null,
  ) => Promise<string | null>;
}) {
  const [edit, setEdit] = useState<Edit | null>(null);
  const [editError, setEditError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const presentation = useMemo(() => {
    try {
      const checked = adaptStatementResult(result);
      const rows = statementRows(model, checked);
      return {
        checked,
        rows: nodeIds
          ? nodeIds.flatMap((id) => rows.filter((row) => row.nodeId === id))
          : rows,
      };
    } catch (error) {
      return { error: error instanceof Error ? error.message : String(error) };
    }
  }, [model, result, nodeIds]);
  if (presentation.error)
    return (
      <p role="alert">Statement result unavailable: {presentation.error}</p>
    );
  const rows = presentation.rows!;
  const checked = presentation.checked!;
  const monetaryCurrencies = new Set(
    rows.flatMap((row) =>
      row.valueType.startsWith("Monetary · ") ? [row.valueType.slice(11)] : [],
    ),
  );
  const hasScalar = rows.some((row) =>
    row.valueType.toLowerCase().includes("scalar"),
  );
  const unitLabel =
    monetaryCurrencies.size === 0
      ? "Ratios in native units"
      : `${monetaryCurrencies.size === 1 ? [...monetaryCurrencies][0] : "Monetary amounts"}${displayMode === "millions" ? " in millions" : monetaryCurrencies.size === 1 ? " · exact amounts" : " at exact values"}${hasScalar ? " · ratios in native units" : ""}`;
  const groups: { label: string; count: number }[] = [];
  for (const period of model.periods) {
    const label = period.is_actual ? "Historical" : "Forecast";
    const last = groups.at(-1);
    if (last?.label === label) last.count += 1;
    else groups.push({ label, count: 1 });
  }
  const save = async (value: string | null) => {
    if (!edit || !onEditValue) return;
    setSaving(true);
    const error = await onEditValue(edit.nodeId, edit.periodId, value);
    setSaving(false);
    if (error) setEditError(error);
    else {
      setEdit(null);
      setEditError(null);
    }
  };
  return (
    <section aria-label={title} className="space-y-2 font-sans text-foreground">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h3 className="text-base font-semibold tracking-tight">{title}</h3>
        <span className="text-[11px] uppercase tracking-widest text-muted-foreground">
          {unitLabel}
        </span>
      </div>
      <div className="overflow-x-auto border-y border-foreground/70 bg-card">
        <table className="w-full min-w-max border-collapse text-sm tabular-nums">
          <caption className="sr-only">{title} by reporting period</caption>
          <thead>
            <tr className="border-b border-border text-[10px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
              <th
                rowSpan={2}
                scope="col"
                className="sticky left-0 z-20 min-w-48 bg-card px-3 py-2 text-left font-semibold"
              >
                Line item
              </th>
              {showType && (
                <th rowSpan={2} scope="col" className="px-3 py-2 text-left">
                  Type
                </th>
              )}
              {groups.map((group, index) => (
                <th
                  key={`${group.label}-${index}`}
                  colSpan={group.count}
                  scope="colgroup"
                  className={`border-l px-3 py-2 text-center ${group.label === "Forecast" ? "bg-primary/5" : ""}`}
                >
                  {group.label}
                </th>
              ))}
            </tr>
            <tr className="border-b border-foreground/50 text-xs">
              {model.periods.map((period) => (
                <th
                  key={period.id}
                  scope="col"
                  aria-label={`${period.id} ${period.is_actual ? "Historical" : "Forecast"}`}
                  className={`min-w-28 px-3 py-2 text-right font-semibold ${period.is_actual ? "" : "bg-primary/5"} ${activePeriod === period.id ? "text-primary underline underline-offset-4" : ""}`}
                >
                  {period.id}
                  <span className="sr-only">
                    {" "}
                    {period.is_actual ? "Historical" : "Forecast"}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const node = model.nodes[row.nodeId];
              const emphasized = node?.tags?.some(
                (tag) => tag === "headline" || tag === "covenant",
              );
              return (
                <tr
                  key={row.nodeId}
                  className={`border-b border-border/60 last:border-b-0 ${emphasized ? "border-t border-t-foreground/50 font-semibold" : ""}`}
                >
                  <th
                    scope="row"
                    className={`sticky left-0 z-10 bg-card px-3 py-2 text-left font-sans ${emphasized ? "font-semibold" : "font-normal"}`}
                  >
                    {row.name}
                  </th>
                  {showType && (
                    <td className="px-3 py-2 text-xs text-muted-foreground">
                      {row.valueType}
                    </td>
                  )}
                  {model.periods.map((period) => {
                    const cell = row.cells[period.id];
                    const key = statementCellKey(row.nodeId, period.id);
                    const selected = link?.selectedKey === key;
                    const editing =
                      edit?.nodeId === row.nodeId &&
                      edit.periodId === period.id;
                    const editable = Boolean(
                      onEditValue && node && node.node_type !== "calculated",
                    );
                    const explicit = node?.values?.[period.id];
                    const unit =
                      node?.value_type?.type === "monetary"
                        ? node.value_type.currency
                        : null;
                    return (
                      <td
                        key={period.id}
                        className={`h-10 border-l border-border/30 px-3 text-right font-mono text-xs ${period.is_actual ? "" : "bg-primary/[0.035]"} ${activePeriod === period.id ? "bg-primary/[0.075]" : ""} ${selected ? "outline outline-1 outline-inset outline-primary" : ""}`}
                      >
                        {editing ? (
                          <div className="min-w-40 py-1 text-left">
                            <div className="flex items-center gap-1">
                              <input
                                autoFocus
                                aria-label={`${row.name} ${period.id} exact value`}
                                className="w-32 rounded border border-primary bg-background px-2 py-1 text-right font-mono text-xs"
                                inputMode="decimal"
                                value={edit.draft}
                                onChange={(event) =>
                                  setEdit({
                                    ...edit,
                                    draft: event.target.value,
                                  })
                                }
                                onKeyDown={(event) => {
                                  if (event.key === "Enter")
                                    void save(edit.draft);
                                  if (event.key === "Escape") {
                                    setEdit(null);
                                    setEditError(null);
                                  }
                                }}
                                disabled={saving}
                              />
                              <button
                                type="button"
                                className="rounded px-1 text-primary"
                                aria-label={`Save ${row.name} ${period.id}`}
                                onClick={() => void save(edit.draft)}
                                disabled={saving}
                              >
                                ✓
                              </button>
                              <button
                                type="button"
                                className="rounded px-1 text-muted-foreground"
                                aria-label={`Cancel ${row.name} ${period.id}`}
                                onClick={() => {
                                  setEdit(null);
                                  setEditError(null);
                                }}
                              >
                                ×
                              </button>
                            </div>
                            <span className="text-[10px] text-muted-foreground">
                              Exact {unit ?? "scalar"} value
                            </span>
                            {explicit !== undefined &&
                              node?.node_type === "mixed" && (
                                <button
                                  type="button"
                                  className="ml-2 text-[10px] text-primary underline"
                                  onClick={() => void save(null)}
                                >
                                  Use forecast
                                </button>
                              )}
                            {editError && (
                              <p
                                role="alert"
                                className="text-[10px] text-error"
                              >
                                {editError}
                              </p>
                            )}
                          </div>
                        ) : (
                          <button
                            type="button"
                            aria-label={`${row.name} ${period.id}${editable ? ", edit value" : ", inspect value"}`}
                            title={
                              editable
                                ? `${cell?.text ?? "—"} · click to edit exact ${unit ?? "scalar"} value`
                                : cell?.text
                            }
                            className={`w-full py-2 text-right ${editable ? "cursor-text hover:text-primary hover:underline" : "cursor-default"}`}
                            onClick={() => {
                              link?.select(key);
                              if (editable) {
                                setEditError(null);
                                setEdit({
                                  nodeId: row.nodeId,
                                  periodId: period.id,
                                  draft:
                                    explicit === undefined
                                      ? ""
                                      : typeof explicit === "number"
                                        ? String(explicit)
                                        : explicit.amount,
                                });
                              }
                            }}
                          >
                            {reportStatementCell(cell, displayMode)}
                          </button>
                        )}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {showRaw && (
        <details>
          <summary>Evaluation metadata</summary>
          <JsonViewer
            label="Evaluation metadata"
            text={serializeHost(checked.meta)}
          />
        </details>
      )}
      {showRaw && (
        <details>
          <summary>Complete statement result</summary>
          <JsonViewer
            label="Statement result JSON"
            text={serializeHost(checked)}
            downloadName="statement-result.json"
          />
        </details>
      )}
    </section>
  );
}
