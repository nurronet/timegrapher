// The view's Periods axis (scoreTicks) against core
// steady_views::score_ticks, by the fixture core's own test reads (#1065).
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { MINUS } = require("./core_digits.cjs");

const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML ||
  path.join(__dirname, "../src/view.html"), "utf8");
const cases = JSON.parse(fs.readFileSync(
  path.join(__dirname, "../../timegrapher-core/tests/fixtures/score_ticks.json"), "utf8"));
const definition = (name) => {
  const match = html.match(new RegExp(`^function ${name}\\([^\\n]*\\{[\\s\\S]*?^\\}`, "m"));
  if (!match) throw new Error(`Missing renderer function: ${name}`);
  return match[0];
};
const context = vm.createContext({ MINUS });
vm.runInContext(["roundStep", "fixedDigits", "axisLabel", "scoreTicks"].map(definition).join("\n"), context);

test("the fixture has cases", () => assert.ok(cases.length > 0));
for (const c of cases) {
  test(`scores ${c.lo} to ${c.hi} on ${c.px} px are marked as core marks them`, () => {
    assert.deepEqual(JSON.parse(JSON.stringify(context.scoreTicks(c.lo, c.hi, c.px, c.min_px))), c.ticks);
  });
}

