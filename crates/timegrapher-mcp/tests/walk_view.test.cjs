// Execute the view's own Isochronism card (#741) in Hugh's look C on core's
// time colour map: the server's points as core merged them, a trail
// shading from each dot's colour to the next and lifted where core says,
// rings at the start and at core's hour beads, a large dot at the end,
// each named; the axes named as the app's strip names them, ending where
// core's walk_axes says; the curve's headline a fact a line; the key in
// core's words on the same map.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML || path.join(__dirname, "../src/view.html"), "utf8");
// Core's time colour map as the server puts it in (views.rs holds the
// fixture to core).
const TIME_MAP = JSON.parse(fs.readFileSync(path.join(__dirname, "fixtures/time_map.json"), "utf8"));
const viewsRs = fs.readFileSync(path.join(__dirname, "../../timegrapher-core/src/steady_views.rs"), "utf8");
// A core string constant, written out or named after another one
// (WALK_AMP_NAME is AMPLITUDE_SHORT, #1061).
const coreStr = (name) => {
  const m = viewsRs.match(new RegExp(`pub const ${name}: &str = (?:"([^"]*)"|([A-Z_]+));`));
  return m[1] ?? coreStr(m[2]);
};
const WALK_WORDS = {
  title: coreStr("WALK_TITLE"), hint: coreStr("WALK_HINT"), since: coreStr("WALK_SINCE"), key_start: coreStr("WALK_KEY_START"),
  start: coreStr("WALK_START"),
  rate_name: coreStr("WALK_RATE_NAME"), rate_unit: coreStr("WALK_RATE_UNIT"),
  amp_name: coreStr("WALK_AMP_NAME"), amp_unit: coreStr("WALK_AMP_UNIT"),
};
function definition(name) {
  const match = html.match(new RegExp(`^function ${name}\\([^\\n]*\\{[\\s\\S]*?^\\}`, "m"));
  if (!match) throw new Error(`Missing renderer ${name}`);
  return match[0];
}
const constant = (name) => html.match(new RegExp(`^const ${name} = .*$`, "m"))[0];
const root = { dataset: { theme: "light" } };
const context = vm.createContext({ TIME_MAP, WALK_WORDS,
  document: { getElementById: () => ({ clientWidth: 624 }), documentElement: root } });
vm.runInContext(["CHAR_W", "DASH", "MARKS", "NAME_CLEAR"].map(constant).join("\n") +
  "\nconst WALK_PLOT = " + html.match(/^const WALK_PLOT = (.*);$/m)[1] + ";\nlet clipCount = 0;\n" +
  ["esc", "chartWidth", "plotArea", "gridDown", "gridAcross", "axisLabel", "tableRgb", "timeRgb", "rampStops",
    "walkTicks", "beadSide", "walkMarks", "xyChart", "walkCard"].map(definition).join("\n"), context);

// What core steady_views::walk_view sends, with the server's axes and lines.
const walk = {
  last_hours: 38, last_text: "38 h",
  axes: [[210, 310, 10], [-3, 5, 1]],
  summary: ["The rate falls 0.23 s/d per degree, 145° to 251°", "Flattest near 213°"],
  beads: [{ dot: 2, text: "12 h" }, { dot: 3, text: "24 h" }],
  dots: [
    { hours: 0.04, when: 0.04 / 38, joined: false, amplitude_deg: 301, rate_s_per_day: 0, hover: "0.0 h after start · 301° · 0.0 s/d" },
    { hours: 3.5, when: 3.5 / 38, joined: true, amplitude_deg: 272, rate_s_per_day: 4.1, hover: "3.5 h after start · 272° · +4.1 s/d" },
    { hours: 12, when: 12 / 38, joined: true, amplitude_deg: 255, rate_s_per_day: 2.0, hover: "12 h after start · 255° · +2.0 s/d" },
    { hours: 24, when: 24 / 38, joined: true, amplitude_deg: 235, rate_s_per_day: 0.5, hover: "24 h after start · 235° · +0.5 s/d" },
    { hours: 38, when: 1, joined: true, amplitude_deg: 211, rate_s_per_day: -2.3, hover: "38 h after start · 211° · -2.3 s/d · 6 readings" },
  ],
};
const rgb = (t) => `rgb(${context.timeRgb(t).join(",")})`;

test("the colours are core's time map for the page's theme, between its points", () => {
  for (const theme of ["dark", "light"]) {
    root.dataset.theme = theme;
    const m = TIME_MAP[theme];
    assert.deepEqual([...context.timeRgb(0)], m[0]);
    assert.deepEqual([...context.timeRgb(1)], m[m.length - 1]);
    assert.deepEqual([...context.timeRgb(0.5)], m[(m.length - 1) / 2]);
  }
  assert.notDeepEqual(TIME_MAP.dark[0], TIME_MAP.light[0], "each theme its own lift");
  root.dataset.theme = "light";
});

test("each dot says its own hover in its time's colour, over a wider target", () => {
  const out = context.walkCard(walk);
  for (const d of walk.dots) {
    assert.ok(out.includes(`<title>${d.hover}</title>`), d.hover);
    assert.ok(out.includes(`r="2.5" fill="${rgb(d.when)}"`), d.hover);
  }
  assert.equal((out.match(/r="6" fill="transparent"/g) || []).length, walk.dots.length);
});

