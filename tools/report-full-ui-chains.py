"""Render current complete scope tables from static evidence; no vendor execution."""
import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(name):
    return json.loads((ROOT / name).read_text('utf-8'))


def full(name):
    index = read(f'docs/re/{name}.json')
    data = (ROOT / 'docs/re' / index['data_file']).read_bytes()
    assert hashlib.sha256(data).hexdigest() == index['data_sha256'], name
    raw = gzip.decompress(data)
    assert hashlib.sha256(raw).hexdigest() == index['uncompressed_sha256'], name
    return index, json.loads(raw)


def cell(value):
    return str(value).replace('|', '&#124;').replace('\n', ' ').replace('\r', '')


def write(name, text, check):
    target = ROOT / 'docs/re' / name
    text = '\n'.join(text) + '\n'
    if check:
        assert target.read_text('utf-8') == text, f'Stale report: {name}'
    else:
        target.write_text(text, encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--products-only', action='store_true', help='Update page/layout reports without touching application/catalog reports')
    args = parser.parse_args()
    pi, graph = full('all-product-page-chains-current')
    ai, apps = full('all-application-chains-current')
    li, layouts = full('all-product-layout-chains-current')
    catalog = read('docs/re/product-catalog.json')
    coverage = {p['product_id']: p for p in read('docs/re/native-product-coverage.json')['products']}
    registered = {p['product_id']: p for p in graph['products']}
    layout_map = {p['product_id']: p for p in layouts['products']}
    lines = ['# 当前产品全量页面引用链', '',
             '证据范围固定为2026-10-02取得的当前源；本次只解析本地源，没有重新联网确认发布日期。当前host仅使用4.0.827。', '',
             '这是全部已注册产品的逐页索引，页内组件引用、状态调用、条件、候选命令与CSS类的原文位置保存在[机器索引](all-product-page-chains-current.json)和其gzip完整记录中。引用链不是完整执行链；本表没有把共享组件、路由存在或AST解析计为完成界面。', '',
             f"共{graph['summary']['products']}款，主导航{graph['summary']['primary_pages']}页，独立模式{graph['summary']['independent_pages']}页；{graph['summary']['resolved_graph_roots']}页取得非空引用子图，{graph['summary']['unresolved_roots']}页根未解，{graph['summary']['truncated_graphs']}页触及每页250组件边界。完整语义/视觉完成页数仍为0。", '',
             '静态链按导航原文→根组件词法绑定→包装器/JSX组件→webpack导出与可定位lazy模块展开；保留每个组件的路径、SHA-256及UTF-16源码offset/end。useState/useSelector/setState/dispatch等是状态观察候选；get/set/read/write等名称只提供定位，不自动推定DLL读写。导航父级displayMode/lazy入口仍由原注册收据保存；导航没有component字段时必须追到实际render挂载，不能凭页面name补根。', '',
             '各页的CSS规则候选见[全量样式链](all-product-layout-chains-current.md)。页内弹窗/控制器/生命周期和服务消息的精确语义仍需按原文逐分支审查；现有已人工核对的细节见[产品分组审查](product-review-current.md)及各产品族current契约。', '',
             '部分新产品的导航表只有id/name，真实页在同一owner的AsyncRouter computed-key children对象中；已按导航name原表达式与computed key严格唯一匹配恢复JSX，并在recovered_component中保存原表达式/SHA/range。另保留函数包装根解析和父displayMode/lazy chain原证据。691等产品manifest使用/synapse/products/{id}/ui/绝对前缀，本地文件对应关系按该前缀规范化，原manifest保持不变。', '',
             '独立multiDevicePairing另按owner.render→children:this.renderView()→this.state.navs.find(name===active_view)→唯一navigation name guard→实际返回JSX恢复；保留整个render/renderView/guard/return原文，静态validator逐个校验。TAB_LIGHTING的成员根通过外层webpack runtime的实际module调用AST确认require，再定位default导出；769 HOME的conditional根保留真实条件与两个分支，未任意选一个。', '',
             '前一轮只重新展开缺根产品，其他原图显式保留旧解析器。本轮进一步按全部331个产品的真实导航根重新解析内层引用；不会把旧子图直接换stamp。先前computed-router、renderView、conditional双分支根原文仍保留并逐区间验证。断点只有scanner与registration指纹同时一致、所有源码摘要复核通过才复用。', '',
             '通用解引用现在包括优化后的0/1/2/3参数webpack factory、ESM getter、CommonJS named assignment/module.exports转发、Object.defineProperty getter、有源码__esModule/default分支证明的Babel互操作、外层webpack runtime实际factory调用证明及可唯一定位的对象字面量成员。React/ReactRedux只有在产品当前HTML的实际script src指向现存UMD、UMD浏览器分支赋值到同名global且唯一导出可定位时展开；Fragment继续追到实际Symbol.for("react.fragment")，作为框架符号终点，并保存原文，未随意将未解标为外部。', '',
             '函数、block/for/switch/catch及解构声明都保留词法遮蔽。require必须实际lookup到工厂第三形参；exports/module出口在整个词法表建成后确认绑定，内层同名参数/变量不会被算作工厂出口。UMD全局只在没有本地binding时采用，UMD依赖只使用对应factory形参；其独立数字loader不混入产品模块表。全部manifest JS先建立模块空间；同id而正文不同的factory保持module_conflicting_factories，附双方当前原文range，不凭先读/后读顺序选一个。部分冲突只是不同chunk的局部变量改名，但未经完整等价证明仍保留未知。', '',
             f"共保存{graph['summary'].get('resolution_receipts', 0)}条通用解引用证明（见gzip各页resolutions），包含原调用、导出/转发、runtime或UMD声明的路径/SHA/range。它们证明结构关联，不证明HOC副作用、真实state更新、分支可达性或任一设备读写成功。", '',
             '| 通用解析证明类型 | 证明条数 |', '| --- | ---: |',
             *[f'| {kind} | {count} |' for kind, count in sorted(graph['summary'].get('resolution_kinds', {}).items())], '',
             '| 未解引用原因 | 引用次数 |', '| --- | ---: |']
    reasons = Counter(u['reason'] for p in graph['products'] for page in p['pages'] for u in page['unresolved'])
    lines += [f'| {reason} | {count} |' for reason, count in sorted(reasons.items())]
    lines += ['', '仍未解的动态props/PortComponent、React state保存的lazy组件、闭包参数、计算属性、非唯一factory/导出及尚未完全传递的Babel default namespace保持原表达式与owner。它们不是缺失界面，也不能因为部分共享Fragment已闭合就宣布业务语义完成；实际消息字段、读写API、清理和Apply/Save仍须逐分支审查。', '',
              '本轮export_not_resolved全部指向4202/54202的Fragment：当前某些产品JSX-runtime先写Fragment=60107，再在Symbol.for可用分支改写为i("react.fragment")。两条原文确实存在，扫描器目前要求唯一出口，因此没有任选一个环境分支后宣称已解；后续应保留条件与赋值顺序展开两个可能值。它们不是模块缺失或新缺失的业务页面。']
    # One Acorn-reviewed source receipt illustrates the remaining conditional
    # export; this is documentation evidence, never a PID-specific resolver.
    fragment_file = '.ref/devices/92/static/js/main.ca936871.js'
    fragment_hash = '77261e574636cc9ca13db973b531bc9b88cb345ce7965af4d95c684d61e3d09a'
    fragment_bytes = (ROOT / fragment_file).read_bytes()
    assert hashlib.sha256(fragment_bytes).hexdigest() == fragment_hash, 'Fragment example source changed'
    fragment_text = fragment_bytes.decode('utf-8')
    fragment_source = 'if(a.Fragment=60107,"function"===typeof Symbol&&Symbol.for){var i=Symbol.for;_=i("react.element"),a.Fragment=i("react.fragment")}'
    assert fragment_text[210754:210883] == fragment_source, 'Fragment example source range changed'
    lines += ['', f'条件多出口原文实例：`{fragment_file}`，SHA-256 `{fragment_hash}`，UTF-16 `[210754,210883)`；test为`[210757,210812)`。', '',
              '```javascript', fragment_source, '```', '',
              'test的逗号表达式先执行Fragment=60107；后半条件成立才执行Symbol.for别名及第二次Fragment赋值。上例从4202 factory的实际IfStatement提取；不能把第一次写入误描述为另一个独立else分支。']
    unresolved_examples = {}
    for product in graph['products']:
        for page in product['pages']:
            for unknown in page['unresolved']:
                example_key = (unknown['reason'], unknown['expression'])
                if example_key not in unresolved_examples:
                    owner = page['components'][unknown['from']] if unknown['from'] is not None else None
                    unresolved_examples[example_key] = {'count': 0, 'product': product['product_id'], 'page': page['page_key'], 'owner': owner}
                unresolved_examples[example_key]['count'] += 1
    lines += ['', '以下每种未解原因列出最常见的实际表达式及首个源owner，便于继续逐条查；统计含多个产品/页面中的共享重复引用，不是独立缺失功能数。', '',
              '| 未解原因 / 原表达式 | 引用次数 | 首个产品 / 页面 | 源owner位置 |', '| --- | ---: | --- | --- |']
    for reason in sorted(reasons):
        examples = sorted(((expr, value) for (kind, expr), value in unresolved_examples.items() if kind == reason), key=lambda entry: (-entry[1]['count'], entry[0]))[:8]
        for expression, example in examples:
            owner = example['owner']
            location = f"{owner['path']}@{owner['offset']} ({owner['symbol']})" if owner else '导航root'
            lines.append(f"| {reason} / `{cell(expression)}` | {example['count']} | {example['product']} / {cell(example['page'])} | {cell(location)} |")
    for product in graph['products']:
        pid = product['product_id']
        lines += ['', f"## {pid} · {cell(product['name'])}", '',
                  f"类别：{', '.join(product['categories']) or '未给出'}；edition：{cell(product['edition_ids'])}；连接别名：{cell(product['connection_aliases'])}。", '',
                  '| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |', '| --- | --- | --- | ---: | ---: | --- | --- |']
        local = {p['page_id']: p for p in coverage[pid]['pages']}
        for page in product['pages']:
            nav = page['navigation']
            route = local.get(page['page_id'], {}).get('status', 'independent_not_primary')
            root_expression = nav.get('recovered_component', {}).get('expression') if nav.get('recovered_component') else nav['component']
            recovered = nav.get('recovered_component')
            recovery_label = f" / {recovered['resolution']}" if recovered else ''
            lines.append(f"| {cell(page['page_key'])} / {cell(page['display_mode'])} | {cell(nav['path'])}@{nav['item_offset']} | `{cell(root_expression)}` | {len(page['components'])} | {len(page['unresolved'])} | {page['trace_status']}{recovery_label}{' / bounded' if page['truncated'] else ''} | {route} |")
    lines += ['', '## 复生成与验证', '',
              '`node tools/extract-all-product-page-chains.cjs`解析全部产品；`node tools/extract-all-product-layout-chains.cjs`连接CSS；`python -X utf8 tools/report-full-ui-chains.py --check`检查报告一致性；`node tools/validate-full-ui-chains.cjs`核对完整压缩记录、工具指纹和当前源原文。', '',
              '未运行厂商JS、应用、DLL、构建或测试。界面运行、视觉、真实服务读取均未验收；DLL写回仍后置。']
    write('all-product-page-chains-current.md', lines, args.check)
    lines = ['# 当前产品全量样式与资源候选链', '',
             '产品asset-manifest→入口/资源路径→页子图字面className→CSS选择器候选连接已经逐款展开。原规则颜色、尺寸、padding/margin/gap、字体、边框及条件以CSS声明原值保存；没有浏览器计算样式。', '',
             f"{layouts['summary']['products']}产品、{layouts['summary']['pages']}页，{layouts['summary']['css_file_references']}次CSS文件引用、{layouts['summary']['unique_css_hashes']}种CSS内容；{layouts['summary']['manifest_asset_references']}项manifest资源声明；共{layouts['summary']['page_css_rule_candidates']}个页/规则候选引用。", '',
             '证据见[轻量索引](all-product-layout-chains-current.json)及其gzip完整规则。className计算表达式、模板动态类、CSS modules、运行追加类尚未展开；同一token命中带祖先/复合条件的selector不表示该规则生效。规则顺序、优先级、继承、@media、伪状态及!important必须结合真实挂载树核对。因此所有页仍标记not_completed_by_selector_candidates。', '',
             '本轮由全部产品内层引用重新展开后的graph生成，新增共享组件的字面CSS类也进入候选；原组件和源规则没有因候选数量变化被改写。React/ReactRedux的框架解析不被记作产品业务组件的实现完成。', '',
             '| 产品 ID | CSS文件 | manifest资源 | 页面 | 静态class token引用 | 页/规则候选 | 未命中token引用 |', '| --- | ---: | ---: | ---: | ---: | ---: | ---: |']
    for p in layouts['products']:
        lines.append(f"| {p['product_id']} | {len(p['css'])} | {len(p['assets'])} | {len(p['pages'])} | {sum(len(v['static_class_tokens']) for v in p['pages'])} | {sum(len(v['css_rule_candidates']) for v in p['pages'])} | {sum(len(v['unmatched_tokens']) for v in p['pages'])} |")
    lines += ['', '具体到每页的token、未命中类、selector和declarations在gzip中；每条记录包含当前源路径和SHA-256、原CSS偏移，外部图片/字体url原值也保留。资源声明和候选规则不证明所有图片/字体已下载或UI已精确还原。']
    write('all-product-layout-chains-current.md', lines, args.check)
    if args.products_only:
        print(f"Product reports: {len(graph['products'])} products / {sum(len(p['pages']) for p in graph['products'])} pages; no semantic completion inferred")
        return
    app_sources = {s['path']: s for s in apps['sources']}
    lines = ['# 当前独立应用全量静态链', '',
             f"全部{apps['summary']['application_endpoints']}端点均已列入：{apps['summary']['html_entries']}个本地HTML入口、2个目录404端点。入口共{apps['summary']['unique_js_files']}个唯一JS文件，本次全部Acorn静态解析成功；带webpack chunk标记的代码中模块factory候选登记{apps['summary']['webpack_modules']}项，CSS文件引用{apps['summary']['css_file_references']}次。webpack4的稀疏数组模块与webpack5数字对象模块分别解析，保留优化后的0/1/2参数factory；background-manager是ESM，不能用0个webpack模块解释为没有代码。", '',
             '每个应用保存HTML script/link→asset-manifest entrypoints→全部JS/CSS→webpack模块声明/导出/字面依赖/lazy引用或ESM imports→JSX/createElement引用与状态/消息候选；CSS selector、原声明、条件、字体声明偏移和资源URL保存在[完整机器记录](all-application-chains-current.json)指向的gzip中。', '',
             '范围仅代表当前本地源结构已全量列出。数字模块及命名调用仍含库代码；未按每个分支还原业务含义，动态模块/模板URL尚未全解，启动candidate并不等于已证明某个page实际挂载。22应用没有任何一个被计为全语义完成。实际本地UI和服务边界见[公共应用审查](application-review-current.md)。', '',
             '| 端点 | manifest入口 | JS / CSS | 模块登记 | ESM import引用 | 状态调用候选 | 消息调用候选 |', '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |']
    for app in apps['applications']:
        files = [app_sources[f] for f in app['js_files']]
        entries = app['manifest']['entrypoints'] if app['manifest'] else ['无当前页源']
        lines.append(f"| {app['route']} | {cell(', '.join(entries))} | {len(files)} / {len(app['css'])} | {sum(len(s['modules']) for s in files)} | {sum(len(s['imports']) for s in files)} | {sum(len(s['state_observations']) for s in files)} | {sum(len(s['message_candidates']) for s in files)} |")
    lines += ['', '## 仍需逐条还原的语义', '',
              '- HTML外部框架脚本与各应用厂商模块的完整调度关系；模块引用并不证明真实执行顺序。',
              '- 每个真实route/displayMode及独立模态的挂载、账号/安装/locale/固件/服务/产品能力门控。',
              '- reducer初值、请求/订阅顺序、失败/取消/超时/销毁、消息字段和后端处理器；名称相似不能视为同一个API。',
              '- 所有CSS动态类、布局/字重/字体fallback/hover/pressed/动画与资源实际消费。',
              '- UI编辑、增删、Apply/Save及本地存储语义与DLL读观察/写回分别记录，局部草稿不冒充设备保存。', '',
              '`node tools/extract-all-application-chains.cjs`只解析文本；`node tools/validate-full-ui-chains.cjs`核实证据；`python -X utf8 tools/report-full-ui-chains.py --check`核实MD表。没有运行应用、厂商JS、DLL、构建或测试。']
    write('all-application-chains-current.md', lines, args.check)
    lines = ['# 产品源码目录', '',
             '本目录记录官方来源、别名、入口和当前源代码；不把路由或资源数量推定为本地UI完成。', '',
             '本次全量静态展开仍固定使用2026-10-02取得的当前版本。源码新鲜度证据见[当前版本](20-current-source-version.md)；host只采用4.0.827。没有重新抓取网络或执行厂商代码。', '',
             f"原目录共{len(catalog['products'])}个已查询产品ID，其中331个取得UI入口与manifest，另265个未取得当前UI入口；不要把596目录ID、331产品页入口、edition、连接别名和DLL数量混为一项。", '',
             '331个注册入口有363组导航、1419主导航页和33独立模式页。当前全量根图、状态/条件/命令候选见[逐产品逐页面链](all-product-page-chains-current.md)；颜色/布局/字体资源的原CSS依据见[样式资源链](all-product-layout-chains-current.md)。完整记录均带当前路径、SHA-256和原文偏移。', '',
             '[原始产品目录](product-catalog.json)、[注册依据](product-registration-audit.md)保留HTTP收据和身份。当前[页面覆盖](native-product-coverage.md)与[分组审查](product-review-current.md)仍是partial_native；没有任何一款被宣布完全逆向或全部接入。', '',
             '| 产品 ID | 原目录名称 | 当前UI入口 | 注册页（主 / 独立） |', '| --- | --- | --- | ---: |']
    for product in catalog['products']:
        registered_product = registered.get(product['product_id'])
        primary = sum(p['primary'] for p in registered_product['pages']) if registered_product else 0
        independent = len(registered_product['pages']) - primary if registered_product else 0
        lines.append(f"| {product['product_id']} | {cell(product['name'])} | {'已取得' if product['ui_entry_found'] else '未取得：详见HTTP原收据'} | {primary} / {independent} |")
    write('16-product-catalog.md', lines, args.check)
    original = (ROOT / 'docs/re/17-application-catalog.md').read_text('utf-8')
    marker = '<!-- FULL-CURRENT-CHAIN-AUDIT -->'
    before = original.split(marker)[0].rstrip()
    addition = [before, '', marker, '', '## 本次全量静态链补充', '',
                f"24个端点的HTML→入口manifest→JS模块/导出/依赖/lazy或ESM→状态/消息候选→CSS规则/字体/资源已经全部列入[全量应用链](all-application-chains-current.md)与[机器索引](all-application-chains-current.json)。本次从本地当前源解析{apps['summary']['unique_js_files']}个唯一JS，全部解析成功，登记{apps['summary']['webpack_modules']}个带webpack标记的模块factory候选及{apps['summary']['css_file_references']}次CSS文件引用。两个无当前页源端点仍保留原404状态，不能从其他应用推算界面。", '',
                '模块、hook和命名调用候选来自语法定位，包含框架/第三方库。真正页面挂载条件、请求/订阅/响应字段、失败/清理以及动态样式尚需逐根审查；此补充没有把“源文件全量取得/解析”改成“全部细节已逆向”。本地功能接入与差异仍以[公共应用审查](application-review-current.md)及各current契约为准。']
    write('17-application-catalog.md', addition, args.check)
    print(f"Reports: {len(graph['products'])} products / {sum(len(p['pages']) for p in graph['products'])} pages / {len(apps['applications'])} applications; no semantic completion inferred")


if __name__ == '__main__':
    main()
