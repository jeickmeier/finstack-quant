"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { covenants as covenantsWire } from "finstack-quant-wasm/types";
import { errorText } from "@/lib/finstack/form";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import type { FieldRenderer } from "@/components/finstack/core/components/schema-form/field-renderer";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { covenantEngineModule } from "./covenant-engine";
import { covenantTemplateModule } from "./covenant-template";

export interface CovenantEngineEditorProps {
  /** Initial CovenantEngine text; omitted starts empty (Rust publishes no example). Remount to load another document. */
  defaultJson?: string;
  /** Native validation returning canonical CovenantEngine text; connect useCovenantEngineValidator. */
  validate: SchemaFormProps["validate"];
  /** Receives the canonical engine on Use covenants; connect it to the useEvaluateCovenants request. */
  onSubmit(json: string): void;
  /** Loads a Rust package template's specs into the form; connect useCovenantTemplate. Omitted hides the templates. */
  loadTemplate?: (
    template: covenantsWire.CovenantTemplate,
  ) => Promise<covenantsWire.CovenantSpec[]>;
  /** Statement node ids the covenants are tested against: suggested for every metric id and checked in the metric list. */
  metricIds: readonly string[];
  disabled?: boolean;
  density?: "compact" | "comfortable";
}
// Metric id fields of top-level and windowed specs: free text with model node ids as suggestions.
const metricPath =
  /^(specs|windows\[\d+\]\.covenants)\[\d+\]\.(metric_id|denominator_metric_id)$/;
const message = (failure: unknown) =>
  failure instanceof Error ? failure.message : String(failure);
// Rust validates a template only by building it, which Load template does.
const structural = async (json: string) => json;
type Doc = {
  revision: number;
  value?: Record<string, unknown>;
  error?: string;
};
function parse(json: string): Record<string, unknown> {
  const value = covenantEngineModule.codec.parse(json);
  if (value === null || typeof value !== "object" || Array.isArray(value))
    throw new TypeError("A covenant engine must be a JSON object");
  return value as Record<string, unknown>;
}
/** One metric a spec reads, in engine order. */
type MetricRow = {
  key: string;
  where: string;
  covenant: string;
  field: "metric_id" | "denominator_metric_id";
  metric: string | null;
};
function metricRows(engine: Record<string, unknown>): MetricRow[] {
  const rows: MetricRow[] = [];
  const add = (where: string, specs: unknown) => {
    if (!Array.isArray(specs)) return;
    specs.forEach((spec: any, index) => {
      const type = spec?.covenant?.covenant_type;
      const covenant = String(
        spec?.covenant?.label ??
          (type && typeof type === "object" ? Object.keys(type)[0] : type),
      );
      for (const field of ["metric_id", "denominator_metric_id"] as const)
        if (field === "metric_id" || spec?.[field] != null)
          rows.push({
            key: `${where}-${index}-${field}`,
            where,
            covenant,
            field,
            metric: spec?.[field] ?? null,
          });
    });
  };
  add("Specs", engine.specs);
  if (Array.isArray(engine.windows))
    engine.windows.forEach((window: any) =>
      add(`Window ${window?.start} to ${window?.end}`, window?.covenants),
    );
  return rows;
}
/**
 * Edits a covenant engine with the generated schema form, optionally starting
 * from a Rust package template chosen and filled in the generated
 * CovenantTemplate form, whose threshold inputs start empty. Every metric id is free
 * text with the model's node ids as suggestions, and a list shows each spec's
 * metric id and whether the model has that node, before any evaluation.
 * Validation, templates and every term are native; nothing is defaulted here.
 */
