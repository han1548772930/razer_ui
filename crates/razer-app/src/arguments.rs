//! Product launch arguments. Native analysis belongs to the maintenance tools.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DriverInstallation {
    Install,
    Uninstall,
}

pub(super) fn driver_installation(arguments: &[String]) -> Option<DriverInstallation> {
    arguments.iter().skip(1).find_map(|argument| match argument.as_str() {
        "--install-hardware-driver" => Some(DriverInstallation::Install),
        "--uninstall-hardware-driver" => Some(DriverInstallation::Uninstall),
        _ => None,
    })
}

pub(super) fn validate(arguments: &[String]) -> Result<(), String> {
    if driver_installation(arguments).is_some() {
        return if arguments.len() == 2 {
            Ok(())
        } else {
            Err("Hardware driver installation must be invoked alone, without application or worker arguments".into())
        };
    }
    let mut arguments = arguments.iter().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--service-worker" => {}
            "--lang" => {
                if arguments
                    .next()
                    .is_none_or(|locale| locale.is_empty() || locale.starts_with("--"))
                {
                    return Err("--lang requires a locale".into());
                }
            }
            "--autoStart=1" | "--url-params=apps=synapse" => {}
            value if value.starts_with("--launch-force-hidden=") => {}
            _ => {
                return Err(format!(
                    "Unknown product argument: {argument}. Native analysis uses tools/workspace.py."
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(arguments: &[&str]) -> Result<(), String> {
        validate(
            &std::iter::once("app")
                .chain(arguments.iter().copied())
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        )
    }
    #[test]
    fn accepts_product_launch_and_installed_startup_arguments() {
        assert!(check(&[]).is_ok());
        assert!(check(&["--service-worker"]).is_ok());
        assert!(check(&["--lang", "zh-CN"]).is_ok());
        assert!(
            check(&[
                "--url-params=apps=synapse",
                "--launch-force-hidden=synapse",
                "--autoStart=1"
            ])
            .is_ok()
        );
    }
    #[test]
    fn native_probe_flags_cannot_fall_through_to_starting_the_application() {
        for flag in [
            "--probe",
            "--lighting",
            "--dll-version",
            "--dll-get",
            "--unknown",
        ] {
            assert!(check(&[flag]).is_err());
        }
        assert!(check(&["--lang"]).is_err());
        assert!(check(&["--lang", "--service-worker"]).is_err());
    }
    #[test]
    fn driver_installation_cannot_be_combined_with_worker_ui_or_another_action() {
        for flag in ["--install-hardware-driver", "--uninstall-hardware-driver"] {
            assert!(check(&[flag]).is_ok());
            assert!(check(&[flag, "--service-worker"]).is_err());
            assert!(check(&["--lang", "en-US", flag]).is_err());
            assert!(check(&[flag, flag]).is_err());
        }
        assert!(check(&["--install-hardware-driver", "--uninstall-hardware-driver"]).is_err());
    }
}
