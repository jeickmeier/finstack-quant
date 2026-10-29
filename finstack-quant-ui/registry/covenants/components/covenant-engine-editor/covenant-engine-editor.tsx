"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { covenants as covenantsWire } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { errorText } from "@/lib/finstack/form";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import type { FieldRenderer } from "@/components/finstack/core/components/schema-form/field-renderer";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { covenantEngineModule } from "./covenant-engine";

/** A Rust covenant package template and its thresholds; the same shape useCovenantTemplate accepts. */
export type CovenantTemplate =
  | { kind: "covLite"; maxLeverage: number; maxSeniorLeverage: number }
  | {
      kind: "lboStandard";
      initialLeverage: number;
      interestCoverage: number;
      fixedChargeCoverage: number;
      maxCapex: number;
    }
  | {
      kind: "realEstate";
      minDscr: number;
      minDebtYield: number;
      maxLtv: number;
    }
  | {
      kind: "projectFinance";
      minDscr: number;
      distributionLockupDscr: number;
      minLiquidity: number;
      maxNetLeverage: number;
    };
export interface CovenantEngineEditorProps {
  /** Initial CovenantEngine text; omitted starts empty (Rust publishes no example). Remount to load another document. */
  defaultJson?: string;
  /** Native validation returning canonical CovenantEngine text; connect useCovenantEngineValidator. */
  validate: SchemaFormProps["validate"];
  /** Receives the canonical engine on Use covenants; connect it to the useEvaluateCovenants request. */
  onSubmit(json: string): void;
  /** Loads a Rust package template's specs into the form; connect useCovenantTemplate. Omitted hides the templates. */
  loadTemplate?: (
    template: CovenantTemplate,
  ) => Promise<covenantsWire.CovenantSpec[]>;
  /** Statement node ids the covenants are tested against: suggested for every metric id and checked in the metric list. */
  metricIds: readonly string[];
  disabled?: boolean;
  density?: "compact" | "comfortable";
}
type Kind = CovenantTemplate["kind"];
// The editor's own template choices, with the facade's parameter names in order.
const templates: readonly {
  kind: Kind;
  label: string;
  description: string;
  parameters: readonly string[];
}[] = [
  {
    kind: "covLite",
    label: "Covenant-lite",
    description: "covenants.covLite",
    parameters: ["max_leverage", "max_senior_leverage"],
  },
  {
    kind: "lboStandard",
    label: "LBO standard",
    description: "covenants.lboStandard",
    parameters: [
      "initial_leverage",
      "interest_coverage",
      "fixed_charge_coverage",
      "max_capex",
    ],
  },
  {
    kind: "realEstate",
    label: "Real estate",
    description: "covenants.realEstate",
    parameters: ["min_dscr", "min_debt_yield", "max_ltv"],
  },
  {
    kind: "projectFinance",
    label: "Project finance",
    description: "covenants.projectFinance",
    parameters: [
      "min_dscr",
      "distribution_lockup_dscr",
      "min_liquidity",
      "max_net_leverage",
    ],
  },
];
const camel = (name: string) =>
  name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());
// Metric id fields of top-level and windowed specs: free text with model node ids as suggestions.
const metricPath =
  /^(specs|windows\[\d+\]\.covenants)\[\d+\]\.(metric_id|denominator_metric_id)$/;
const message = (failure: unknown) =>
  failure instanceof Error ? failure.message : String(failure);
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
 * from a Rust package template (covenant-lite, LBO standard, real estate or
 * project finance) whose threshold inputs start empty. Every metric id is free
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
  const [kind, setKind] = useState<Kind>("covLite");
  const [thresholds, setThresholds] = useState<Record<string, string>>({});
  const [templateError, setTemplateError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [input, setInput] = useState<string | null>(null);
  const template = templates.find((entry) => entry.kind === kind)!;
  const known = useMemo(() => new Set(metricIds), [metricIds]);
  const rows = useMemo(() => {
    if (input === null) return null;
    try {
      return metricRows(parse(input));
    } catch {
      return null;
    }
  }, [input]);
  async function load() {
    if (!loadTemplate) return;
    setLoading(true);
    setTemplateError(null);
    try {
      // A blank threshold is NaN, for Rust to reject.
      const request = Object.fromEntries([
        ["kind", kind],
        ...template.parameters.map((name) => {
          const text = thresholds[`${kind}.${name}`] ?? "";
          return [camel(name), text === "" ? Number.NaN : Number(text)];
        }),
      ]) as CovenantTemplate;
      const specs = await loadTemplate(request);
      setInput(null);
      setDoc((current) => ({
        revision: current.revision + 1,
        value: { specs: specs as unknown[] },
      }));
    } catch (failure) {
      setTemplateError(message(failure));
    } finally {
      setLoading(false);
    }
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
        <form
          aria-label="Covenant package templates"
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            void load();
          }}
        >
          <h3 className="text-sm font-medium">Covenant package templates</h3>
          <p className="text-xs text-muted-foreground">
            Loading a template replaces the engine with the Rust package. Ratios
            are in turns (3.5 means 3.5x), yields and loan-to-value as decimals,
            amounts in the reporting currency; Rust validates every threshold.
          </p>
          <div className="grid items-end gap-3 @lg:grid-cols-3">
            <EnumField
              label="Template"
              value={kind}
              onValueChange={(value) => setKind(value as Kind)}
              options={templates.map(({ kind, label, description }) => ({
                value: kind,
                label,
                description,
              }))}
              disabled={disabled || loading}
            />
            {template.parameters.map((name) => (
              <DecimalInput
                key={`${kind}.${name}`}
                label={labelFor(name)}
                path={`${kind}.${name}`}
                value={thresholds[`${kind}.${name}`] ?? ""}
                onValueChange={(value) =>
                  setThresholds((current) => ({
                    ...current,
                    [`${kind}.${name}`]: value,
                  }))
                }
                disabled={disabled || loading}
              />
            ))}
          </div>
          <Button
            type="submit"
            variant="outline"
            disabled={disabled || loading}
          >
            Load template
          </Button>
          {templateError ? (
            <p role="alert" className="text-error">
              {templateError}
            </p>
          ) : null}
        </form>
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
