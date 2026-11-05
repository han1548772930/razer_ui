"""A matching label must not hide executable device behavior."""
import sys
import unittest

import analysis as a
import code_identity_roles as roles
import evidence


class FactoryRoleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))

    def classify(self, code):
        source = evidence._LocaleSource(b"(" + code + b");")
        factory = source.tree.root_node.named_children[0].named_children[0].named_children[0]
        return roles.classify(source, factory)

    def test_static_exports_resolve_literals_and_aliases(self):
        result = self.classify(b'(e,t,n)=>{"use strict";n.r(t);n.d(t,{TAB_LIGHTING:()=>b});const a="Lighting",b=a;}')
        self.assertEqual(result["role"], "static_string_exports")
        self.assertEqual(result["lighting_named_export"], "Lighting")

    def test_device_side_effect_is_not_data(self):
        result = self.classify(b'(e,t,n)=>{n.d(t,{TAB_LIGHTING:()=>a});const a="Lighting";device.setBrightness(0);}')
        self.assertEqual(result["role"], "unclassified")

    def test_imported_or_dynamic_value_is_not_data(self):
        for value in (b'n(2)', b'other', b'()=>"Lighting"'):
            result = self.classify(b'(e,t,n)=>{n.d(t,{TAB_LIGHTING:()=>a});const a=' + value + b';}')
            self.assertEqual(result["role"], "unclassified")

    def test_executable_locale_runtime_is_kept(self):
        result = self.classify(b'(e,t,n)=>{n.d(t,{getLanguageItem:()=>a,getTextItem:()=>b,swapLang:()=>c,updateLanguage:()=>d});const a=()=>"en",b=()=>"Lighting",c=()=>{},d=()=>{};}')
        self.assertEqual(result["role"], "locale_runtime_with_dictionaries")
        self.assertFalse(result["pure_data"])
        self.assertEqual(set(result["implementations"]), roles.LANGUAGE_API)

    def test_extra_export_prevents_runtime_shortcut(self):
        result = self.classify(b'(e,t,n)=>{n.d(t,{getLanguageItem:()=>a,getTextItem:()=>b,swapLang:()=>c,updateLanguage:()=>d,setBrightness:()=>d});const a=()=>"en",b=()=>"Lighting",c=()=>{},d=()=>{};}')
        self.assertEqual(result["role"], "unclassified")


if __name__ == "__main__":
    unittest.main()
