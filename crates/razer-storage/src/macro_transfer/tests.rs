use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-macro-transfer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        for entry in fs::read_dir(&self.0).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                fs::remove_dir(entry.path()).unwrap();
            } else {
                fs::remove_file(entry.path()).unwrap();
            }
        }
        fs::remove_dir(&self.0).unwrap();
    }
}

#[test]
fn imported_file_requires_utf8_xml_and_preserves_native_text() {
    let directory = Directory::new();
    let path = directory.0.join("macro.XML");
    let request = Transfer::new();
    let xml = "\u{feff}<Macro><Name>导入</Name><Version>4</Version><MacroEvents><MacroEvent><Type>actionBar</Type></MacroEvent><MacroEvent><Type>4</Type><Text>你好</Text></MacroEvent></MacroEvents></Macro>";
    fs::write(&path, xml).unwrap();
    let imported = request.accept().unwrap().read_xml(&path).unwrap();
    assert_eq!(imported.name, "导入");
    assert_eq!(imported.actions[0].value, "你好");
    fs::write(&path, [0xff, 0xfe]).unwrap();
    assert!(
        request
            .accept()
            .unwrap()
            .read_xml(&path)
            .unwrap_err()
            .contains("UTF-8")
    );
    let invalid_extension = directory.0.join("macro.json");
    fs::write(&invalid_extension, xml).unwrap();
    assert!(
        request
            .accept()
            .unwrap()
            .read_xml(&invalid_extension)
            .is_err()
    );
}
