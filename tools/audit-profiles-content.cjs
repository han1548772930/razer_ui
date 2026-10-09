// Current mounted Profiles contents, controls and state handlers; static only.
const fs = require('fs'), path = require('path');
const {Source, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const source = new Source('synapse/profiles');
const problems = [];
const components = [[43, ['Ba', 'oe', 'Ga', '$n', 'Ua', 'Kn', 'cs', 'ta', 'xa']],
  [8844, ['m']], [3137, ['f']], [5529, ['r']], [7693, ['I']], [1867, ['p']]]
  .flatMap(([module, names]) => names.map(name => ({module, name,
    ...source.receipt(module, source.binding(module, name))})));
const code = (module, name) => source.snippet(module, source.binding(module, name));
const check = (text, snippet, context) => {
  if (!text.includes(snippet)) problems.push(`${context}: missing ${snippet}`);
};
for (const [module, name, snippets] of [
  [43, 'Ba', ['name:r.ey1,component:', 'name:r.db_,component:', 'extraClass:"no-scroll razer-profiles"']],
  [43, 'oe', ['className:"profiles-link-games"', 'className:"list-box"', '2!==this.state.filterType', 'this.state.addGameOpen.toString()']],
  [43, 'Ga', ['className:"listDevices_wrapper"', '(0,p.jsx)($n,', '(0,p.jsx)(Ua,']],
  [43, '$n', ['className:"device-tile"', 'className:"device-name"', 'className:"device-edition"']],
  [43, 'Ua', ['Y=e=>{L(t=>(0,n.A)((0,n.A)({},t),{},{activeProfileIndex:e}))}',
    'selectOption:Y', '"LINKED GAMES TO ".concat(se)', 'maxLength:32', 'const l=(0,pe.$B)(r,e)',
    'extraClass:"no-popup"', 'show:!P.showLinkedGames', 'close:B,goBack:B', 'className:"device_to_linked_game"']],
  [8844, 'm', ['removeSearchResult=()=>{this.setState({searchKey:"",searching:!1}', 'this.navRef.current', 'this.searchRef.current']],
  [3137, 'f', ['removeSearch=()=>{this.setState({searchText:""}', 'className:"content",children:this.renderAppRows()',
    'name:this.props.t(i.II$)', 'extraClass:this.props.extraClass']],
  [5529, 'r', ['void 0!==this.props.goBack?', 'this.props.name', 'className:"close"', 'className:"choose-a-mat linked-games-popup"']],
  [1867, 'p', ['t.findIndex(t=>t.name===e)']],
  [7693, 'I', ['className:"linked-game-tile ', 'r?(0,u.jsxs)("div",{className:"linked-profile"', 'extraClass:"indeterminate"']],
]) for (const snippet of snippets) check(code(module, name), snippet, `${module}/${name}`);
if (code(3137, 'f').includes('goBack')) problems.push('3137 now forwards goBack; revisit native Add header');
const css = ['main.ee3cb5b6.css', '9449.b8e7f39a.chunk.css'].flatMap(name => {
  const file = `${source.directory}/static/css/${name}`, text = read(file);
  return parseCSS(text).filter(rule => /^(body,html|div$|div\.nav-tabs|\.body-wrapper$|\.razer-profiles|\.profiles-link-games|\.listDevices_wrapper|\.content-wrapper|\.main-nav|\.nav-tabs|\.game-tile|\.plus-icon|\.popup-widget|\.choose-a-mat|\.backdrop|\.device_to_linked_game|\.check-box|\.check-item|\.profile-act|\.profile-bar|\.hover-border|input\[name=profile\]|\.s3-dropdown|\.s3-options|\.dropdown-area$)/.test(rule.selector))
    .map(rule => ({path: file, sha256: hash(text), ...rule}));
});
const rule = selector => css.filter(rule => rule.selector === selector).at(-1);
for (const [selector, declarations] of [
  ['body,html', ['font-family:Roboto,sans-serif', 'font-size:16px']],
  ['div', ['box-sizing:border-box']],
  ['.profiles-link-games>.main-nav', ['padding:11px', 'border-bottom:2px solid #000']],
  ['.nav-tabs .nav', ['line-height:14px', 'padding:7px 10px']],
  ['.content-wrapper', ['top:50px']],
  ['.profiles-link-games .drag-area', ['min-width:900px']],
  ['.listDevices_wrapper .list-device', ['padding:0 20px 80px']],
  ['.main-nav li:hover .tooltip', ['margin-left:15px', 'margin-top:30px', 'padding:8px 10px']],
  ['.choose-a-mat', ['width:1050px!important']],
  ['.device_to_linked_game .choose-a-mat', ['min-width:0', 'width:calc(100vw - 40px)']],
  ['.device_to_linked_game .linked-game-tile.add-new .game-footer', ['padding:0', 'font-size:14px', 'line-height:16px']],
]) for (const declaration of declarations) check(rule(selector)?.declarations || '', declaration, selector);
const deviceWide = rule('.device_to_linked_game .choose-a-mat.linked-games-popup');
check(deviceWide?.conditions.join(' ') || '', '@media(min-width:1600px)', 'DeviceGames wide media');
check(deviceWide?.declarations || '', 'width:1300px!important', 'DeviceGames wide media');
check(deviceWide?.declarations || '', 'max-width:none', 'DeviceGames wide media');
const iconMap = {
  '.main-nav .not-grey': 'profiles-add.svg', '.main-nav .add:hover': 'profiles-add-hover.svg',
  '.main-nav .refresh_games': 'profiles-scan.svg', '.main-nav .refresh_games:hover': 'profiles-scan-hover.svg',
  '.main-nav .search': 'profiles-search.svg', '.main-nav .search:hover': 'profiles-search-hover.svg',
  '.main-nav .refresh': 'profiles-refresh.svg', '.main-nav .refresh:hover': 'profiles-refresh-hover.svg',
  '.main-nav .search-wrapper span:before': 'profiles-search-grey.svg',
  '.main-nav .search-wrapper #remove-icon': 'profiles-clear.svg',
  '.main-nav .search-wrapper #remove-icon:hover': 'profiles-clear-hover.svg',
  '.choose-a-mat .head .close': 'profiles-close.svg', '.plus-icon': 'dashboard-add.svg',
  '.profile-bar .dots3': 'profile-more.svg',
};
const icons = Object.entries(iconMap).map(([selector, output]) => {
  const sourceRule = rule(selector);
  const url = sourceRule.declarations.match(/url\(([^)]+)\)/)[1];
  const input = path.posix.normalize(`${path.posix.dirname(sourceRule.path)}/${url}`);
  output = `assets/synapse/${output}`;
  const sourceHash = hash(fs.readFileSync(path.join(root, input)));
  const outputHash = hash(fs.readFileSync(path.join(root, output)));
  if (sourceHash !== outputHash) problems.push(`Icon differs from source: ${output}`);
  return {selector, source: input, output, source_sha256: sourceHash, output_sha256: outputHash};
});
const nativePaths = ['crates/razer-app-pages/src/profiles_page.rs', 'crates/razer-app-pages/src/profiles_page/devices.rs',
  'crates/razer-app-pages/src/profiles_page/controls.rs', 'crates/razer-widgets/src/source_tooltip.rs'];
