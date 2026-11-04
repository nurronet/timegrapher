const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const html = fs.readFileSync(path.join(__dirname, '../src/view.html'), 'utf8');
const source = html.match(/^function trendStrip\([^\n]*\{[\s\S]*?^\}/m)[0];
const context = vm.createContext({});
vm.runInContext(source, context);

test('live trend lines use the recorded source and leave other readings as holes', () => {
  const strip = context.trendStrip({t_s: [0, 1, 2, 3], beat_error: [-0.2, 0.8, 0.3, 0.4],
    beat_error_source: ['unlock', 'drop', 'unlock', null]});
  assert.equal(strip.beat_error_unlock[0][1], 0.2);
  assert.ok(Number.isNaN(strip.beat_error_unlock[1][1]));
  assert.equal(strip.beat_error_unlock[2][1], 0.3);
  assert.ok(Number.isNaN(strip.beat_error_unlock[3][1]));
  assert.ok(Number.isNaN(strip.beat_error_drop[0][1]));
  assert.equal(strip.beat_error_drop[1][1], 0.8);
  assert.ok(Number.isNaN(strip.beat_error_drop[2][1]));
});

vm.runInContext(html.match(/^function chosenBeatError\([^\n]*\{[\s\S]*?^\}/m)[0], context);
test('current documents use core source even with both raw values present', () => {
  assert.equal(context.chosenBeatError(0.2, 0.8, 'drop').value, 0.8);
  assert.equal(context.chosenBeatError(0.2, 0.8, 'unlock').value, 0.2);
  assert.equal(context.chosenBeatError(0.2, 0.8, null).value, 0.2);
  assert.equal(context.chosenBeatError(0.2, 0.8, null).source, null);
  assert.equal(context.chosenBeatError(null, 0.8, null).value, 0.8);
  assert.equal(context.chosenBeatError(null, null, undefined).source, null);
});


test('old live trends keep unknown-source values in their own series', () => {
  const strip = context.trendStrip({t_s: [0, 1, 2], beat_error: [-0.2, null, 0.8]});
  assert.equal(strip.beat_error_unknown[0][1], 0.2);
  assert.ok(Number.isNaN(strip.beat_error_unknown[1][1]));
  assert.equal(strip.beat_error_unknown[2][1], 0.8);
  assert.equal(strip.beat_error_unlock.length, 0);
  assert.equal(strip.beat_error_drop.length, 0);
  const mixed = context.trendStrip({t_s: [0, 1], beat_error: [0.2, 0.8], beat_error_source: ['unlock', null]});
  assert.ok(Number.isNaN(mixed.beat_error_unknown[0][1]));
  assert.equal(mixed.beat_error_unknown[1][1], 0.8);
});
