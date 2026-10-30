"use client";
import { useMemo, useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { mertonModule } from "./merton";

/** A native result or the native error of that one measure. */
type Outcome<T> =
  { ok: true; value: T } | { ok: false; error: { message: string } };
/** Where the model comes from: canonical MertonModel JSON, or a KMV equity calibration. */
export type MertonCalculatorSource =
  | { kind: "model"; json: string }
  | {
      kind: "equity";
      equityValue: number;
      equityVol: number;
      shortTermDebt: number;
      longTermDebt: number;
      riskFreeRate: number;
      payoutRate: number;
      maturity: number;
    };
/** The request the calculator submits; the worker's MertonReportRequest. */
export interface MertonCalculatorRequest {
  source: MertonCalculatorSource;
  horizons: number[];
  maturity: number;
  recovery: number;
}
/** The native report the worker's mertonReport returns. */
export interface MertonReportData {
  model: modelsWire.MertonModel;
  defaultPoint: number | null;
  horizons: readonly number[];
  defaultProbabilities: readonly number[];
  distanceToDefault: readonly number[];
  grades: readonly modelsWire.MasterScaleResult[];
  impliedSpread: Outcome<readonly number[]>;
  debtSpread: Outcome<readonly number[]>;
  maturity: number;
  recovery: number;
  cdsParSpread: Outcome<number>;
  /** [equity value, equity volatility] as Rust returns them. */
  impliedEquity: Outcome<readonly number[]>;
}
export interface MertonCalculatorProps {
  /** The initial inputs; remount to load another request. */
  defaultRequest: MertonCalculatorRequest;
  /** Native MertonModel validation returning canonical JSON; connect useMertonModelValidator. */
  validate: SchemaFormProps["validate"];
  /** Receives the request on Calculate; connect it to the useMertonReport request. */
  onSubmit(request: MertonCalculatorRequest): void;
  /** The returned report; null while none is available. */
  report: MertonReportData | null;
  loading?: boolean;
  /** The native error of the whole report, shown as returned. */
  error?: string;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
const equityFields = [
  [
    "equityValue",
    "Equity value",
    "Market value of equity, in the firm's monetary unit.",
  ],
  [
    "equityVol",
    "Equity volatility",
    "Annualized equity-return volatility, decimal.",
  ],
  [
    "shortTermDebt",
    "Short-term debt",
    "Liabilities due within one year; enter total debt here with no long-term debt for a total-debt barrier.",
  ],
  [
    "longTermDebt",
    "Long-term debt",
    "Liabilities maturing beyond one year; Rust's KMV default point takes half of it.",
  ],
  ["riskFreeRate", "Risk-free rate", "Continuously compounded, decimal."],
  ["payoutRate", "Payout rate", "Continuous payout yield on assets, decimal."],
  ["maturity", "Calibration maturity", "Years."],
] as const;
type EquityKey = (typeof equityFields)[number][0];
const text = (value: number) => String(value);
/** Typed text as a number; an empty field is NaN, never 0, so Rust rejects it. */
const typed = (value: string) =>
  value.trim() === "" ? Number.NaN : Number(value);
function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {children}
    </section>
  );
}
/**
 * Calculates a Merton structural-credit report: the model is edited with the
 * generated MertonModel form or calibrated from equity (KMV default point),
 * and the returned default probabilities, distances to default, spreads, CDS
 * par spread, implied equity and master-scale grades are presented as Rust
 * returned them. Validation and the calculation are the caller's native
 * callbacks; nothing is computed here.
 */
