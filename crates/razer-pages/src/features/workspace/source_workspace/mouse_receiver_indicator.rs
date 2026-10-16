//! Receiver lifecycle and observations forwarded to the retained mouse owner.
use super::*;
use crate::features::mouse_products::{
    MouseReceiverIndicatorRequested, MouseReceiverMonitorRequested,
};
use razer_device::mouse_indicator_led::{Quality, Report};

impl SourceProductWorkspace {
    pub fn receiver_indicator_request_matches(
        &self,
        request: &MouseReceiverIndicatorRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).receiver_indicator_request_matches(request, cx))
    }
    pub fn receiver_monitor_matches(
        &self,
        request: &MouseReceiverMonitorRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).receiver_monitor_matches(request, cx))
    }
    pub fn finish_receiver_indicator(
        &mut self,
        request: &MouseReceiverIndicatorRequested,
        report: Option<Report>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_receiver_indicator(request, report, cx)
            });
        }
    }
    pub fn observe_receiver_initialization(
        &mut self,
        request: &MouseReceiverIndicatorRequested,
        configuration: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_receiver_initialization(request, configuration, window, cx)
            });
        }
    }
    pub fn observe_receiver_quality(
        &mut self,
        request: &MouseReceiverMonitorRequested,
        quality: Quality,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_receiver_quality(request, quality, cx)
            });
        }
    }
    pub fn observe_receiver_battery(
        &mut self,
        monitor: &MouseReceiverMonitorRequested,
        battery: razer_model::model::PowerStatus,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            if !body.read(cx).receiver_monitor_matches(monitor, cx) {
                return;
            }
            body.update(cx, |body, cx| {
                body.observe_receiver_battery(monitor, f64::from(battery.level), cx)
            });
            self.device.power_status = Some(battery.clone());
            self.device.dashboard.readonly_power = Some(battery);
            cx.notify();
        }
    }
    pub fn restart_receiver_initialization(
        &mut self,
        monitor: &MouseReceiverMonitorRequested,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.restart_receiver_initialization(monitor, cx)
            });
        }
    }
}
