use super::*;
use zeromq::{RepSocket, XPubSocket};

struct SocketFiles(Vec<PathBuf>);
impl Drop for SocketFiles {
    fn drop(&mut self) {
        for file in &self.0 {
            let _ = std::fs::remove_file(file);
        }
    }
}
fn header(id: &str) -> Option<StateHeader> {
    Some(StateHeader {
        id: id.to_owned(),
        revision: 1,
    })
}
fn initial_states() -> Vec<Payload> {
    let mut wave = vec![0; 40];
    wave[2] = 2;
    let endpoint = |id: &str, role| EndpointProperties {
        endpoint_id: id.to_owned(),
        friendly_name: id.to_owned(),
        form_factor: 1,
        wave_format_extensible: wave.clone(),
        thx_endpoint_id: role,
    };
    vec![
        Payload::WasapiManagerState(WasapiManagerState {
            state_header: header("GLOBAL"),
            active_streams: vec![],
        }),
        Payload::StreamStates(StreamStates {
            state_header: header("GLOBAL"),
            endpoint_stream_state: vec![],
        }),
        Payload::ChannelMapsState(ChannelMapsState {
            state_header: header("GLOBAL"),
            vector_clock: vec![1],
            endpoint_channel_maps: vec![routing::rear_map("rear", false, false)],
        }),
        Payload::EqState(EqState {
            state_header: header("GLOBAL"),
            endpoint_eqs: vec![],
        }),
        Payload::DelayState(DelayState {
            state_header: header("GLOBAL"),
            endpoint_delays: vec![],
        }),
        Payload::EndpointVolumeState(EndpointVolumeState {
            state_header: header("front"),
            muted: true,
            volume: 0.37,
        }),
        Payload::EndpointVolumeState(EndpointVolumeState {
            state_header: header("rear"),
            muted: false,
            volume: 0.64,
        }),
        Payload::EndpointsState(EndpointsState {
            state_header: header("GLOBAL"),
            endpoints_properties: vec![
                endpoint("global", 1),
                endpoint("rear", 2),
                endpoint("front", 0),
            ],
        }),
    ]
}
async fn fixture() -> (Client, RepSocket, XPubSocket, SocketFiles) {
    let stem = std::env::temp_dir().join(format!("rzs-{}", uuid::Uuid::new_v4().simple()));
    let request = stem.with_extension("req");
    let events = stem.with_extension("pub");
    let request_address = format!("ipc://{}", request.display());
    let event_address = format!("ipc://{}", events.display());
    let mut rep = RepSocket::new();
    rep.bind(&request_address).await.unwrap();
    let mut publisher = XPubSocket::new();
    publisher.bind(&event_address).await.unwrap();
    let service = tokio::spawn(async move {
        // An actual subscription notification is the barrier; no timing sleep is needed.
        let subscription = publisher.recv().await.unwrap();
        assert_eq!(subscription.get(0).unwrap().as_ref(), &[1]);
        let request = Payload::decode(&single_frame(rep.recv().await.unwrap()).unwrap()).unwrap();
        let Payload::ClientConnect(connect) = request else {
            panic!("initial request must only handshake")
        };
        assert_eq!(connect.api_version, "6");
        rep.send(
            Payload::ClientConnectAcknowledge(ClientConnectAcknowledge {
                file_version: "simulated-routing-1".to_owned(),
                api_version: "6".to_owned(),
            })
            .encode()
            .into(),
        )
        .await
        .unwrap();
        for state in initial_states() {
            publisher.send(state.encode().into()).await.unwrap();
        }
        (rep, publisher)
    });
    let client = Client::connect_at(Product::Clio, request_address, event_address)
        .await
        .unwrap();
    let (rep, publisher) = service.await.unwrap();
    (client, rep, publisher, SocketFiles(vec![request, events]))
}
async fn close(mut client: Client, rep: RepSocket, publisher: XPubSocket) {
    client.close_sockets().await.unwrap();
    assert!(rep.close().await.is_empty());
    assert!(publisher.close().await.is_empty());
}
fn run(test: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { timeout(Duration::from_secs(10), test).await.unwrap() });
}

#[test]
fn actual_req_sub_handshake_reads_service_values_without_default_writes() {
    run(async {
        let (client, rep, publisher, _files) = fixture().await;
        assert!(client.state.connected);
        assert_eq!(client.state.observations.volumes["front"].volume, 0.37);
        assert!(client.state.observations.volumes["front"].muted);
        assert_eq!(client.state.front_endpoint, None);
        close(client, rep, publisher).await;
    });
}

