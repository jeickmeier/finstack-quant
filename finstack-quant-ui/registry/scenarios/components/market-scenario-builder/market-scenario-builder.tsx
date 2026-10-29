"use client";
import { useState } from "react";
import type { ScenarioSpec, TemplateMetadata } from "finstack-quant-wasm";
import type { scenarios as scenarioWire } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { errorText } from "@/lib/finstack/form";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import type { FieldRenderer } from "@/components/finstack/core/components/schema-form/field-renderer";
import {
  EnumField,
  type EnumOption,
} from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { scenarioSpecModule } from "./scenario-spec";

/** Operations to build in Rust; the same shape useBuildMarketScenario accepts. */
export interface MarketScenarioOperationsRequest {
  id: string;
  shifts: {
    curveKind: scenarioWire.CurveKind;
    curveIds: string[];
    /** Basis points; NaN when the field is blank, for Rust to reject. */
    bp: number;
  }[];
  roll?: {
    period: string;
    rollMode: scenarioWire.TimeRollMode | null;
  } | null;
}
/** One component of a built-in composite template; the shape useBuildTemplateComponent accepts. */
export interface TemplateComponentChoice {
  templateId: string;
  componentId: string;
}
export interface MarketScenarioBuilderProps {
  /** Initial ScenarioSpec text; omitted starts from the schema's Rust example. Remount to load another document. */
  defaultJson?: string;
  /** Native validation returning canonical ScenarioSpec text; connect useScenarioSpecValidator. */
  validate: SchemaFormProps["validate"];
  /** Canonical spec after native validation, or null while the edit is pending or rejected. */
  onSpecChange?: (json: string | null) => void;
  /** Market ids offered as suggestions on curve, surface and equity id fields; ids stay free text. */
  curveIds: readonly string[];
  /** Built-in template metadata (useListBuiltinTemplateMetadata); omitted hides the template picker. */
  templates?: readonly TemplateMetadata[];
  /** Native error listing the templates. */
  templatesError?: string;
  /** Builds a whole template; connect useBuildFromTemplate. */
  buildFromTemplate?: (templateId: string) => Promise<ScenarioSpec>;
  /** Builds one component of a composite template; connect useBuildTemplateComponent. */
  buildTemplateComponent?: (
    request: TemplateComponentChoice,
  ) => Promise<ScenarioSpec>;
  /** Composes ScenarioSpec texts in Rust; connect useComposeScenarios. Omitted hides Compose. */
  composeScenarios?: (specJsons: readonly string[]) => Promise<ScenarioSpec>;
  /** Builds operations in Rust for the add actions; connect useBuildMarketScenario. Omitted hides them. */
  buildMarketScenario?: (
    request: MarketScenarioOperationsRequest,
  ) => Promise<string>;
  disabled?: boolean;
  density?: "compact" | "comfortable";
}
// Every wire value of each enum, checked exhaustively against the generated types.
const curveKinds = Object.keys({
  discount: true,
  forward: true,
  par_cds: true,
  inflation: true,
  commodity: true,
} satisfies Record<scenarioWire.CurveKind, true>) as scenarioWire.CurveKind[];
const rollModes = Object.keys({
  business_days: true,
  calendar_days: true,
  approximate: true,
} satisfies Record<scenarioWire.TimeRollMode, true>);
const options = (values: readonly string[]): EnumOption[] =>
  values.map((value) => ({ value, label: value }));
// Identifier fields of the OperationSpec variants: free text with market ids as suggestions.
const idPath =
  /^operations\[\d+\]\.(curve_id|discount_curve_id|vol_surface_id|surface_id|ids\[\d+\])$/;
const message = (failure: unknown) =>
  failure instanceof Error ? failure.message : String(failure);