const [main, devices, controls, tooltip] = nativePaths.map(read);
for (const snippet of ['.h(surface::css(52.))', 'body.mt(-surface::css(2.))',
  'self.view == ProfilesView::Games', 'Both mounted ancestries cancel', '.font_family("Roboto")',
  'on_clear(event, window, cx)', 'controls::nav_tip(']) check(main, snippet, 'native main');
for (const forbidden of ['profiles-add-back', 'Game discovery service unavailable', 'GAME_NOT_SEE']) {
  if (main.includes(forbidden)) problems.push(`Unmatched Add content remains: ${forbidden}`);
}
if (main.includes('width.max(800.)')) problems.push('Current lazy DeviceGames selector cancels the base 800px popup minimum');
if (devices.includes('.select_profile(')) problems.push('Assignment target must not activate the hardware profile');
if (devices.includes('.child("✓")')) problems.push('Font checkmark must be source CSS geometry');
for (const snippet of ['value.encode_utf16().count() <= 32', 'profile.name == name', 'controls::trim_name(',
  'controls::linked_check(', 'controls::linked_add(', 'controls::close_button(', 'controls::more_button(']) check(devices, snippet, 'native device dialog');
for (const snippet of ['0x2f941e', '0x7ce26c', '0x737373', '0.8, 10.2', '15.4 * top', '9.6 * bottom',
  'Duration::from_millis(300)', 'Duration::from_millis(200)', 'Duration::from_millis(100)',
  'SourceTooltipKind::ProfilesNav', '0xcccccc']) check(controls, snippet, 'native controls');