export function CovenantEngineEditor({
  defaultJson,
  validate,
  onSubmit,
  loadTemplate,
  metricIds,
  disabled,
  density = "compact",
}: CovenantEngineEditorProps) {
  const [doc, setDoc] = useState<Doc>(() => {
    if (defaultJson === undefined) return { revision: 0 };
    try {
      return { revision: 0, value: parse(defaultJson) };
    } catch (failure) {
      return { revision: 0, error: message(failure) };
    }
  });
  const [input, setInput] = useState<string | null>(null);
  const known = useMemo(() => new Set(metricIds), [metricIds]);
  const rows = useMemo(() => {
    if (input === null) return null;
    try {
      return metricRows(parse(input));
    } catch {
      return null;
    }
  }, [input]);
  // A rejected template rejects the submission, which the template form shows.
  async function load(json: string) {
    const specs = await loadTemplate!(
      covenantTemplateModule.codec.parse(
        json,
      ) as covenantsWire.CovenantTemplate,
    );
    setInput(null);
    setDoc((current) => ({
      // finstack-arith: remount revision counter, not a value
      revision: current.revision + 1,
      value: { specs: specs as unknown[] },
    }));
  }
  const metricField: FieldRenderer = ({ form, path, label, location }) => {
    if (!metricPath.test(path)) return undefined;
    return (
      <form.AppField name={path}>
        {(field) => (
          <IdCombobox
            label={label}
            help={location.schema.description}
            path={path}
            error={errorText(field.state.meta.errors)}
            value={String(field.state.value ?? "")}
            onValueChange={field.handleChange}
            options={metricIds}
            disabled={disabled}
          />
        )}
      </form.AppField>
    );
  };
  const columns: ColumnDef<{}, MetricRow, any>[] = [
    {
      id: "where",
      header: "Tested",
      accessorFn: (row) => row.where,
      meta: { className: "", rowHeader: true },
    },
    { id: "covenant", header: "Covenant", accessorFn: (row) => row.covenant },
    {
      id: "field",
      header: "Field",
      accessorFn: (row) => row.field,
      meta: { className: "font-mono" },
    },
    {
      id: "metric",
      header: "Metric id",
      accessorFn: (row) => row.metric ?? "—",
      meta: { className: "font-mono" },
    },
    {
      id: "model",
      header: "In model",
      accessorFn: (row) =>
        row.metric === null
          ? "Not set: Rust reads the covenant type's conventional metric"
          : known.has(row.metric)
            ? "Yes"
            : "Missing",
      cell: ({ getValue }) =>
        getValue() === "Missing" ? (
          <span className="font-medium text-negative">Missing</span>
        ) : (
          getValue()
        ),
    },
  ];
  if (doc.error) return <p role="alert">{doc.error}</p>;
  return (
    <section
      aria-label="Covenant engine editor"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      {loadTemplate ? (
        <section aria-label="Covenant package templates" className="space-y-3">
          <h3 className="text-sm font-medium">Covenant package templates</h3>
          <p className="text-xs text-muted-foreground">
            Loading a template replaces the engine with the Rust package. Ratios
            are in turns (3.5 means 3.5x), yields and loan-to-value as decimals,
            amounts in the reporting currency; Rust validates every threshold.
          </p>
          <SchemaForm
            module={covenantTemplateModule}
            validate={structural}
            onSubmit={load}
            submitLabel="Load template"
            submitVariant="outline"
            layout="full"
          />
        </section>
      ) : null}
      <section aria-label="Covenant metrics" className="space-y-2">
        <h3 className="text-sm font-medium">Covenant metrics</h3>
        {rows ? (
          <FinstackTable
            data={rows}
            columns={columns}
            getRowId={(row) => row.key}
            caption="Metric each covenant reads and whether the model has that node"
            captionVisibility="sr-only"
            emptyState="The engine has no covenants"
            density={density}
          />
        ) : (
          <p className="text-xs text-muted-foreground">
            The metric list waits for a structurally valid engine.
          </p>
        )}
      </section>
      <SchemaForm
        key={doc.revision}
        module={covenantEngineModule}
        defaultValues={doc.value}
        validate={validate}
        onSubmit={onSubmit}
        onInputJson={setInput}
        renderField={metricField}
        submitLabel="Use covenants"
        layout="basic"
      />
    </section>
  );
}
