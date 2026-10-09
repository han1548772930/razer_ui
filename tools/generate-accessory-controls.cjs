// Generate native descriptors only from the current product's mounted source.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
const evidence = JSON.parse(read('docs/re/accessory-product-evidence.json'));
if (evidence.parser_sha256 !== hash(fs.readFileSync(path.join(__dirname, 'extract-audio-evidence.cjs')))) throw Error('Regenerate accessory evidence');
const products = [], audit = [];
for (const product of evidence.products) {
  for (const source of product.source_files) if (hash(read(source.path)) !== source.sha256) throw Error(`Changed ${source.path}`);
  const pid = product.product_id, info = product.config.DeviceInfo ?? {}, pages = [];
  const profile = structuredClone(product.config.DEFAULTPROFILE ?? {});
  const initialSources = [];
  // These two CONFIG profiles deliberately contain only identity fields. Their
  // mounted lighting pages consume the current reducer seeds before hardware
  // hydration. Do not apply this fallback to unrelated products.
  if ([3893, 3907].includes(pid)) {
    for (const field of ['brightness', 'switchOffLighting', 'selectedEffectId']) {
      const state = product.states.find(s => Object.hasOwn(s.value, field));
      if (!state) throw Error(`Missing current reducer seed ${pid}:${field}`);
      if (field === 'selectedEffectId') {
        if (!profile.quickEffects) profile.quickEffects = {selectedEffectId: state.value[field]};
      } else if (!profile[field]) profile[field] = structuredClone(state.value[field]);
      initialSources.push({field, path: state.path, offset: state.offset, end: state.end,
        kind: 'reducer seed before hardware hydration'});
    }
  }
  const record = { product_id: pid, profile, pages, support: info.supportPage };
  const pending = [];
  const page = key => { let p = pages.find(p => p.key === key); if (!p) pages.push(p = {key, sections: []}); return p; };
  const section = (key, title) => { const s = {title, controls: []}; page(key).sections.push(s); return s; };
  const add = (s, kind, key, label, binding, proof, extra = {}) => {
    if (!proof || proof.source.startsWith('{')) throw Error(`Invalid mounted component ${pid}:${key}`);
    const c = {kind, key: `${pid}:${key}`, label, path: binding, ...extra,
      source: {path: proof.path, offset: proof.offset, end: proof.end}};
    s.controls.push(c); return c;
  };
  const slider = (s, key, label, binding, proof, jsx, extra = {}) => {
    const {min, max, step = 1} = jsx.props;
    if (![min, max, step].every(Number.isFinite) || min >= max || step <= 0) throw Error(`Invalid source slider ${pid}:${key}`);
    return add(s, 'slider', key, label, binding, proof, {min, max, step, ...extra});
  };
  const runtime = (field, proof) => {
    const initial = product.states.find(s => Object.hasOwn(s.value, field));
    if (!initial || !proof) return null;
    profile.runtime ??= {}; profile.runtime[field] = structuredClone(initial.value[field]);
    initialSources.push({field, path: initial.path, offset: initial.offset, end: initial.end, kind: 'reducer seed before hardware hydration'});
    return `/runtime/${field}`;
  };
  // Display-mode navigation can repeat the same page. Union its traced source
  // components while retaining one native page and one copy of each control.
  const sourcePages = new Map();
  for (const p of product.pages) {
    if (!sourcePages.has(p.key)) sourcePages.set(p.key, {key: p.key, components: []});
    const components = sourcePages.get(p.key).components;
    for (const component of p.components) if (!components.some(c => c.path === component.path && c.offset === component.offset && c.end === component.end)) components.push(component);
  }
  for (const sourcePage of sourcePages.values()) {
    const key = sourcePage.key;
    // A webpack table is never a page component; reject broad old trace records.
    const components = sourcePage.components.filter(c => !c.source.startsWith('{') && c.source.length < 120000);
    const jsx = components.flatMap(c => c.jsx.map(j => ({...j, owner: c})));
    // Monitor Stand Chroma mounts its lighting controls inside Customize.
    // Preserve that navigation identity instead of inventing a Lighting tab.
    if (key === 'TAB_LIGHTING' || (pid === 3929 && key === 'TAB_CUSTOMIZE')) {
      const brightnessBounds = j => j.props.title === 'BRIGHTNESS_HEADER' ||
        (pid === 3893 && j.props.extraClass === 'brightness' && j.props.min === 0 && j.props.max === 100);
      const brightness = components.find(c => c.source.includes('this.state.brightness.value') && c.jsx.some(brightnessBounds)) ??
        (pid === 3886 ? components.find(c => c.jsx.some(j => j.expressions.changeValue === 'this.changeBrightness' && j.props.min === 0 && j.props.max === 100)) : null);
      if (brightness && profile.brightness) {
        const s = section(key, 'BRIGHTNESS_HEADER');
        add(s, 'switch', 'brightness-on', 'BRIGHTNESS_HEADER', '/brightness/isEnabled', brightness);
        const bounds = brightness.jsx.find(j => (brightnessBounds(j) || (pid === 3886 && j.expressions.changeValue === 'this.changeBrightness')) && j.props.min === 0 && j.props.max === 100);
        if (bounds) {
          if (pid === 3886) add(s, 'toggle', 'global-brightness', 'GLOBAL_BRIGHTNESS', '/brightness/isGlobalBrightness', brightness, {disabled_unless: '/brightness/isEnabled'});
          const steps = jsx.find(j => j.expressions.steps?.includes('backlightStep')) ? info.backlightStep : undefined;
          slider(s, 'brightness', 'BRIGHTNESS_HEADER', '/brightness/value', brightness, {...bounds, props: {...bounds.props, step: steps ?? bounds.props.step ?? 1}},
            {disabled_unless: '/brightness/isEnabled', enabled_from_value: '/brightness/isEnabled', ...(pid === 3886 ? {visible_when: {path: '/brightness/isGlobalBrightness', value: true}} : {})});
          if (pid === 3886) s.description = 'GLOBAL_BRIGHTNESS';
        }
      }
      const off = components.find(c => (c.source.includes('this.checkDisplay=') || (pid === 3886 && c.source.includes('.checkDisplay=function'))) && c.source.includes('switchOffLighting'));
      if (off && profile.switchOffLighting) {
        const s = section(key, 'SWITCH_OFF_LIGHTING_HEADER');
        const display = off.jsx.find(j => j.props.id === 'checkDisplay');
        if (display) add(s, 'toggle', 'display-off', display.props.name ?? 'SWITCH_OFF_LIGHTING_DISPLAY', '/switchOffLighting/isDisplayOn', off, {disabled_unless: '/brightness/isEnabled'});
        const idle = off.jsx.find(j => j.expressions.value === 'this.props.switchOffLighting.idleMinutes' && Number.isFinite(j.props.max));
        if (idle && !info.hideLightingIdle) {
          const toggle = off.jsx.find(j => j.props.id === 'checkIdle');
          add(s, 'toggle', 'idle-off', toggle?.props.name ?? 'SWITCH_OFF_LIGHTING_IDLE', '/switchOffLighting/isIdleEnabled', off, {disabled_unless: '/brightness/isEnabled'});
          slider(s, 'idle-minutes', 'IDLE_FOR_MIN', '/switchOffLighting/idleMinutes', off, idle, {disabled_unless_all: ['/brightness/isEnabled','/switchOffLighting/isIdleEnabled']});
        }
      }
      const effects = product.config.QUICK_EFFECTS;
      // 164/241 mount the ordinary quick-effects branch without portComponent.
      // The trace also contains the inactive port branch; first match is not a
      // valid implementation receipt for these two independently reviewed roots.
      const effectProof = [164,241].includes(pid)
        ? components.find(c => c.source.includes('this.handleDefaultWaveDirection=') && c.source.includes('deviceQuickEffect'))
        : components.find(c => c.source.includes('deviceQuickEffect') && c.source.includes('selectedEffect'));
      if (effectProof && effects?.length && profile.quickEffects?.selectedEffectId !== undefined && effects.every(e => typeof e.name === 'string' && Number.isInteger(e.id))) {
        const s = section(key, 'QUICK_EFFECTS');
        add(s, 'select', 'quick-effect', 'QUICK_EFFECTS', '/quickEffects/selectedEffectId', effectProof, {options: effects.map(e => ({label: e.name, value: e.id}))});
      }
    }
    if (pid === 179 && key === 'TAB_CUSTOMIZE') {
      const indicator = components.find(c => c.source.includes('this.toggleConnectionStatus=') && c.source.includes('setIndicatorLedStatus'));
      const binding = runtime('indicatorLedStatus', indicator);
      if (!binding) throw Error('Missing HyperPolling indicator source');
      const enumSource = product.source_files.map(s => ({...s, text: read(s.path)})).find(s => /Connection_Status:\d+,Battery_Status:\d+,Battery_Warning:\d+/.test(s.text));
      const match = enumSource?.text.match(/Connection_Status:(\d+),Battery_Status:(\d+),Battery_Warning:(\d+)/);
      if (!match) throw Error('Missing indicator enum source');
      const labels = ['connectionstatus','batterystatus','batterywarning'].map(id => indicator.jsx.find(j => j.props.id === id)?.props.name);
      if (!labels.every(label => typeof label === 'string')) throw Error('Missing indicator labels');
      const s = section(key, 'INDICATOR_LED');
      s.description = 'INDICATOR_LED_DESC';
      const current = JSON.parse(read('docs/re/receiver-current-evidence.json'));
      const mounted = current.components.OE, source = read(mounted.path);
      if(hash(source)!==mounted.sha256 || source.slice(mounted.offset,mounted.end)!==mounted.source)throw Error('Changed current indicator descriptions');
      const descriptions = ['q2H','JhZ','q0f'].map(key=>current.labels[key]);
      if(!descriptions.every(value=>typeof value==='string'))throw Error('Missing current indicator descriptions');
      record.layout = 'accessory';
      add(s, 'options', 'indicator-mode', 'INDICATOR_LED', binding, indicator, {renderer:'indicator-radio', options: labels.map((label, ix) => ({label, value: Number(match[ix+1]), description:descriptions[ix]}))});
      initialSources.push({field: 'indicator modes', path: enumSource.path, offset: match.index, end: match.index + match[0].length, kind: 'literal source enum'});
    }
    if (pid === 207 && key === 'TAB_CUSTOMIZE') {
      const pairing = components.find(c => c.source.includes('this.props.setAutoPairing('));
      const pairingPath = runtime('autoPairing', pairing);
      if (!pairingPath) throw Error('Missing auto-pairing source');
      const pairingSection = section(key, 'SEAMLESS_AUTO_PAIRING');
      pairingSection.description = 'SEAMLESS_AUTO_PAIRING_DESCRIPTION';
      add(pairingSection, 'switch', 'auto-pairing', 'SEAMLESS_AUTO_PAIRING', `${pairingPath}/isEnabled`, pairing);
      const optimizer = components.find(c => c.source.includes('setBatteryOptimizer') && c.jsx.some(j => j.props.min === 50 && j.props.max === 80 && j.props.step === 5));
      const batteryPath = runtime('batteryOptimizer', optimizer);
      if (!batteryPath || !jsx.some(j => j.props.isMouseMat === true)) throw Error('Missing mouse-mat battery source');
      const batterySection = section(key, 'BATTERY_HEALTH_OPTIMIZER');
      batterySection.description = '等待已配对且开启的鼠标。';
      // The mounted source disables this entire section until a paired mouse
      // reports power-on. No profile-owned flag may simulate that observation.
      const hardwareGate = '/hardware/pairedMousePowered';
      add(batterySection, 'switch', 'battery-optimizer', 'BATTERY_HEALTH_OPTIMIZER', `${batteryPath}/isEnabled`, optimizer, {disabled_unless: hardwareGate});
      const bounds = optimizer.jsx.find(j => j.props.min === 50 && j.props.max === 80 && j.props.step === 5);
      slider(batterySection, 'battery-limit', 'BATTERY_HEALTH_OPTIMIZER', `${batteryPath}/value`, optimizer, bounds,
        {disabled_unless_all: [hardwareGate, `${batteryPath}/isEnabled`], disabled_when: `${batteryPath}/batteryChargingOverrideEnabled`});
    }
    if (['TAB_AUDIO', 'TAB_SOUND'].includes(key) && profile.volume) {
      const volume = components.find(c => c.source.includes('this.state.volume.value') && c.source.includes('this.props.setVolume'));
      const bounds = volume?.jsx.find(j => j.expressions.value === 'this.state.volume.value' && Number.isFinite(j.props.max));
      if (bounds) {
        const s = section(key, 'VOLUME_HEADER');
        add(s, 'switch', 'volume-on', 'VOLUME_HEADER', '/volume/isEnabled', volume);
        slider(s, 'volume', 'VOLUME_HEADER', '/volume/value', volume, bounds, {disabled_unless: '/volume/isEnabled', enabled_from_value: '/volume/isEnabled'});
      }
    }
    if (pid === 3331 && key === 'TAB_SETTING') {
      const mixer = components.find(c => c.source.includes('this.setMasterVolume='));
      if (mixer) for (const [field, label] of [['masterVolume', 'MASTER_VOLUME'], ['pcVolume', 'PC'], ['hdmiVolume', 'HDMI'], ['micVolume', 'MICROPHONE']]) {
        const binding = runtime(field, mixer);
        const bounds = mixer.jsx.find(j => j.expressions.value === `this.state.${field}.value`);
        if (!binding || !bounds) throw Error(`Ripsaw source missing ${field}`);
        const s = section(key, label);
        add(s, 'switch', `${field}-on`, label, `${binding}/isEnabled`, mixer);
        slider(s, field, label, `${binding}/value`, mixer, bounds, {disabled_unless: `${binding}/isEnabled`});
      }
    }
    if (key === 'TAB_POWER' && product.config.POWER_SAVING_VALUE_FROM_PRODUCT_INFO?.length) {
      const power = components.find(c => c.source.includes('powerReducer.powerSaving') && c.source.includes('POWER_SAVING_VALUE_FROM_PRODUCT_INFO'));
      const binding = runtime('powerSaving', power);
      if (binding) {
        const s = section(key, 'POWER_SAVING');
        add(s, 'switch', 'power-on', 'POWER_SAVING', `${binding}/isEnabled`, power);
        add(s, 'options', 'power-minutes', 'IDLE_FOR_MIN', `${binding}/value`, power, {disabled_unless: `${binding}/isEnabled`, options: product.config.POWER_SAVING_VALUE_FROM_PRODUCT_INFO.map(o => ({label: String(o.value), value: o.value}))});
      }
    }
    if (!pages.find(p => p.key === key)?.sections.length) pending.push(key);
  }
  if (pages.length) products.push(record);
  audit.push({product_id: pid, source_files: product.source_files, initial_sources: initialSources, pages: pages.map(p => ({key: p.key, controls: p.sections.flatMap(s => s.controls).length})), pending_pages: [...new Set(pending)]});
}
fs.writeFileSync(path.join(root, 'crates/razer-pages/src/features/accessory_controls_data.json'), JSON.stringify(products, null, 2) + '\n');
fs.writeFileSync(path.join(root, 'docs/re/accessory-controls-audit.json'), JSON.stringify({schema_version: 1, generator_sha256: hash(fs.readFileSync(__filename)), products: audit,
  limitations: ['Native local controls only. Lighting color parameters, pairing, controller-port discovery, IoT/Hue device discovery, Chroma and hardware conditions remain unfinished. Complex fan/monitor/audio pages are handled by their independent adapters.']}, null, 2) + '\n');
console.log(`Generated native accessory controls for ${products.length} products.`);
