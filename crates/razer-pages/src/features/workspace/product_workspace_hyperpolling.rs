use super::*;
use crate::features::HyperpollingIndicatorRequested;

impl ProductWorkspace {
    pub fn begin_hyperpolling_connection(&mut self, epoch: Option<u64>, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.begin_hyperpolling_connection(epoch, cx));
        }
    }
    pub fn hyperpolling_request_matches(
        &self,
        request: &HyperpollingIndicatorRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, Body::Source(body) if body.read(cx).hyperpolling_request_matches(request, cx))
    }
    pub fn initialize_hyperpolling_indicator(
        &mut self,
        epoch: u64,
        status: u8,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.initialize_hyperpolling_indicator(epoch, status, cx)
            });
        }
    }
    pub fn publish_hyperpolling_indicator(
        &mut self,
        request: &HyperpollingIndicatorRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.publish_hyperpolling_indicator(request, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_hyperpolling_identity(&mut self, serial: &str, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_hyperpolling_identity(serial, cx)
            });
        }
    }
    pub fn finish_hyperpolling_indicator(
        &mut self,
        request: &HyperpollingIndicatorRequested,
        diagnostic: Option<razer_device::hyperpolling_receiver::Submission>,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_hyperpolling_indicator(request, diagnostic, cx)
            });
        }
    }
}
