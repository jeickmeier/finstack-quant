"use client";
import { useState } from "react";
import { ExchangeOfferCard } from "@/components/finstack/models/components/liability-management-card/exchange-offer-card";
import { LiabilityManagementCard } from "@/components/finstack/models/components/liability-management-card/liability-management-card";
import type {
  ExchangeOfferRequest,
  LmeRequest,
} from "@/workers/finstack-contract";
import type { models } from "finstack-quant-wasm/types";

import data from "./data.json";
// The results are the supplied native fixture: the item installs without a worker.
const { exchange, lme, hurdle } = data.liabilityManagement;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>Supplied valid fixture; standalone liability management card.</p>
      <ExchangeOfferCard
        defaultTerms={exchange.recommended.request as ExchangeOfferRequest}
        onSubmit={() => setCount((n) => n + 1)}
        state={{
          result: exchange.recommended.result
            .value as models.ExchangeOfferAnalysis,
        }}
        hurdle={hurdle}
      />
      <LiabilityManagementCard
        defaultTerms={lme.tender.request as LmeRequest}
        onSubmit={() => setCount((n) => n + 1)}
        state={{ result: lme.tender.result.value as models.LmeAnalysis }}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
