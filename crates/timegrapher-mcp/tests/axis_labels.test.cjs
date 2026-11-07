// The view's axis rule (axisLabel) against core chart_scale::axis_number,
// by the fixture core's own test reads (#815).
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { MINUS } = require("./core_digits.cjs");

const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML ||
  path.join(__dirname, "../src/view.html"), "utf8");
const cases = JSON.parse(fs.readFileSync(
  path.join(__dirname, "../../timegrapher-core/tests/fixtures/axis_numbers.json"), "utf8"));
const source = html.match(/^function axisLabel\([^\n]*\{[\s\S]*?^\}/m);
if (!source) throw new Error("Missing renderer function: axisLabel");
const digits = html.match(/^function fixedDigits\([^\n]*\{[\s\S]*?^\}/m);
if (!digits) throw new Error("Missing renderer function: fixedDigits");
// The Steadiness view's labels (stepText) follow the same rule.
const step = html.match(/^function stepText\([^\n]+\}\s*$/m);
if (!step) throw new Error("Missing renderer function: stepText");
const context = vm.createContext({ MINUS });
vm.runInContext(digits[0] + "\n" + source[0] + "\n" + step[0], context);

test("the fixture has cases", () => assert.ok(cases.length > 0));
// Core's axis_number with its true minus, as the app's axes set it
// (true_minus(axis_number), #1431).
for (const c of cases) {
  const shown = c.text.replace("-", MINUS);
  test(`${c.v} on a step of ${c.step} reads ${shown}, as core writes it on a screen`, () => {
    assert.equal(context.axisLabel(c.v, c.step), shown);
    assert.equal(context.stepText(c.v, c.step), shown, "the Steadiness view's labels");
  });
}

test("the unit follows the number", () => assert.equal(context.axisLabel(-0.04, 0.1, " ms"), "0.0 ms"));
