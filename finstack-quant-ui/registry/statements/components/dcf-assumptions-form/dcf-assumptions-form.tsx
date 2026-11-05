"use client";
import { useCallback, useState } from "react";
import type { statements_analytics as wire } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { dcfOptionsModule } from "./dcf-options";
import {
  passThrough,
  typedNumber,
} from "@/components/finstack/shared/primitives/native-display/native-display";

/** Rust TerminalValueSpec: Gordon growth, exit multiple or H-model. */
export type TerminalValueSpec = wire.TerminalValueSpec;
type TerminalType = TerminalValueSpec["type"];
/**
 * The TerminalValueSpec variants and their fields, in Rust's declaration
 * order. TerminalValueSpec is not a published contract root, so this wire
 * union is written here; Rust validates every value.
 */
const terminalFields: Record<TerminalType, readonly string[]> = {
  gordon_growth: ["stable_growth_rate"],
  exit_multiple: ["multiple", "terminal_metric"],
  h_model: ["high_growth_rate", "stable_growth_rate", "half_life_years"],
};
const terminalHelp: Record<string, string> = {
  stable_growth_rate: "Perpetual growth, decimal (0.02 = 2%); below the WACC.",
  multiple: "Turns of the terminal metric (10 = 10x).",
  terminal_metric:
    "Metric value the multiple applies to; Rust replaces it with the trailing year of the exit-multiple metric node when the options name one.",
  high_growth_rate: "Initial growth, decimal.",
  half_life_years: "Half-life of the growth fade, years.",
};
/** DCF assumptions as the caller supplies them; omitted optional values stay unset. */
export interface DcfAssumptions {
  /** Weighted average cost of capital as a decimal (0.10 = 10%). */
  wacc: number;
  terminalValue: TerminalValueSpec;
  /** Node of unlevered free cash flow; omitted uses Rust's `ufcf`. */
  ufcfNode?: string;
  /** Flat net debt in model currency in place of the model's debt and cash. */
  netDebtOverride?: number;
  /** Rust DcfOptions; every omitted field takes its Rust default. */
  options?: wire.DcfOptions;
}
/** The submitted assumptions, with the options as the JSON Rust will validate. */
export interface DcfAssumptionsSubmission {
  wacc: number;
  terminalValue: TerminalValueSpec;
  ufcfNode?: string;
  netDebtOverride?: number;
  optionsJson?: string;
}
export interface DcfAssumptionsFormProps {
  /** Initial assumptions; remount to load others. Omitted starts empty. */
  defaultAssumptions?: DcfAssumptions;
  /** Model node ids offered for the UFCF and exit-multiple metric nodes. */
  nodeIds: readonly string[];
  /** WACC field text when the host holds it, so a rate can be set without remounting the rest of the form; undefined, the form holds it and shows `defaultAssumptions.wacc`. Pass undefined, not `""`, until the host has text: `""` is an empty field. */
  wacc?: string;
  /** Receives each edit of a host-held WACC field. */
  onWaccChange?: (wacc: string) => void;
  /** Receives the assumptions on Calculate; Rust validates them when it values the model. */
  onSubmit(submission: DcfAssumptionsSubmission): void;
  submitLabel?: string;
  disabled?: boolean;
  density?: "compact" | "comfortable";
}
const text = (value: number | undefined) =>
  value === undefined ? "" : String(value);
/**
 * Edits the arguments of statements_analytics.evaluateDcf: WACC, the
 * terminal-value variant and its fields, the UFCF node, an optional net-debt
 * override and the generated DcfOptions form. Nothing is defaulted or
 * checked here beyond the options' structure; Rust validates every value
 * when it values the model and its error is shown by the caller.
 */
