import { expose } from "comlink";
import init, { core, valuations, models } from "finstack-quant-wasm";
import { createService } from "./finstack-service";
// WASM initialization occurs only in this worker, after the initialize request.
expose(
  createService({
    initialize: (url) => init(url ? { module_or_path: url } : undefined),
    core,
    models,
    valuations,
  }),
);
