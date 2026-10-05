// Current Macro source is parsed as data. No downloaded JS is evaluated.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/macro');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const contracts = [
  [58190, ['va','ha','H','F','B','V','b','k','ja','br','je','yt','La','Le']],
  [25572, ['O','R','C','P','M']], [15030, ['c']],
].flatMap(([module, names]) => names.map(name => ({module, name,
  ...source.receipt(module, source.binding(module, name))})));
if(source.exported(15030, 'cf').name !== 'c') throw Error('Pair finder export changed');
const main = source.files.find(file => /\/main\.[^/]+\.js$/.test(file));
const text = source.text(main), reducers = [], common = [];
walk(acorn.parse(text, {ecmaVersion:'latest'}), node => {
  if(node.type === 'SwitchCase' && node.test?.object?.name === 'h'
    && ['HH','Xg','T9','dt','FZ'].includes(key(node.test.property))) {
    const snippet = text.slice(node.start, node.end);
    if(!snippet.includes('Ps(e)')) return;
    reducers.push({name:key(node.test.property), action:source.literal(4173,
      source.exported(4173, key(node.test.property))), path:main, sha256:hash(text),
      offset:node.start, end:node.end, source:snippet});
  }
  if(node.type === 'VariableDeclarator' && node.id?.name === 'Gs') common.push({
    name:'Gs', path:main, sha256:hash(text), offset:node.start, end:node.end,
    source:text.slice(node.start, node.end)});
});
if(reducers.length !== 5 || common.length !== 1) throw Error('Scoped row reducers changed');
if(!reducers.find(r=>r.name==='HH').source.includes('Ms.splice(t.index+1,0,...Rs)')
  || !reducers.find(r=>r.name==='T9').source.includes('n.includes(o)')) throw Error('Insertion contract changed');
const labels = Object.fromEntries(['KUz','Ci7','Cgw','QR$','hUZ','rN5','XEq','mBO','w34','nmU','LKO','qAC','N4v','P71']
  .map(name=>[name,source.literal(37927,source.exported(37927,name))]));
const templates = [];
walk(source.module(13139).fn, node => {
  if(node.type==='Property' && ['mouse','loop'].includes(key(node.key))
    && node.value.type==='ObjectExpression') templates.push({name:key(node.key),
      ...source.receipt(13139,node.value)});
});
if(templates.length!==2) throw Error('Mouse/Loop template changed');
const manifest=JSON.parse(read(source.directory+'/asset-manifest.json'));
const css=[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css')).map(file=>{
  const path=source.directory+'/'+file.slice(2), text=read(path);
  return {path,sha256:hash(text),rules:parseCSS(text).filter(r=>/MacroItem_|data-tooltip|#drag-image|#line/.test(r.selector))};
});
const resourceManifest=JSON.parse(read('assets/synapse/manifest.json'));
const embedded=read('assets/synapse/embedded.rs');
const names=['duplicate','delete','delete-hover','drag','drag-delay','drag-keyboard','drag-mouse','drag-macro',
  'drag-launch','drag-command','drag-text','drag-loop','drag-layers'];
const assets=names.map(name=>{
  const output=`assets/synapse/macro/${name}.svg`, entry=resourceManifest.entries.find(e=>e.output===output);
  if(!entry||!entry.source.startsWith(source.directory+'/static/media/')) throw Error('Unknown asset '+output);
  const original=fs.readFileSync(path.join(root,entry.source)), bytes=fs.readFileSync(path.join(root,output));
  if(!original.equals(bytes)||hash(bytes)!==entry.sha256||hash(original)!==entry.source_sha256
    ||!embedded.includes(`"synapse/macro/${name}.svg"`)) throw Error('Unverified asset '+output);
  return entry;
});
const files=['src/features/macro_library.rs','src/shell/macro_page.rs','src/shell/macro_page/state.rs',
  'src/shell/macro_page/body.rs','src/shell/macro_page/selection.rs','src/shell/macro_page/row_actions.rs',
  'src/shell/macro_page/row_drag.rs','src/shell/macro_page/row_controls.rs'];
const native=files.map(path=>({path,sha256:hash(read(path))}));
const body=read(files[3]), actions=read(files[5]), drag=read(files[6]);
for(const token of ['this.drop_actions(drag, index + 1, cx)','this.drop_actions(drag, 0, cx)',
  'this.choose_mouse_action(index, choice as u8, cx)','self.row_controls'])
  if(!body.includes(token)) throw Error('Missing native hookup '+token);
if(body.includes('this.move_action(')||body.includes('.border_dashed()')) throw Error('Obsolete row drag remains');
if(!actions.includes('self.actions() != drag.baseline.as_slice()')
  ||!actions.includes('drag.document != self.current')||!drag.includes('!indices.contains(&pair)'))
  throw Error('Missing native stale-drag / pairing protection');
const evidence={method:'Manifest-scoped Acorn and CSS parsing, byte-equal assets and native source fingerprints. No behavior tests or runtime visual certification.',
  generator_sha256:hash(fs.readFileSync(__filename)),contracts,reducers,common,templates,labels,css,assets,native,
  contracts_applied:{
    index:'Native arrays exclude the actionBar sentinel. Drop after row i uses insertion i+1; actionBar uses zero; trailing space appends.',
    drag:'A selected row moves all selected rows in original order; an unselected row moves only itself. A moved counterpart removes its pair bound. Bounds intersect in insertion-index space; a selected target is a no-op.',
    duplicate:'R inserts after the clicked row; keyboard/mouse/Loop get fresh pair IDs. Keyboard duplicates always use down/up, even from a Sequence null-state row. Wheel 6..9 stays one mouse row.',
    delete:'Xg removes only the clicked key/mouse row, both matching Loop ends; FZ deletes exactly the selected rows. Native local IDs are not hardware IDs.',
    mouse:'O creates paired normal/Phased rows and one Sequence row. C switches 0..5 paired functions to a single 6..9 wheel row and inserts a down row when switching back. Old display-only rows have no inferred counterpart.',
    loop:'O creates start/end with the same ID; C synchronizes the numeric value; T9 repairs crossed Loop bounds. No clickable start/end toggle exists in the source.',
    presentation:'70/30 row columns, 20px action slots with 16px margins, actual 20px draggable CSS cascade, opacity 200ms, action pressed opacity .3, 35px tooltip top, source dark drag icons and 280x40 preview at pointer minus 10px.',
  },
  remaining:['Complete Phased grouping/phase targets and services are not implemented by this row patch.',
    'Source 200ms leading/trailing selection and duplicate debounce, pair-line animation and command-warning overlay remain separate work.',
    'Actual pointer capture, tooltip clipping/z-order, scrolling, focus and rendered animation require runtime verification, which is prohibited.',
    'Legacy rows without typed event data are not promoted into recorded key/button facts or inferred pairs. Drag snapshot changes reject stale drops.']};
const target=path.join(root,'docs/re/macro-row-actions-current-evidence.json'), output=JSON.stringify(evidence,null,2)+'\n';
if(process.argv.includes('--check')) { if(fs.readFileSync(target,'utf8')!==output) throw Error('Stale Macro row evidence'); }
else fs.writeFileSync(target,output);
console.log(`Macro rows: ${contracts.length} components/helpers, ${reducers.length} reducers, ${assets.length} original SVGs; static hooks checked.`);
