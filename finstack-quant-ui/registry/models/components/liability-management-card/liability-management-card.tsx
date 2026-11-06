"use client";
import { useState } from "react";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import type { LmeRequest } from "@/workers/finstack-contract";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import {
  Outcome,
  typedNumber,
  type OutcomeState,
} from "@/components/finstack/shared/primitives/native-display/native-display";
import { AnalysisTable, useFields, type Density } from "./liability-fields";

export interface LiabilityManagementCardProps {
  /** The initial LME terms; remount to load others. */
  defaultTerms: LmeRequest;
  /** Receives the terms on Analyze LME; connect it to the useLme request. */
  onSubmit(terms: LmeRequest): void;
  /** The useLme state. */
  state: OutcomeState<modelsWire.LmeAnalysis>;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: Density;
}
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
/**
 * Issuer-side economics of a liability management exercise, as Rust returned
 * them: nothing is compared, computed or rescaled here.
 */
export function LiabilityManagementCard({
  defaultTerms,
  onSubmit,
  state,
  formatValue = String,
  density = "compact",
}: LiabilityManagementCardProps) {
  const [lmeType, setLmeType] = useState(defaultTerms.lme_type);
  const exercise = useFields(defaultTerms);
  const [priceLabel, priceHelp] = priceInput[lmeType];
  return (
    <section
      aria-label="Liability management exercise"
      data-density={density}
      className="@container space-y-4 font-sans text-sm text-foreground"
    >
      <h3 className="text-sm font-medium">
        Liability management exercise (issuer)
      </h3>
      <form
        aria-label="LME terms"
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit({
            lme_type: lmeType,
            notional: typedNumber(exercise.values.notional),
            repurchase_price_pct: typedNumber(
              exercise.values.repurchase_price_pct,
            ),
            opt_acceptance_pct: typedNumber(exercise.values.opt_acceptance_pct),
            // An empty EBITDA stays unset, so Rust leaves the leverage block out.
            ebitda: exercise.values.ebitda?.trim()
              ? typedNumber(exercise.values.ebitda)
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
          {exercise.field("repurchase_price_pct", priceLabel, priceHelp)}
          {exercise.field(
            "opt_acceptance_pct",
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
      <Outcome state={state} empty="No LME analysis" working="Analyzing…">
        {({ leverage_impact, ...economics }) => (
          <div className="space-y-3">
            <AnalysisTable
              caption="LME economics"
              rows={[
                ...Object.entries(economics),
                ...Object.entries(leverage_impact ?? {}),
              ]}
              format={formatValue}
              density={density}
            />
            <p className="text-xs text-muted-foreground">
              Amounts in the unit entered. Fields ending in pct are fractions;
              leverage is a multiple of EBITDA, for this instrument&apos;s debt
              only.
            </p>
          </div>
        )}
      </Outcome>
    </section>
  );
}