for (const snippet of ['SourceTooltipKind::ProfilesNav', 'window.rem_size() * (15. / 16.)',
  'window.rem_size() * (30. / 16.)']) check(tooltip, snippet, 'native nav tooltip');
const report = {
  schema_version: 1,
  method: 'Acorn module-local declarations and current mounted JSX/handlers; static CSS preserving media conditions; byte-identical icons',
  components, css, icons, native: nativePaths.map(file => ({path: file, sha256: hash(read(file))})),
  verified_changes: ['Games toolbar 52px outer height while content starts 50px; Devices content follows 52px navigation',
    '900px grid minimum belongs to Games only; both mounted popup ancestries cancel the base 800px popup minimum',
    'Games clear-search collapses; Add clear-search keeps its input; filter/order clicks do not dismiss an empty input',
    'Add mounts only program rows: removed unmatched empty-message content and unforwarded goBack control',
    'Profile dropdown changes local assignment target without selecting a hardware profile',
    'Add/Duplicate/Delete mutate only the local collection; Delete requires confirmation, last-profile protection and uncommitted-editor guards in the owning workspace',
    'DeviceGames observes live profile identities and labels and preserves a valid independent selection; removed targets cancel pending rename/delete',
    'Rename 32 UTF-16 limit and source case-sensitive uniqueness; source ECMAScript outer trim',
    'Linked add footer block alignment/#ccc and 200ms border; check-box source bars/colors/states; popup close/more/menu transitions',
    'Profiles toolbar source tooltip position +15/+30, immediate visibility; current icon bytes unchanged'],
  limitations: ['No application, build, test or downloaded-JavaScript execution; no rendered acceptance',
    'CSS normal line-height, browser form-control font defaults and mixed three-column intrinsic shrinking require separate metric validation',
    'Both Games/Add and DeviceGames use explicit calc(100vw - 40px); DeviceGames current lazy selector cancels the base 800px minimum; media bounds audited',
    'Shared SynapseSelect has source sizing and menu data but instantaneous hover border; no shared dropdown edits in this batch',
    'Native 32-unit validator rejects an overlong edit; browser maxlength may truncate a paste. Product metadata applies its own final whitespace trim.',
    'Profile menu import/export now mounts local selection and decoded preview UI; product conversion/application and official file output remain unfinished (profiles-transfer-current-evidence.json). Source window-blur, deletion popup overflow adjustment and exact focus/keyboard parity need follow-up',
    'No game catalog, program rows, cover art, missing-game state, IOT subdevice or editionName service result is invented',
    'Games selected-game detail/cover-art flows are source-receipted historically but not implemented by the current native page',
    'Known local game data lacks timing fields, so Last Played/Most Played cannot synthesize an ordering'],
  problems,
};
const target = 'docs/re/profiles-content-current-evidence.json', serialized = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(target) !== serialized) throw Error(`Stale ${target}`);
} else fs.writeFileSync(path.join(root, target), serialized);
if (problems.length) throw Error(problems.join('\n'));
process.stdout.write(`Profiles content: ${components.length} scoped bindings, ${css.length} CSS rules, ${icons.length} original icons, problems=0\n`);
