import * as wasm from '../../pkg/finstack_quant_wasm.js';

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
  // Liability management
  analyzeExchangeOffer: wasm.analyzeExchangeOffer,
  analyzeLme: wasm.analyzeLme,
  tenderRecommendationHurdle: wasm.tenderRecommendationHurdle,
  // Loss given default and exposure at default
  BetaRecovery: wasm.BetaRecovery,
  WorkoutCosts: wasm.WorkoutCosts,
  WorkoutLgd: wasm.WorkoutLgd,
  WorkoutLgdBuilder: wasm.WorkoutLgdBuilder,
  DownturnLgd: wasm.DownturnLgd,
  EadCalculator: wasm.EadCalculator,
  seniorityRecoveryStats: wasm.seniorityRecoveryStats,
  betaRecoverySample: wasm.betaRecoverySample,
  betaRecoveryQuantile: wasm.betaRecoveryQuantile,
  workoutLgd: wasm.workoutLgd,
  downturnLgdStressed: wasm.downturnLgdStressed,
  downturnLgdRegulatoryFloor: wasm.downturnLgdRegulatoryFloor,
  eadTermLoan: wasm.eadTermLoan,
  eadRevolver: wasm.eadRevolver,
  // PD calibration
  MasterScale: wasm.MasterScale,
  baselIrbPdFloor: wasm.baselIrbPdFloor,
  applyBaselIrbPdFloor: wasm.applyBaselIrbPdFloor,
  pitToTtc: wasm.pitToTtc,
  ttcToPit: wasm.ttcToPit,
  centralTendency: wasm.centralTendency,
  // Credit migration
  RatingScale: wasm.RatingScale,
  TransitionMatrix: wasm.TransitionMatrix,
  GeneratorMatrix: wasm.GeneratorMatrix,
  RatingPath: wasm.RatingPath,
  RatingPaths: wasm.RatingPaths,
  MigrationSimulator: wasm.MigrationSimulator,
  project: wasm.project,
  // Accounting-based scoring
  altmanZScore: wasm.altmanZScore,
  altmanZPrime: wasm.altmanZPrime,
  altmanZDoublePrime: wasm.altmanZDoublePrime,
  altmanEmScore: wasm.altmanEmScore,
  ohlsonOScore: wasm.ohlsonOScore,
  zmijewskiScore: wasm.zmijewskiScore,
  // Recovery waterfall
  allocateRecovery: wasm.allocateRecovery,
};
