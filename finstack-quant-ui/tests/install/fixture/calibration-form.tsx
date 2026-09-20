"use client";
import { useState } from "react";
import { CalibrationForm } from "@/components/finstack/components/calibration-form/calibration-form";

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
      <CalibrationForm
        defaultJson={JSON.stringify(data.calibration[0]!.input)}
        validate={validate}
        onSubmit={() => setCount((n) => n + 1)}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
