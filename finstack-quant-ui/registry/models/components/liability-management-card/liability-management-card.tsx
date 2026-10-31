"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** The terms the exchange form submits; the worker's ExchangeOfferRequest. */
export interface ExchangeOfferTerms {
  oldPv: number;
  newPv: number;
  consentFee: number;
  equitySweetenerValue: number;
  exchangeType: modelsWire.ExchangeType;
}
/** The terms the LME form submits; the worker's LmeRequest. */
export interface LmeTerms {
  lmeType: modelsWire.LmeType;
  notional: number;
  repurchasePricePct: number;
  optAcceptancePct: number;
  /** Unset leaves the leverage block out, as a non-positive value does in Rust. */
  ebitda?: number | null;
}
/** One analysis's state, from its hook. */
interface AnalysisState<T> {
  result: T | null;
  loading?: boolean;
  /** The native error of this analysis, shown as returned. */
  error?: string;
}
export interface LiabilityManagementCardProps {
  /** The initial exchange terms; remount to load others. */
  defaultExchange: ExchangeOfferTerms;
  /** The initial LME terms; remount to load others. */
  defaultLme: LmeTerms;
  /** Receives the terms on Analyze offer; connect it to the useExchangeOffer request. */
  onExchangeSubmit(terms: ExchangeOfferTerms): void;
  /** Receives the terms on Analyze LME; connect it to the useLme request. */
  onLmeSubmit(terms: LmeTerms): void;
  exchange: AnalysisState<modelsWire.ExchangeOfferAnalysis>;
  lme: AnalysisState<modelsWire.LmeAnalysis>;
  /** The multiple Rust's recommendation applies to the hold-out value (useTenderRecommendationHurdle); shown as supplied, never compared here. */
  hurdle?: number | null;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
const exchangeTypes: readonly modelsWire.ExchangeType[] = [
  "par_for_par",
  "discount",
  "uptier",
  "downtier",
];
const lmeTypes: readonly modelsWire.LmeType[] = [
  "open_market_repurchase",
  "tender_offer",
  "amend_and_extend",
  "dropdown",
];
/** What Rust reads the price input as, and the range it admits, per structure. */
const priceInput = {
  open_market_repurchase: [
    "Repurchase price",
    "Price as a fraction of par (0.60 is 60 cents), above 0 and up to 1.5.",
  ],
  tender_offer: [
    "Tender price",
    "Price as a fraction of par (0.60 is 60 cents), above 0 and up to 1.5.",
  ],
  amend_and_extend: [
    "Extension fee",
    "Fee as a fraction of par, from 0 to 0.10. No par is retired.",
  ],
  dropdown: [
    "Assets transferred",
    "Fraction of assets transferred away, from 0 to 1. No par is retired and no cash is paid.",
  ],
} as const satisfies Record<
  modelsWire.LmeType,
  readonly [label: string, help: string]
>;
/** Typed text as a number; an empty field is NaN, never 0, so Rust rejects it. */
const typed = (value: string | undefined) =>
  !value || value.trim() === "" ? Number.NaN : Number(value);
/** The supplied numbers as field text. */
function fieldText(terms: object) {
  return Object.fromEntries(
    Object.entries(terms).flatMap(([key, value]) =>
      typeof value === "number" ? [[key, String(value)]] : [],
    ),
  );
}
function useFields(terms: object) {
  const [values, setValues] = useState<Record<string, string>>(() =>
    fieldText(terms),
  );
  const field = (key: string, label: string, help: string) => (
    <DecimalInput
      key={key}
      label={label}
      help={help}
      value={values[key] ?? ""}
      onValueChange={(value) =>
        setValues((prior) => ({ ...prior, [key]: value }))
      }
    />
  );
  return { values, field };
}
/**
 * Hold-versus-tender economics of an exchange offer and issuer-side economics of
 * a liability management exercise, as Rust returned them. The recommendation is
 * Rust's own field: nothing is compared, computed or rescaled here.
 */
export function LiabilityManagementCard({
  defaultExchange,
  defaultLme,
  onExchangeSubmit,
  onLmeSubmit,
  exchange,
  lme,
  hurdle,
  formatValue = String,
  density = "compact",
}: LiabilityManagementCardProps) {
  const [exchangeType, setExchangeType] = useState(
    defaultExchange.exchangeType,
  );
  const [lmeType, setLmeType] = useState(defaultLme.lmeType);
  const offer = useFields(defaultExchange);
  const exercise = useFields(defaultLme);
  const [priceLabel, priceHelp] = priceInput[lmeType];
  return (
    <section
      aria-label="Liability management"
      data-density={density}
      className="@container space-y-8 font-sans text-sm text-foreground"
    >
      <section aria-label="Exchange offer" className="space-y-4">
        <h3 className="text-sm font-medium">Exchange offer (holder)</h3>
        <form
          aria-label="Exchange offer terms"
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            onExchangeSubmit({
              oldPv: typed(offer.values.oldPv),
              newPv: typed(offer.values.newPv),
              consentFee: typed(offer.values.consentFee),
              equitySweetenerValue: typed(offer.values.equitySweetenerValue),
              exchangeType,
            });
          }}
        >
          <div className="grid gap-4 @lg:grid-cols-3">
            <EnumField
              label="Offer structure"
              value={exchangeType}
              onValueChange={(value) =>
                setExchangeType(value as modelsWire.ExchangeType)
              }
              layout="radio"
              options={exchangeTypes.map((value) => ({
                value,
                label: labelFor(value),
              }))}
            />
            {offer.field(
              "oldPv",
              "Hold-out PV",
              "Present value of the existing claim if not tendered, in one monetary unit you choose.",
            )}
            {offer.field(
              "newPv",
              "New instrument PV",
              "Present value of what a tender receives, same unit.",
            )}
            {offer.field(
              "consentFee",
              "Consent fee",
              "Cash consent or early-tender fee, same unit.",
            )}
            {offer.field(
              "equitySweetenerValue",
              "Equity sweetener",
              "Estimated value of attached equity or warrants, same unit.",
            )}
          </div>
          <Button type="submit">Analyze offer</Button>
        </form>
        <AnalysisView
          state={exchange}
          empty="No offer analysis"
          caption="Offer economics"
          // The badge above shows the recommendation, so it is not repeated as a row.
          rows={({ tender_recommended: _shown, ...economics }) =>
            Object.entries(economics)
          }
          format={formatValue}
          density={density}
          lead={(result) => (
            <div className="flex flex-wrap items-center gap-3">
              <Badge
                variant={result.tender_recommended ? "default" : "secondary"}
              >
                {result.tender_recommended
                  ? "Tender recommended"
                  : "Tender not recommended"}
              </Badge>
              {hurdle != null && (
                <span className="text-xs text-muted-foreground">
                  Rust recommends a tender only above{" "}
                  <span className="finstack-numeric">{formatRaw(hurdle)}</span>{" "}
                  times the hold-out PV.
                </span>
              )}
            </div>
          )}
          note="Amounts in the unit entered. Breakeven recovery is a fraction of the hold-out PV, capped at 1."
        />
      </section>
      <section aria-label="Liability management exercise" className="space-y-4">
        <h3 className="text-sm font-medium">
          Liability management exercise (issuer)
        </h3>
        <form
          aria-label="LME terms"
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            onLmeSubmit({
              lmeType,
              notional: typed(exercise.values.notional),
              repurchasePricePct: typed(exercise.values.repurchasePricePct),
              optAcceptancePct: typed(exercise.values.optAcceptancePct),
              // An empty EBITDA stays unset, so Rust leaves the leverage block out.
              ebitda: exercise.values.ebitda?.trim()
                ? typed(exercise.values.ebitda)
                : null,
            });
          }}
        >
          <div className="grid gap-4 @lg:grid-cols-3">
            <EnumField
              label="Structure"
              value={lmeType}
              onValueChange={(value) => setLmeType(value as modelsWire.LmeType)}
              layout="radio"
              options={lmeTypes.map((value) => ({
                value,
                label: labelFor(value),
              }))}
            />
            {exercise.field(
              "notional",
              "Notional",
              "Outstanding face of the target instrument, positive, in one monetary unit you choose.",
            )}
            {exercise.field("repurchasePricePct", priceLabel, priceHelp)}
            {exercise.field(
              "optAcceptancePct",
              "Participation",
              "Fraction of holders participating, from 0 to 1.",
            )}
            {exercise.field(
              "ebitda",
              "EBITDA",
              "Same unit as the notional. Empty or not positive leaves leverage out.",
            )}
          </div>
          <Button type="submit">Analyze LME</Button>
        </form>
        <AnalysisView
          state={lme}
          empty="No LME analysis"
          caption="LME economics"
          rows={({ leverage_impact, ...economics }) => [
            ...Object.entries(economics),
            ...Object.entries(leverage_impact ?? {}),
          ]}
          format={formatValue}
          density={density}
          note="Amounts in the unit entered. Fields ending in pct are fractions; leverage is a multiple of EBITDA, for this instrument's debt only."
        />
      </section>
    </section>
  );
}
interface Field {
  name: string;
  value: unknown;
}
/** Finite numbers through the host format; words as Rust spells them. */
function shown(value: unknown, format: (value: number) => string) {
  return typeof value === "number" && Number.isFinite(value)
    ? format(value)
    : formatRaw(value as string | number | null | undefined);
}
function AnalysisView<T>({
  state,
  empty,
  caption,
  rows,
  lead,
  note,
  format,
  density,
}: {
  state: AnalysisState<T>;
  empty: string;
  caption: string;
  rows: (result: T) => [string, unknown][];
  lead?: (result: T) => React.ReactNode;
  note: string;
  format: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  const columns = useMemo<ColumnDef<{}, Field, any>[]>(
    () => [
      {
        id: "name",
        header: "Field",
        accessorFn: (row) => row.name,
        cell: ({ getValue }) => labelFor(getValue() as string),
        meta: { rowHeader: true },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (row) => row.value,
        cell: ({ getValue }) => {
          const value = getValue();
          return <span title={String(value)}>{shown(value, format)}</span>;
        },
        meta: { className: "text-right finstack-numeric" },
      },
    ],
    [format],
  );
  if (state.error)
    return (
      <p role="alert" className="text-error">
        {state.error}
      </p>
    );
  if (state.loading)
    return (
      <p role="status" className="text-muted-foreground">
        Analyzing…
      </p>
    );
  if (!state.result)
    return (
      <p role="status" className="text-muted-foreground">
        {empty}
      </p>
    );
  return (
    <div className="space-y-3">
      {lead?.(state.result)}
      <FinstackTable
        data={rows(state.result).map(([name, value]) => ({ name, value }))}
        columns={columns}
        getRowId={(row) => row.name}
        caption={caption}
        density={density}
        emptyState="No fields returned"
      />
      <p className="text-xs text-muted-foreground">{note}</p>
    </div>
  );
}