test("the trail shades from each dot's colour to the next's and is lifted where core says", () => {
  const out = context.walkCard(walk);
  const steps = [...out.matchAll(/<linearGradient [^>]*><stop offset="0" stop-color="([^"]+)"\/><stop offset="1" stop-color="([^"]+)"\/>/g)];
  assert.equal(steps.length, 4, "four joined steps");
  assert.deepEqual([steps[0][1], steps[0][2]], [rgb(walk.dots[0].when), rgb(walk.dots[1].when)]);
  assert.ok(/stroke="url\(#walk\d+s1\)" stroke-width="2" stroke-linecap="round"/.test(out));
  const gapped = { ...walk, dots: walk.dots.map((d, i) => (i === 3 ? { ...d, joined: false } : d)) };
  const lifted = context.walkCard(gapped);
  assert.equal((lifted.match(/<linearGradient /g) || []).length, 3, "no step into the stretch core left unjoined");
  assert.ok(!/walk\d+s3"/.test(lifted));
});

test("the start is a named ring, the end a large dot named with its hour, each bead a named ring", () => {
  const out = context.walkCard(walk);
  assert.equal((out.match(/r="7" fill="var\(--card\)"/g) || []).length, 1, "the start's ring");
  assert.equal((out.match(/r="5" fill="var\(--card\)" stroke="[^"]+" stroke-width="2.5"/g) || []).length, 2, "a ring per bead");
  assert.ok(out.includes(`r="6.5" fill="${rgb(1)}" stroke="var(--card)" stroke-width="2"`), "the end's dot");
  for (const name of ["Start", "38 h", "12 h", "24 h"]) {
    assert.ok(new RegExp(`filter="url\\(#walk\\d+plate\\)" style="fill:var\\(--text\\);font-size:12px">${name}</text>`).test(out), `${name} on a plate`);
  }
  assert.ok(/<filter id="walk\d+plate"[^>]*><feFlood style="flood-color:var\(--card\);flood-opacity:0.85"\/><feComposite in="SourceGraphic"\/><\/filter>/.test(out), "the plate: the card's colour at 85%");
});

test("the end's hour goes 14 px off the trail on its roomier side, as a bead's does", () => {
  const out = context.walkCard(walk);
  const m = out.match(/<text x="([\d.]+)" y="([\d.]+)" text-anchor="(\w+)"[^>]*>38 h<\/text>/);
  assert.ok(m, out);
  // The same placement beadSide gives the last dot, 14 px out.
  const ends = [...out.matchAll(/<circle cx="([\d.]+)" cy="([\d.]+)" r="2.5"/g)].map((c) => [+c[1], +c[2]]);
  const at = context.beadSide(ends, ends.length - 1, 14);
  assert.deepEqual([+m[1], +m[2], m[3]], [+at.x.toFixed(1), +at.y.toFixed(1), at.anchor]);
});

test("a bead's name goes 11 px out, square to the trail, on the side with more room", () => {
  // A trail along x with a crowd of dots below the middle one: the name goes above.
  const P = [[0, 100], [50, 100], [100, 100], [50, 112], [60, 115]];
  const at = context.beadSide(P, 1, 11);
  assert.equal(at.anchor, "middle");
  assert.ok(Math.abs(at.y - (100 - 11)) < 1e-9, `${at.y}`);
  const flipped = context.beadSide([[0, 100], [50, 100], [100, 100], [50, 88], [60, 85]], 1, 11);
  assert.ok(Math.abs(flipped.y - (100 + 11 + 9)) < 1e-9, `${flipped.y}`);
});

test("the axes end where core says, bare numbers with a true minus, named over and beside", () => {
  const out = context.walkCard(walk);
  const labels = [...out.matchAll(/<text x="[\d.]+" y="[\d.]+" text-anchor="(?:start|middle|end)">([−\d.]+)<\/text>/g)].map((m) => m[1]);
  for (const t of ["210", "310", "−3", "5", "0"]) assert.ok(labels.includes(t), `${t} in ${labels}`);
  assert.ok(!labels.some((t) => t.includes("-")), "no hyphen minus");
  assert.ok(/>Rate<\/text><text[^>]*>s\/d<\/text>/.test(out), out);
  assert.ok(out.includes(">Amp. °</text>"));
});

test("the plot is square within 280 to 420 px", () => {
  const plotH = (out) => +out.match(/<rect class="plot" x="[\d.]+" y="[\d.]+" width="[\d.]+" height="([\d.]+)"/)[1];
  const plotW = (out) => +out.match(/<rect class="plot" x="[\d.]+" y="[\d.]+" width="([\d.]+)"/)[1];
  const wide = context.walkCard(walk);
  assert.equal(plotH(wide), 420);
  context.document.getElementById = () => ({ clientWidth: 400 });
  const narrow = context.walkCard(walk);
  assert.ok(plotH(narrow) === 306 && plotW(narrow) >= 300, `${plotH(narrow)} ${plotW(narrow)}`);
  context.document.getElementById = () => ({ clientWidth: 300 });
  assert.equal(plotH(context.walkCard(walk)), 280);
  context.document.getElementById = () => ({ clientWidth: 624 });
});

test("the curve's headline goes over the walk a fact a line, then the hint", () => {
  const out = context.walkCard(walk);
  assert.ok(out.includes('<h2>Isochronism</h2><div class="facts"><p>The rate falls 0.23 s/d per degree, 145° to 251°</p><p>Flattest near 213°</p></div><p class="hint">Rate at each amplitude as the watch runs down.</p>'), out);
  assert.ok(!context.walkCard({ ...walk, summary: [] }).includes('class="facts"'));
});

test("the key runs from the take's start to the last point on the same map, in core's words", () => {
  const out = context.walkCard(walk);
  assert.ok(out.includes(`<span>0 h</span><span class="ramp" style="background:linear-gradient(to right, ${context.rampStops(context.timeRgb)})"></span><span>38 h</span><span>since start</span>`), out);
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
