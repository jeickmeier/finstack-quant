"use client";
import { Button } from "@/components/ui/button";
import { useState } from "react";
import fixture from "@/lib/finstack/fixtures/results/bond.json";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { InstrumentForm } from "@/components/finstack/valuations/components/instrument-form/instrument-form";
import {
  PricingParamsForm,
  jsonError,
  type PricingParams,
} from "@/components/finstack/valuations/components/pricing-params-form/pricing-params-form";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import { useInstrumentValidator } from "@/hooks/valuations/use-instrument-validator/use-instrument-validator";
import {
  useModels,
  useMetrics,
  usePriceInstrument,
} from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import type { PriceRequest } from "@/workers/finstack-contract";
function Forms() {
  const [type, setType] = useState("bond");
  const [instrument, setInstrument] = useState<string | null>(null);
  const [params, setParams] = useState<PricingParams>({
    asOf: fixture.request.asOf,
    model: fixture.request.model,
    metrics: [],
  });
  const [request, setRequest] = useState<PriceRequest | null>(null);
  const validate = useInstrumentValidator();
  const models = useModels(),
    metrics = useMetrics();
  const price = usePriceInstrument(
    request ?? fixture.request,
    request !== null,
  );
  return (
    <section
      aria-label="Standalone pricing forms"
      className="space-y-6 font-sans text-foreground"
    >
      <InstrumentForm
        type={type}
        onTypeChange={setType}
        defaultJson={fixture.request.instrumentJson}
        validate={validate}
        onSubmit={setInstrument}
        onValidated={setInstrument}
      />
      <PricingParamsForm
        value={params}
        onValueChange={setParams}
        instrumentType={type}
        models={models.data ?? {}}
        metrics={metrics.data ?? {}}
        loading={models.isPending || metrics.isPending}
        error={models.error?.message ?? metrics.error?.message}
      />
      <Button
        type="button"
        disabled={
          !instrument ||
          type !== "bond" ||
          Boolean(
            jsonError(params.pricingOptions) || jsonError(params.marketHistory),
          )
        }
        onClick={() =>
          setRequest({
            instrumentJson: instrument!,
            marketJson: fixture.request.marketJson,
            ...params,
          })
        }
      >
        Price supplied request
      </Button>
      <JsonViewer
        label="Submitted request"
        text={request ? JSON.stringify(request) : null}
      />
      <JsonViewer
        label="Pricing result"
        text={price.data ? serializeHost(price.data) : null}
        loading={price.isFetching}
        error={price.error?.message}
      />
    </section>
  );
}
/** Standalone examples with caller-owned state and the existing shared worker hooks. */
export function PricingFormsExample() {
  return (
    <FinstackQueryProvider>
      <Forms />
    </FinstackQueryProvider>
  );
}
