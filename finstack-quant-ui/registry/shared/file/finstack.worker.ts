import { expose } from "comlink";
import init, {
  core,
  valuations,
  cashflows,
  covenants,
  scenarios,
  attribution,
  analytics,
  models,
  calibration,
  statements,
  statements_analytics,
  portfolio,
  margin,
} from "finstack-quant-wasm";
import { createService } from "./finstack-service";
// WASM initialization occurs only in this worker, after the initialize request.
expose(
  createService({
    initialize: (url) => init(url ? { module_or_path: url } : undefined),
    core,
    cashflows,
    covenants,
    scenarios,
    attribution,
    analytics,
    models,
    calibration,
    statements,
    statements_analytics,
    valuations,
    portfolio,
    margin,
  }),
);
