import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';

import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const document = () =>
  JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant-py/tests/data/agency_style_rating_scales.json',
        import.meta.url
      ),
      'utf8'
    )
  );

test('rating registry normalizes ids and rejects unknown ids without fallback', () => {
  const value = document();
  assert.equal(value.schema, 'finstack_quant.rating_scales/2');
  value.rating_scales[0].id = ' Sp ';
  const registry = core.RatingScaleRegistry.fromJson(value);
  assert.deepEqual(registry.scaleIds(), ['sp', 'moodys']);
  assert.equal(registry.ratingScale(' SP ').scale_name, 'S&P-style test scale');
  assert.match(registry.entry(' MOODYS ').source, /^Test fixture/);
  for (const alias of ['S&P', 'Fitch', 'standard_and_poors', "Moody's"]) {
    assert.equal(registry.isKnownRatingScale(alias), false);
    assert.throws(
      () => registry.ratingScale(alias),
      (e) => e.kind === 'validation'
    );
    assert.throws(
      () => registry.entry(alias),
      (e) => e.kind === 'validation'
    );
  }
  registry.free();
});

test('rating registry rejects legacy fields, empty lists and invalid ids or sources', () => {
  const value = document();
  for (const mutate of [
    (v) => {
      v.schema = 'finstack_quant.rating_scales/1';
    },
    (v) => {
      v.scorecard_policy = { default_score: 50 };
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
    (v) => {
      v.rating_scales[0].source = ' ';
    },
    (v) => {
      v.rating_scales = [];
    },
  ]) {
    const invalid = structuredClone(value);
    mutate(invalid);
    assert.throws(
      () => core.RatingScaleRegistry.fromJson(invalid),
      (e) => e.kind === 'validation'
    );
  }
});

test('scales cover [0, 100], classify to the better band and hold their representatives', () => {
  const scale = core.scorecardScaleTryNew('Internal', undefined, [
    core.ratingLevelTryNew('A', 90, 80),
    core.ratingLevelTryNew('B', 70, 0),
  ]);
  assert.equal(core.scorecardScaleClassify(scale, 80).name, 'A');
  assert.equal(core.scorecardScaleClassify(scale, 0).name, 'B');
  assert.equal(core.scorecardScaleLevelPosition(scale, ' b '), 1);
  for (const bad of [-0.1, 100.1, Number.NaN]) {
    assert.throws(
      () => core.scorecardScaleClassify(scale, bad),
      (e) => e.kind === 'validation'
    );
  }
  assert.throws(
    () => core.scorecardScaleTryNew('gap', undefined, [core.ratingLevelTryNew('A', 90, 80)]),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => core.ratingLevelTryNew('Not Rated', 50, 0),
    (e) => e.kind === 'validation'
  );
  const registry = new core.RatingScaleRegistry([
    core.ratingScaleEntryTryNew(' Internal ', 'credit policy 2026', scale),
  ]);
  assert.deepEqual(registry.scaleIds(), ['internal']);
  assert.equal(registry.ratingScales()[0].id, 'internal');
  assert.throws(
    () => new core.RatingScaleRegistry([]),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => core.registryFromConfig(new core.FinstackConfig()),
    (e) => e.kind === 'validation'
  );
  registry.free();
});
