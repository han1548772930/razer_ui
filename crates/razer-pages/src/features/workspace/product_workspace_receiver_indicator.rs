//! Shell-facing receiver requests retain the source mouse owner and scope.
use super::*;
use serde_json::Value;
use crate::features::mouse_products::{MouseReceiverIndicatorRequested, MouseReceiverMonitorRequested};
use razer_device::mouse_indicator_led::{Quality, Report};

impl ProductWorkspace {
    pub fn receiver_indicator_request_matches(&self, request: &MouseReceiverIndicatorRequested, cx: &App) -> bool {
        matches!(&self.body, Body::Source(body) if body.read(cx).receiver_indicator_request_matches(request, cx))
    }
    pub fn receiver_monitor_matches(&self, request: &MouseReceiverMonitorRequested, cx: &App) -> bool {
        matches!(&self.body, Body::Source(body) if body.read(cx).receiver_monitor_matches(request, cx))
    }
    pub fn finish_receiver_indicator(&mut self, request: &MouseReceiverIndicatorRequested, report: Option<Report>, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.finish_receiver_indicator(request, report, cx));
        }
    }
    pub fn observe_receiver_initialization(&mut self, request: &MouseReceiverIndicatorRequested, configuration: Value, window: &mut Window, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.observe_receiver_initialization(request, configuration, window, cx));
        }
    }
    pub fn observe_receiver_quality(&mut self, request: &MouseReceiverMonitorRequested, quality: Quality, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.observe_receiver_quality(request, quality, cx));
        }
    }
    pub fn observe_receiver_battery(&mut self, monitor: &MouseReceiverMonitorRequested, battery: razer_model::model::PowerStatus, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.observe_receiver_battery(monitor, battery, cx));
        }
    }
    pub fn restart_receiver_initialization(&mut self, monitor: &MouseReceiverMonitorRequested, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.restart_receiver_initialization(monitor, cx));
        }
    }
}
