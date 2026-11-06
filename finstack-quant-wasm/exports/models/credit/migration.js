import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * Rating scales, transition and generator matrices, and rating-migration simulation.
 * Mirrors Rust `credit::migration`.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const scale = models.credit.migration.RatingScale.custom(["A", "B", "D"]);
 * const generator = new models.credit.migration.GeneratorMatrix(scale, [
 *   -0.1, 0.08, 0.02, 0.1, -0.2, 0.1, 0, 0, 0,
 * ]);
 * console.log(models.credit.migration.project(generator, 2).probability("A", "D"));
 * ```
 */
export const migration = {
  RatingScale: wasm.RatingScale,
  TransitionMatrix: wasm.TransitionMatrix,
  GeneratorMatrix: wasm.GeneratorMatrix,
  RatingPath: wasm.RatingPath,
  RatingPaths: wasm.RatingPaths,
  MigrationSimulator: wasm.MigrationSimulator,
  project: wasm.project,
};