#[test]
fn different_commit_is_retained_and_reported_as_failure() {
    run(async {
        let (mut client, mut rep, publisher, _files) = fixture().await;
        let service = tokio::spawn(async move {
            let Payload::EndpointVolumeState(mut request) =
                Payload::decode(&single_frame(rep.recv().await.unwrap()).unwrap()).unwrap()
            else {
                panic!()
            };
            assert_eq!(request.volume, 0.2);
            assert!(request.muted);
            request.volume = 0.83;
            request.state_header.as_mut().unwrap().revision = 2;
            rep.send(Payload::EndpointVolumeState(request).encode().into())
                .await
                .unwrap();
            rep
        });
        let request = client
            .state
            .observations
            .volume_request("front", Some(0.2), None)
            .unwrap();
        assert!(client.commit(request).await.is_err());
        assert_eq!(client.state.observations.volumes["front"].volume, 0.83);
        close(client, service.await.unwrap(), publisher).await;
    });
}

#[test]
fn cancellation_retains_a_response_received_during_the_inflight_request() {
    run(async {
        let (mut client, mut rep, publisher, _files) = fixture().await;
        let cancellation = Arc::new(std::sync::atomic::AtomicBool::new(false));
        client.cancellation = Some(cancellation.clone());
        let service = tokio::spawn(async move {
            let Payload::EndpointVolumeState(mut request) =
                Payload::decode(&single_frame(rep.recv().await.unwrap()).unwrap()).unwrap()
            else {
                panic!()
            };
            request.state_header.as_mut().unwrap().revision = 2;
            cancellation.store(true, std::sync::atomic::Ordering::Release);
            rep.send(Payload::EndpointVolumeState(request).encode().into())
                .await
                .unwrap();
            rep
        });
        let request = client
            .state
            .observations
            .volume_request("front", Some(0.71), None)
            .unwrap();
        assert!(client.commit(request).await.is_err());
        assert_eq!(client.state.observations.volumes["front"].volume, 0.71);
        close(client, service.await.unwrap(), publisher).await;
    });
}

#[test]
fn empty_audition_reply_only_records_delivery() {
    run(async {
        let (mut client, mut rep, publisher, _files) = fixture().await;
        let service = tokio::spawn(async move {
            assert!(matches!(
                Payload::decode(&single_frame(rep.recv().await.unwrap()).unwrap()).unwrap(),
                Payload::PlayDelayTestSignal(_)
            ));
            rep.send(Vec::<u8>::new().into()).await.unwrap();
            rep
        });
        assert!(client.call(&NativeCall::SetTestSignal(true)).await.unwrap());
        assert_eq!(client.state.signal_delivered, Some(true));
        assert_eq!(client.state.signal_requested, Some(true));
        close(client, service.await.unwrap(), publisher).await;
    });
}

#[test]
fn stale_reply_does_not_replace_a_newer_publication() {
    run(async {
        let (mut client, mut rep, publisher, _files) = fixture().await;
        client
            .state
            .observations
            .volumes
            .get_mut("front")
            .unwrap()
            .state_header
            .as_mut()
            .unwrap()
            .revision = 10;
        let service = tokio::spawn(async move {
            let Payload::EndpointVolumeState(mut request) =
                Payload::decode(&single_frame(rep.recv().await.unwrap()).unwrap()).unwrap()
            else {
                panic!()
            };
            request.state_header.as_mut().unwrap().revision = 9;
            rep.send(Payload::EndpointVolumeState(request).encode().into())
                .await
                .unwrap();
            rep
        });
        let request = client
            .state
            .observations
            .volume_request("front", Some(0.2), None)
            .unwrap();
        assert!(client.commit(request).await.is_err());
        assert_eq!(client.state.observations.volumes["front"].volume, 0.37);
        close(client, service.await.unwrap(), publisher).await;
    });
}

#[test]
fn cleanup_sends_stop_then_global_rear_front_flat_filters_in_order() {
    run(async {
        let (mut client, mut rep, publisher, _files) = fixture().await;
        client.state.front_endpoint = Some("front".to_owned());
        let service = tokio::spawn(async move {
            assert!(matches!(
                Payload::decode(&single_frame(rep.recv().await.unwrap()).unwrap()).unwrap(),
                Payload::StopDelayTestSignal(_)
            ));
            rep.send(Vec::<u8>::new().into()).await.unwrap();
            for (endpoint, count) in [("global", 10), ("rear", 3), ("front", 3)] {
                for index in 0..count {
                    let Payload::EqState(mut request) =
                        Payload::decode(&single_frame(rep.recv().await.unwrap()).unwrap()).unwrap()
                    else {
                        panic!()
                    };
                    let target = request
                        .endpoint_eqs
                        .iter()
                        .find(|e| e.endpoint_id == endpoint)
                        .unwrap();
                    let band = target.eq_bands.iter().find(|b| b.index == index).unwrap();
                    assert_eq!(band.gain_db, 0.0);
                    request.state_header.as_mut().unwrap().revision += 1;
                    rep.send(Payload::EqState(request).encode().into())
                        .await
                        .unwrap();
                }
            }
            rep
        });
        client.restore().await.unwrap();
        assert_eq!(
            client
                .state
                .observations
                .equalizer
                .as_ref()
                .unwrap()
                .endpoint_eqs
                .len(),
            3
        );
        close(client, service.await.unwrap(), publisher).await;
    });
}
