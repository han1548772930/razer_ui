//! Local Settings Ks -> RazerApp.updateAppsAutoStart -> SysUtilsNative Run key.
//! The standalone application owns `razer_ui`, separate from RazerAppEngine.
use std::path::Path;

pub fn auto_start() -> anyhow::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        let command = crate::platform::windows::startup::read_command()?;
        Ok(command_has_synapse(command.as_deref()))
    }
    #[cfg(not(target_os = "windows"))]
    anyhow::bail!("automatic startup is only available on Windows")
}

/// Write the OS registration. Success is distinct from the local editor draft.
pub fn set_auto_start(enabled: bool, minimized: bool) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let executable = std::env::current_exe()?;
        apply(
            &mut crate::platform::windows::startup::RunKey,
            &executable,
            enabled,
            minimized,
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (enabled, minimized);
        anyhow::bail!("automatic startup is only available on Windows")
    }
}

pub(crate) trait Registration {
    fn set_command(&mut self, command: &str) -> anyhow::Result<()>;
    fn remove_command(&mut self) -> anyhow::Result<()>;
}

pub(crate) fn apply(
    registration: &mut impl Registration,
    executable: &Path,
    enabled: bool,
    minimized: bool,
) -> anyhow::Result<()> {
    if enabled {
        registration.set_command(&startup_command(executable, minimized)?)
    } else {
        registration.remove_command()
    }
}

fn startup_command(executable: &Path, minimized: bool) -> anyhow::Result<String> {
    let executable = executable
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("startup executable is not valid Unicode"))?;
    anyhow::ensure!(
        !executable.is_empty() && !executable.contains(['"', '\0', '\r', '\n']),
        "invalid startup executable path"
    );
    // SDK 714 strips &autoStart=1; native 0x6e3c0 quotes the executable,
    // appends --url-params and then --autoStart=1. No remote URL is involved.
    let hidden = if minimized {
        " --launch-force-hidden=synapse"
    } else {
        ""
    };
    Ok(format!(
        "\"{executable}\" --url-params=apps=synapse{hidden} --autoStart=1"
    ))
}

fn command_has_synapse(command: Option<&str>) -> bool {
    // 0x6e950 extracts the --url-params token up to its first space;
    // RazerApp.getAppAutoStart("synapse") then uses String.includes.
    command
        .and_then(|command| command.split_once("--url-params="))
        .map(|(_, parameters)| parameters.split(' ').next().unwrap_or_default())
        .is_some_and(|parameters| parameters.contains("synapse"))
}

/// The SDK encodes its saved preference into launch-force-hidden. Host xn
/// reads that flag; autoStart alone only marks the launch and does not hide it.
pub fn starts_hidden<'a>(arguments: impl IntoIterator<Item = &'a str>) -> bool {
    arguments.into_iter().any(|argument| {
        argument
            .strip_prefix("--launch-force-hidden=")
            .is_some_and(|apps| apps.split(',').any(|app| app == "synapse"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct SimulatedRegistration {
        command: Option<String>,
        fail: bool,
        writes: usize,
        removals: usize,
    }
    impl Registration for SimulatedRegistration {
        fn set_command(&mut self, command: &str) -> anyhow::Result<()> {
            self.writes += 1;
            anyhow::ensure!(!self.fail, "simulated registry permission failure");
            self.command = Some(command.into());
            Ok(())
        }
        fn remove_command(&mut self) -> anyhow::Result<()> {
            self.removals += 1;
            anyhow::ensure!(!self.fail, "simulated registry permission failure");
            self.command = None;
            Ok(())
        }
    }

    #[test]
    fn local_command_quotes_unicode_paths_and_keeps_source_launch_flags() {
        let executable = Path::new(r"C:\本地应用\Razer UI\razer_ui.exe");
        let mut registration = SimulatedRegistration::default();
        apply(&mut registration, executable, true, true).unwrap();
        assert_eq!(
            registration.command.as_deref(),
            Some(
                "\"C:\\本地应用\\Razer UI\\razer_ui.exe\" --url-params=apps=synapse --launch-force-hidden=synapse --autoStart=1"
            )
        );
        assert!(command_has_synapse(registration.command.as_deref()));
        apply(&mut registration, executable, true, false).unwrap();
        assert!(
            !registration
                .command
                .as_deref()
                .unwrap()
                .contains("force-hidden")
        );
        apply(&mut registration, executable, false, false).unwrap();
        assert!(registration.command.is_none());
        assert_eq!((registration.writes, registration.removals), (2, 1));
    }

    #[test]
    fn failed_os_writes_preserve_the_previous_registration_and_report_failure() {
        let mut registration = SimulatedRegistration {
            command: Some("previous command".into()),
            fail: true,
            ..Default::default()
        };
        assert!(apply(&mut registration, Path::new("razer_ui.exe"), true, true).is_err());
        assert!(apply(&mut registration, Path::new("razer_ui.exe"), false, false).is_err());
        assert_eq!(registration.command.as_deref(), Some("previous command"));
    }

    #[test]
    fn query_matches_the_source_parameter_token_without_using_other_flags() {
        for command in [
            None,
            Some(""),
            Some("exe --url-params=apps=chroma-app --launch-force-hidden=synapse"),
            Some("exe --autoStart=1"),
        ] {
            assert!(!command_has_synapse(command));
        }
        assert!(command_has_synapse(Some(
            "exe --url-params=apps=chroma-app,synapse --autoStart=1"
        )));
    }

    #[test]
    fn only_the_source_hidden_app_list_hides_a_launch() {
        assert!(!starts_hidden(["razer_ui.exe"]));
        assert!(!starts_hidden(["--autoStart=1"]));
        assert!(starts_hidden(["--launch-force-hidden=chroma-app,synapse"]));
        assert!(!starts_hidden(["--launch-force-hidden=chroma-app"]));
    }

    #[test]
    fn invalid_paths_fail_before_mutating_the_registration() {
        let mut registration = SimulatedRegistration::default();
        for path in ["", "bad\"path.exe", "bad\0path.exe"] {
            assert!(apply(&mut registration, Path::new(path), true, false).is_err());
        }
        assert_eq!(registration.writes, 0);
    }
}
