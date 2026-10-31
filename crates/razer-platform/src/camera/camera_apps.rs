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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CamoStudio {
    Absent,
    Installed {
        version: std::result::Result<Option<String>, String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CamoDriver {
    Absent,
    Installed {
        enabled: std::result::Result<bool, String>,
    },
}

/// Independent local observations. A failed status query cannot become an
/// installed/enabled default, and a missing version does not erase registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CamoInstallation {
    studio: std::result::Result<CamoStudio, String>,
    driver: std::result::Result<CamoDriver, String>,
}

impl CamoInstallation {
    pub fn from_observations(
        studio: std::result::Result<CamoStudio, String>,
        driver: std::result::Result<CamoDriver, String>,
    ) -> Self {
        Self { studio, driver }
    }

    pub fn studio(&self) -> &std::result::Result<CamoStudio, String> {
        &self.studio
    }

    pub fn driver(&self) -> &std::result::Result<CamoDriver, String> {
        &self.driver
    }

    /// CheckSpecificCameraAppInstalled, used by the mounted camera UI.
    pub fn camera_available(&self) -> std::result::Result<bool, String> {
        if matches!(self.studio, Ok(CamoStudio::Absent))
            || matches!(self.driver, Ok(CamoDriver::Absent))
        {
            return Ok(false);
        }
        self.studio.as_ref().map_err(Clone::clone)?;
        match self.driver.as_ref().map_err(Clone::clone)? {
            CamoDriver::Installed { enabled } => enabled.clone(),
            CamoDriver::Absent => Ok(false),
        }
    }

    /// ON_LAUNCH_CAMO_STUDIO checks Studio OR driver registration. Driver
    /// enablement is a separate fact and is not this command's admission rule.
    pub fn help_launch_available(&self) -> std::result::Result<bool, String> {
        if matches!(self.studio, Ok(CamoStudio::Installed { .. }))
            || matches!(self.driver, Ok(CamoDriver::Installed { .. }))
        {
            return Ok(true);
        }
        self.studio.as_ref().map_err(Clone::clone)?;
        self.driver.as_ref().map_err(Clone::clone)?;
        Ok(false)
    }
}

pub fn observe_camo() -> CamoInstallation {
    #[cfg(windows)]
    return windows::observe_camo();
    #[cfg(not(windows))]
    CamoInstallation {
        studio: Err("Camo application registration requires Windows".into()),
        driver: Err("Camo driver observation requires Windows".into()),
    }
}

pub fn launch_camo_from_help(validate: &impl Fn() -> Result<()>) -> Result<()> {
    validate()?;
    let observed = observe_camo();
    anyhow::ensure!(
        observed
            .help_launch_available()
            .map_err(anyhow::Error::msg)?,
        "Camo Studio and its driver are not installed"
    );
    validate()?;
    #[cfg(windows)]
    return windows::launch_camo(validate);
    #[cfg(not(windows))]
    bail!("Camo application launch requires Windows");
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

fn package_version(full_name: &str) -> Option<String> {
    let (_, remaining) = full_name.split_once('_')?;
    let (version, _) = remaining.split_once('_')?;
    (!version.is_empty()).then(|| version.to_owned())
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
        assert_eq!(
            package_version("ReincubateLtd.CamoStudio_1.2.3.4_x64__publisher"),
            Some("1.2.3.4".into())
        );
        assert_eq!(package_version("ReincubateLtd.CamoStudio__publisher"), None);
    }

    #[test]
    fn help_and_generic_camera_commands_preserve_distinct_source_gates() {
        for (studio, driver, enabled) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (false, true, true),
            (true, true, false),
            (true, true, true),
        ] {
            let observation = CamoInstallation {
                studio: Ok(if studio {
                    CamoStudio::Installed { version: Ok(None) }
                } else {
                    CamoStudio::Absent
                }),
                driver: Ok(if driver {
                    CamoDriver::Installed {
                        enabled: Ok(enabled),
                    }
                } else {
                    CamoDriver::Absent
                }),
            };
            assert_eq!(
                observation.camera_available(),
                Ok(studio && driver && enabled)
            );
            assert_eq!(observation.help_launch_available(), Ok(studio || driver));
        }
    }

    #[test]
    fn failed_queries_remain_independent_of_proven_registration() {
        let observation = CamoInstallation {
            studio: Ok(CamoStudio::Installed { version: Ok(None) }),
            driver: Ok(CamoDriver::Installed {
                enabled: Err("CM status failed".into()),
            }),
        };
        assert_eq!(
            observation.camera_available(),
            Err("CM status failed".into())
        );
        assert_eq!(observation.help_launch_available(), Ok(true));
        let observation = CamoInstallation {
            studio: Err("registry access denied".into()),
            driver: Ok(CamoDriver::Absent),
        };
        assert_eq!(observation.camera_available(), Ok(false));
        assert_eq!(
            observation.help_launch_available(),
            Err("registry access denied".into())
        );
        assert!(observation.studio().is_err());
    }
}
