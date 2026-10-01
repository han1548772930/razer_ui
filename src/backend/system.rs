//! Explicit user-triggered Windows property dialogs; no device-write simulation.
#[derive(Clone, Copy)]
pub(crate) enum Properties {
    Mouse,
    Keyboard,
    Sound,
    Volume,
}
pub(crate) fn open(properties: Properties) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let mut command = match properties {
            Properties::Volume => std::process::Command::new("sndvol.exe"),
            _ => {
                let mut command = std::process::Command::new("control.exe");
                match properties {
                    Properties::Mouse => {
                        command.arg("main.cpl");
                    }
                    Properties::Keyboard => {
                        command.args(["main.cpl", ",@1"]);
                    }
                    Properties::Sound => {
                        command.arg("mmsys.cpl");
                    }
                    Properties::Volume => unreachable!(),
                }
                command
            }
        };
        command.spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = properties;
        anyhow::bail!("Windows only")
    }
}
