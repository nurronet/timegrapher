// Execute the view's own Steadiness card: each plot's title line carries
// core's summary, the full text sits in the Info panel (closed until asked
// for), and the Readings view keys its dots.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML || path.join(__dirname, "../src/view.html"), "utf8");
function definition(name) {
  const match = html.match(new RegExp(`^function ${name}\\([^\\n]*\\{[\\s\\S]*?^\\}`, "m"));
  if (!match) throw new Error(`Missing renderer ${name}`);
  return match[0];
}
const constant = (name) => html.match(new RegExp(`^const ${name} = [\\s\\S]*?;\\n(?=\\S|\\n)`, "m"))[0];
const context = vm.createContext({ fmtTime: (t) => `${t / 60}:00`, secs: (t) => `${t} s`, spectrumOf: () => null, steadyView: "readings",
  periodsLog: false, fftTiming: false, mapsLog: false, showWheels: true, showInfo: false });
vm.runInContext([constant("SERIES"), constant("STEADY_VIEWS"), constant("INFO_HINT"), constant("WHEELS_HINT"),
  constant("SWITCH_DEFAULTS"), constant("viewSwitches"),
  ...["esc", "num", "pText", "steadyStats", "steadySwitch", "switchView", "dotKey", "banded", "steadinessCard"].map(definition)].join("\n"), context);

const rate = { series: "rate", unit: "s/d", readings: 179, step_s: 10, median: 1.5, autocorrelation: { p_value: 0.5 },
  headline: "The rate repeats every 46 s, 4.0 s/d peak to peak.", summary: "repeats every 46 s, 4.0 s/d peak to peak" };
const doc = { duration_s: 1800, series: [rate] };

test("each plot's title line carries the series and core's summary; the full text waits in Info", () => {
  context.steadyPlot = () => "<svg/>";
  context.showInfo = false;
  const shut = context.steadinessCard(doc);
  assert.ok(shut.includes('title="Rate · repeats every 46 s, 4.0 s/d peak to peak">Rate<span class="summary"> · repeats every 46 s, 4.0 s/d peak to peak</span>'), shut);
  assert.ok(!shut.includes(rate.headline) && !shut.includes("Tested on"), "closed: no headline, no statistics");
  assert.ok(shut.includes('data-switch="info" aria-pressed="false"'));
  context.showInfo = true;
  const open = context.steadinessCard(doc);
  assert.ok(open.includes(rate.headline) && open.includes("Tested on 179 10 s readings") && open.includes("Tested over 30:00 of beats"), open);
  context.showInfo = false;
});

test("a trend or a level split the headline leaves out says why, in core's words", () => {
  const holm = "not significant after Holm's correction (p 0.031 alone)";
  const stats = context.steadyStats({ ...rate,
    trend: { per_hour: 0.42, note: holm },
    changes: { segments: [{}, {}, {}], note: "under the 20% a level change needs" } });
  assert.ok(stats.includes(`trend +0.42 s/d per hour, ${holm}`), stats);
  assert.ok(stats.includes("3 stretches of one level, under the 20% a level change needs"), stats);
  // A trend the headline names has no note; one stretch is no split.
  const named = context.steadyStats({ ...rate, trend: { per_hour: -1.5, note: null }, changes: { segments: [{}], note: null } });
  assert.ok(named.endsWith("trend -1.50 s/d per hour"), named);
  assert.ok(!named.includes("stretches"), named);
});

test("the Readings view's key says what a dot is and how many are drawn", () => {
  assert.equal(context.dotKey({ step_s: 10, n: 179, counts: [1, 1] }, 179), "10 s readings (179)");
  assert.equal(context.dotKey({ step_s: 2, n: 912, counts: [1, 1] }, 912), "2 s windows (912)");
  assert.equal(context.dotKey({ step_s: 2, n: 9000, counts: Array(3000).fill(3) }, 3000), "2 s windows in 3,000 bands (9,000)");
  // The readings the bands hold, not the take's slots: a missing one is in no band.
  assert.equal(context.dotKey({ step_s: 2, n: 9001, counts: [3, 2] }, 2), "2 s windows in 2 bands (5)");
  assert.equal(context.dotKey({ step_s: 20, n: 90, counts: [1] }, 90), "20 s readings (90)");
  assert.equal(context.dotKey({ step_s: 5, n: 40, counts: [1] }, 40), "5 s windows (40)");
});

test("the Long Recording view says what limits its readings, as the analyze and session views do", () => {
  const c = vm.createContext({ steadinessCard: () => "<steady/>", walkCard: () => "", distributionCards: () => "<dist/>", fileName: () => "a.wav", timeLabel: String });
  vm.runInContext(["esc", "limitsCard", "seriesView"].map(definition).join("\n"), c);
  const limits = [{ where: null, text: "The beat error's sign differs either side of a cut in the stretch, so it is given as a size." }];
  const html = c.seriesView({ duration_s: 60 }, null, limits);
  assert.ok(html.includes("<steady/><div class=\"card\"><h2>What Limits These Readings</h2>") && html.includes("given as a size.</li>"), html);
  assert.ok(!c.seriesView({ duration_s: 60 }, null, []).includes("What Limits"));
});

test("a series' own caveat shows once, on that series, in the server's words", () => {
  const c = vm.createContext({ fmtTime: (t) => `${t / 60}:00`, secs: (t) => `${t} s`, spectrumOf: () => null, steadyView: "readings",
    periodsLog: false, fftTiming: false, mapsLog: false, showWheels: true, showInfo: false, steadyPlot: () => "<svg/>",
    distributionCards: () => "", walkCard: () => "", fileName: () => "a.wav", timeLabel: String });
  vm.runInContext([constant("SERIES"), constant("STEADY_VIEWS"), constant("INFO_HINT"), constant("WHEELS_HINT"),
    constant("SWITCH_DEFAULTS"), constant("viewSwitches"),
    ...["esc", "num", "pText", "steadyStats", "steadySwitch", "switchView", "dotKey", "banded", "steadinessCard", "limitsCard", "seriesView"].map(definition)].join("\n"), c);
  // The sentence core's PeriodMayBeNoise gives; the view draws it as sent.
  const noise = "On a take this short with its 2 s windows this correlated (ρ 0.74 from one to the next, over 30 min), the period search names a cycle in noise alone about 1 time in 10, not 1 in 33: this cycle may be noise.";
  const series = [rate, { ...rate, series: "amplitude", unit: "°", caveats: [{ code: "period_may_be_noise", rho: 0.74, span_s: 1800 }] },
    { ...rate, series: "beat_error", unit: "ms" }];
  const take = [{ where: null, text: "The beat error is given as a size." }];
  const out = c.seriesView({ duration_s: 1800, series }, null, take, [[], [noise], []]);
  assert.equal(out.split(noise).length - 1, 1, "said once");
  const at = out.indexOf(noise);
  assert.ok(out.lastIndexOf('<p class="series-name"', at) === out.indexOf('<p class="series-name" title="Amplitude'), "under the Amplitude title");
  assert.ok(at < out.indexOf('<p class="series-name" title="Beat Error'), "before the next series");
  const limitsCard = out.slice(out.indexOf("What Limits These Readings"));
  assert.ok(!limitsCard.includes(noise), "not with the take's caveats");
  // The document the tool returned is left as it was.
  assert.equal(series[1].limits, undefined);
  // With no series caveats from the server, none is drawn.
  assert.ok(!c.seriesView({ duration_s: 1800, series }, null, take).includes("may be noise"));
});
