"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  LiabilityManagementCard,
  type ExchangeOfferTerms,
  type LmeTerms,
} from "@/components/finstack/models/components/liability-management-card/liability-management-card";
import {
  useExchangeOffer,
  useLme,
  useTenderRecommendationHurdle,
} from "@/hooks/models/use-liability-management/use-liability-management";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const defaultExchange = data.liabilityManagement.exchange.recommended
  .request as ExchangeOfferTerms;
const defaultLme = data.liabilityManagement.lme.tender.request as LmeTerms;
function Card({ density }: ExampleProps) {
  // Starts from the fixture's terms; each form replaces its request.
  const [offer, setOffer] = useState(defaultExchange);
  const [exercise, setExercise] = useState(defaultLme);
  const exchange = useExchangeOffer(offer);
  const lme = useLme(exercise);
  const hurdle = useTenderRecommendationHurdle();
  return (
    <LiabilityManagementCard
      defaultExchange={defaultExchange}
      defaultLme={defaultLme}
      onExchangeSubmit={setOffer}
      onLmeSubmit={setExercise}
      exchange={{
        result: exchange.data ?? null,
        loading: exchange.isFetching,
        error: exchange.error?.message,
      }}
      lme={{
        result: lme.data ?? null,
        loading: lme.isFetching,
        error: lme.error?.message,
      }}
      hurdle={hurdle.data}
      density={density}
    />
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with supplied results.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Card density={density} />
    </WorkerPreview>
  );
}
