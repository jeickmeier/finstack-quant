import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('canonical parser labels normalize ASCII case and outer whitespace', () => {
  assert.equal(core.DayCount.fromName(' ACT_360 ').toString(), 'act_360');
  assert.equal(
    core.adjust(core.createDate(2025, 1, 4), ' MODIFIED_FOLLOWING ', 'weekends_only'),
    core.createDate(2025, 1, 6)
  );
  assert.equal(core.PeriodKind.fromName('quarterly').toString(), 'quarterly');
  assert.equal(core.cfKindParse(' FIXED '), 'fixed');
  assert.equal(core.Tenor.parse(' 3m ').toString(), '3M');
});

test('canonical parsers reject retired aliases', () => {
  for (const [parse, inputs] of [
    [
      (name) => core.DayCount.fromName(name),
      ['ACT/360', 'act-360', 'act_365', 'act_act_icma', '30_360_isda'],
    ],
    [
      (name) => core.adjust(core.createDate(2025, 1, 4), name, 'weekends_only'),
      ['MF', 'F', 'P', 'MP', 'NONE', 'modfollowing'],
    ],
    [
      (name) => core.PeriodKind.fromName(name),
      [
        'D',
        'B',
        'W',
        'M',
        'ME',
        'Q',
        'QE',
        'A',
        'Y',
        'YE',
        'semiannual',
        ' QUARTERLY ',
        'DAILY',
        'monthly ',
      ],
    ],
    [(name) => core.Tenor.parse(name), ['ON', 'TN', 'SN', 'O/N', '+1D', '0D', '1 M']],
    [(name) => core.cfKindParse(name), ['coupon', 'principal', 'float-reset']],
  ]) {
    for (const input of inputs)
      assert.throws(
        () => parse(input),
        (error) => error.kind === 'validation'
      );
  }
});
