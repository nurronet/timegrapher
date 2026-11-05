// Execute the view's own Isochronism card (#741): the server's points as
// core merged them, each dot coloured by when on the Cycle view's viridis
// and saying its own hover, joined in time order under the dots, the axes
// named as the app's strip names them, and the key in core's words.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML || path.join(__dirname, "../src/view.html"), "utf8");
const palette = fs.readFileSync(path.join(__dirname, "../../timegrapher-core/src/palette.rs"), "utf8");
const VIRIDIS = JSON.parse(palette.match(/pub const VIRIDIS: \[\[u8; 3\]; 17\] = (\[[\s\S]*?\]);/)[1].replace(/,\s*\]$/, "]"));
const viewsRs = fs.readFileSync(path.join(__dirname, "../../timegrapher-core/src/steady_views.rs"), "utf8");
// A core string constant, written out or named after another one
// (WALK_AMP_NAME is AMPLITUDE_SHORT, #1061).
const coreStr = (name) => {
  const m = viewsRs.match(new RegExp(`pub const ${name}: &str = (?:"([^"]*)"|([A-Z_]+));`));
  return m[1] ?? coreStr(m[2]);
};
const WALK_WORDS = {
  title: coreStr("WALK_TITLE"), hint: coreStr("WALK_HINT"), since: coreStr("WALK_SINCE"), key_start: coreStr("WALK_KEY_START"),
  rate_name: coreStr("WALK_RATE_NAME"), rate_unit: coreStr("WALK_RATE_UNIT"),
  amp_name: coreStr("WALK_AMP_NAME"), amp_unit: coreStr("WALK_AMP_UNIT"),
};
function definition(name) {
  const match = html.match(new RegExp(`^function ${name}\\([^\\n]*\\{[\\s\\S]*?^\\}`, "m"));
  if (!match) throw new Error(`Missing renderer ${name}`);
  return match[0];
}
const constant = (name) => html.match(new RegExp(`^const ${name} = .*$`, "m"))[0];
const context = vm.createContext({ VIRIDIS, WALK_WORDS, document: { getElementById: () => ({ clientWidth: 624 }) } });
vm.runInContext(["CHAR_W", "DASH", "MARKS", "NAME_CLEAR"].map(constant).join("\n") + "\nlet clipCount = 0;\n" +
  ["esc", "chartWidth", "plotArea", "gridDown", "gridAcross", "roundStep", "gridFor", "axisLabel", "viridisRgb", "viridisStops",
    "roundAxis", "xyChart", "walkCard"].map(definition).join("\n"), context);

// What core steady_views::walk_view sends: hours, where in the take for
// the colour, amplitude, rate, hover.
const walk = {
  last_hours: 38, last_text: "38 h",
  dots: [
    { hours: 0.04, when: 0.04 / 38, joined: false, amplitude_deg: 301, rate_s_per_day: 0, hover: "0.0 h after start · 301° · 0.0 s/d" },
    { hours: 3.5, when: 3.5 / 38, joined: true, amplitude_deg: 272, rate_s_per_day: 4.1, hover: "3.5 h after start · 272° · +4.1 s/d" },
    { hours: 38, when: 1, joined: true, amplitude_deg: 211, rate_s_per_day: -2.3, hover: "38 h after start · 211° · -2.3 s/d · 6 readings" },
  ],
};

test("the card says each dot's hover as core wrote it, coloured earliest to latest", () => {
  const out = context.walkCard(walk);
  assert.ok(out.startsWith('<div class="card"><h2>Isochronism</h2><p class="hint">Rate at each amplitude as the watch runs down.</p>'), out);
  for (const d of walk.dots) assert.ok(out.includes(`<title>${d.hover}</title>`), d.hover);
  const rgb = (t) => `rgb(${context.viridisRgb(t).join(",")})`;
  assert.ok(out.includes(`<g fill="${rgb(0.04 / 38)}">`) && out.includes(`<g fill="${rgb(1)}">`), out);
});

test("the joining line is at axis strength and under the dots", () => {
  const out = context.walkCard(walk);
  const join = out.indexOf('stroke="var(--axis)" stroke-width="1" stroke-linejoin="round"');
  assert.ok(join > 0 && join < out.indexOf("<circle"), out);
});

test("a gap core marks breaks the line into two runs, never joined across", () => {
  const run = (w) => {
    const out = context.walkCard(w);
    const d = out.match(/<path d="([^"]*)" fill="none" stroke="var\(--axis\)"/)[1];
    return (d.match(/M/g) || []).length;
  };
  assert.equal(run(walk), 1);
  const gapped = { ...walk, dots: walk.dots.map((d, i) => (i === 2 ? { ...d, joined: false } : d)) };
  assert.equal(run(gapped), 2);
});

test("the axes carry the strip's short names, the unit under the name", () => {
  const out = context.walkCard(walk);
  assert.ok(/>Rate<\/text><text[^>]*>\(s\/d\)<\/text>/.test(out), out);
  assert.ok(out.includes(">Amp. (°)</text>"), out);
  // Round ends: both ends of the amplitude a tick, round the 211° to 301° read.
  const ticks = [...out.matchAll(/<text x="[\d.]+" y="[\d.]+" text-anchor="(?:start|middle|end)">(\d+)<\/text>/g)].map((m) => +m[1]);
  assert.ok(ticks.includes(200) && ticks.includes(320), ticks.join());
});

test("the y axis' name stands clear of a top tick at the plot's top", () => {
  const out = context.walkCard(walk);
  const y = (re) => +out.match(re)[1];
  const unit = y(/<text x="[\d.]+" y="([-\d.]+)" text-anchor="end">\(s\/d\)<\/text>/);
  const name = y(/<text x="[\d.]+" y="([-\d.]+)" text-anchor="end">Rate<\/text>/);
  const ticks = [...out.matchAll(/<text x="[\d.]+" y="([-\d.]+)" text-anchor="end">-?[\d.]+<\/text>/g)].map((m) => +m[1]);
  const top = Math.min(...ticks);
  assert.ok(top - unit >= 12, `unit at ${unit}, top tick at ${top}`);
  assert.equal(unit - name, 14);
  assert.ok(name - 10 >= 0, "the name inside the chart");
});

test("the key runs from the take's start to the last point, in core's words", () => {
  const out = context.walkCard(walk);
  assert.ok(out.includes('<span>0 h</span><span class="ramp"') && out.includes("<span>38 h</span><span>since start</span>"), out);
});

test("no walk, no card", () => {
  assert.equal(context.walkCard(null), "");
  assert.equal(context.walkCard({ dots: [] }), "");
});

test("the Long Recording view draws the server's walk after Steadiness", () => {
  const c = vm.createContext({ steadinessCard: () => "<steady/>", walkCard: (w) => `<walk ${w.last_text}/>`, distributionCards: () => "",
    fileName: () => "a.wav", timeLabel: String });
  vm.runInContext(["esc", "limitsCard", "seriesView"].map(definition).join("\n"), c);
  assert.ok(c.seriesView({ duration_s: 60 }, { walk }, []).includes("<steady/><walk 38 h/>"));
});
