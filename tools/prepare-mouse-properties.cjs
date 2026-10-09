// Prepare capability data from complete current-source audits, without executing vendor code.
const fs = require('fs');
const path = require('path');
const acorn = require('acorn');
const {hash, walk} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const requireValue = (value, message) => { if (!value) throw Error(message); return value; };
const pages = JSON.parse(read('docs/re/mouse-page-source.json')).products;
const embedded = [...read('assets/synapse/embedded.rs').matchAll(/\("([^"]+)",\s*include_bytes!\("([^"]+)"\)/g)]
    .filter(match => match[2].endsWith('.svg'))
    .map(match => ({key: match[1], sha256: hash(fs.readFileSync(path.join(root, 'assets/synapse', match[2])))}));
const specs = [], evidence = [];
for (const name of fs.readdirSync(path.join(root, 'docs/re')).sort()) {
    if (!/^mouse-\d+-properties-current-evidence\.json$/.test(name)) continue;
    const auditPath = `docs/re/${name}`, audit = JSON.parse(read(auditPath));
    const directory = `.ref/devices/${audit.product_id}/`;
    requireValue(audit.manifest.path === `${directory}asset-manifest.json`, 'Unexpected manifest ' + auditPath);
    requireValue(hash(read(audit.manifest.path)) === audit.manifest.sha256, 'Stale manifest ' + auditPath);
    const manifest = JSON.parse(read(audit.manifest.path));
    const declared = new Set(Object.values(manifest.files).map(file => directory + file.replace(/^\.\//, '')));
    const texts = new Map();
    const current = receipt => {
        requireValue(receipt.path.startsWith(directory) && declared.has(receipt.path), 'Undeclared current source ' + receipt.path);
        if (!texts.has(receipt.path)) texts.set(receipt.path, read(receipt.path));
        const text = texts.get(receipt.path);
        requireValue(hash(text) === receipt.sha256, 'Source changed ' + receipt.path);
        return text;
    };
    for (const receipt of audit.receipts) {
        const text = current(receipt);
        requireValue(text.slice(receipt.offset, receipt.end) === receipt.source, 'Source receipt changed ' + receipt.symbol);
        // Parse the exact audited expression as data. No import/eval is permitted.
        acorn.parse(receipt.source.startsWith('class ') ? receipt.source : `(${receipt.source})`, {ecmaVersion: 'latest'});
    }
    const widgets = audit.receipts.filter(receipt => /this\.openMouseProperties=\(\)=>\{[^{}]*\.OpenMouseProperties\(\)\}/.test(receipt.source));
    requireValue(widgets.length === 1, 'Ambiguous properties widget ' + auditPath);
    const widget = widgets[0];
    const mounted = pages.find(product => product.product_id === audit.product_id)?.pages
        .find(page => page.key === 'TAB_PERFORMANCE' && page.components.some(component => component.source === widget.source));
    requireValue(mounted && current(mounted), 'Properties widget not mounted in Performance ' + auditPath);
    const ast = acorn.parse(widget.source, {ecmaVersion: 'latest'}), labels = {};
    walk(ast, node => {
        if (node.type !== 'Property') return;
        const key = node.key.name ?? node.key.value;
        if (!['title', 'tips', 'text'].includes(key) || node.value.type !== 'MemberExpression') return;
        const symbol = node.value.property.name;
        const label = audit.labels[symbol];
        const receipt = audit.receipts.find(item => item.symbol === `label:${symbol}`);
        requireValue(label && receipt && JSON.parse(receipt.source) === label, 'Unverified label ' + symbol);
        labels[key] = label;
    });
    requireValue(labels.title && labels.tips && labels.text, 'Incomplete property labels ' + auditPath);
    const styles = new Map();
    for (const sheet of audit.css) {
        const rules = parseCSS(current(sheet));
        for (const rule of sheet.rules) {
            requireValue(rules.some(candidate => JSON.stringify(candidate) === JSON.stringify(rule)), 'CSS receipt changed ' + sheet.path);
            requireValue(!rule.conditions.length, 'Conditional CSS needs its own adapter');
            for (const selector of rule.selector.split(',')) {
                if (!styles.has(selector)) styles.set(selector, {});
                const style = styles.get(selector);
                for (const property of rule.properties) style[property.property] = property.value;
            }
        }
    }
    const style = selector => requireValue(styles.get(selector), 'Missing style ' + selector);
    const px = value => { requireValue(/^\d+(?:\.\d+)?px$/.test(value), 'Unsupported pixel value ' + value); return Number(value.slice(0, -2)); };
    const color = value => { requireValue(/^#[\da-f]{3}(?:[\da-f]{3})?$/i.test(value), 'Unsupported color ' + value); return parseInt(value.length === 4 ? [...value.slice(1)].map(c => c + c).join('') : value.slice(1), 16); };
    const icon = selector => {
        const url = requireValue(/^url\(([^)]+)\)$/.exec(style(selector)['background-image']), 'Missing icon URL')[1];
        const asset = requireValue(audit.assets.find(item => path.posix.basename(item.source) === path.posix.basename(url)), 'Missing audited icon ' + url);
        requireValue(declared.has(asset.source), 'Icon not declared by current manifest');
        const bytes = fs.readFileSync(path.join(root, asset.source));
        requireValue(hash(bytes) === asset.sha256, 'Source icon changed ' + asset.source);
        return requireValue(embedded.find(item => item.sha256 === asset.sha256), 'No byte-identical embedded icon ' + asset.source).key;
    };
    const windows = style('.img-text .windows'), action = style('.img-text .external');
    requireValue(windows['min-width'] === windows['max-width'], 'Nonfixed icon width');
    requireValue(action['text-decoration'] === 'underline' && style('.img-text')['align-items'] === 'center', 'Unsupported properties layout');
    requireValue(audit.receipts.some(item => item.source.includes('getWindowsVersion') && item.source.includes('"windows windows-11"')), 'Missing OS icon selector');
    const opacity = Number(style('.img-text .external:active').opacity);
    requireValue(Number.isFinite(opacity) && opacity >= 0 && opacity <= 1, 'Invalid active opacity');
    specs.push({product_id: audit.product_id, title: labels.title, tooltip: labels.tips, action: labels.text,
        legacy_icon: icon('.img-text .windows'), windows_11_icon: icon('.img-text .windows-11'),
        icon_width: px(windows['min-width']), icon_height: px(windows.height), icon_margin: px(windows['margin-right']),
        font_size: px(action['font-size']), line_height: px(action['line-height']), foreground: color(action.color),
        hover_foreground: color(style('.img-text .external:hover').color), active_opacity: opacity});
    evidence.push({product_id: audit.product_id, audit: auditPath, sha256: hash(read(auditPath)),
        mounted_page: {path: mounted.path, sha256: mounted.sha256, nav_offset: mounted.nav_offset},
        source_files: [...texts].map(([file, text]) => ({path: file, sha256: hash(text)}))});
}
requireValue(specs.length && new Set(specs.map(spec => spec.product_id)).size === specs.length, 'Missing or duplicate capabilities');
specs.sort((a, b) => a.product_id - b.product_id);
for (const [file, value] of [
    ['crates/razer-pages/src/features/mouse_properties_data.json', specs],
    ['docs/re/mouse-properties-capabilities-current-evidence.json', {method: 'Complete audited capabilities only; current manifest/AST/CSS/SVG verified statically', products: evidence}],
]) {
    const output = JSON.stringify(value, null, 2) + '\n';
    if (check) requireValue(read(file) === output, 'Generated data changed ' + file);
    else fs.writeFileSync(path.join(root, file), output);
}
console.log(`Mouse Properties: ${specs.length} source-verified capabilities; no product IDs in renderer or generator`);
