import * as wasm from '../../pkg/finstack_quant_wasm.js';
import { lgd } from './credit/lgd.js';
import { liabilityManagement } from './credit/liabilityManagement.js';
import { migration } from './credit/migration.js';
import { pd } from './credit/pd.js';
import { recoveryWaterfall } from './credit/recoveryWaterfall.js';
import { scoring } from './credit/scoring.js';

/**
 * Structural credit models, LGD / EAD / PD calibration, rating migration,
 * credit scoring, recovery waterfalls and liability-management analytics.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const model = new models.credit.MertonModel(100, 0.25, 60, 0.03);
 * console.log(model.defaultProbability(1), models.credit.pd.pitToTtc(0.02, 0.15, -1.0));
 * model.free();
 * ```
 */
export const credit = {
  // Structural (Merton-family) models and PIK specifications
  MertonModel: wasm.MertonModel,
  MertonBarrierType: wasm.MertonBarrierType,
  AssetDynamics: wasm.AssetDynamics,
  SimulatedPaths: wasm.SimulatedPaths,
  DynamicRecoverySpec: wasm.DynamicRecoverySpec,
  EndogenousHazardSpec: wasm.EndogenousHazardSpec,
  ToggleExerciseModel: wasm.ToggleExerciseModel,
  RatingFactorTable: wasm.RatingFactorTable,
  mertonReport: wasm.mertonReport,
  moodysWarfFactor: wasm.moodysWarfFactor,
  // Submodules (Rust `credit::{lgd, liability_management, migration, pd,
  // recovery_waterfall, scoring}`)
  lgd,
  liabilityManagement,
  migration,
  pd,
  recoveryWaterfall,
  scoring,
};
