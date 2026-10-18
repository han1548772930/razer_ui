//! The keyboard mapping publisher retains its shared service scope, execution
//! gate and confirmed cleanup through transport destruction.
use super::*;
use anyhow::{Context as _, ensure};
use razer_ipc::{ServiceRequest, mapping_control};
use razer_service::mapping::Client;

pub(super) trait Transport {
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value>;
}
impl Transport for Client {
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        Client::request(self, request)
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| anyhow::anyhow!("756 mapping owner panicked"))?
}

fn shutdown(client: &mut impl Transport) -> anyhow::Result<()> {
    guarded(|| {
        let receipt = client.request(ServiceRequest::Shutdown)?;
        ensure!(
            receipt["stopped"] == true,
            "756 mapping cleanup was not acknowledged"
        );
        Ok(())
    })
}

pub(super) struct Publisher<T: Transport = Client> {
    client: Option<T>,
    owner: String,
    control: mapping_control::Owner,
    failed: Option<String>,
}

impl Publisher {
    pub(super) fn start(container: &str) -> anyhow::Result<Self> {
        Self::start_with(
            container,
            Client::spawn,
            mapping_control::Registry::global(),
        )
    }
}

impl<T: Transport> Publisher<T> {
    fn start_with(
        container: &str,
        factory: impl FnOnce() -> anyhow::Result<T>,
        registry: mapping_control::Registry,
    ) -> anyhow::Result<Self> {
        let mut client = guarded(factory)?;
        let owner = uuid::Uuid::new_v4().to_string();
        let control = match registry.register(owner.clone()) {
            Ok(control) => control,
            Err(error) => {
                let cleanup = shutdown(&mut client);
                drop(client);
                return Err(match cleanup {
                    Ok(()) => anyhow::anyhow!(error),
                    Err(cleanup) => anyhow::anyhow!("{error}; {cleanup:#}"),
                });
            }
        };
        let mut publisher = Self {
            client: Some(client),
            owner,
            control,
            failed: None,
        };
        let started = guarded(|| -> anyhow::Result<()> {
            let receipt = publisher.request(ServiceRequest::MappingDeviceStart {
                product_id: 756,
                device_container_id: container.into(),
                owner_guid: publisher.owner.clone(),
            })?;
            ensure!(
                receipt["owner_guid"] == publisher.owner
                    && receipt["registered"] == true
                    && receipt["driver_ready"] == true,
                "756 native mapping registration receipt differs"
            );
            publisher.apply_control()
        });
        if let Err(error) = started {
            let cleanup = publisher.finish();
            return Err(match cleanup {
                Ok(()) => error,
                Err(cleanup) => anyhow::anyhow!("{error:#}; {cleanup:#}"),
            });
        }
        Ok(publisher)
    }

    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        let client = self
            .client
            .as_mut()
            .context("756 mapping transport was released")?;
        guarded(|| client.request(request))
    }

    pub(super) fn apply_control(&mut self) -> anyhow::Result<()> {
        if let Some(error) = &self.failed {
            anyhow::bail!("{error}");
        }
        let client = self
            .client
            .as_mut()
            .context("756 mapping transport was released")?;
        let result = razer_service::mapping::apply_execution_control(&self.control, |request| {
            client.request(request)
        });
        if let Err(error) = &result {
            self.failed = Some(format!("{error:#}"));
        }
        result
    }

    pub(super) fn publish(
        &mut self,
        document: Value,
        generation: &Generation,
    ) -> anyhow::Result<()> {
        generation.check()?;
        self.apply_control()?;
        let receipt = self.request(ServiceRequest::MappingDevicePublish {
            owner_guid: self.owner.clone(),
            document,
        })?;
        ensure!(
            receipt["owner_guid"] == self.owner && receipt["published"] == true,
            "756 native mapping publication receipt differs"
        );
        generation.check()
    }

    pub(super) fn events(&mut self) -> anyhow::Result<Vec<Value>> {
        self.apply_control()?;
        let receipt = self.request(ServiceRequest::MappingDeviceEvents {
            owner_guid: self.owner.clone(),
        })?;
        ensure!(
            receipt["owner_guid"] == self.owner,
            "756 native mapping event owner differs"
        );
        Ok(receipt["events"]
            .as_array()
            .context("756 mapping event list is missing")?
            .clone())
    }

    pub(super) fn macro_mode(&mut self, generation: &Generation) -> anyhow::Result<String> {
        self.macro_mode_with(generation, razer_service::mapping::recorder::mode)
    }

    fn macro_mode_with(
        &mut self,
        generation: &Generation,
        read: impl FnOnce() -> anyhow::Result<razer_platform::macro_recorder::Mode>,
    ) -> anyhow::Result<String> {
        generation.check()?;
        self.apply_control()?;
        let mode = read()?;
        generation.check()?;
        Ok(mode.as_str().to_owned())
    }

    pub(super) fn hypershift(&mut self, generation: &Generation) -> anyhow::Result<bool> {
        generation.check()?;
        self.apply_control()?;
        let receipt = self.request(ServiceRequest::MappingDeviceHypershiftActive {
            owner_guid: self.owner.clone(),
        })?;
        ensure!(
            receipt["owner_guid"] == self.owner,
            "756 Hypershift owner differs"
        );
        let enabled = receipt["isEnabled"]
            .as_bool()
            .context("756 Hypershift callback value is absent")?;
        generation.check()?;
        Ok(enabled)
    }

    fn cleanup(&mut self) -> anyhow::Result<()> {
        let Some(mut client) = self.client.take() else {
            return Ok(());
        };
        let cleanup = shutdown(&mut client).map_err(|error| format!("{error:#}"));
        drop(client);
        let finished = self.control.finish(cleanup.clone());
        let failures: Vec<_> = [cleanup, finished]
            .into_iter()
            .filter_map(Result::err)
            .collect();
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }

    pub(super) fn finish(mut self) -> anyhow::Result<()> {
        self.cleanup()
    }
}
impl<T: Transport> Drop for Publisher<T> {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("[keyboard756 mapping] {error:#}");
        }
    }
}

#[cfg(test)]
#[path = "keyboard_latest_restore_publisher_tests.rs"]
mod tests;
