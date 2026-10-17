"""Pure structural tests for the GPUI resource owner guard."""
from pathlib import Path
import tempfile
import unittest

import resource_scope


class ResourceScopeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parser = resource_scope.parser(Path(__file__).resolve().parent.parent)

    def inspect(self, source, owner="crates/razer-pages/src/example.rs"):
        return resource_scope.inspect_source(source.encode(), owner=owner, rust_parser=self.parser)

    def test_raw_deferred_alias_fails_in_another_crate(self):
        for call in ["gpui_kit::deferred(div())", "later(div())"]:
            with self.subTest(call=call):
                result = self.inspect("use gpui_kit::deferred as later; fn view() { " + call + "; }")
                self.assertEqual([v["kind"] for v in result["violations"]], ["raw_deferred"])

    def test_owner_is_allowed_but_same_basename_in_another_crate_is_not(self):
        source = "fn view() { gpui_kit::deferred(resource_scope(child)); window.defer_draw(child, p, 1, None); }"
        self.assertEqual(self.inspect(source, resource_scope.OWNER)["violations"], [])
        self.assertEqual(len(self.inspect(source, "crates/consumer/src/scope.rs")["violations"]), 2)

    def test_popup_content_and_early_returns_require_scope(self):
        for early, count in [("return div();", 1), ("return resource_scope(div());", 0)]:
            result = self.inspect("fn view() { BasePopup::new().content(move |_, _| resource_scope({ "
                                  + early + " div() })); }")
            self.assertEqual(len(result["violations"]), count)

    def test_nested_closure_return_does_not_escape_the_resource_scope(self):
        result = self.inspect("fn view() { Popover::new().content(move |_, _| resource_scope({ "
                              "let handler = || { return false; }; div().on_click(handler) })); }")
        self.assertEqual(result["violations"], [])

    def test_dialog_alias_and_window_content_are_checked(self):
        result = self.inspect('use gpui_kit::base::Dialog as Warning; fn view() { '
                              'Warning::new(cx).popup(panel); '
                              'window.open_dialog(cx, move |d, _, _| d.title(resource_scope("Title"))'
                              '.child(resource_scope(page)).footer(footer)); }')
        self.assertEqual([v["kind"] for v in result["violations"]],
                         ["dialog_popup", "window_overlay_content"])
        self.assertEqual(len(result["edges"]), 4)

    def test_unrelated_methods_comments_strings_and_test_code_are_not_rejected(self):
        result = self.inspect('fn view() { // gpui_kit::deferred(div())\n'
                              'let s = "window.defer_draw(...)"; ble.deferred(); model.popup(); '
                              'data.content("text"); } #[cfg(test)] mod tests { '
                              'fn sample() { gpui_kit::deferred(div()); } }')
        self.assertEqual(result["violations"], [])
        self.assertEqual(result["test_edges"], 1)

    def test_macro_overlay_is_reported_as_unverified(self):
        result = self.inspect("fn view() { custom!(deferred(div())); }")
        self.assertEqual([v["kind"] for v in result["violations"]], ["overlay_macro_requires_audit"])

    def test_actual_workspace_paths_and_source_identity_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            owner = root / resource_scope.OWNER
            owner.parent.mkdir(parents=True)
            owner.write_text("fn view() { deferred(resource_scope(child)); }", encoding="utf-8")
            consumer = root / "crates/consumer/src/view.rs"
            consumer.parent.mkdir(parents=True)
            consumer.write_text("fn view() { deferred(child); }", encoding="utf-8")
            result = resource_scope.audit_sources(root)
            self.assertFalse(result["passed"])
            self.assertTrue(result["inputs_unchanged"])
            self.assertEqual(len(result["inputs"]), 2)
            self.assertEqual([v["owner"] for v in result["violations"]],
                             ["crates/consumer/src/view.rs"])


if __name__ == "__main__":
    unittest.main()