type Doc = {
  revision: number;
  value?: Record<string, unknown>;
  error?: string;
};
function parse(json: string): Record<string, unknown> {
  const value = scenarioSpecModule.codec.parse(json);
  if (value === null || typeof value !== "object" || Array.isArray(value))
    throw new TypeError("A scenario specification must be a JSON object");
  return value as Record<string, unknown>;
}
function initial(defaultJson: string | undefined): Doc {
  if (defaultJson === undefined) return { revision: 0 };
  try {
    return { revision: 0, value: parse(defaultJson) };
  } catch (failure) {
    return { revision: 0, error: message(failure) };
  }
}
function TextField({
  label,
  value,
  onValueChange,
  disabled,
  placeholder,
}: {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
}) {
  return (
    <FieldFrame label={label}>
      {(control) => (
        <Input
          {...control}
          value={value}
          placeholder={placeholder}
          disabled={disabled}
          onChange={(event) => onValueChange(event.currentTarget.value)}
        />
      )}
    </FieldFrame>
  );
}
/** The selected template's returned metadata, shown as returned. */
function TemplateDetails({ template }: { template: TemplateMetadata }) {
  const rows: [string, string][] = [
    ["Description", template.description],
    ["Severity", template.severity],
    ["Event date", template.event_date],
    ["Asset classes", template.asset_classes.join(", ")],
    ["Tags", template.tags.join(", ")],
  ];
  return (
    <dl
      aria-label="Template details"
      className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs"
    >
      {rows.map(([term, detail]) => (
        <div key={term} className="contents">
          <dt className="text-muted-foreground">{term}</dt>
          <dd>{detail || "—"}</dd>
        </div>
      ))}
    </dl>
  );
}
/**
 * Full editor of a ScenarioSpec: built-in templates and their components
 * loaded or composed in Rust, add-parallel-shift and add-roll actions whose
 * operations Rust builds, and the generated schema form over every
 * operation. Validation, templates, composition and operations are native;
 * nothing is defaulted or computed here.
 */
