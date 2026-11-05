// Execute the view's own charts over time: each states the averaging time
// the server read them at, and takes its scale from the server.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { BASE_DIGITS } = require("./core_digits.cjs");
const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML || path.join(__dirname, "../src/view.html"), "utf8");
const definition = (name) => {
  const match = html.match(new RegExp(`^function ${name}\\([^\\n]*\\{[\\s\\S]*?^\\}`, "m"));
  if (!match) throw new Error(`Missing renderer ${name}`);
  return match[0];
};
const charts = [];
const context = vm.createContext({
  BASE_DIGITS,
  num: (v, d) => v.toFixed(d),
  signed: (v, d) => (v >= 0 ? "+" : "") + v.toFixed(d),
  lineChart: (c) => { charts.push(JSON.parse(JSON.stringify(c))); return "<svg/>"; },
});
vm.runInContext(definition("rateBasis") + "\n" + definition("stripCards"), context);

const strip = {
  duration_s: 30, average_s: 20,
  y_range: { rate: [4, 5], amplitude: [260, 280], beat_error: [-0.5, -0.3] },
  rate: [[2, 4.5]], amplitude: [[2, 269]], beat_error_unlock: [[2, -0.38]],
};

test("each chart over time names the averaging time and keeps the server's scale", () => {
  charts.length = 0;
  const markup = context.stripCards(strip);
  assert.equal(charts[0].series[0].name, "Rate in seconds per day, 20 s average");
  assert.deepEqual(charts.map((c) => c.yRange), [[4, 5], [260, 280], [-0.5, -0.3]]);
  assert.equal((markup.match(/20 s average<\/p>/g) || []).length, 3, "every chart's hint");
  assert.ok(markup.includes("on the sound card's clock, 20 s average"), markup);
});

test("a rate on a corrected clock says so, with the correction", () => {
  const markup = context.stripCards({ ...strip, clock_ppm: 20 });
  assert.ok(markup.includes("corrected for the sound card's clock (+20.0 ppm), 20 s average"), markup);
});

test("without an averaging time no average is claimed", () => {
  charts.length = 0;
  const markup = context.stripCards({ ...strip, average_s: null });
  assert.equal(charts[0].series[0].name, "Rate in seconds per day");
  assert.ok(!markup.includes("average"));
});

test("a line lifts where a reading has no figure, as the app leaves a cut empty", () => {
  const constant = (n) => html.match(new RegExp(`^const ${n} = [\\s\\S]*?;$`, "m"))[0].replace("const ", "var ");
  const real = vm.createContext({ chartWidth: () => 600, CHAR_W: 6.6, clipCount: 0 });
  vm.runInContext(["DASH", "TIME_STEPS", "LONE_DOT_R", "charts"].map(constant).join("\n") + "\n" +
    ["plotArea", "gridDown", "gridAcross", "lineSample", "lineChart", "nearest", "roundStep", "gridFor", "axisLabel", "esc", "fmtTime", "markLabels"].map(definition).join("\n"), real);
  const svg = real.lineChart({
    title: "Rate", time: true, yRange: [4, 5], unit: " s/d", decimals: 1,
    series: [{ name: "Rate", color: "--rate", points: [[1, 4.5], [2, 4.6], [3, null], [6, 4.7], [7, 4.8]] }],
  });
  const d = svg.match(/<path d="([^"]*)"/)[1];
  assert.equal((d.match(/M/g) || []).length, 2, d);
  assert.ok(!svg.includes("off the scale"), "a gap is not an outlier");
});

test("isolated live source readings are visible dots without joining across holes", () => {
  const constant = (n) => html.match(new RegExp(`^const ${n} = [\\s\\S]*?;$`, "m"))[0].replace("const ", "var ");
  const real = vm.createContext({ chartWidth: () => 600, CHAR_W: 6.6, clipCount: 0 });
  vm.runInContext(["DASH", "TIME_STEPS", "LONE_DOT_R", "charts"].map(constant).join("\n") + "\n" +
    ["plotArea", "gridDown", "gridAcross", "lineSample", "lineChart", "nearest", "roundStep", "gridFor", "axisLabel", "esc", "fmtTime", "markLabels"].map(definition).join("\n"), real);
  for (const points of [[[4, 0.2]], [[4, NaN], [40, 0.5], [44, NaN]], [[4, 0.2], [8, NaN], [12, 0.4]]]) {
    const svg = real.lineChart({ title: "Beat Error", time: true, unit: " ms",
      series: [{ name: "From the Drop", color: "--beat-error", points }] });
    const count = points.filter(([, y]) => Number.isFinite(y)).length;
    assert.equal((svg.match(/<circle /g) || []).length, count, svg);
    assert.ok(!svg.match(/<path d="[^"]*L/), "isolated points must not be joined");
  }
});

test("a lone reading at the first or newest time is a dot drawn whole, bigger than the line", () => {
  const constant = (n) => html.match(new RegExp(`^const ${n} = [\\s\\S]*?;$`, "m"))[0].replace("const ", "var ");
  const real = vm.createContext({ chartWidth: () => 600, CHAR_W: 6.6, clipCount: 0 });
  vm.runInContext(["DASH", "TIME_STEPS", "LONE_DOT_R", "charts"].map(constant).join("\n") + "\n" +
    ["plotArea", "gridDown", "gridAcross", "lineSample", "lineChart", "nearest", "roundStep", "gridFor", "axisLabel", "esc", "fmtTime", "markLabels"].map(definition).join("\n"), real);
  // Walter's live Beat Error: lone drop readings at 0:04 and 0:40, the ends of the axis.
  const svg = real.lineChart({ title: "Beat Error", time: true, unit: " ms", yRange: [0, 1],
    series: [{ name: "From the Drop", color: "--beat-error", points: [[4, 0.4], [10, NaN], [20, 0.5], [24, 0.6], [30, NaN], [40, 0.45]] }] });
  const clips = Object.fromEntries([...svg.matchAll(/<clipPath id="([^"]+)"><rect x="([^"]+)" y="([^"]+)" width="([^"]+)" height="([^"]+)"\/>/g)]
    .map(([, id, x, y, w, h]) => [id, { x: +x, y: +y, w: +w, h: +h }]));
  const groups = [...svg.matchAll(/<g clip-path="url\(#([^)]+)\)">([\s\S]*?)<\/g>/g)];
  const dots = groups.flatMap(([, id, body]) => [...body.matchAll(/<circle cx="([^"]+)" cy="([^"]+)" r="([^"]+)"/g)]
    .map(([, cx, cy, r]) => ({ clip: clips[id], cx: +cx, cy: +cy, r: +r })));
  assert.equal(dots.length, 2, svg);
  for (const d of dots) {
    assert.ok(d.r >= 3, `a dot of radius ${d.r} is too small to see beside the 1.8 px line`);
    assert.ok(d.cx - d.r >= d.clip.x - 1e-9 && d.cx + d.r <= d.clip.x + d.clip.w + 1e-9, `the dot at ${d.cx} is cut by its clip`);
  }
  // Only across: the dots' clip is as tall as the plot, and the lines keep the plot's clip.
  const plot = svg.match(/<rect class="plot" x="([^"]+)" y="([^"]+)" width="([^"]+)" height="([^"]+)"/).slice(1).map(Number);
  assert.deepEqual([dots[0].clip.y, dots[0].clip.h], [plot[1], plot[3]]);
  const lines = groups.find(([, , body]) => body.includes("<path d="));
  assert.deepEqual(Object.values(clips[lines[1]]), plot);
});
