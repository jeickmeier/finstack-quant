"use client";
import { useState } from "react";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import type { ExchangeOfferRequest } from "@/workers/finstack-contract";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import {
  Outcome,
  typedNumber,
  type OutcomeState,
} from "@/components/finstack/shared/primitives/native-display/native-display";
import { AnalysisTable, useFields, type Density } from "./liability-fields";

export interface ExchangeOfferCardProps {
  /** The initial exchange terms; remount to load others. */
  defaultTerms: ExchangeOfferRequest;
  /** Receives the terms on Analyze offer; connect it to the useExchangeOffer request. */
  onSubmit(terms: ExchangeOfferRequest): void;
  /** The useExchangeOffer state. */
  state: OutcomeState<modelsWire.ExchangeOfferAnalysis>;
  /** The multiple Rust's recommendation applies to the hold-out value (useTenderRecommendationHurdle); shown as supplied, never compared here. */
  hurdle?: number | null;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: Density;
}
const exchangeTypes: readonly modelsWire.ExchangeType[] = [
  "par_for_par",
  "discount",
  "uptier",
  "downtier",
];
/**
 * Hold-versus-tender economics of an exchange offer, the holder's view, as
 * Rust returned them. The recommendation is Rust's own field: nothing is
 * compared, computed or rescaled here.
 */
export function ExchangeOfferCard({
  defaultTerms,
  onSubmit,
  state,
  hurdle,
  formatValue = String,
  density = "compact",
}: ExchangeOfferCardProps) {
  const [exchangeType, setExchangeType] = useState(defaultTerms.exchangeType);
  const offer = useFields(defaultTerms);
  return (
    <section
      aria-label="Exchange offer"
      data-density={density}
      className="@container space-y-4 font-sans text-sm text-foreground"
    >
      <h3 className="text-sm font-medium">Exchange offer (holder)</h3>
      <form
        aria-label="Exchange offer terms"
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit({
            oldPv: typedNumber(offer.values.oldPv),
            newPv: typedNumber(offer.values.newPv),
            consentFee: typedNumber(offer.values.consentFee),
            equitySweetenerValue: typedNumber(
              offer.values.equitySweetenerValue,
            ),
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
      <Outcome state={state} empty="No offer analysis" working="Analyzing…">
        {({ tender_recommended, ...economics }) => (
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-3">
              <Badge variant={tender_recommended ? "default" : "secondary"}>
                {tender_recommended
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
            {/* The badge shows the recommendation, so it is not repeated as a row. */}
            <AnalysisTable
              caption="Offer economics"
              rows={Object.entries(economics)}
              format={formatValue}
              density={density}
            />
            <p className="text-xs text-muted-foreground">
              Amounts in the unit entered. Breakeven recovery is a fraction of
              the hold-out PV, capped at 1.
            </p>
          </div>
        )}
      </Outcome>
    </section>
  );
}
