"use client";
import { useState, type ReactNode } from "react";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { scoringModels, type ScoringContract } from "./scoring";

/** A native result or the native error of that one call. */
type Outcome<T> =
  { ok: true; value: T } | { ok: false; error: { message: string } };
/** A models.credit.scoring function name. */
export type CreditScorecardModel = (typeof scoringModels)[number]["model"];
/** The request the scorecard submits; the worker's CreditScoreRequest. */
export type CreditScorecardRequest =
  | { model: "altmanZScore"; input: modelsWire.AltmanZScoreInput }
  | { model: "altmanZPrime"; input: modelsWire.AltmanZPrimeInput }
  | {
      model: "altmanZDoublePrime" | "altmanEmScore";
      input: modelsWire.AltmanZDoublePrimeInput;
    }
  | { model: "ohlsonOScore"; input: modelsWire.OhlsonOScoreInput }
  | { model: "zmijewskiScore"; input: modelsWire.ZmijewskiInput };
/** The native score the worker's creditScore returns. */
export interface CreditScoreData {
  model: CreditScorecardModel;
  result: modelsWire.ScoringResult;
  /** The default PD master-scale grade, or Rust's error (a result without an implied PD has no grade). */
  grade: Outcome<modelsWire.MasterScaleResult>;
}
export interface CreditScorecardProps {
  /** The model selected first; remount to load another. */
  defaultModel: CreditScorecardModel;
  /** Starting ratios per model, keyed by Rust field name; a model without them starts empty. */
  defaultInputs?: Partial<
    Record<CreditScorecardModel, Readonly<Record<string, unknown>>>
  >;
  /** Receives the request on Score; connect it to the useCreditScore request. */
  onSubmit(request: CreditScorecardRequest): void;
  /** The returned score; null while none is available. */
  score: CreditScoreData | null;
  loading?: boolean;
  /** The native error of the whole call, shown as returned. */
  error?: string;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/** Typed text as a number; an empty field is NaN, never 0, so Rust rejects it. */
const typed = (value: string) =>
  value.trim() === "" ? Number.NaN : Number(value);
/** The first paragraph of a Rust field description. */
const summary = (text: string | undefined) => text?.split("\n\n")[0];
const contractOf = (model: CreditScorecardModel): ScoringContract =>
  scoringModels.find((entry) => entry.model === model)!.contract;
/**
 * Scores a firm with one of the accounting-based credit models of
 * models.credit.scoring: the ratios are the fields of the generated input
 * contract (Rust order and descriptions), and the returned score, zone,
 * implied PD and default master-scale grade are presented as Rust returned
 * them. The calculation is the caller's native callback; nothing is computed
 * or classified here.
 */
export function CreditScorecard({
  defaultModel,
  defaultInputs = {},
  onSubmit,
  score,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: CreditScorecardProps) {
  const [model, setModel] = useState<CreditScorecardModel>(defaultModel);
  // Typed text per model, so switching models keeps every model's entries.
  const [entries, setEntries] = useState<
    Record<CreditScorecardModel, Record<string, string>>
  >(
    () =>
      Object.fromEntries(
        scoringModels.map(({ model, contract }) => [
          model,
          Object.fromEntries(
            contract.required.map((field) => {
              const value = defaultInputs[model]?.[field];
              return [field, value === undefined ? "" : String(value)];
            }),
          ),
        ]),
      ) as Record<CreditScorecardModel, Record<string, string>>,
  );
  const contract = contractOf(model);
  return (
    <section
      aria-label="Credit scorecard"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <form
        aria-label="Scoring inputs"
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit({
            model,
            input: Object.fromEntries(
              contract.required.map((field) => [
                field,
                typed(entries[model][field] ?? ""),
              ]),
            ),
          } as CreditScorecardRequest);
        }}
      >
        <EnumField
          label="Model"
          help={contract.description}
          value={model}
          onValueChange={(value) => setModel(value as CreditScorecardModel)}
          options={scoringModels.map(({ model, label }) => ({
            value: model,
            label,
          }))}
          layout="select"
        />
        <div className="grid gap-4 @lg:grid-cols-3">
          {contract.required.map((field) => (
            <DecimalInput
              key={`${model}:${field}`}
              label={labelFor(field)}
              help={summary(contract.properties[field]?.description)}
              value={entries[model][field] ?? ""}
              onValueChange={(value) =>
                setEntries((prior) => ({
                  ...prior,
                  [model]: { ...prior[model], [field]: value },
                }))
              }
            />
          ))}
        </div>
        <Button type="submit">Score</Button>
      </form>
      <ScoreView
        score={score}
        loading={loading}
        error={error}
        format={formatValue}
      />
    </section>
  );
}
/** Finite numbers through the host format; anything else as returned. */
function shown(
  value: number | null | undefined,
  format: (v: number) => string,
) {
  return typeof value === "number" && Number.isFinite(value)
    ? format(value)
    : formatRaw(value);
}
function Tile({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section
      aria-label={label}
      className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
    >
      <h4 className="text-xs font-medium text-muted-foreground">{label}</h4>
      {children}
    </section>
  );
}
function ScoreView({
  score,
  loading,
  error,
  format,
}: {
  score: CreditScoreData | null;
  loading?: boolean;
  error?: string;
  format: (value: number) => string;
}) {
  if (error)
    return (
      <p role="alert" className="text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="text-muted-foreground">
        Scoring…
      </p>
    );
  if (!score)
    return (
      <p role="status" className="text-muted-foreground">
        No score
      </p>
    );
  const { result, grade } = score;
  const value = (text: string) => (
    <p className="finstack-numeric text-base">{text}</p>
  );
  return (
    <section aria-label="Credit score" className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{result.model}</h3>
      <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
        <Tile label={labelFor("score")}>
          {value(shown(result.score, format))}
        </Tile>
        <Tile label={labelFor("zone")}>
          <p className="text-base">{result.zone}</p>
        </Tile>
        <Tile label={labelFor("implied_pd")}>
          {value(shown(result.implied_pd, format))}
        </Tile>
        <Tile label={labelFor("grade")}>
          {grade.ok ? (
            <p className="text-base">{grade.value.grade}</p>
          ) : (
            <p className="text-xs text-muted-foreground">
              {grade.error.message}
            </p>
          )}
        </Tile>
        <Tile label={labelFor("central_pd")}>
          {value(grade.ok ? shown(grade.value.central_pd, format) : "—")}
        </Tile>
      </div>
      <p className="text-xs text-muted-foreground">
        Score, zone and implied PD as Rust returned them (probabilities as
        fractions); the grade is the default PD master scale&apos;s grade of the
        implied PD.
      </p>
    </section>
  );
}
