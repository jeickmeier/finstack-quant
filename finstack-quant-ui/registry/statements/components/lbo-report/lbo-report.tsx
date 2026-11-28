"use client";
import { useCallback, useState } from "react";
import type { MoneyValue as Money } from "finstack-quant-wasm";
import type { DatedLboRequestWire } from "@/lib/finstack/generated/types/dated_lbo_request";
import type { DatedLboResultWire } from "@/lib/finstack/generated/types/dated_lbo_result";
import { formatExact, formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { StatementCheckReport } from "@/components/finstack/statements/components/statement-diagnostics/statement-check-report";
import { lboRequestModule } from "./lbo-request";
import {
  Pairs,
  passThrough,
  ReportSection,
  UndefinedValue,
} from "@/components/finstack/shared/primitives/native-display/native-display";

export interface LboReportProps {
  /** Initial DatedLboRequest; remount to load another. Omitted starts empty. */
  defaultRequest?: DatedLboRequestWire;
  /** Model node ids offered for every node field of the request. */
  nodeIds: readonly string[];
  /** Receives the structurally valid DatedLboRequest JSON on Evaluate; Rust validates it. */
  onSubmit(requestJson: string): void;
  /** Native DatedLboResult; null while none is available. */
  result: DatedLboResultWire | null | undefined;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  /** Display text of a returned number; defaults to every supplied digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Edits a Rust DatedLboRequest with the schema form over the generated
 * contract (node fields as id-comboboxes over the model's nodes) and presents
 * the native DatedLboResult as returned: the funding reconciliation, the exit
 * bridge with its signed residual and floored sponsor proceeds, the sized and
 * funded MOIC, the statement run the figures come from, and the check report
 * when the request elects one. Nothing is calculated here; a return Rust does
 * not report is shown as undefined, and IRR is not part of the result.
 */
export function LboReport({
  defaultRequest,
  nodeIds,
  onSubmit,
  result,
  loading,
  error,
  formatValue = formatExact,
  density = "compact",
}: LboReportProps) {
  const [initial] = useState(() => defaultRequest);
  const nodeField = useCallback<NonNullable<SchemaFormProps["renderField"]>>(
    ({ form, path, label, value, location }) =>
      /(^|\.)([a-z_]+_)?node$/.test(path) &&
      location.schema.type === "string" ? (
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
  const money = (value: Money | null | undefined) =>
    value ? (
      <MoneyValue value={value} />
    ) : (
      <UndefinedValue reason="Rust reports this amount only when a sponsor commitment is supplied" />
    );
  const multiple = (value: number | null | undefined, reason: string) =>
    typeof value === "number" ? (
      <span className="finstack-numeric">{formatValue(value)}</span>
    ) : (
      <UndefinedValue reason={reason} />
    );
  const sized = (value: number | null | undefined) =>
    multiple(
      value,
      "Rust sizes a return only on positive required sponsor cash",
    );
  const funded = (value: number | null | undefined) =>
    multiple(
      value,
      "Rust reports a funded return only when the commitment closes the funding gap",
    );
  return (
    <section
      aria-label="Leveraged buyout"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      <SchemaForm
        module={lboRequestModule}
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
          <div className="grid grid-cols-1 gap-2 @lg:grid-cols-4">
            {(
              [
                [
                  "required_sponsor_cash",
                  money(result.funding.required_sponsor_cash),
                ],
                ["sponsor_proceeds", money(result.exit.sponsor_proceeds)],
                ["sized_moic", sized(result.exit.sized_moic)],
                ["funded_moic", funded(result.exit.funded_moic)],
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
            Returns are the multiples of invested capital Rust reports; IRR is
            not part of the native LBO result (pair the sponsor proceeds on the
            exit date with the required sponsor cash on the entry date in
            portfolio.mwrXirr).
          </p>
          <div className="grid gap-6 @lg:grid-cols-2">
            <ReportSection title="Sources and uses">
              <Pairs
                label="Returned sources and uses"
                rows={[
                  [labelFor("cash_uses"), money(result.funding.cash_uses)],
                  [
                    labelFor("debt_cash_proceeds"),
                    money(result.funding.debt_cash_proceeds),
                  ],
                  [
                    labelFor("opening_debt_face"),
                    money(result.funding.opening_debt_face),
                  ],
                  [
                    labelFor("required_sponsor_cash"),
                    money(result.funding.required_sponsor_cash),
                  ],
                  [
                    labelFor("committed_cash_sources"),
                    money(result.funding.committed_cash_sources),
                  ],
                  [labelFor("funding_gap"), money(result.funding.funding_gap)],
                ]}
              />
            </ReportSection>
            <ReportSection title="Entry and exit">
              <Pairs
                label="Returned entry and exit"
                rows={[
                  [labelFor("entry_date"), formatRaw(result.entry_date)],
                  [labelFor("entry_earnings"), money(result.entry_earnings)],
                  [
                    labelFor("entry_enterprise_value"),
                    money(result.entry_enterprise_value),
                  ],
                  [
                    labelFor("exit_valuation_date"),
                    formatRaw(result.exit_valuation_date),
                  ],
                  [labelFor("exit_earnings"), money(result.exit_earnings)],
                  [
                    labelFor("exit_enterprise_value"),
                    money(result.exit.enterprise_value),
                  ],
                  [labelFor("net_debt"), money(result.exit.net_debt)],
                  [labelFor("other_claims"), money(result.exit.other_claims)],
                  [
                    labelFor("common_equity_residual"),
                    money(result.exit.common_equity_residual),
                  ],
                  [
                    labelFor("sponsor_proceeds"),
                    money(result.exit.sponsor_proceeds),
                  ],
                  [labelFor("sized_moic"), sized(result.exit.sized_moic)],
                  [labelFor("funded_moic"), funded(result.exit.funded_moic)],
                ]}
              />
            </ReportSection>
          </div>
          <ReportSection title="Statement run">
            <Pairs
              label="Returned statement run"
              rows={[
                [labelFor("model_id"), formatRaw(result.run.model_id)],
                [
                  labelFor("source_snapshot"),
                  formatRaw(result.run.source_snapshot),
                ],
                [
                  labelFor("valuation_date"),
                  formatRaw(result.run.valuation_date),
                ],
                [
                  labelFor("model_content_hash"),
                  formatRaw(result.run.model_content_hash),
                ],
              ]}
            />
          </ReportSection>
          <ReportSection title="LBO checks">
            {result.checks ? (
              <StatementCheckReport report={result.checks} density={density} />
            ) : (
              <p className="text-sm text-muted-foreground">
                No check report: the request elects no check suite (checks).
              </p>
            )}
          </ReportSection>
        </>
      )}
    </section>
  );
}
