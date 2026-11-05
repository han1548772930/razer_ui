"""Mounted roots must not be inferred from unused navigation declarations."""
from contextlib import ExitStack
import hashlib
from pathlib import Path
import tempfile
import sys
import unittest
from unittest.mock import patch

import analysis as a
import code_identity as ci
import code_identity_navigation as nav


def navigation_class(name, page, *, selected=True):
    value = "row.component" if selected else "null"
    return f'''class {name} {{constructor(){{this.state={{active_view:"{page}",navs:[{{id:1,name:"{page}",component:J.jsx(Page,{{}})}}]}}}}
        renderView(){{const row=this.state.navs.find(x=>x.name===this.state.active_view);return {value}}}
        render(){{return this.renderView()}}changeView(v){{this.setState({{active_view:v}})}}}}'''


class NavigationTests(unittest.TestCase):
    def setUp(self):
        sys.path.insert(0,str(a.ROOT/"tools/analyzers/python-static"))
        self.stack=ExitStack();self.addCleanup(self.stack.close)
        self.root=Path(self.stack.enter_context(tempfile.TemporaryDirectory()))
        self.stack.enter_context(patch.object(a,"ROOT",self.root))
        self.db_path=self.root/"index.sqlite3"
        self.db=ci.connect(self.db_path);self.addCleanup(self.db.close)

    def inspect(self, code):
        raw=code.encode();path=self.root/".analysis/inputs/products/1/ui/main.test.js"
        path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw)
        source_sha=hashlib.sha256(raw).hexdigest()
        self.db.execute("INSERT INTO sources VALUES(?,?,?)",(a.rel(path),source_sha,len(raw)))
        self.db.execute("INSERT INTO deployment_sources VALUES(?,?,?,?)",(1,"ui",a.rel(path),"main.test.js"));self.db.commit()
        job=nav.Navigation(self.db_path,{"product_id":1,"family":"keyboard","acquisition":"acquisition.json","acquisition_sha":"a"*64})
        self.addCleanup(job.close)
        return job.run()

    def test_only_consumed_navigation_is_recorded(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')+navigation_class('Unused','FAKE')
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'
        result=self.inspect(code)
        self.assertEqual([p['kind'] for p in result['pages']],['TAB_LIGHTING'])
        self.assertTrue(result['source_scope_complete'])
        self.assertTrue(result['pages'][0]['navigation_consumers'])

    def test_resolved_ascii_case_name_keeps_transformation_owner(self):
        code='class Page{render(){return null}}'+navigation_class('Live','ACTUATION')
        code=code.replace('name:"ACTUATION"','name:"actuation".toUpperCase()')
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'
        result=self.inspect(code)
        self.assertEqual([p['kind'] for p in result['pages']],['ACTUATION'])
        self.assertTrue(any('navigation_ascii_case_transform' in o['roles'] for o in result['owners'].values()))

    def test_custom_case_method_is_not_evaluated_as_a_string(self):
        code='class Page{render(){return null}};const custom={toUpperCase(){return "ACTUATION"}};'+navigation_class('Live','ACTUATION')
        code=code.replace('name:"ACTUATION"','name:custom.toUpperCase()')
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'
        result=self.inspect(code)
        self.assertEqual(result['pages'],[])
        self.assertTrue(result['failures'])

    def test_conditional_append_before_state_keeps_actual_guard(self):
        code='''class Page{render(){return null}}class Live{
          constructor(p){const items=[{id:1,name:"MAIN",component:J.jsx(Page,{})}];
            (!p.isBle||p.calibration)&&items.push({id:2,name:"TAB_CALIBRATION",component:J.jsx(Page,{})});
            this.state={navs:items,active_view:"MAIN"}}
          renderView(){const row=this.state.navs.find(x=>x.name===this.state.active_view);return row.component}
          render(){return this.renderView()}}
          R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'''
        result=self.inspect(code)
        self.assertEqual([p['kind'] for p in result['pages']],['MAIN','TAB_CALIBRATION'])
        self.assertEqual(result['pages'][1]['conditions'][0]['operator'],'&&')
        self.assertTrue(any('navigation_literal_append' in o['roles'] for o in result['owners'].values()))

    def test_deferred_append_does_not_invent_initial_navigation(self):
        code='''class Page{render(){return null}}class Live{
          constructor(){const items=[{id:1,name:"MAIN",component:J.jsx(Page,{})}];
            const later=()=>items.push({id:2,name:"UNINVOKED",component:J.jsx(Page,{})});
            this.state={navs:items,active_view:"MAIN"}}
          renderView(){const row=this.state.navs.find(x=>x.name===this.state.active_view);return row.component}
          render(){return this.renderView()}}
          R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'''
        self.assertEqual([p['kind'] for p in self.inspect(code)['pages']],['MAIN'])

    def test_unused_lookup_is_not_a_rendered_page(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING',selected=False)
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'
        result=self.inspect(code)
        self.assertEqual(result['pages'],[])
        self.assertFalse(result['source_scope_complete'])

    def test_unused_lazy_prop_does_not_mount_its_argument(self):
        code='const Fake=()=>null; R.createRoot(document.getElementById("root")).render(J.jsx(Fake,{lazy:()=>n.e(1).then(n.bind(n,2))}));'
        with self.assertRaisesRegex(ValueError,'lazy component differs'):
            self.inspect(code)

    def test_uninvoked_function_does_not_establish_startup(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')
        code+='function unused(){R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));}'
        with self.assertRaisesRegex(ValueError,'uninvoked function'):
            self.inspect(code)

    def test_fake_display_selector_does_not_mount_declared_children(self):
        code='const Fake=()=>null; class Page{};'+navigation_class('Live','TAB_LIGHTING')
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Fake,{$name:"default",children:{default:J.jsx(Live,{})}}));'
        with self.assertRaisesRegex(ValueError,'display selector differs'):
            self.inspect(code)

    def test_fragment_keeps_all_actual_navigation_siblings(self):
        code='class Page{render(){return null}}'+navigation_class('First','ONE')+navigation_class('Second','TWO')
        code+='R.createRoot(document.getElementById("root")).render(J.jsxs(J.Fragment,{children:[J.jsx(First,{}),J.jsx(Second,{})]}));'
        result=self.inspect(code)
        self.assertEqual({p['kind'] for p in result['pages']},{'ONE','TWO'})

    def test_same_wrapper_keeps_different_children(self):
        code='class Page{render(){return null}}'+navigation_class('First','ONE')+navigation_class('Second','TWO')
        code+='const Wrap=({children:e})=>J.jsx(J.Fragment,{children:e});'
        code+='R.createRoot(document.getElementById("root")).render(J.jsxs(J.Fragment,{children:[J.jsx(Wrap,{children:J.jsx(First,{})}),J.jsx(Wrap,{children:J.jsx(Second,{})})]}));'
        result=self.inspect(code)
        self.assertEqual({p['kind'] for p in result['pages']},{'ONE','TWO'})

    def test_spread_props_keep_known_component_and_do_not_invent_values(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Live,{...unknownState}));'
        result=self.inspect(code)
        self.assertEqual([p['kind'] for p in result['pages']],['TAB_LIGHTING'])
        self.assertTrue(any('jsx_props_spread' in o['roles'] for o in result['owners'].values()))

    def test_direct_display_query_does_not_merge_pairing_into_default(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')+navigation_class('Pairing','PAIRING')
        code+='class Root{render(){const url=new URL(window.location.href),mode=url.searchParams.get("displayMode");return "multiDevicePairing"===mode?J.jsx(Pairing,{}):J.jsx(Live,{})}};'
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Root,{}));'
        result=self.inspect(code)
        self.assertEqual([p['kind'] for p in result['pages']],['TAB_LIGHTING'])
        self.assertIn('multiDevicePairing',{row['mode'] for row in result['display_roots']})

    def test_duplicate_names_preserve_both_occurrences_and_first_match(self):
        code='class Page{render(){return null}}'+navigation_class('Live','HELP')
        code=code.replace('component:J.jsx(Page,{})}]','component:J.jsx(Page,{})},{id:2,name:"HELP",component:J.jsx(Page,{resetObm:()=>{}})}]')
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'
        report=self.inspect(code);nav.annotate_selection(report)
        self.assertEqual(len(report['pages']),2)
        self.assertEqual(report['pages'][1]['selection_status'],'shadowed_by_prior_match')
        self.assertNotEqual(report['pages'][0]['occurrence_sha256'],report['pages'][1]['occurrence_sha256'])
        self.db.execute('CREATE TABLE navigation_pages(product_id INTEGER,kind TEXT,mode TEXT,component_owner TEXT,condition_scope TEXT,evidence TEXT,PRIMARY KEY(product_id,evidence))')
        nav._write_projection(self.db,report)
        self.assertEqual(self.db.execute('SELECT count(*) FROM navigation_pages').fetchone()[0],2)

    def test_explicit_root_container_alias(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')
        code+='const container=document.getElementById("root");R.createRoot(container).render(J.jsx(Live,{}));'
        self.assertEqual(len(self.inspect(code)['pages']),1)

    def test_called_children_iife_and_body_destructuring_are_followed(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')
        code+='const Wrap=p=>{let{children:c}=p;return J.jsx(J.Fragment,{children:c})};'
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Wrap,{children:(()=>J.jsx(Live,{}))()}));'
        self.assertEqual(len(self.inspect(code)['pages']),1)

    def test_literal_array_getter_is_a_real_navigation_source(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')
        array='[{id:1,name:"TAB_LIGHTING",component:J.jsx(Page,{})}]'
        code=code.replace('navs:'+array,'navs:this.getDeviceNav()').replace('renderView(){','getDeviceNav(){return '+array+'}renderView(){')
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'
        self.assertEqual(len(self.inspect(code)['pages']),1)

    def test_forwarding_hoc_keeps_hook_owners_and_reaches_component(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')
        code+='function WithHooks(Component,...hooks){return props=>{for(const hook of hooks)hook(props);return J.jsx(Component,{...props})}}const hook=p=>effect(p),Wrapped=WithHooks(Live,hook);'
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Wrapped,{}));'
        report=self.inspect(code)
        self.assertEqual([p['kind'] for p in report['pages']],['TAB_LIGHTING'])
        self.assertTrue(any('hoc_argument_callable' in o['roles'] for o in report['owners'].values()))

    def test_unresolved_name_does_not_hide_later_known_row(self):
        code='class Page{render(){return null}}'+navigation_class('Live','TAB_LIGHTING')
        code=code.replace('navs:[','navs:[{id:9,name:unknownName,component:J.jsx(Page,{})},')
        code+='R.createRoot(document.getElementById("root")).render(J.jsx(Live,{}));'
        report=self.inspect(code);nav.annotate_selection(report)
        self.assertEqual([p['kind'] for p in report['pages']],['TAB_LIGHTING'])
        self.assertEqual(report['pages'][0]['selection_status'],'preceding_name_unresolved')


if __name__=='__main__':unittest.main()
