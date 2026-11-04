// The view's round-step rule (roundStep) against core chart_scale::round_step,
// by the fixture core's own test reads.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML ||
  path.join(__dirname, "../src/view.html"), "utf8");
const cases = JSON.parse(fs.readFileSync(
  path.join(__dirname, "../../timegrapher-core/tests/fixtures/round_steps.json"), "utf8"));
const source = html.match(/^function roundStep\([^\n]*\{[\s\S]*?^\}/m);
if (!source) throw new Error("Missing renderer function: roundStep");
const context = vm.createContext({});
vm.runInContext(source[0], context);

test("the fixture has cases", () => assert.ok(cases.length > 0));
for (const c of cases) {
  test(`a step of at least ${c.x} is ${c.step}, as core rounds it`, () => {
    const got = context.roundStep(c.x);
    assert.ok(Math.abs(got - c.step) < c.step * 1e-9, `${got}`);
  });
}
