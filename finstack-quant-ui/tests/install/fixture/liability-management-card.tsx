"use client";
import { useState } from "react";
import {
  LiabilityManagementCard,
  type ExchangeOfferTerms,
  type LmeTerms,
} from "@/components/finstack/models/components/liability-management-card/liability-management-card";
import type { models } from "finstack-quant-wasm/types";

import data from "./data.json";
// The results are the supplied native fixture: the item installs without a worker.
const { exchange, lme, hurdle } = data.liabilityManagement;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>Supplied valid fixture; standalone liability management card.</p>
      <LiabilityManagementCard
        defaultExchange={exchange.recommended.request as ExchangeOfferTerms}
        defaultLme={lme.tender.request as LmeTerms}
        onExchangeSubmit={() => setCount((n) => n + 1)}
        onLmeSubmit={() => setCount((n) => n + 1)}
        exchange={{
          result: exchange.recommended.result
            .value as models.ExchangeOfferAnalysis,
        }}
        lme={{ result: lme.tender.result.value as models.LmeAnalysis }}
        hurdle={hurdle}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
