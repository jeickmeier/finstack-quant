"use client";
import { Button } from "@/components/ui/button";
import { useState } from "react";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import { InstrumentForm } from "@/components/finstack/valuations/components/instrument-form/instrument-form";
import { MarketContextForm } from "@/components/finstack/core/components/market-context-form/market-context-form";
import { CalibrationForm } from "@/components/finstack/calibration/components/calibration-form/calibration-form";
import { useInstrumentValidator } from "@/hooks/valuations/use-instrument-validator/use-instrument-validator";
import { useMarketValidator } from "@/hooks/core/use-market-validator/use-market-validator";
import { useCalibrationValidator } from "@/hooks/calibration/use-calibrate/use-calibrate";
import * as bond from "@/lib/finstack/generated/instrument/bond";
import data from "./data.json";
export function NativeForms({ name }: { name: string }) {
  const [type, setType] = useState("bond"),
    [submissions, setSubmissions] = useState(0);
  const [marketDocument, setMarketDocument] = useState({
    json: JSON.stringify(data.market.supplemental),
    revision: 0,
  });
  const validate = useInstrumentValidator(),
    market = useMarketValidator(),
    calibration = useCalibrationValidator();
  const submitted = () => setSubmissions((count) => count + 1);
  return (
    <>
      <output aria-label="Submission count" className="sr-only">
        {submissions}
      </output>
      {name === "schema-form" ? (
        <SchemaForm
          module={bond}
          defaultValues={
            bond.codec.parse(data.bond.request.instrumentJson) as Record<
              string,
              unknown
            >
          }
          validate={validate}
          onSubmit={submitted}
        />
      ) : name === "instrument-form" ? (
        <InstrumentForm
          type={type}
          onTypeChange={setType}
          defaultJson={data.bond.request.instrumentJson}
          validate={validate}
          onSubmit={submitted}
        />
      ) : name === "market-context-form" ? (
        <>
          <Button
            variant="outline"
            onClick={() =>
              setMarketDocument((current) => ({
                json: data.bond.request.marketJson,
                revision: current.revision + 1,
              }))
            }
          >
            Load bond market example
          </Button>
          <MarketContextForm
            key={marketDocument.revision}
            defaultJson={marketDocument.json}
            validate={market}
            onSubmit={submitted}
          />
        </>
      ) : (
        <CalibrationForm
          defaultJson={JSON.stringify(data.calibration[0]!.input)}
          validate={calibration}
          onSubmit={submitted}
        />
      )}
    </>
  );
}
