import * as wasm from '../../pkg/finstack_quant_wasm.js';
import { lgd } from './credit/lgd.js';
import { liabilityManagement } from './credit/liabilityManagement.js';
import { migration } from './credit/migration.js';
import { pd } from './credit/pd.js';
import { recoveryWaterfall } from './credit/recoveryWaterfall.js';
import { scoring } from './credit/scoring.js';

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
