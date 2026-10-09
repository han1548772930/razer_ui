// Current Dashboard row presentation, parsed as data; no vendor execution.
const fs = require('fs'), path = require('path');
const {Source, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/dashboard');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const fact = (condition, message) => { if (!condition) throw Error(message); };
const contracts = [[44442, 'w'], [44442, 'L'], [75551, 'o']].map(([module, symbol]) => ({module, symbol, ...source.receipt(module, source.binding(module, symbol))}));
const animation = contracts.find(c => c.module === 75551).source;
for (const fragment of ['if(t<r.current)', 'r.current<100', 's(100)', 'e.setAttribute("data-transition","0"),s(0)', 'e.removeAttribute("data-transition"),s(t),r.current=t']) fact(animation.includes(fragment), `Progress animation changed: ${fragment}`);
const files = ['6505.9782778c.chunk.css', '55.4e8559cb.chunk.css'];
const css = files.flatMap(name => {
  const file = `${source.directory}/static/css/${name}`, text = read(file);
  return parseCSS(text).filter(rule => /progress-bar|firmwareDescription|item-description-image|item-description-info|updateWarning/.test(rule.selector)).map(rule => ({path:file, sha256:hash(text), ...rule}));
});
const base = css.find(rule => rule.selector === '.progress-bar');
const indicator = css.find(rule => rule.selector === '.progress-bar .progress');
fact(base && indicator, 'Missing progress CSS');
const has = (rule, property, value) => rule.properties.some(d => d.property === property && d.value === value);
fact(has(base, 'background-color', '#2c5824') && has(base, 'overflow', 'hidden'), 'Track color or clipping changed');
fact(has(indicator, 'border-radius', '15px') && has(indicator, 'transition', 'width .3s linear'), 'Indicator shape or timing changed');
const link = css.find(rule => rule.selector === '.firmware .firmwareDescription .rightContent a:after');
fact(link && has(link,'right','-25px') && has(link,'top','-2px') && has(link,'width','20px') && has(link,'height','20px'), 'Firmware link geometry changed');
const original = `${source.directory}/static/media/icon_external_link.48227e72.svg`, asset = 'assets/synapse/external-link.svg';
fact(read(original) === read(asset), 'Existing external-link image differs from current Dashboard');
fact(read('assets/synapse/embedded.rs').includes('synapse/external-link.svg'), 'External-link is not embedded');
const native = read('crates/razer-app-pages/src/module_service_rows.rs');
const compact = native.replace(/\s+/g, '');
for (const fragment of ['struct ServiceProgressBar', 'window.use_keyed_state', 'Duration::from_millis(300)', 'Duration::from_millis(1)', '.rounded(surface::css(15.))', '.overflow_hidden()', '.value(self.percent)', '.bg(MainPageColors.service_progress_track())', '.bg(MainPageColors.service_progress_fill())', '.right(surface::css(-25.))', '.top(surface::css(-2.))', 'service_icon_with_warning(row, None, warning, cx)', 'RESTART_SYNAPSE_REQUIRED']) fact(compact.includes(fragment.replace(/\s+/g,'')), `Missing native row presentation: ${fragment}`);
fact(!native.includes('crate::resources::dashboard_image('), 'Description must not substitute dashboard artwork for detail.srcImage');
const theme = read('crates/razer-widgets/src/theme.rs');
fact(/fn service_progress_track[\s\S]*?rgb\(0x2c5824\)/.test(theme) && /fn service_progress_fill[\s\S]*?rgb\(0x44d62c\)/.test(theme), 'Source progress palette missing');
const report = {
  method: 'Manifest-owned current Dashboard AST/CSS and existing asset hash comparison; no reference execution.',
  contracts, css,
  external_link: {source:original, output:asset, sha256:hash(read(asset))},
  native: {
    progress:'Retained RenderOnce presentation starts at zero and linearly approaches observed targets in 300ms. Decrease fills to 100 when the previous target is below 100, resets to zero in 1ms, then approaches the new target in 300ms. Track is #2c5824, 200x8px, radius 4px and clipped; indicator is #44d62c with radius 15px.',
    observations:'Only width is sampled. Service snapshot and install phase never change; accessibility exposes the observed percentage. Reduced motion displays the target immediately.',
    image:'w consumes detail.srcImage, which is not established as the ae dashboard thumbnail. The native description leaves its 288x162 image area empty pending a proven embedded mapping.',
    firmware:'Only firmware severity warning colors the category icon; installed/new rows remain ordinary. Firmware title prefers productName before title. Current external-link.svg is used at the original 20px, right -25px/top -2px position.',
    limits:['Overlapping source transitionend callback races are not reproduced; a newer observation starts a new visual sequence from the displayed width.', 'The 1ms reset is clock-sampled; it may not occupy a whole display frame.', 'Description error fallback artwork is not shown without observing an image request failure.', 'No app/build/tests/vendor JS/DLL ran; pixel geometry, OS font metrics and animation have not been visually verified.'],
  },
};
const file = 'docs/re/module-service-rows-current-evidence.json', output = JSON.stringify(report,null,2)+'\n';
if (process.argv.includes('--check')) fact(read(file) === output, 'Stale service row evidence');
else fs.writeFileSync(path.join(root,file),output);
console.log('Current service rows: 75551 progress, detail image boundary, firmware link and warning presentation verified statically.');
