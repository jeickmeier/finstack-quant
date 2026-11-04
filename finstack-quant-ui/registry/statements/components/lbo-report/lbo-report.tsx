"use client";
import { useCallback, useState } from "react";
import type { MoneyValue as Money } from "finstack-quant-wasm";
import type { LboResultWire } from "@/lib/finstack/generated/types/lbo_result";
import { formatExact } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { StatementCheckReport } from "@/components/finstack/statements/components/statement-diagnostics/statement-check-report";
import { lboConfigModule } from "./lbo-config";
import {
  Pairs,
  passThrough,
  ReportSection,
  yesNo,
} from "@/components/finstack/shared/primitives/native-display/native-display";

export interface LboReportProps {
  /** Initial LboConfig; remount to load another. Omitted starts empty. */
  defaultConfig?: Record<string, unknown>;
  /** Model node ids offered for every `*_node` field of the config. */
  nodeIds: readonly string[];
  /** Receives the structurally valid LboConfig JSON on Evaluate; Rust validates it. */
  onSubmit(configJson: string): void;
  /** Native LboResult; null while none is available. */
  result: LboResultWire | null | undefined;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  /** Display text of a returned number; defaults to every supplied digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Edits a Rust LboConfig with the schema form over the generated contract
 * (node fields as id-comboboxes over the model's nodes) and presents the
 * native LboResult as returned: entry valuation, sources and uses, exit
 * returns as MOIC, and the native LBO check report when the config maps
 * one. Nothing is calculated here; IRR is not part of the result.
 */
export function LboReport({
  defaultConfig,
  nodeIds,
  onSubmit,
  result,
  loading,
  error,
  formatValue = formatExact,
  density = "compact",
}: LboReportProps) {
  const [initial] = useState(() => defaultConfig);
  const nodeField = useCallback<NonNullable<SchemaFormProps["renderField"]>>(
    ({ form, path, label, value, location }) =>
      /(^|\.)[a-z_]+_node$/.test(path) && location.schema.type === "string" ? (
        <IdCombobox
          label={label}
          help={
            typeof location.schema.description === "string"
              ? location.schema.description
              : undefined
          }
          value={typeof value === "string" ? value : ""}
          onValueChange={(next) => form.setFieldValue(path, next)}
          options={nodeIds}
        />
      ) : undefined,
    [nodeIds],
  );
  const money = (value: Money | null | undefined) => (
    <MoneyValue value={value ?? null} />
  );
  return (
    <section
      aria-label="Leveraged buyout"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      <SchemaForm
        module={lboConfigModule}
        defaultValues={initial}
        layout="full"
        explicitNull={false}
        renderField={nodeField}
        validate={passThrough}
        onSubmit={onSubmit}
        submitLabel="Evaluate LBO"
      />
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !result ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Evaluating the LBO…" : "No LBO evaluation available."}
        </p>
      ) : (
        <>
          <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
            {(
              [
                ["equity_check", money(result.equity_check)],
                ["exit_equity_proceeds", money(result.exit_equity_proceeds)],
                [
                  "moic",
                  <span className="finstack-numeric">
                    {formatValue(result.moic)}
                  </span>,
                ],
              ] as const
            ).map(([name, body]) => (
              <section
                key={name}
                aria-label={labelFor(name)}
                className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
              >
                <h4 className="text-xs font-medium text-muted-foreground">
                  {labelFor(name)}
                </h4>
                <p className="text-base">{body}</p>
              </section>
            ))}
          </div>
          <p className="text-xs text-muted-foreground">
            Returns are the multiple of invested capital Rust reports; IRR is
            not part of the native LBO result (pair the exit equity proceeds
            with the equity check in portfolio.mwrXirr).
          </p>
          <div className="grid gap-6 @lg:grid-cols-2">
            <ReportSection title="Sources and uses">
              <Pairs
                label="Returned sources and uses"
                rows={[
                  [labelFor("debt_total"), money(result.debt_total)],
                  [labelFor("equity_check"), money(result.equity_check)],
                  [labelFor("sources_total"), money(result.sources_total)],
                  [
                    labelFor("entry_enterprise_value"),
                    money(result.entry_enterprise_value),
                  ],
                  [labelFor("uses_total"), money(result.uses_total)],
                  [
                    labelFor("sources_uses_balanced"),
                    yesNo(result.sources_uses_balanced),
                  ],
                ]}
              />
            </ReportSection>
            <ReportSection title="Entry and exit">
              <Pairs
                label="Returned entry and exit"
                rows={[
                  [labelFor("entry_metric"), formatValue(result.entry_metric)],
                  [
                    labelFor("entry_enterprise_value"),
                    money(result.entry_enterprise_value),
                  ],
                  [labelFor("exit_metric"), formatValue(result.exit_metric)],
                  [
                    labelFor("exit_enterprise_value"),
                    money(result.exit_enterprise_value),
                  ],
                  [labelFor("exit_net_debt"), money(result.exit_net_debt)],
                  [
                    labelFor("exit_equity_proceeds"),
                    money(result.exit_equity_proceeds),
                  ],
                  [labelFor("moic"), formatValue(result.moic)],
                ]}
              />
            </ReportSection>
          </div>
          {result.sources_uses_balanced === false && (
            <p role="alert" className="text-sm text-error">
              Rust reports that sources do not reconcile to uses: the capital
              structure is numerically degenerate.
            </p>
          )}
          <ReportSection title="LBO checks">
            {result.checks ? (
              <StatementCheckReport report={result.checks} density={density} />
            ) : (
              <p className="text-sm text-muted-foreground">
                No check report: the config maps no LBO checks (check_mappings).
              </p>
            )}
          </ReportSection>
        </>
      )}
    </section>
  );
}
