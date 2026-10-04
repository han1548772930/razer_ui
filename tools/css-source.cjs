// Read CSS rules as data, preserving enclosing @media/@supports blocks.
// This is a static receipt parser, not a browser cascade/layout engine.
function boundary(text, start, end, stops) {
  let quote = '', parens = 0, brackets = 0;
  for (let i = start; i < end; i++) {
    const ch = text[i];
    if (ch === '\\') { i++; continue; }
    if (quote) { if (ch === quote) quote = ''; continue; }
    if (ch === '"' || ch === "'") { quote = ch; continue; }
    if (ch === '/' && text[i + 1] === '*') {
      const close = text.indexOf('*/', i + 2);
      if (close < 0) throw Error('Unclosed CSS comment');
      i = close + 1; continue;
    }
    if (ch === '(') parens++;
    else if (ch === ')') parens--;
    else if (ch === '[') brackets++;
    else if (ch === ']') brackets--;
    if (!parens && !brackets && stops.includes(ch)) return i;
  }
  return end;
}
function declarations(text) {
  const result = [];
  for (let start = 0; start < text.length;) {
    const end = boundary(text, start, text.length, ';');
    const pair = text.slice(start, end), colon = boundary(pair, 0, pair.length, ':');
    if (colon < pair.length) {
      const value = pair.slice(colon + 1).trim();
      result.push({property: pair.slice(0, colon).trim(), value: value.replace(/\s*!important\s*$/i, ''),
        important: /!important\s*$/i.test(value)});
    }
    start = end + 1;
  }
  return result;
}
function parseCSS(text) {
  const rules = [];
  function block(start, end, conditions) {
    while (start < end) {
      const stop = boundary(text, start, end, '{;');
      if (stop === end) break;
      if (text[stop] === ';') { start = stop + 1; continue; }
      const selector = text.slice(start, stop).replace(/\/\*[\s\S]*?\*\//g, '').trim();
      let cursor = stop + 1, depth = 1;
      while (depth) {
        cursor = boundary(text, cursor, end, '{}');
        if (cursor === end) throw Error(`Unclosed CSS block: ${selector}`);
        depth += text[cursor] === '{' ? 1 : -1;
        cursor++;
      }
      const close = cursor - 1;
      if (/^@(media|supports|container|layer|scope)\b/i.test(selector)) {
        block(stop + 1, close, [...conditions, selector]);
      } else if (!selector.startsWith('@')) {
        const body = text.slice(stop + 1, close);
        rules.push({offset: start, selector, conditions, declarations: body, properties: declarations(body)});
      }
      start = cursor;
    }
  }
  block(0, text.length, []);
  return rules;
}
module.exports = {parseCSS, declarations};
