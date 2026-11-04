"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import type {
  ExchangeOfferRequest,
  LmeRequest,
} from "@/workers/finstack-contract";
import { ExchangeOfferCard } from "@/components/finstack/models/components/liability-management-card/exchange-offer-card";
import { LiabilityManagementCard } from "@/components/finstack/models/components/liability-management-card/liability-management-card";
import {
  useExchangeOffer,
  useLme,
  useTenderRecommendationHurdle,
} from "@/hooks/models/use-liability-management/use-liability-management";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const defaultExchange = data.liabilityManagement.exchange.recommended
  .request as ExchangeOfferRequest;
const defaultLme = data.liabilityManagement.lme.tender.request as LmeRequest;
function Cards({ density }: ExampleProps) {
  // Starts from the fixture's terms; each form replaces its request.
  const [offer, setOffer] = useState(defaultExchange);
  const [exercise, setExercise] = useState(defaultLme);
  const exchange = useExchangeOffer(offer);
  const lme = useLme(exercise);
  const hurdle = useTenderRecommendationHurdle();
  return (
    <div className="space-y-8">
      <ExchangeOfferCard
        defaultTerms={defaultExchange}
        onSubmit={setOffer}
        state={{
          result: exchange.data,
          loading: exchange.isFetching,
          error: exchange.error?.message,
        }}
        hurdle={hurdle.data}
        density={density}
      />
      <LiabilityManagementCard
        defaultTerms={defaultLme}
        onSubmit={setExercise}
        state={{
          result: lme.data,
          loading: lme.isFetching,
          error: lme.error?.message,
        }}
        density={density}
      />
    </div>
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with supplied results.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Cards density={density} />
    </WorkerPreview>
  );
}
