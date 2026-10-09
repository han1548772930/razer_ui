// Current mounted Dashboard content only. Vendor code is parsed, never run.
const fs = require('fs'), path = require('path');
const {Source, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/dashboard');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const requireFact = (fact, message) => { if (!fact) throw Error(message); };
const entries = [[22534,'ee'],[22534,'de'],[22534,'Pi'],[44442,'w'],[44442,'O'],[44442,'L'],[44442,'H'],[44442,'ae'],[19388,'u'],[19388,'He']];
const contracts = entries.map(([module, symbol]) => ({module,symbol,...source.receipt(module,source.binding(module,symbol))}));
const css = ['55.4e8559cb.chunk.css','6505.9782778c.chunk.css'].map(name => {
  const file = `${source.directory}/static/css/${name}`, text = read(file);
  return {path:file, sha256:hash(text), rules:parseCSS(text).filter(r =>
    name.startsWith('6505') || /^(?:body,html|div\.nav-tabs|\.nav-tabs \.nav(?:[ :.]|$)|\.body-wrapper$|\.dashboard$|\.dashboard\.reflow|\.box-no-device|\.box-item \.name-tag|\.progress-bar-wrapper|#gamerRoom \.gr-banner)/.test(r.selector))};
});
const styles = css.flatMap(file => file.rules);
const declaration = (selector, part) => styles.some(r => r.selector === selector && r.declarations.includes(part));
requireFact(declaration('.items .item .item-main-content .info-text', 'font-size:14px'), 'Source service info size changed');
requireFact(declaration('.items .item .item-main-content .item-tooltip .tip','right:-160px') && declaration('.items .item .item-main-content .item-tooltip .tip','top:32px'), 'Source offline tooltip anchor changed');
const w = contracts.find(c => c.module === 44442 && c.symbol === 'w').source;
requireFact(w.includes('case"canceled":r=s?') && w.includes(' disabled no-internet'), 'Offline/canceled source branch changed');
const rows = read('crates/razer-app-pages/src/module_service_rows.rs'), removal = read('crates/razer-app-pages/src/module_service_remove.rs');
requireFact(/if phase == "error"[\s\S]*?text_size\(surface::css\(14\.\)\)/.test(rows), 'Missing error 14px correction');
requireFact(/let action = if removing[\s\S]*?text_size\(surface::css\(14\.\)\)/.test(removal), 'Missing removing 14px correction');
for (const fragment of ['struct ServiceOfflineAction', '.right(surface::css(-160.))', '.top(surface::css(32.))', '.line_height(surface::css(16.))', '"downloading" | "installing" | "canceled"']) requireFact(rows.includes(fragment), `Missing native tooltip contract: ${fragment}`);
requireFact(!rows.includes('gpui_kit::component::tooltip::Tooltip::new'), 'Generic service tooltip remains');
// Read sfnt tables with standard Buffer operations. This is not font execution.
function fontMetrics(file) {
  const b = fs.readFileSync(path.join(root,file)), tables = {};
  for (let i=0;i<b.readUInt16BE(4);i++) { const p=12+i*16; tables[b.toString('ascii',p,p+4)] = b.readUInt32BE(p+8); }
  const head=tables.head, hhea=tables.hhea, os=tables['OS/2'];
  return {path:file,sha256:hash(b),units_per_em:b.readUInt16BE(head+18),hhea:{ascent:b.readInt16BE(hhea+4),descent:b.readInt16BE(hhea+6),line_gap:b.readInt16BE(hhea+8)},os2:{weight:b.readUInt16BE(os+4),selection:b.readUInt16BE(os+62),typo_ascent:b.readInt16BE(os+68),typo_descent:b.readInt16BE(os+70),typo_line_gap:b.readInt16BE(os+72),win_ascent:b.readUInt16BE(os+74),win_descent:b.readUInt16BE(os+76)}};
}
const fontManifest = JSON.parse(read('assets/synapse/manifest.json')).entries.filter(e=>e.output.endsWith('.ttf'));
const alexaManifest = JSON.parse(read('.ref/applications/synapse/alexa/asset-manifest.json'));
const fontFaceSources = [css[0].path,'.ref/host-4.0.827/electron/assets/style/Roboto.css',`.ref/applications/synapse/alexa/${alexaManifest.files['main.css'].slice(2)}`].map(file=>({path:file,sha256:hash(read(file)),font_faces:read(file).match(/@font-face\{[^}]+\}/g)}));
for(const font of fontManifest) {
  const filename = path.basename(font.source);
  requireFact(hash(fs.readFileSync(path.join(root,font.source)))===font.source_sha256,'Changed source font '+filename);
  requireFact(hash(fs.readFileSync(path.join(root,font.output)))===font.sha256,'Changed converted font '+filename);
  requireFact(fontFaceSources.some(source=>source.font_faces.some(face=>face.includes(filename)&&face.includes(`font-family:${font.css_face.family};`)&&face.includes(`font-weight:${font.css_face.weight};`))), 'No current CSS face for '+filename);
  requireFact(read('crates/razer-assets/src/lib.rs').includes(path.basename(font.output)),'Missing native font registration '+filename);
}
const report = {
  method:'Current manifest-owned AST, mounted CSS and font-table reads; no application/build/test/vendor JS/DLL execution.',
  contracts,css,fontFaceSources,fonts:fontManifest.map(entry=>({...entry,metrics:fontMetrics(entry.output)})),
  corrected:['Roboto Light 300, RazerF5 Thin 100 and RazerF5 Bold 700 are now registered. Font metadata applies the current CSS family/weight aliases; source outline, cmap, layout and advance tables are unchanged.','Installing error and removing labels use the mounted info-text 14px, not body 16px.','Offline install/retry tooltip is a row-relative 14px/16px CSS surface at right -160/top 32, with 300ms linear opacity and immediate visibility changes.','Downloading/installing and canceled-offline branches do not show the offline tooltip.'],
  unresolved:['CSS normal line-height is font/backend dependent. GPUI defaults to phi(); no arbitrary fixed 1.2/17px replacement is asserted equivalent.','Current CSS uses Roboto,sans-serif or RazerF5,sans-serif and some font faces prefer local fonts. GPUI registered font priority and OS CJK fallback are not proven identical; no guessed CJK family is forced.','Tooltip pre-wrap space preservation, shrink-to-fit width and native clipped overlap require further renderer comparison.','Dashboard state trees now have a dedicated dashboard-card-state-current-evidence.json receipt. Service transport, firmware-row smooth scrolling and native visual equivalence remain incomplete.','Gamer Room missing backdrop blur and CSS normal title line-height remain open; overview comparison does not verify all device/hover/tutorial subtrees.','Main body wrapper responsive containing blocks require full route-level cascade verification; no global wrapper rule was changed.','Settings service previews are developer wrappers; sharing a body component does not validate its complete appearance.','No pixel or runtime visual comparison was performed.'],
};
const target='docs/re/shell-content-consistency-current-evidence.json', output=JSON.stringify(report,null,2)+'\n';
if(process.argv.includes('--check')) requireFact(read(target)===output,'Stale shell content consistency evidence');
else fs.writeFileSync(path.join(root,target),output);
console.log(`Shell content: ${contracts.length} mounted contracts, ${styles.length} CSS rules, ${report.fonts.length} font tables; documented partial coverage.`);
