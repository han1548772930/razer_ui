//! Installed camera apps: current GenericCamera 1.0.36.1 RVAs 41390,
//! 40c50/41010/411d0, 5820, 6670, 6b70, 73c0, 7e50, 81c0, 8c40.
//! These are local registry, driver and shell operations, with no downloads.
use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraApp {
    Camo,
    NvidiaBroadcast,
    XSplitVCam,
}
impl CameraApp {
    pub fn from_source_name(name: &str) -> Result<Self> {
        match name {
            "Camo Studio" => Ok(Self::Camo),
            "NVIDIA Broadcast" => Ok(Self::NvidiaBroadcast),
            "XSplit VCam" => Ok(Self::XSplitVCam),
            _ => bail!("unknown camera application: {name}"),
        }
    }
}

pub fn launch_installed(app: CameraApp) -> Result<()> {
    #[cfg(windows)]
    return windows::launch(app);
    #[cfg(not(windows))]
    {
        let _ = app;
        bail!("the audited camera application adapter requires Windows");
    }
}

fn package_app_id(full_name: &str) -> Option<String> {
    let first = full_name.find('_')?;
    let last = full_name.rfind('_')?;
    (first != last && first > 0 && last + 1 < full_name.len())
        .then(|| format!("{}{}!App", &full_name[..first], &full_name[last..]))
}

fn vcam_executable(uninstall: &str) -> Option<String> {
    let begin = uninstall.find('"')? + 1;
    let path = &uninstall[begin..begin + uninstall[begin..].find('"')?];
    Some(format!("{}VCam.exe", &path[..=path.rfind('\\')?]))
}

#[cfg(windows)]
#[path = "camera_apps/windows.rs"]
mod windows;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_package_family_and_uninstaller_conversions() {
        assert_eq!(
            package_app_id("ReincubateLtd.CamoStudio_1.2.3.4_x64__publisher"),
            Some("ReincubateLtd.CamoStudio_publisher!App".into())
        );
        assert_eq!(package_app_id("bad"), None);
        assert_eq!(
            vcam_executable(r#""C:\Program Files\VCam\uninstall.exe" /remove"#),
            Some(r"C:\Program Files\VCam\VCam.exe".into())
        );
        assert_eq!(vcam_executable(r"C:\VCam\uninstall.exe"), None);
        assert!(CameraApp::from_source_name("untrusted.exe").is_err());
    }
}