export function MertonCalculator({
  defaultRequest,
  validate,
  onSubmit,
  report,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: MertonCalculatorProps) {
  const initial = defaultRequest.source;
  const [kind, setKind] = useState(initial.kind);
  const [horizons, setHorizons] = useState(
    defaultRequest.horizons.map(text).join(", "),
  );
  const [maturity, setMaturity] = useState(text(defaultRequest.maturity));
  const [recovery, setRecovery] = useState(text(defaultRequest.recovery));
  const [equity, setEquity] = useState<Record<EquityKey, string>>(
    () =>
      Object.fromEntries(
        equityFields.map(([key]) => [
          key,
          initial.kind === "equity" ? text(initial[key]) : "",
        ]),
      ) as Record<EquityKey, string>,
  );
  const [modelValues] = useState(() => {
    if (initial.kind !== "model") return { value: undefined };
    try {
      const value = mertonModule.codec.parse(initial.json);
      if (value === null || typeof value !== "object" || Array.isArray(value))
        throw new TypeError("MertonModel must be a JSON object");
      return { value: value as Record<string, unknown> };
    } catch (failure) {
      return {
        error: failure instanceof Error ? failure.message : String(failure),
      };
    }
  });
  // Typed text becomes numbers as entered; Rust validates every value.
  const terms = () => ({
    horizons: horizons
      .split(",")
      .map((part) => part.trim())
      .filter((part) => part !== "")
      .map(typed),
    maturity: typed(maturity),
    recovery: typed(recovery),
  });
  if ("error" in modelValues) return <p role="alert">{modelValues.error}</p>;
  return (
    <section
      aria-label="Merton calculator"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <div className="grid gap-4 @lg:grid-cols-3">
        <FieldFrame
          label="Horizons"
          help="Years, comma-separated, in display order."
        >
          {(control) => (
            <Input
              {...control}
              value={horizons}
              onChange={(event) => setHorizons(event.target.value)}
            />
          )}
        </FieldFrame>
        <DecimalInput
          label="CDS maturity"
          help="Years; also the implied-equity maturity."
          value={maturity}
          onValueChange={setMaturity}
        />
        <DecimalInput
          label="Recovery"
          help="Fraction of par at default, for the implied and CDS spreads."
          value={recovery}
          onValueChange={setRecovery}
        />
      </div>
      <EnumField
        label="Model source"
        value={kind}
        onValueChange={(value) => setKind(value as typeof kind)}
        options={[
          { value: "model", label: "Model parameters" },
          { value: "equity", label: "Equity calibration (KMV)" },
        ]}
        layout="radio"
      />
      {kind === "model" ? (
        <SchemaForm
          module={mertonModule}
          defaultValues={modelValues.value}
          validate={validate}
          onSubmit={(json) =>
            onSubmit({ source: { kind: "model", json }, ...terms() })
          }
          submitLabel="Calculate"
          layout="full"
        />
      ) : (
        <form
          aria-label="Equity calibration"
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            onSubmit({
              source: {
                kind: "equity",
                ...(Object.fromEntries(
                  equityFields.map(([key]) => [key, typed(equity[key])]),
                ) as Record<EquityKey, number>),
              },
              ...terms(),
            });
          }}
        >
          <div className="grid gap-4 @lg:grid-cols-3">
            {equityFields.map(([key, label, help]) => (
              <DecimalInput
                key={key}
                label={label}
                help={help}
                value={equity[key]}
                onValueChange={(value) =>
                  setEquity((prior) => ({ ...prior, [key]: value }))
                }
              />
            ))}
          </div>
          <Button type="submit">Calculate</Button>
        </form>
      )}
      <MertonReportView
        report={report}
        loading={loading}
        error={error}
        format={formatValue}
        density={density}
      />
    </section>
  );
}
interface HorizonRow {
  horizon: number;
  index: number;
}
interface ParameterRow {
  name: string;
  value: unknown;
}
/** Finite numbers through the host format; anything else as returned. */
function shown(value: unknown, format: (value: number) => string) {
  if (typeof value === "number" && Number.isFinite(value)) return format(value);
  if (value !== null && typeof value === "object") return JSON.stringify(value);
  return formatRaw(value as string | number | null | undefined);
}
function MertonReportView({
  report,
  loading,
  error,
  format,
  density,
}: {
  report: MertonReportData | null;
  loading?: boolean;
  error?: string;
  format: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  const horizonColumns = useMemo<ColumnDef<{}, HorizonRow, any>[]>(() => {
    if (!report) return [];
    const numeric = (
      id: string,
      header: string,
      pick: (index: number) => unknown,
    ): ColumnDef<{}, HorizonRow, any> => ({
      id,
      header,
      accessorFn: (row) => pick(row.index),
      cell: ({ getValue }) => shown(getValue(), format),
      meta: { className: "text-right finstack-numeric" },
    });
    const spread = (outcome: Outcome<readonly number[]>) => (index: number) =>
      outcome.ok ? outcome.value[index] : "—";
    return [
      // The requested horizon, echoed as supplied rather than host-formatted.
      {
        id: "horizon",
        header: "Horizon (years)",
        accessorFn: (row) => row.horizon,
        cell: ({ getValue }) => formatRaw(getValue() as number),
        meta: { className: "text-right finstack-numeric" },
      },
      numeric(
        "default_probability",
        labelFor("default_probability"),
        (i) => report.defaultProbabilities[i],
      ),
      numeric(
        "distance_to_default",
        labelFor("distance_to_default"),
        (i) => report.distanceToDefault[i],
      ),
      numeric(
        "implied_spread",
        labelFor("implied_spread"),
        spread(report.impliedSpread),
      ),
      numeric(
        "debt_spread",
        labelFor("debt_spread"),
        spread(report.debtSpread),
      ),
      {
        id: "grade",
        header: labelFor("grade"),
        accessorFn: (row) => report.grades[row.index]?.grade,
        meta: { className: "text-left" },
      },
      numeric(
        "central_pd",
        labelFor("central_pd"),
        (i) => report.grades[i]?.central_pd,
      ),
    ];
  }, [report, format]);
  const parameterColumns = useMemo<ColumnDef<{}, ParameterRow, any>[]>(
    () => [
      {
        id: "name",
        header: "Parameter",
        accessorFn: (row) => row.name,
        meta: { className: "text-left" },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (row) => row.value,
        cell: ({ getValue }) => shown(getValue(), format),
        meta: { className: "text-right finstack-numeric" },
      },
    ],
    [format],
  );
  if (error)
    return (
      <p role="alert" className="text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="text-muted-foreground">
        Calculating Merton report…
      </p>
    );
  if (!report)
    return (
      <p role="status" className="text-muted-foreground">
        No Merton report
      </p>
    );
  const parameters: ParameterRow[] = [
    ...Object.entries(report.model).map(([key, value]) => ({
      name: labelFor(key),
      value,
    })),
    ...(report.defaultPoint === null
      ? []
      : [{ name: "KMV default point", value: report.defaultPoint }]),
  ];
  const errors = (
    [
      ["Implied spread", report.impliedSpread],
      ["Debt spread", report.debtSpread],
    ] as const
  ).flatMap(([name, outcome]) =>
    outcome.ok ? [] : [`${name}: ${outcome.error.message}`],
  );
  const tile = (label: string, body: ReactNode) => (
    <section
      key={label}
      aria-label={label}
      className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
    >
      <h4 className="text-xs font-medium text-muted-foreground">{label}</h4>
      {body}
    </section>
  );
  const value = (outcome: Outcome<number>) =>
    outcome.ok ? (
      <p className="finstack-numeric text-base">
        {shown(outcome.value, format)}
      </p>
    ) : (
      <p role="alert" className="text-xs text-error">
        {outcome.error.message}
      </p>
    );
  const equity = report.impliedEquity;
  const equityPart = (index: number): Outcome<number> =>
    equity.ok ? { ok: true, value: equity.value[index]! } : equity;
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
        {tile(
          `CDS par spread (${report.maturity}y, recovery ${report.recovery})`,
          value(report.cdsParSpread),
        )}
        {tile(
          `Implied equity value (${report.maturity}y)`,
          value(equityPart(0)),
        )}
        {tile(
          `Implied equity volatility (${report.maturity}y)`,
          value(equityPart(1)),
        )}
      </div>
      <p className="text-xs text-muted-foreground">
        Decimals as Rust returned them: spreads per year, probabilities and
        volatilities as fractions; grades from the default PD master scale.
      </p>
      <Section title="Term structure">
        <FinstackTable
          data={report.horizons.map((horizon, index) => ({ horizon, index }))}
          columns={horizonColumns}
          getRowId={(row) => String(row.index)}
          caption="Merton term structure by horizon"
          captionVisibility="sr-only"
          density={density}
          emptyState="No horizons returned"
        />
        {errors.map((message) => (
          <p key={message} role="alert" className="text-xs text-error">
            {message}
          </p>
        ))}
      </Section>
      <Section title="Model">
        <FinstackTable
          data={parameters}
          columns={parameterColumns}
          getRowId={(row) => row.name}
          caption="Canonical Merton model parameters"
          captionVisibility="sr-only"
          density={density}
          emptyState="No parameters returned"
        />
      </Section>
    </div>
  );
}