export function DcfAssumptionsForm({
  defaultAssumptions,
  nodeIds,
  wacc: hostWacc,
  onWaccChange,
  onSubmit,
  submitLabel = "Value",
  disabled,
  density = "compact",
}: DcfAssumptionsFormProps) {
  const initial = defaultAssumptions;
  const [ownWacc, setOwnWacc] = useState(text(initial?.wacc));
  const wacc = hostWacc ?? ownWacc;
  const [type, setType] = useState<TerminalType>(
    initial?.terminalValue.type ?? "gordon_growth",
  );
  // Field text per variant, so switching variants keeps what was typed.
  const [terminal, setTerminal] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      Object.entries(initial?.terminalValue ?? {})
        .filter(([key]) => key !== "type")
        .map(([key, value]) => [
          `${initial!.terminalValue.type}.${key}`,
          text(value as number),
        ]),
    ),
  );
  const [ufcfNode, setUfcfNode] = useState(initial?.ufcfNode ?? "");
  const [netDebt, setNetDebt] = useState(text(initial?.netDebtOverride));
  const [optionsJson, setOptionsJson] = useState<string | null>(null);
  const metricNode = useCallback<NonNullable<SchemaFormProps["renderField"]>>(
    ({ form, path, label, value }) =>
      path === "exit_multiple_metric_node" ? (
        <IdCombobox
          label={label}
          help="Flow node whose complete trailing year supplies the exit-multiple metric; empty leaves it unset."
          value={typeof value === "string" ? value : ""}
          onValueChange={(next) =>
            form.setFieldValue(path, next === "" ? undefined : next)
          }
          options={nodeIds}
        />
      ) : undefined,
    [nodeIds],
  );
  // The options SchemaForm renders its own <form>, so this editor is a
  // section and Calculate a plain button rather than an enclosing form.
  const submit = () => {
    if (optionsJson === null) return;
    onSubmit({
      wacc: typedNumber(wacc),
      terminalValue: {
        type,
        ...Object.fromEntries(
          terminalFields[type].map((field) => [
            field,
            typedNumber(terminal[`${type}.${field}`] ?? ""),
          ]),
        ),
      } as TerminalValueSpec,
      ...(ufcfNode.trim() === "" ? {} : { ufcfNode }),
      ...(netDebt.trim() === ""
        ? {}
        : { netDebtOverride: typedNumber(netDebt) }),
      ...(optionsJson === "{}" ? {} : { optionsJson }),
    });
  };
  return (
    <section
      aria-label="DCF assumptions"
      data-density={density}
      className="@container space-y-4 font-sans text-sm text-foreground"
    >
      <div className="grid gap-4 @lg:grid-cols-3">
        <DecimalInput
          label="WACC"
          help="Weighted average cost of capital, decimal (0.10 = 10%)."
          value={wacc}
          onValueChange={(value) => {
            setOwnWacc(value);
            onWaccChange?.(value);
          }}
        />
        <IdCombobox
          label="UFCF node"
          help="Monetary unlevered free cash flow in model currency; empty uses Rust's ufcf."
          value={ufcfNode}
          onValueChange={setUfcfNode}
          options={nodeIds}
        />
        <DecimalInput
          label="Net debt override"
          help="Flat net debt in model currency; empty derives it from the model's debt and cash."
          value={netDebt}
          onValueChange={setNetDebt}
        />
      </div>
      <fieldset className="space-y-3 rounded-md border border-border p-3">
        <legend className="px-1 text-sm font-medium">Terminal value</legend>
        <EnumField
          label="Method"
          layout="radio"
          value={type}
          onValueChange={(value) => setType(value as TerminalType)}
          options={(Object.keys(terminalFields) as TerminalType[]).map(
            (value) => ({ value, label: labelFor(value) }),
          )}
        />
        <div className="grid gap-4 @lg:grid-cols-3">
          {terminalFields[type].map((field) => (
            <DecimalInput
              key={`${type}.${field}`}
              label={labelFor(field)}
              help={terminalHelp[field]}
              value={terminal[`${type}.${field}`] ?? ""}
              onValueChange={(value) =>
                setTerminal((prior) => ({
                  ...prior,
                  [`${type}.${field}`]: value,
                }))
              }
            />
          ))}
        </div>
      </fieldset>
      <details className="rounded-md border border-border p-3">
        <summary className="cursor-pointer text-sm font-medium">
          DCF options
        </summary>
        <div className="pt-3">
          <SchemaForm
            module={dcfOptionsModule}
            defaultValues={initial?.options}
            layout="basic"
            actions={false}
            explicitNull={false}
            renderField={metricNode}
            validate={passThrough}
            onSubmit={() => {}}
            onInputJson={setOptionsJson}
          />
        </div>
      </details>
      <Button
        type="button"
        onClick={submit}
        disabled={disabled || optionsJson === null}
      >
        {submitLabel}
      </Button>
    </section>
  );
}
