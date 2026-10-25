use super::*;

pub(super) struct Owner {
    driver: Driver,
    mixer: Mixer,
    channels: Vec<protocol::Channel>,
    endpoints: BTreeMap<u32, Endpoint>,
    product: Product,
    container: String,
    stopping: Arc<AtomicBool>,
    meter_enabled: bool,
    events: BTreeMap<(String, u32), Value>,
    poll_error: Option<String>,
    last_endpoint_read: Instant,
    closed: bool,
    deadline: Option<Instant>,
    notifications: crate::audio_notification::AudioNotifications,
    endpoint_diagnostic: Option<String>,
    recovery_at: Option<Instant>,
    restore: BTreeMap<u32, EndpointObservation>,
    restore_routes: bool,
    observed: routing::LiveState,
}
impl Owner {
    pub(super) fn open(
        product_id: u32,
        product: Product,
        container: &str,
        stopping: Arc<AtomicBool>,
    ) -> Result<Self> {
        let deadline = Instant::now() + NativeOperation::DeviceInitialize.service_budget();
        let driver = Driver::open(product_id, container, deadline, stopping.clone())?;
        let counts = protocol::channel_counts(&driver)?;
        let mode = protocol::mode(&driver)?;
        let mixer = Mixer::new(counts, mode)?;
        let mut owner = Self {
            driver,
            mixer,
            channels: protocol::meter_channels(counts)?,
            endpoints: BTreeMap::new(),
            product,
            container: container.into(),
            stopping,
            meter_enabled: false,
            events: BTreeMap::new(),
            poll_error: None,
            last_endpoint_read: Instant::now(),
            closed: false,
            deadline: Some(deadline),
            notifications: crate::audio_notification::AudioNotifications::default(),
            endpoint_diagnostic: None,
            recovery_at: None,
            restore: BTreeMap::new(),
            restore_routes: false,
            observed: routing::LiveState::default(),
        };
        owner.notifications.enable(true)?;
        owner.meter(true)?;
        owner.acquire_endpoints(Duration::from_secs(10))?;
        owner.deadline = None;
        owner.driver.set_deadline(None);
        Ok(owner)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.stopping.load(Ordering::Acquire),
            "Marie session cancelled"
        );
        ensure!(!self.closed, "Marie session is closed");
        ensure!(
            self.deadline
                .is_none_or(|deadline| Instant::now() < deadline),
            "Marie operation deadline exceeded"
        );
        Ok(())
    }
    fn acquire_once(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for id in [routing::MICROPHONE, routing::SERVICE, routing::HEADPHONES] {
            self.validate()?;
            if self.endpoints.contains_key(&id) {
                continue;
            }
            let result = (|| -> Result<()> {
                let endpoint = Endpoint::open(self.product, &self.container, id)?;
                // 500b0 sets Service to unity, but only mic and HP are required
                // to finish acquisition. A missing Service cannot hide HP.
                let observed = if id == routing::SERVICE {
                    endpoint.set_volume(100)?
                } else {
                    endpoint.observe()?
                };
                self.record_observation(id, observed)?;
                self.endpoints.insert(id, endpoint);
                Ok(())
            })();
            if let Err(error) = result {
                self.observed
                    .set_endpoint(id, routing::Reading::Failed(format!("{error:#}")));
                errors.push(format!("endpoint {id}: {error:#}"));
            }
        }
        self.endpoint_diagnostic = (!errors.is_empty()).then(|| errors.join("; "));
        ensure!(
            self.endpoints.contains_key(&routing::MICROPHONE)
                && self.endpoints.contains_key(&routing::HEADPHONES),
            "Marie audio controls unavailable: {}",
            errors.join("; ")
        );
        Ok(())
    }
    fn acquire_endpoints(&mut self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            self.validate()?;
            let result = self.acquire_once();
            match result {
                Ok(()) => return Ok(()),
                Err(error) if Instant::now() >= deadline => {
                    return Err(error.context("Marie endpoint acquisition deadline"));
                }
                Err(_) => {
                    // Source 500b0 retries at one second; short cancellation
                    // checks keep teardown responsive without moving COM.
                    for _ in 0..10 {
                        self.validate()?;
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        }
    }
    fn endpoint(&self, id: u32) -> Result<&Endpoint> {
        self.endpoints
            .get(&id)
            .with_context(|| format!("Marie endpoint {id} is absent"))
    }
    fn observe(&mut self, id: u32) -> Result<EndpointObservation> {
        let observed = match self.endpoint(id).and_then(Endpoint::observe) {
            Ok(observed) => observed,
            Err(error) => {
                self.invalidate_endpoint(id);
                self.observed
                    .set_endpoint(id, routing::Reading::Failed(format!("{error:#}")));
                return Err(error);
            }
        };
        self.record_observation(id, observed)?;
        Ok(observed)
    }
    fn invalidate_endpoint(&mut self, id: u32) {
        self.mixer.invalidate_endpoint(id);
        self.observed
            .set_endpoint(id, routing::Reading::Disconnected);
        self.events.remove(&("deviceVolume".into(), id));
        self.events.remove(&("deviceMute".into(), id));
        self.events.remove(&("levelMeterPumpup".into(), 0));
    }
    fn record_observation(&mut self, id: u32, observed: EndpointObservation) -> Result<()> {
        let before = self.mixer.endpoint_observation(id);
        self.mixer.observe_endpoint(id, observed)?;
        self.observed.observe_endpoint(id, observed)?;
        for (kind, changed) in [
            (
                "deviceVolume",
                before.is_none_or(|before| before.level != observed.level),
            ),
            (
                "deviceMute",
                before.is_none_or(|before| before.muted != observed.muted),
            ),
        ] {
            if changed && id != routing::SERVICE {
                self.events.insert((kind.into(), id), json!({"event":"deviceCamyChanged", "eventType":kind,
                    "containerId":self.container, "deviceId":id, "mute":u32::from(observed.muted), "level":observed.level}));
            }
        }
        Ok(())
    }
    fn writes(&mut self, writes: Vec<Write>) -> Result<()> {
        for write in writes {
            self.validate()?;
            match write {
                Write::Dsp(bytes) => {
                    self.driver.exchange(protocol::SET_DSP, &bytes, 0)?;
                }
                Write::Control(request) => self.driver.control(&request)?,
                Write::EndpointVolume { device_id, level } => {
                    let observed = match self
                        .endpoint(device_id)
                        .and_then(|endpoint| endpoint.set_volume(level))
                    {
                        Ok(observed) => observed,
                        Err(error) => {
                            self.invalidate_endpoint(device_id);
                            self.observed.set_endpoint(
                                device_id,
                                routing::Reading::Failed(format!("{error:#}")),
                            );
                            return Err(error);
                        }
                    };
                    self.record_observation(device_id, observed)?;
                    ensure!(
                        observed.level == level,
                        "Marie volume readback differs: requested {level}, actual {}",
                        observed.level
                    );
                    if device_id == routing::MICROPHONE {
                        self.writes(self.mixer.refresh_monitor()?)?;
                    }
                }
                Write::EndpointMute { device_id, muted } => {
                    let observed = match self
                        .endpoint(device_id)
                        .and_then(|endpoint| endpoint.set_muted(muted))
                    {
                        Ok(observed) => observed,
                        Err(error) => {
                            self.invalidate_endpoint(device_id);
                            self.observed.set_endpoint(
                                device_id,
                                routing::Reading::Failed(format!("{error:#}")),
                            );
                            return Err(error);
                        }
                    };
                    self.record_observation(device_id, observed)?;
                    ensure!(observed.muted == muted, "Marie mute readback differs");
                    if device_id == routing::MICROPHONE {
                        self.writes(self.mixer.refresh_monitor()?)?;
                    }
                }
            }
        }
        Ok(())
    }
    fn meter(&mut self, enabled: bool) -> Result<()> {
        self.driver
            .exchange(protocol::SET_DSP, &protocol::meter_enable(enabled), 0)?;
        self.meter_enabled = enabled;
        self.observed.set_meters(if enabled {
            routing::Reading::Reading
        } else {
            routing::Reading::Unobserved
        });
        Ok(())
    }
    fn release_endpoints(&mut self, include_service: bool) -> Result<()> {
        let mut errors = Vec::new();
        for id in [routing::MICROPHONE, routing::HEADPHONES, routing::SERVICE] {
            if id == routing::SERVICE && !include_service {
                continue;
            }
            if let Some(endpoint) = self.endpoints.get_mut(&id) {
                match endpoint.close() {
                    Ok(()) => {
                        self.endpoints.remove(&id);
                        self.invalidate_endpoint(id);
                    }
                    Err(error) => errors.push(format!("{error:#}")),
                }
            }
        }
        ensure!(
            errors.is_empty(),
            "Marie endpoint cleanup: {}",
            errors.join("; ")
        );
        Ok(())
    }
    fn restore_controls(&mut self) -> Result<()> {
        // Only a requested transition may retain these fresh observations.
        // Spontaneous reconnects never write cached values or defaults.
        for id in [routing::MICROPHONE, routing::HEADPHONES] {
            if let Some(observed) = self.restore.get(&id).copied() {
                let writes = self.mixer.set_volume(id, observed.level)?;
                self.writes(writes)?;
                let writes = self.mixer.set_muted(id, observed.muted)?;
                self.writes(writes)?;
                self.restore.remove(&id);
            }
        }
        if self.restore_routes {
            let writes = self.mixer.restore_mode_routes()?;
            self.writes(writes)?;
            self.restore_routes = false;
        }
        Ok(())
    }
    fn reconfigure(&mut self, advanced: Option<bool>, rate: Option<u32>) -> Result<()> {
        self.meter(false)?;
        let result = (|| -> Result<()> {
            if let Some(advanced) = advanced {
                let expected = if advanced {
                    protocol::Mode::Advanced
                } else {
                    protocol::Mode::Basic
                };
                if protocol::mode(&self.driver)? == expected {
                    return Ok(());
                }
            }
            if let Some(rate) = rate {
                ensure!(
                    matches!(rate, 48_000 | 96_000),
                    "Marie sample rate differs from current source"
                );
                if protocol::sample_rate(&self.driver)? == rate {
                    return Ok(());
                }
            }
            // Source 51850 restores mic/HP after endpoint reconstruction. Only
            // fresh pre-transition observations may supply those values.
            let restore = [
                (routing::MICROPHONE, self.observe(routing::MICROPHONE)?),
                (routing::HEADPHONES, self.observe(routing::HEADPHONES)?),
            ];
            self.restore = BTreeMap::from(restore);
            self.recovery_at = Some(Instant::now() + Duration::from_secs(1));
            self.release_endpoints(false)?;
            if let Some(advanced) = advanced {
                let playback = protocol::current_stream_id(&self.driver, protocol::Flow::Playback)?;
                let capture = protocol::current_stream_id(&self.driver, protocol::Flow::Capture)?;
                self.driver.exchange(
                    protocol::SET_PROFILES,
                    &protocol::profiles(playback, capture, advanced),
                    0,
                )?;
                self.restore_routes = true;
            } else if let Some(rate) = rate {
                self.driver
                    .exchange(protocol::SET_SAMPLE_RATE, &rate.to_le_bytes(), 0)?;
            }
            // 51850 delays endpoint reconstruction by one second after the
            // transition. The worker owns the wait and cancellation checks.
            while self.recovery_at.is_some_and(|at| Instant::now() < at) {
                self.validate()?;
                thread::sleep(Duration::from_millis(100));
            }
            self.acquire_endpoints(Duration::from_secs(if advanced.is_some() {
                60
            } else {
                30
            }))?;
            if let Some(advanced) = advanced {
                let actual = protocol::mode(&self.driver)?;
                ensure!(
                    actual
                        == if advanced {
                            protocol::Mode::Advanced
                        } else {
                            protocol::Mode::Basic
                        },
                    "Marie mode readback differs"
                );
                self.mixer.observe_mode_value(actual)?;
            }
            self.restore_controls()?;
            self.recovery_at = None;
            if let Some(rate) = rate {
                ensure!(
                    protocol::sample_rate(&self.driver)? == rate,
                    "Marie sample-rate readback differs"
                );
            }
            Ok(())
        })();
        // The source wrapper resumes meter collection on success and failure.
        let resume = self.meter(true);
        match (result, resume) {
            (Err(error), Err(resume)) => {
                Err(anyhow::anyhow!("{error:#}; meter resume: {resume:#}"))
            }
            (Err(error), _) => Err(error),
            (_, result) => result,
        }
    }
    pub(super) fn call(&mut self, operation: &NativeOperation) -> Result<NativeReply> {
        let deadline = Instant::now() + operation.service_budget();
        self.deadline = Some(deadline);
        self.driver.set_deadline(Some(deadline));
        let result = self.call_inner(operation);
        self.deadline = None;
        self.driver.set_deadline(None);
        result
    }
    fn call_inner(&mut self, operation: &NativeOperation) -> Result<NativeReply> {
        self.validate()?;
        ensure!(
            self.restore.is_empty() && !self.restore_routes,
            "Marie endpoint restoration has not completed"
        );
        if !matches!(
            operation,
            NativeOperation::GetMode
                | NativeOperation::GetDeviceVolume { .. }
                | NativeOperation::GetDeviceMute { .. }
        ) {
            let actual = protocol::mode(&self.driver)?;
            self.mixer.observe_mode_value(actual)?;
        }
        let writes = match *operation {
            NativeOperation::GetMode => {
                let actual = protocol::mode(&self.driver)?;
                let value = match &actual {
                    protocol::Mode::Basic => 0,
                    protocol::Mode::Advanced => 1,
                    protocol::Mode::Unknown(value) => {
                        anyhow::bail!("Marie mode profile is unknown: {value}")
                    }
                };
                self.mixer.observe_mode_value(actual)?;
                return Ok(NativeReply::Long(value));
            }
            NativeOperation::GetDeviceVolume { device_id } => {
                return Ok(NativeReply::Long(self.observe(device_id)?.level as i32));
            }
            NativeOperation::GetDeviceMute { device_id } => {
                return Ok(NativeReply::Boolean(self.observe(device_id)?.muted));
            }
            NativeOperation::SetDeviceVolume { device_id, level } => {
                self.mixer.set_volume(device_id, level)?
            }
            NativeOperation::SetDeviceMute { device_id, muted } => {
                self.mixer.set_muted(device_id, muted)?
            }
            NativeOperation::SetMixLevel {
                output_id,
                input_id,
                level,
            } => self.mixer.set_route_level(output_id, input_id, level)?,
            NativeOperation::SetMixEnable {
                output_id,
                input_id,
                enabled,
            } => self.mixer.set_route_enabled(output_id, input_id, enabled)?,
            NativeOperation::SetMicMonitorLevel { level } => {
                self.mixer.set_monitor_level(u32::try_from(level)?)?
            }
            NativeOperation::SetMicMonitorEnable { enabled } => {
                self.mixer.set_monitor_enabled(enabled)
            }
            NativeOperation::SetPlaybackExternal { enabled } => {
                self.mixer.set_playback_external(enabled)?
            }
            NativeOperation::SetStreamMixMonitor { enabled } => {
                self.mixer.set_stream_monitor(enabled)?
            }
            NativeOperation::SetLevelMeterEnable { enabled } => {
                self.meter(enabled)?;
                Vec::new()
            }
            NativeOperation::SetMasterSampleRate { rate } => {
                self.reconfigure(None, Some(rate))?;
                Vec::new()
            }
            NativeOperation::SetMode { enabled } => {
                self.reconfigure(Some(enabled), None)?;
                Vec::new()
            }
            NativeOperation::DeviceInitialize | NativeOperation::DeviceTerminate => {
                anyhow::bail!("Marie lifecycle belongs to the session")
            }
        };
        self.writes(writes)?;
        Ok(NativeReply::Boolean(true))
    }
    pub(super) fn poll(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(20);
        self.deadline = Some(deadline);
        self.driver.set_deadline(Some(deadline));
        let result = self.poll_inner();
        self.deadline = None;
        self.driver.set_deadline(None);
        self.poll_error = result.err().map(|error| format!("{error:#}"));
        if let Some(error) = &self.poll_error {
            self.events.remove(&("levelMeterPumpup".into(), 0));
            self.observed
                .set_meters(routing::Reading::Failed(error.clone()));
        }
    }
    fn poll_inner(&mut self) -> Result<()> {
        self.validate()?;
        let changed = !self.notifications.drain()?.is_empty();
        let refresh = changed || self.last_endpoint_read.elapsed() >= Duration::from_secs(1);
        if refresh {
            self.mixer
                .observe_mode_value(protocol::mode(&self.driver)?)?;
            for id in [routing::MICROPHONE, routing::HEADPHONES, routing::SERVICE] {
                if let Some(endpoint) = self.endpoints.get_mut(&id) {
                    if endpoint.current().is_err() {
                        endpoint.close()?;
                        self.endpoints.remove(&id);
                        self.invalidate_endpoint(id);
                        self.recovery_at
                            .get_or_insert_with(|| Instant::now() + Duration::from_secs(1));
                    }
                }
            }
        }
        if self.recovery_at.is_some_and(|at| Instant::now() >= at) {
            // One acquisition attempt per source retry interval keeps events
            // and cancellation responsive during a disconnected endpoint.
            self.recovery_at = Some(Instant::now() + Duration::from_secs(1));
            self.acquire_once()?;
            let actual = protocol::mode(&self.driver)?;
            self.mixer.observe_mode_value(actual)?;
            self.restore_controls()?;
            self.recovery_at = None;
        }
        for id in [routing::MICROPHONE, routing::HEADPHONES] {
            if self.endpoint(id)?.take_dirty() || refresh {
                let before = self.mixer.endpoint_observation(id);
                let actual = self.observe(id)?;
                if id == routing::MICROPHONE && before != Some(actual) {
                    self.writes(self.mixer.refresh_monitor()?)?;
                }
            }
        }
        if refresh {
            self.last_endpoint_read = Instant::now();
        }
        if self.meter_enabled {
            let readings = match protocol::meters(&self.driver, &self.channels) {
                Ok(readings) => readings,
                Err(error) => {
                    self.events.remove(&("levelMeterPumpup".into(), 0));
                    // 4cd0 re-enables DSP meter collection after a failed
                    // pump. Preserve the failed observation independently.
                    let resume = self.meter(true);
                    return Err(match resume {
                        Ok(()) => error,
                        Err(resume) => anyhow::anyhow!("{error:#}; meter recovery: {resume:#}"),
                    });
                }
            };
            let levels = self.mixer.meter_levels(&readings)?;
            self.observed.set_meters(routing::Reading::Observed(()));
            let info = levels
                .into_iter()
                .map(|(id, levels)| json!({"deviceId":id,"levelMeter":levels}))
                .collect::<Vec<_>>();
            self.events.insert(
                ("levelMeterPumpup".into(), 0),
                json!({"event":"deviceCamyChanged", "eventType":"levelMeterPumpup",
                "containerId":self.container, "levelMeters":{"meterInfo":info}}),
            );
        }
        Ok(())
    }
    pub(super) fn drain(&mut self) -> Result<EventBatch> {
        Ok(EventBatch {
            events: std::mem::take(&mut self.events).into_values().collect(),
            diagnostics: self
                .poll_error
                .iter()
                .chain(self.endpoint_diagnostic.iter())
                .cloned()
                .collect(),
            observed: self.observed.clone(),
        })
    }
    pub(super) fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.deadline = None;
        self.driver.set_deadline(None);
        self.driver.set_cancellation(None);
        let notifications = self.notifications.shutdown();
        let endpoints = self.release_endpoints(true);
        let meters = self.meter(false);
        let errors = [notifications, endpoints, meters]
            .into_iter()
            .filter_map(Result::err)
            .map(|error| format!("{error:#}"))
            .collect::<Vec<_>>();
        ensure!(
            errors.is_empty(),
            "Marie session cleanup: {}",
            errors.join("; ")
        );
        self.closed = true;
        self.observed = routing::LiveState::disconnected();
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("[Marie driver cleanup] {error:#}");
        }
    }
}
