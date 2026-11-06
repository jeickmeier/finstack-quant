import type { valuations } from "finstack-quant-wasm/types";
import type { PriceRequest } from "../../registry/shared/file/finstack-contract";
export function priceExtras(
  request: Pick<PriceRequest, "metricPricingOverrides" | "marketHistory">,
): valuations.PriceExtras;