export function MarketScenarioBuilder({
  defaultJson,
  validate,
  onSpecChange,
  curveIds,
  templates,
  templatesError,
  buildFromTemplate,
  buildTemplateComponent,
  composeScenarios,
  buildMarketScenario,
  disabled,
  density = "compact",
}: MarketScenarioBuilderProps) {
  const [doc, setDoc] = useState<Doc>(() => initial(defaultJson));
  // Structurally valid working text; null while an edit is pending or malformed.
  const [input, setInput] = useState<string | null>(null);
  const [canonical, setCanonical] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [templateId, setTemplateId] = useState("");
  const [componentId, setComponentId] = useState("");
  const [shift, setShift] = useState({
    curveKind: "discount" as scenarioWire.CurveKind,
    curveId: "",
    bp: "",
  });
  const [roll, setRoll] = useState({ period: "", rollMode: "" });
  const template = templates?.find((entry) => entry.id === templateId);
  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setActionError(null);
    try {
      const value = (await action()) as Record<string, unknown>;
      setInput(null);
      setCanonical(null);
      onSpecChange?.(null);
      setDoc((current) => ({ revision: current.revision + 1, value }));
    } catch (failure) {
      setActionError(message(failure));
    } finally {
      setBusy(false);
    }
  }
  const selected = (): Promise<ScenarioSpec> =>
    componentId
      ? buildTemplateComponent!({ templateId, componentId })
      : buildFromTemplate!(templateId);
  const append = (request: Omit<MarketScenarioOperationsRequest, "id">) =>
    run(async () => {
      const current = parse(input!);
      const built = parse(
        await buildMarketScenario!({ id: String(current.id), ...request }),
      );
      return {
        ...current,
        operations: [
          ...(current.operations as unknown[]),
          ...(built.operations as unknown[]),
        ],
      };
    });
  const idField: FieldRenderer = ({ form, path, label, location }) => {
    if (!idPath.test(path)) return undefined;
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
            options={curveIds}
            disabled={disabled}
          />
        )}
      </form.AppField>
    );
  };
  const locked = disabled || busy;
  const editable = input !== null;
  const canLoad =
    templateId !== "" &&
    (componentId ? buildTemplateComponent : buildFromTemplate) !== undefined;
  if (doc.error) return <p role="alert">{doc.error}</p>;
  return (
    <section
      aria-label="Market scenario builder"
      data-density={density}
      className="finstack-market-scenario @container flex flex-col gap-4 font-sans text-sm text-foreground"
    >
      {templates || templatesError ? (
        <section
          aria-label="Scenario templates"
          className="flex flex-col gap-3"
        >
          <h3 className="text-sm font-medium">Templates</h3>
          {templatesError ? (
            <p role="alert" className="text-error">
              {templatesError}
            </p>
          ) : null}
          <div className="grid items-end gap-2 @lg:grid-cols-2">
            <EnumField
              label="Template"
              value={templateId}
              onValueChange={(id) => {
                setTemplateId(id);
                setComponentId("");
              }}
              options={[
                { value: "", label: "None" },
                ...(templates ?? []).map((entry) => ({
                  value: entry.id,
                  label: entry.name,
                })),
              ]}
              layout="select"
              disabled={locked || !templates}
            />
            <EnumField
              label="Component"
              value={componentId}
              onValueChange={setComponentId}
              options={[
                { value: "", label: "Whole template" },
                ...options(template?.components ?? []),
              ]}
              layout="select"
              disabled={locked || !template?.components.length}
            />
          </div>
          {template ? <TemplateDetails template={template} /> : null}
          <p className="text-xs text-muted-foreground">
            Template curve, surface and equity ids are placeholders; map them to
            your market&apos;s ids before applying.
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={locked || !canLoad}
              onClick={() => run(selected)}
            >
              Load template
            </Button>
            {composeScenarios ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={locked || !canLoad || !editable}
                onClick={() =>
                  run(async () =>
                    composeScenarios([
                      input!,
                      JSON.stringify(await selected()),
                    ]),
                  )
                }
              >
                Compose with scenario
              </Button>
            ) : null}
          </div>
        </section>
      ) : null}
      {buildMarketScenario ? (
        <section aria-label="Add operations" className="flex flex-col gap-3">
          <h3 className="text-sm font-medium">Add operations</h3>
          <div
            role="group"
            aria-label="Parallel shift"
            className="grid items-end gap-2 @lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,8rem)_auto]"
          >
            <EnumField
              label="Curve kind"
              value={shift.curveKind}
              onValueChange={(curveKind) =>
                setShift({
                  ...shift,
                  curveKind: curveKind as scenarioWire.CurveKind,
                })
              }
              options={options(curveKinds)}
              layout="select"
              disabled={locked}
            />
            <IdCombobox
              label="Curve id"
              value={shift.curveId}
              onValueChange={(curveId) => setShift({ ...shift, curveId })}
              options={curveIds}
              disabled={locked}
            />
            <DecimalInput
              label="Shift size"
              value={shift.bp}
              onValueChange={(bp) => setShift({ ...shift, bp })}
              suffix="bp"
              disabled={locked}
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={locked || !editable}
              onClick={() =>
                append({
                  shifts: [
                    {
                      curveKind: shift.curveKind,
                      curveIds: [shift.curveId],
                      bp:
                        shift.bp.trim() === "" ? Number.NaN : Number(shift.bp),
                    },
                  ],
                })
              }
            >
              Add parallel shift
            </Button>
          </div>
          <div
            role="group"
            aria-label="Time roll"
            className="grid items-end gap-2 @lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]"
          >
            <TextField
              label="Roll period"
              value={roll.period}
              placeholder="1M"
              onValueChange={(period) => setRoll({ ...roll, period })}
              disabled={locked}
            />
            <EnumField
              label="Roll mode"
              value={roll.rollMode}
              onValueChange={(rollMode) => setRoll({ ...roll, rollMode })}
              options={[
                {
                  value: "",
                  label: "Default",
                  description: "Omitted; Rust's default",
                },
                ...options(rollModes),
              ]}
              layout="select"
              disabled={locked}
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={locked || !editable}
              onClick={() =>
                append({
                  shifts: [],
                  roll: {
                    period: roll.period,
                    rollMode:
                      (roll.rollMode as scenarioWire.TimeRollMode) || null,
                  },
                })
              }
            >
              Add roll
            </Button>
          </div>
        </section>
      ) : null}
      {(buildMarketScenario || composeScenarios) && !editable ? (
        <p className="text-xs text-muted-foreground">
          Adding and composing wait for a structurally valid scenario.
        </p>
      ) : null}
      {actionError ? (
        <p role="alert" className="text-error">
          {actionError}
        </p>
      ) : null}
      <SchemaForm
        key={doc.revision}
        module={scenarioSpecModule}
        defaultValues={doc.value}
        validate={validate}
        onSubmit={() => undefined}
        onInputJson={setInput}
        onValidated={(json) => {
          setCanonical(json);
          onSpecChange?.(json);
        }}
        renderField={idField}
        actions={false}
        layout="basic"
      />
      <JsonViewer
        label="Scenario spec"
        text={canonical}
        downloadName="scenario-spec.json"
        density={density}
      />
    </section>
  );
}
