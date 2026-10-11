// Parse current mounted source, never evaluate vendor JavaScript.
const fs = require('fs'), acorn = require('acorn'), crypto = require('crypto');
for (const pid of [2676, 2684]) {
  const locator = JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-enums-live-source.json`, 'utf8'));
  const sourcePath = locator.source.path;
  const bytes = fs.readFileSync(sourcePath), source = bytes.toString('utf8');
  const needle = 'controllerCalibrationReducer:function';
  const offset = source.indexOf(needle);
  if (offset < 0 || source.indexOf(needle, offset + 1) !== -1) throw Error(`ambiguous reducer ${pid}`);
  const start = offset + 'controllerCalibrationReducer:'.length;
  let node = acorn.parseExpressionAt(source, start, {ecmaVersion: 'latest'});
  if (node.type === 'SequenceExpression') node = node.expressions[0];
  const initName = node.body.body[0].declarations[0].init.alternate.name;
  const initNeedle = `${initName}={`;
  const initOffset = source.lastIndexOf(initNeedle, start) + initName.length + 1;
  const init = acorn.parseExpressionAt(source, initOffset, {ecmaVersion: 'latest'});
  const output = {product_id: pid, source: sourcePath,
    sha256: crypto.createHash('sha256').update(bytes).digest('hex'),
    address_unit: 'UTF-16 code units',
    initial_state: {offset: initOffset, end: init.end, source: source.slice(initOffset, init.end)},
    reducer: {offset: start, end: node.end, source: source.slice(start, node.end)}};
  fs.writeFileSync(`docs/re/gamepad-${pid}-trigger-reducer-live-source.json`, JSON.stringify(output, null, 2) + '\n');
  console.log(JSON.stringify({pid, sha256: output.sha256, offset: start, end: node.end}));
}
