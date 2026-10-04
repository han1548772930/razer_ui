// Current product profile menus, inspected as syntax/data only.
// Never imports or evaluates reference JavaScript. Each product is resolved
// independently through its own lexical bindings and webpack export getters.
const fs = require('node:fs');
const path = require('node:path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const FIRST = [164, 241, 778, 784, 3871, 3884, 3886, 3946];
const nodeSources = new WeakMap();
const commandKeys = {
  ADD: 'add', IMPORT: 'import', RENAME: 'rename', DUPLICATE: 'duplicate',
  EXPORT: 'export', RESET: 'reset', DELETE: 'delete',
  LINKED_GAMES: 'linked_games', LINK_GAMES: 'link_games',
  RESET_PROFILE: 'reset', SHARE_TO_WORKSHOP: 'share', EDIT_IN_WORKSHOP: 'share',
  RESHARE_TO_WORKSHOP: 'reshare',
};
function firstExpression(node) { return node?.type === 'SequenceExpression' ? node.expressions[0] : node; }
function returned(node) {
  if (node?.type !== 'BlockStatement') return node;
  return node.body.find(statement => statement.type === 'ReturnStatement')?.argument;
}
function property(node, name) {
  return node?.type === 'ObjectExpression' ? node.properties.find(p => p.type === 'Property' && key(p.key) === name)?.value : undefined;
}
function member(node, name) { return node?.type === 'MemberExpression' && key(node.property) === name; }
function snippet(source, node) { return node ? source.slice(node.start, node.end) : ''; }
function cssValues(rules, selector) {
  const values = {};
  for (const rule of rules) if (!rule.conditions.length && rule.selector.split(',').map(s => s.trim()).includes(selector)) {
    for (const entry of rule.properties) values[entry.property] = entry.value;
  }
  return values;
}
function px(value) {
  if (!/^[-\d.]+px$/.test(value ?? '')) throw Error('Expected source CSS pixel dimension: ' + value);
  return Number(value.slice(0, -2));
}
function literalStrings(source, node) {
  const strings = new Map();
  walk(node, child => {
    if (child.type !== 'MemberExpression') return;
    try { const value = source.literal(child); if (typeof value === 'string') strings.set(value, source.receipt(child)); } catch (_) {}
  });
  return [...strings].map(([value, evidence]) => ({value, evidence}));
}
function helperReceipts(source, method) {
  const helpers = new Map();
  if (method) walk(method, node => {
    if (node.type !== 'CallExpression') return;
    const fn = source.resolve(node.callee);
    if (fn?.type && /Function/.test(fn.type)) helpers.set(fn, source.receipt(fn));
  });
  return [...helpers.values()];
}

class ProductSource {
  constructor(pid, file, cache = new Map()) {
    this.pid = pid; this.file = file; this.source = read(file);
    this.cache = cache; cache.set(file, this);
    this.ast = acorn.parse(this.source, {ecmaVersion: 'latest'});
    this.scopes = new WeakMap(); this.parents = new WeakMap(); this.owners = new WeakMap();
    this.functions = new WeakMap(); this.modules = new Map(); this.assignments = []; this.calls = []; this.properties = [];
    const scope = {parent: null, bindings: new Map(), node: this.ast};
    const visit = (node, environment, parent, owner) => {
      if (!node?.type) return;
      nodeSources.set(node, this);
      if (parent) this.parents.set(node, parent);
      if (node.type === 'FunctionDeclaration' || node.type === 'ClassDeclaration') environment.bindings.set(node.id.name, node);
      if (/Function/.test(node.type)) {
        environment = {parent: environment, bindings: new Map(), node};
        this.functions.set(node, environment);
        for (const param of node.params) if (param.type === 'Identifier') environment.bindings.set(param.name, null);
        if (!owner || !/Class/.test(owner.type)) owner = node;
      }
      if (/Class/.test(node.type)) owner = node;
      this.scopes.set(node, environment); this.owners.set(node, owner);
      if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier') environment.bindings.set(node.id.name, node.init);
      if (node.type === 'VariableDeclarator' && node.id.type === 'ObjectPattern' && node.init) {
        for (const entry of node.id.properties) if (entry.type === 'Property' && entry.value.type === 'Identifier') {
          const binding = {type:'MemberExpression',object:node.init,property:entry.key,computed:entry.computed,start:node.start,end:node.end};
          nodeSources.set(binding,this); this.scopes.set(binding,environment);
          environment.bindings.set(entry.value.name,binding);
        }
      }
      if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression') this.assignments.push(node);
      if (node.type === 'CallExpression') this.calls.push(node);
      if (node.type === 'Property') {
        this.properties.push(node);
        if (Number.isInteger(key(node.key)) && /Function/.test(node.value?.type)) this.modules.set(key(node.key), {fn: node.value, exports: new Map()});
      }
      for (const [name, value] of Object.entries(node)) {
        if (name === 'type') continue;
        if (Array.isArray(value)) for (const child of value) visit(child, environment, node, owner);
        else if (value?.type) visit(value, environment, node, owner);
      }
    };
    visit(this.ast, scope, null, null);
    for (const module of this.modules.values()) {
      const fn = module.fn, requireName = fn.params[2]?.name, exportsName = fn.params[1]?.name;
      walk(fn.body, node => {
        if (/Function|Class/.test(node.type)) return false;
        if (node.type !== 'CallExpression' || !member(node.callee, 'd') || node.callee.object.name !== requireName || node.arguments[0]?.name !== exportsName) return;
        if (node.arguments[1]?.type === 'ObjectExpression') for (const p of node.arguments[1].properties) module.exports.set(key(p.key), returned(p.value.body));
        else if (node.arguments[1]?.type === 'Literal') module.exports.set(node.arguments[1].value, returned(node.arguments[2]?.body));
      });
    }
  }
  lookup(name, node) {
    for (let scope = this.scopes.get(node); scope; scope = scope.parent) if (scope.bindings.has(name)) return scope.bindings.get(name);
  }
  findModule(id) {
    if (this.modules.has(id)) return this;
    for (const source of this.cache.values()) if (source.modules.has(id)) return source;
    const directory = this.file.split('/static/')[0];
    const manifest = JSON.parse(read(`${directory}/asset-manifest.json`));
    const locator = new RegExp(`(?:[,{])${id}(?::|\\()`);
    for (const value of new Set(Object.values(manifest.files))) {
      if (!value.endsWith('.js')) continue;
      const file = `${directory}/${value.replace(/^\.\//, '')}`;
      if (this.cache.has(file) || !fs.existsSync(path.join(root, file)) || !locator.test(read(file))) continue;
      const source = new ProductSource(this.pid, file, this.cache);
      if (source.modules.has(id)) return source;
    }
  }
  resolve(node, seen = new Set()) {
    if (!node || seen.has(node)) return undefined;
    const source = nodeSources.get(node);
    if (source && source !== this) return source.resolve(node, seen);
    seen = new Set([...seen, node]);
    if (node.type === 'Identifier') return this.resolve(this.lookup(node.name, node), seen);
    if (node.type === 'SequenceExpression') return this.resolve(node.expressions.at(-1), seen);
    if (node.type === 'MemberExpression') {
      const object = this.resolve(node.object, seen);
      if (object?.module) return object.host.resolve(object.host.modules.get(object.module)?.exports.get(key(node.property)), seen);
      if (object?.type === 'ObjectExpression') return this.resolve(property(object, key(node.property)), seen);
      return undefined;
    }
    if (node.type === 'CallExpression' && node.callee.type === 'Identifier' && node.arguments.length === 1 && Number.isInteger(node.arguments[0]?.value)) {
      const host = this.findModule(node.arguments[0].value);
      if (host) return {module: node.arguments[0].value, host};
    }
    return node;
  }
  literal(node) {
    node = this.resolve(node);
    if (!node) throw Error('Unresolved lexical value');
    if (node.type === 'Literal') return node.value;
    if (node.type === 'UnaryExpression') {
      const value = this.literal(node.argument);
      if (node.operator === '!') return !value;
      if (node.operator === '-') return -value;
      if (node.operator === 'void') return null;
    }
    if (node.type === 'ArrayExpression') return node.elements.map(child => this.literal(child));
    if (node.type === 'ObjectExpression') return Object.fromEntries(node.properties.map(p => {
      if (p.type !== 'Property' || p.computed) throw Error('Computed menu literal');
      return [key(p.key), this.literal(p.value)];
    }));
    throw Error('Nonliteral expression: ' + snippet(this.source, node).slice(0, 100));
  }
  component(node, seen = new Set()) {
    node = this.resolve(node);
    if (!node || seen.has(node)) return;
    seen = new Set([...seen, node]);
    if (/Class/.test(node.type) || /Function/.test(node.type)) return node;
    if (node.type === 'CallExpression') {
      // React-redux connect(...)(Class) keeps the class as its last argument.
      for (const argument of [...node.arguments].reverse()) {
        const resolved = this.component(argument, seen);
        if (resolved) return resolved;
      }
      // Babel class IIFEs are the component's constructor owner.
      return this.component(node.callee, seen);
    }
  }
  receipt(node) {
    const source = nodeSources.get(node) ?? this;
    return {path: source.file, offset: node.start, end: node.end, source: snippet(source.source, node)};
  }
  assignment(owner, name) {
    return this.assignments.filter(node => this.owners.get(node) === owner && key(node.left.property) === name);
  }
}

function inspect(pid) {
  const directory = `.ref/devices/${pid}`;
  const manifest = JSON.parse(read(`${directory}/asset-manifest.json`));
  const declared = [...new Set(Object.values(manifest.files))].map(value => `${directory}/${value.replace(/^\.\//, '')}`);
  const main = `${directory}/${manifest.files['main.js'].replace(/^\.\//, '')}`;
  const candidates = [main, ...declared.filter(file => file.endsWith('.js') && file !== main)].filter(file => fs.existsSync(path.join(root, file)) && /\.profileMenu\s*=/.test(read(file)));
  if (candidates.length !== 1) throw Error(`Expected one menu-bearing bundle, found ${candidates.length}`);
  const source = new ProductSource(pid, candidates[0]);
  const bars = source.assignments.filter(node => key(node.left.property) === 'renderProfileBar' && /profile-bar flex/.test(snippet(source.source, node)) && /profileMenu/.test(snippet(source.source, node)));
  if (bars.length !== 1) throw Error(`Expected one profile bar consumer, found ${bars.length}`);
  const bar = bars[0], owner = source.owners.get(bar);
  const methods = Object.fromEntries(['getProfileMenu','getDisabledItems','menuClick','updateProfileSet','confirmDel','resetProfile','resetKeyMapping','resetAllKeyActuation','setProfileName','renameProfile','addProfileAction','duplicateProfileAction','openArmoryEditView','triggerArmoryReshare','enableSwitchProfile','closeImportExportModel','importProfiles'].map(name => [name, source.assignment(owner, name)[0]]));
  if (!methods.getProfileMenu || !methods.getDisabledItems || !methods.updateProfileSet) throw Error('Missing profile menu methods on its bar owner');
  const menuAssignments = source.assignment(owner, 'profileMenu');
  let menuNode;
  for (const assignment of menuAssignments) {
    let expression = assignment.right;
    if (expression.type === 'ConditionalExpression') expression = expression.alternate;
    if (expression.type === 'CallExpression' && member(expression.callee, 'getProfileMenu')) {
      try { const value = source.literal(expression.arguments[1]); if (Array.isArray(value)) { menuNode = expression.arguments[1]; break; } } catch (_) {}
    }
  }
  if (!menuNode) throw Error('Unresolved initial profile menu list');
  const menu = source.literal(menuNode);
  const calls = source.calls.filter(call => call.start > bar.start && call.end < bar.end && call.arguments[1]?.type === 'ObjectExpression' && member(property(call.arguments[1], 'menu'), 'profileMenu'));
  if (calls.length !== 1) throw Error('Unresolved mounted more-menu component');
  const more = source.component(calls[0].arguments[0]);
  if (!more) throw Error('Unresolved mounted more-menu implementation');
  const cssFile = `${directory}/${manifest.files['main.css'].replace(/^\.\//, '')}`;
  const css = read(cssFile), cssFiles = declared.filter(file => file.endsWith('.css') && fs.existsSync(path.join(root,file)));
  const allRules = cssFiles.flatMap(file => parseCSS(read(file)).map(rule => ({...rule,source_path:file})));
  const rules = allRules.filter(rule => /profile-act|profile-del|reset-profile|profile-reset|rename-rect|hover-border|\.dots3|ImportExportModal_|cloud-switch|import-profile-btn-group/.test(rule.selector));
  const evidence = {product_id: pid, file: source.file, sha256: hash(source.source),
    bar: source.receipt(bar), menu_literal: source.receipt(source.resolve(menuNode)),
    menu_component: source.receipt(more), methods: Object.fromEntries(Object.entries(methods).filter(([,node]) => node).map(([name,node]) => [name,source.receipt(node)])),
    css: {path: cssFile, sha256: hash(css), bundles:cssFiles.map(file=>({path:file,sha256:hash(read(file))})), rules}};
  const initial = JSON.parse(JSON.stringify(menu));
  evidence.naming_helpers = {add:helperReceipts(source,methods.addProfileAction),duplicate:helperReceipts(source,methods.duplicateProfileAction)};
  evidence.action_creators = [];
  const actionTypes = new Set();
  for (const host of source.cache.values()) for (const entry of host.properties) {
    if (!['resetProfile','deleteProfile','duplicateProfile'].includes(key(entry.key)) || entry.value?.type !== 'ArrowFunctionExpression') continue;
    const fn = host.resolve(returned(entry.value.body));
    if (!fn?.type || !/Function/.test(fn.type)) continue;
    const types = [];
    walk(fn, node => { if (node.type === 'Property' && key(node.key) === 'type') try { const type=host.literal(node.value); types.push(type); actionTypes.add(type); } catch (_) {} });
    if (types.length) evidence.action_creators.push({name:key(entry.key),types,function:host.receipt(fn)});
  }
  evidence.action_reducers = [];
  for (const host of source.cache.values()) walk(host.ast, node => {
    if (node.type !== 'SwitchCase' || !node.test) return;
    try { const type = host.literal(node.test); if (actionTypes.has(type) && !['IS_HYPERSHIFT_ON'].includes(type)) evidence.action_reducers.push({type,case:host.receipt(node),helpers:helperReceipts(host,node).filter(item=>item.source.length<20000)}); } catch (_) {}
  });
  const iconRule = rules.find(rule=>rule.selector.split(',').map(s=>s.trim()).includes('.dots3') && /url\(/.test(rule.declarations));
  if (iconRule) {
    const relative = /url\(([^)]+)\)/.exec(iconRule.declarations)[1].replace(/^['"]|['"]$/g,'');
    const file = path.posix.normalize(path.posix.dirname(iconRule.source_path)+'/'+relative);
    const alias = 'assets/synapse/profile-more.svg';
    const exists = fs.existsSync(path.join(root,file));
    const bytes = exists ? fs.readFileSync(path.join(root,file)) : null;
    evidence.more_icon = {path:file,sha256:bytes?hash(bytes):null,alias,alias_matches:bytes?bytes.equals(fs.readFileSync(path.join(root,alias))):null,
      coverage:exists?'Referenced product asset compared byte-for-byte':'Product asset not stored; CSS resource identity proven, byte equality not asserted',css:iconRule};
  }
  evidence.linked_games_mounts = source.calls.filter(call=>property(call.arguments[1],'showLinkedGames')).map(call=>{
    const component=source.component(call.arguments[0]);
    return {call:source.receipt(call),component:component?source.receipt(component):null};
  });
  const classNode = owner.type === 'ClassBody' ? source.parents.get(owner) : owner;
  evidence.bar_mounts = source.calls.filter(call => call.arguments[1]?.type === 'ObjectExpression' && source.component(call.arguments[0]) === classNode).map(call => source.receipt(call));
  const conditionalRules = [];
  walk(methods.getDisabledItems, node => {
    if (node.type !== 'AssignmentExpression') return;
    let valueNode = node.right.type === 'ConditionalExpression' ? node.right.alternate : node.right;
    let actions = [];
    try { const value = source.literal(valueNode); if (Array.isArray(value)) actions = value; } catch (_) {
      if (valueNode.type === 'ArrayExpression') for (const element of valueNode.elements) try { const value = source.literal(element); if (typeof value === 'string') actions.push(value); } catch (_) {}
    }
    if (!actions.length) return;
    let predicate;
    for (let current = node; current && current !== methods.getDisabledItems; current = source.parents.get(current)) {
      const parent = source.parents.get(current);
      if (parent?.type === 'LogicalExpression' && parent.operator === '&&' && parent.right === current) { predicate = parent.left; break; }
    }
    if (!predicate) return;
    const text = source.receipt(predicate).source;
    const condition = /isObmProfile/.test(text) ? 'onboard_profile' : /analogPresetProfiles/.test(text) ? 'analog_preset_profile' : /dynamicProfile/.test(text) ? 'dynamic_profile_locked' : /armoryMaintenanceModeEnabled/.test(text) ? 'armory_maintenance' : /profiles\.length/.test(text) ? 'single_profile' : actions.includes('SHARE_TO_WORKSHOP') ? 'profile_already_shared' : 'unresolved_source_condition';
    conditionalRules.push({condition, actions, predicate:source.receipt(predicate), assignment:source.receipt(node)});
  });
  evidence.conditional_rules = conditionalRules;
  let category, resetSupported = null, shareSupported = null;
  walk(methods.getProfileMenu.right, node => {
    if (node.type !== 'CallExpression' || !member(node.callee, 'includes') || !member(node.arguments[0], 'category')) return;
    try {
      const list = source.literal(node.callee.object), kind = source.literal(node.arguments[0]);
      category = kind;
      if (node.callee.object.type === 'ArrayExpression') resetSupported = list.includes(kind);
      else shareSupported = list.includes(kind);
    } catch (_) {}
  });
  walk(methods.getProfileMenu.right, node => {
    if (member(node, 'supportResetProfile')) try { if (source.literal(node)) resetSupported = true; } catch (_) {}
  });
  const menuData = menu.map(entry => {
    if (entry.action === 'divider') return {kind: 'separator'};
    const command = commandKeys[entry.action];
    if (!command) throw Error('Unmapped semantic menu action: ' + entry.action);
    const disabled = ['profile_switch_disabled'];
    const hidden = [];
    if (command !== 'add') disabled.push('no_selected_profile');
    if (command === 'delete') disabled.push('single_profile');
    for (const rule of conditionalRules) if (rule.actions.includes(entry.action) && !disabled.includes(rule.condition)) disabled.push(rule.condition);
    if (entry.action === 'LINKED_GAMES') hidden.push('linked_games_empty');
    if (entry.action === 'LINK_GAMES') hidden.push('linked_games_present');
    if (command === 'reset' && resetSupported === false) hidden.push('reset_unsupported');
    if (command === 'share' && shareSupported === false) hidden.push('share_category_unsupported');
    return {kind: 'action', command, label_key: entry.action, disabled_when: disabled, hidden_when: hidden};
  });
  const confirmations = [];
  const dialogEvidence = [];
  for (const call of source.calls.filter(call => call.start > bar.start && call.end < bar.end && call.arguments[1]?.type === 'ObjectExpression')) {
    const props = call.arguments[1];
    if (!member(property(props, 'active'), 'delActive') && !member(property(props, 'show'), 'showResetPopup')) continue;
    const name = property(props, 'deleteMsg') ? 'delete' : 'reset';
    const component = source.component(call.arguments[0]);
    const value = field => { try { return source.literal(property(props, field)); } catch (_) { return null; } };
    if (!component) throw Error('Unresolved confirmation: ' + name);
    const strings = literalStrings(source,component), keys = strings.map(s => s.value);
    const geometry = cssValues(allRules,'.profile-del');
    confirmations.push({name, label_key:value('title') ?? keys.find(k => k === 'RESET_PROFILE_TITLE'), description_key:value('deleteMsg') ?? keys.find(k => k === 'RESET_PROFILE_DESC'),
      width:px(geometry['min-width']), top:px(geometry.top), delay_ms:100,
      border:geometry.border, border_radius:px(geometry['border-radius']),
      button_keys:keys.filter(k=>/^(RESET_PROFILE_KEYBINDS_BUTTON|RESET_PROFILE_BUTTON|REMOVE)$/.test(k)), outside_click_closes:true});
    dialogEvidence.push({name, call:source.receipt(call), component:source.receipt(component), strings});
  }
  evidence.dialogs = dialogEvidence;
  const ieCalls = source.calls.filter(call => call.start > bar.start && call.end < bar.end && call.arguments[1]?.type === 'ObjectExpression' && property(call.arguments[1],'IEMode'));
  if (ieCalls.length !== 1) throw Error('Unresolved import/export mount');
  const ie = source.component(ieCalls[0].arguments[0]);
  if (!ie) throw Error('Unresolved import/export component');
  const ieStrings = literalStrings(source,ie);
  const children = source.calls.filter(call => call.start > ie.start && call.end < ie.end && call.arguments[1]?.type === 'ObjectExpression' && (property(call.arguments[1],'setImportData') || (property(call.arguments[1],'data') && property(call.arguments[1],'select')))).map(call => {
    const component = source.component(call.arguments[0]);
    if (!component) throw Error('Unresolved import/export child');
    return {call:source.receipt(call), component:source.receipt(component), strings:literalStrings(source,component)};
  });
  evidence.import_export = {call:source.receipt(ieCalls[0]),component:source.receipt(ie),strings:ieStrings,children};
  const modalClass = ieStrings.find(s=>/^ImportExportModal_modal__/.test(s.value))?.value;
  if (!modalClass) throw Error('Unresolved import/export modal CSS class');
  const modal = cssValues(allRules,'.'+modalClass), menuGeometry=cssValues(allRules,'.profile-act');
  const importExport = {width:px(modal.width),height:px(modal.height),top:px(modal.top),left:modal.left,
    locale_keys:[...new Set([...ieStrings,...children.flatMap(c=>c.strings)].map(s=>s.value).filter(s=>/^[A-Z][A-Z_0-9]+$/.test(s)))],
    source_only:true};
  const data = {product_id: pid, category, menu: menuData, rename:{max_length:32}, menu_width:px(menuGeometry['min-width']??menuGeometry.width),menu_max_width:menuGeometry['max-width']?px(menuGeometry['max-width']):undefined,menu_width_mode:menuGeometry.width,
    confirmations, capabilities:{reset_profile:resetSupported,share_profile:shareSupported},
    import_export:importExport,
    source:{path:source.file,sha256:hash(source.source),offset:bar.start},
    coverage:{menu:true, dynamic_conditions:'See evidence; only positively known model states activate conditional rules'}};
  return {source, owner, menu:initial, data, evidence, methods, bar};
}

if (require.main === module) {
  const ids = process.argv.includes('--all') ? fs.readdirSync(path.join(root, '.ref/devices')).filter(name => /^\d+$/.test(name)).map(Number).sort((a,b) => a-b) : FIRST;
  const results = [], products = [], uncovered = [];
  for (const pid of ids) {
    try {
      const result = inspect(pid);
      results.push(result.evidence);
      products.push(result.data);
      console.log(pid, JSON.stringify(result.menu));
    } catch (error) { uncovered.push({product_id: pid, reason: error.message}); console.log(pid, 'UNCOVERED', error.message); }
  }
  if (!process.argv.includes('--all') && (uncovered.length || products.length !== FIRST.length)) {
    throw Error('Required first-eight coverage failed; refusing to overwrite evidence/data: ' + JSON.stringify(uncovered));
  }
  const target = path.join(root, 'docs/re/source-profile-menu-current-evidence.json');
  const output = JSON.stringify({schema_version:1, method:'Independent product Acorn lexical/module resolution; reference code never executed', products:results, uncovered},null,2)+'\n';
  if (process.argv.includes('--check')) {
    if (fs.readFileSync(target, 'utf8') !== output) throw Error('Stale source profile menu evidence');
  } else fs.writeFileSync(target, output);
  const dataTarget = path.join(root, 'src/features/source_profile_menu_data.json');
  const data = JSON.stringify({schema_version:1,products,uncovered},null,2)+'\n';
  if (process.argv.includes('--check')) {
    if (fs.readFileSync(dataTarget,'utf8') !== data) throw Error('Stale source profile menu data');
  } else fs.writeFileSync(dataTarget,data);
}

module.exports = {ProductSource, inspect, property, member, snippet};
