"use client";
import type { scenarios as scenarioWire } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  EnumField,
  type EnumOption,
} from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
/** One parallel curve shift as edited: the curve family, one market curve id and basis points as typed. */
export interface MarketScenarioShiftDraft {
  curveKind: scenarioWire.CurveKind;
  curveId: string;
  /** Basis points exactly as typed; Rust validates the number. */
  bp: string;
}
/** Editable market scenario; an empty option is omitted so Rust applies its default. */
export interface MarketScenarioDraft {
  id: string;
  shifts: MarketScenarioShiftDraft[];
  /** Time roll; null means no roll. The period is tenor text such as "1M". */
  roll: { period: string; rollMode: scenarioWire.TimeRollMode | "" } | null;
  hazardBumpMode: scenarioWire.HazardBumpMode | "";
}
export interface MarketScenarioBuilderProps {
  value: MarketScenarioDraft;
  onValueChange: (value: MarketScenarioDraft) => void;
  /** Curve ids offered as suggestions for every curve family; ids stay free text. */
  curveIds: readonly string[];
  /** Canonical ScenarioSpec text returned by Rust for the current draft. */
  specJson?: string | null;
  loading?: boolean;
  /** Native error for the current draft, shown as returned. */
  error?: string;
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
const hazardModes = Object.keys({
  solve_to_par: true,
  first_order_shift: true,
} satisfies Record<scenarioWire.HazardBumpMode, true>);
const options = (values: readonly string[]): EnumOption[] =>
  values.map((value) => ({ value, label: value }));
const withDefault = (values: readonly string[]): EnumOption[] => [
  { value: "", label: "Default", description: "Omitted; Rust's default" },
  ...options(values),
];
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
/**
 * Controlled editor of a market-only scenario: parallel curve shifts, an
 * optional time roll and the hazard bump mode. It calls no worker; the host
 * builds the canonical spec through Rust and passes it back as `specJson`.
 */
export function MarketScenarioBuilder({
  value,
  onValueChange,
  curveIds,
  specJson,
  loading,
  error,
  disabled,
  density,
}: MarketScenarioBuilderProps) {
  const setShift = (index: number, shift: MarketScenarioShiftDraft) =>
    onValueChange({
      ...value,
      shifts: value.shifts.map((current, at) =>
        at === index ? shift : current,
      ),
    });
  return (
    <div
      className="finstack-market-scenario flex flex-col gap-4 font-sans text-sm text-foreground"
      data-density={density}
    >
      <TextField
        label="Scenario id"
        value={value.id}
        onValueChange={(id) => onValueChange({ ...value, id })}
        disabled={disabled}
      />
      <fieldset className="flex flex-col gap-3">
        <legend className="text-sm font-medium">Parallel shifts</legend>
        {value.shifts.length === 0 && (
          <p className="text-xs text-muted-foreground">No curve shifts.</p>
        )}
        {value.shifts.map((shift, index) => (
          <div
            key={index}
            role="group"
            aria-label={`Shift ${index + 1}`}
            className="grid items-end gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,8rem)_auto]"
          >
            <EnumField
              label={`Shift ${index + 1} curve kind`}
              value={shift.curveKind}
              onValueChange={(curveKind) =>
                setShift(index, {
                  ...shift,
                  curveKind: curveKind as scenarioWire.CurveKind,
                })
              }
              options={options(curveKinds)}
              layout="select"
              disabled={disabled}
            />
            <IdCombobox
              label={`Shift ${index + 1} curve id`}
              value={shift.curveId}
              onValueChange={(curveId) =>
                setShift(index, { ...shift, curveId })
              }
              options={curveIds}
              disabled={disabled}
            />
            <DecimalInput
              label={`Shift ${index + 1} size`}
              value={shift.bp}
              onValueChange={(bp) => setShift(index, { ...shift, bp })}
              suffix="bp"
              disabled={disabled}
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={disabled}
              onClick={() =>
                onValueChange({
                  ...value,
                  shifts: value.shifts.filter((_, at) => at !== index),
                })
              }
            >
              Remove
            </Button>
          </div>
        ))}
        <div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={disabled}
            onClick={() =>
              onValueChange({
                ...value,
                shifts: [
                  ...value.shifts,
                  { curveKind: "discount", curveId: "", bp: "" },
                ],
              })
            }
          >
            Add shift
          </Button>
        </div>
      </fieldset>
      <fieldset className="flex flex-col gap-3">
        <legend className="text-sm font-medium">Time roll</legend>
        {value.roll ? (
          <div className="grid items-end gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]">
            <TextField
              label="Roll period"
              value={value.roll.period}
              placeholder="1M"
              onValueChange={(period) =>
                onValueChange({ ...value, roll: { ...value.roll!, period } })
              }
              disabled={disabled}
            />
            <EnumField
              label="Roll mode"
              value={value.roll.rollMode}
              onValueChange={(rollMode) =>
                onValueChange({
                  ...value,
                  roll: {
                    ...value.roll!,
                    rollMode: rollMode as scenarioWire.TimeRollMode | "",
                  },
                })
              }
              options={withDefault(rollModes)}
              layout="radio"
              disabled={disabled}
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={disabled}
              onClick={() => onValueChange({ ...value, roll: null })}
            >
              Remove roll
            </Button>
          </div>
        ) : (
          <div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={disabled}
              onClick={() =>
                onValueChange({
                  ...value,
                  roll: { period: "", rollMode: "" },
                })
              }
            >
              Add roll
            </Button>
          </div>
        )}
      </fieldset>
      <EnumField
        label="Hazard bump mode"
        help="How par CDS shifts rewrite hazard curves."
        value={value.hazardBumpMode}
        onValueChange={(mode) =>
          onValueChange({
            ...value,
            hazardBumpMode: mode as scenarioWire.HazardBumpMode | "",
          })
        }
        options={withDefault(hazardModes)}
        layout="radio"
        disabled={disabled}
      />
      <JsonViewer
        label="Scenario spec"
        text={specJson ?? null}
        loading={loading}
        error={error}
        downloadName="scenario-spec.json"
        density={density}
      />
    </div>
  );
}
