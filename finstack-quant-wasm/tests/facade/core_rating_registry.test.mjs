import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';

import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('rating registry normalizes canonical ids and rejects retired aliases', () => {
  const source = core.embeddedRegistry();
  const value = JSON.parse(source.toJson());
  source.free();
  assert.equal(value.schema, 'finstack_quant.rating_scales/2');
  value.scorecard_policy.unknown_scale_policy = 'error';
  value.scorecard_policy.default_scale_id = ' SP ';
  value.rating_scales[0].id = ' Sp ';
  const registry = core.RatingScaleRegistry.fromJson(value);
  assert.deepEqual(registry.scaleIds(), ['sp', 'moodys']);
  assert.equal(registry.defaultScaleId(), 'sp');
  assert.equal(registry.ratingScale(' SP ').scale_name, 'S&P');
  assert.equal(registry.ratingScale(' MOODYS ').scale_name, "Moody's");
  for (const alias of ['S&P', 'Fitch', 'standard_and_poors', "Moody's"]) {
    assert.equal(registry.isKnownRatingScale(alias), false);
    assert.throws(() => registry.ratingScale(alias));
  }
  registry.free();
  for (const policy of ['fallback_to_default', 'warn_and_fallback']) {
    value.scorecard_policy.unknown_scale_policy = policy;
    const fallback = core.RatingScaleRegistry.fromJson(value);
    assert.equal(fallback.isKnownRatingScale('Fitch'), false);
    assert.equal(fallback.ratingScale('Fitch').scale_name, 'S&P');
    fallback.free();
  }
});

test('rating registry rejects legacy schema and fields and invalid normalized ids', () => {
  const source = core.embeddedRegistry();
  const value = JSON.parse(source.toJson());
  source.free();
  for (const mutate of [
    (v) => {
      v.schema = 'finstack_quant.rating_scales/1';
    },
    (v) => {
      v.scorecard_policy.aliases = [];
    },
    (v) => {
      v.rating_scales[0].ids = ['sp'];
    },
    (v) => {
      v.rating_scales[1].id = ' SP ';
    },
    (v) => {
      v.rating_scales[1].id = '  ';
    },
  ]) {
    const invalid = structuredClone(value);
    mutate(invalid);
    assert.throws(() => core.RatingScaleRegistry.fromJson(invalid));
  }
});
