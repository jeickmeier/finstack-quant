import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Listed-market product coverage and exchange routing metadata.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const rows = valuations.market.listedProductCatalog("cme");
 * console.log(rows.every((row) => row.exchange === "cme"));
 * ```
 */
export const market = {
  listedProductCatalog: wasm.listedProductCatalog,
  ConventionRegistry: wasm.ConventionRegistry,
};
