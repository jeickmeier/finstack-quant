"use client";
import { useState } from "react";
import { MarketContextForm } from "@/components/finstack/core/components/market-context-form/market-context-form";

import data from "./data.json";
const validate = async (json: string) => json;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>
        Supplied valid fixture; standalone structural form with a caller
        validation callback.
      </p>
      <MarketContextForm
        defaultJson={JSON.stringify(data.market.supplemental)}
        validate={validate}
        onSubmit={() => setCount((n) => n + 1)}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
