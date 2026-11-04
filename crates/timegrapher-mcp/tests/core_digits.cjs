// Each figure's base decimals as core gives them (figure_text::base_digits),
// read from core's source, so the tests hold the view to core's numbers
// the way the server fills in the page's BASE_DIGITS.
const fs = require("node:fs");
const path = require("node:path");
const lib = fs.readFileSync(path.join(__dirname, "../../timegrapher-core/src/figure_text.rs"), "utf8");
const block = lib.match(/pub mod base_digits \{([\s\S]*?)\n\}/)[1];
const BASE_DIGITS = Object.fromEntries([...block.matchAll(/pub const (\w+): usize = (\d+);/g)]
  .map(([, name, d]) => [name.toLowerCase(), Number(d)]));
module.exports = { BASE_DIGITS };
