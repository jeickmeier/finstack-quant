import * as wasm from '../../../pkg/finstack_quant_wasm.js';

export const migration = {
  RatingScale: wasm.RatingScale,
  TransitionMatrix: wasm.TransitionMatrix,
  GeneratorMatrix: wasm.GeneratorMatrix,
  RatingPath: wasm.RatingPath,
  RatingPaths: wasm.RatingPaths,
  MigrationSimulator: wasm.MigrationSimulator,
  project: wasm.project,
};
