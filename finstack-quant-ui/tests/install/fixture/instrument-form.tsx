"use client";
import { useState } from "react";
import { InstrumentForm } from "@/components/finstack/components/instrument-form/instrument-form";

import data from "./data.json";
const validate = async (json: string) => json;
export function InstalledItem() {
  const [type, setType] = useState("bond"),
    [count, setCount] = useState(0);
  return (
    <>
      <p>
        Supplied valid fixture; standalone structural form with a caller
        validation callback.
      </p>
      <InstrumentForm
        type={type}
        onTypeChange={setType}
        defaultJson={data.bond.request.instrumentJson}
        validate={validate}
        onSubmit={() => setCount((n) => n + 1)}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
