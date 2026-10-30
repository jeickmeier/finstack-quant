"use client";
import { useState } from "react";
import {
  CreditScorecard,
  type CreditScoreData,
  type CreditScorecardRequest,
} from "@/components/finstack/models/components/credit-scorecard/credit-scorecard";

import data from "./data.json";
// The score is the supplied native fixture: the item installs without a worker.
const entry = data.scorecard.zmijewskiScore;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>
        Supplied valid fixture; standalone credit scorecard with a supplied
        Zmijewski score.
      </p>
      <CreditScorecard
        defaultModel={(entry.request as CreditScorecardRequest).model}
        defaultInputs={{ zmijewskiScore: entry.request.input }}
        onSubmit={() => setCount((n) => n + 1)}
        score={entry.score as unknown as CreditScoreData}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
