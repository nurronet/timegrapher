// Each figure's base decimals as core gives them (figure_text::base_digits),
// read from core's source, so the tests hold the view to core's numbers
// the way the server fills in the page's BASE_DIGITS.
const fs = require("node:fs");
const path = require("node:path");
const lib = fs.readFileSync(path.join(__dirname, "../../timegrapher-core/src/figure_text.rs"), "utf8");
const block = lib.match(/pub mod base_digits \{([\s\S]*?)\n\}/)[1];
const BASE_DIGITS = Object.fromEntries([...block.matchAll(/pub const (\w+): usize = (\d+);/g)]
  .map(([, name, d]) => [name.toLowerCase(), Number(d)]));
// The screen's minus sign (figure_text::true_minus), as the server puts it
// in the page's MINUS.
const minus = lib.match(/pub fn true_minus\(text: &str\) -> String \{\s*text\.replace\('-', "\\u\{([0-9a-fA-F]+)\}"\)/);
if (!minus) throw new Error("Missing core figure_text::true_minus");
const MINUS = String.fromCodePoint(parseInt(minus[1], 16));
module.exports = { BASE_DIGITS, MINUS };
