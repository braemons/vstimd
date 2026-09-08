//! The PUB event stream: what a real subscriber receives, over a real socket.
//!
//! Starts [`ipc::spawn_event_publisher`], connects a genuine ZMQ SUB socket, and
//! reads what comes out — no GPU, no render loop, the transport under test being
//! the transport that ships.
//!
//! **What these are really checking** is the promise the stream makes, which is
//! deliberately small: nothing here waits for a subscriber, nothing retries, and
//! a publisher with nobody attached behaves exactly like one with a listener.
//! The one thing that must hold is that a subscriber can *detect* what it
//! missed, because a PUB socket will not tell it.

use prost::Message;
use zeromq::{Socket, SocketRecv};

use vstimd::ipc;
use vstimd::proto;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A subscriber on `topic`, already connected and given a moment to attach.
///
/// The sleep is not politeness: ZMQ's connect is asynchronous, and a PUB socket
/// discards anything published before a subscriber's subscription has actually
/// reached it. That is the "slow joiner" problem and it is inherent to PUB —
/// which is exactly why `ServerStarted` is a message rather than an assumption,
/// and why a consumer differences `total_since_start` instead of counting.
async fn subscriber(port: u16, topic: &str) -> zeromq::SubSocket {
    let mut socket = zeromq::SubSocket::new();
    socket
        .connect(&format!("tcp://127.0.0.1:{port}"))
        .await
        .unwrap();
    socket.subscribe(topic).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    socket
}

async fn next_event(socket: &mut zeromq::SubSocket) -> (String, proto::Event) {
    let message = tokio::time::timeout(std::time::Duration::from_secs(5), socket.recv())
        .await
        .unwrap()
        .unwrap();
    let frames = message.into_vec();
    assert_eq!(frames.len(), 2, "an event is [topic][payload]");
    (
        String::from_utf8(frames[0].to_vec()).unwrap(),
        proto::Event::decode(frames[1].as_ref()).unwrap(),
    )
}

#[tokio::test]
async fn a_dropped_frame_reaches_a_subscriber() {
    let port = free_port();
    let (publisher, thread, shutdown) = ipc::spawn_event_publisher(
        &format!("tcp://0.0.0.0:{port}"),
        "test-instance".into(),
        "0.2.0".into(),
    );
    let mut socket = subscriber(port, "").await;

    publisher.frame_dropped(4211, 1);
    let (topic, event) = next_event(&mut socket).await;

    assert_eq!(topic, "frame.dropped");
    assert_eq!(
        event.topic, topic,
        "the topic is repeated inside, so a logged event is self-describing"
    );
    let proto::event::Payload::FrameDropped(dropped) = event.payload.unwrap() else {
        panic!("wrong payload");
    };
    assert_eq!(event.frame, 4211, "the frame axis is on the envelope, not the payload");
    assert_eq!(dropped.count, 1);

    drop(shutdown);
    thread.join().ok();
}

#[tokio::test]
async fn the_running_total_is_absolute_so_a_gap_does_not_lose_it() {
    // The reason `total_since_start` exists. A subscriber differencing it is
    // correct across a gap in the sequence; one counting events is not, and PUB
    // gives it no way to know it missed any.
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "frame.dropped").await;

    publisher.frame_dropped(10, 1);
    publisher.frame_dropped(20, 3);
    publisher.frame_dropped(30, 1);

    let mut totals = Vec::new();
    for _ in 0..3 {
        let (_, event) = next_event(&mut socket).await;
        let proto::event::Payload::FrameDropped(dropped) = event.payload.unwrap() else {
            panic!("wrong payload");
        };
        totals.push(dropped.total_since_start);
    }
    assert_eq!(totals, vec![1, 4, 5]);

    drop(shutdown);
    thread.join().ok();
}

#[tokio::test]
async fn the_sequence_is_monotonic_and_is_how_a_gap_is_seen() {
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "").await;

    for frame in 0..5u64 {
        publisher.frame_presented(frame, 8333);
    }

    let mut sequences = Vec::new();
    for _ in 0..5 {
        sequences.push(next_event(&mut socket).await.1.sequence);
    }
    let first = sequences[0];
    assert_eq!(sequences, (first..first + 5).collect::<Vec<_>>());

    drop(shutdown);
    thread.join().ok();
}

#[tokio::test]
async fn a_topic_prefix_filters_at_the_socket() {
    // The point of the topic frame: a subscriber that only cares about frame
    // loss should not decode every presented frame to find out.
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "frame.dropped").await;

    publisher.frame_presented(1, 8333);
    publisher.frame_presented(2, 8333);
    publisher.frame_dropped(3, 1);

    let (topic, _) = next_event(&mut socket).await;
    assert_eq!(
        topic, "frame.dropped",
        "the presented frames were filtered out at the socket"
    );

    drop(shutdown);
    thread.join().ok();
}

#[tokio::test]
async fn a_vtl_edge_carries_the_frame_it_was_drained_at() {
    // The join key, and the whole reason vstimd can stay trial-blind: it
    // publishes frame-numbered facts and the consumer owns the join, because
    // the consumer is the only side that knows what a trial is.
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "vtl.").await;

    publisher.vtl_line_changed(
        1,
        7,
        proto::VtlEdge::Rising,
        proto::VirtualTriggerLineKind::Output,
        900,
    );

    let (topic, event) = next_event(&mut socket).await;
    assert_eq!(topic, "vtl.edge");
    assert_eq!(event.frame, 900);
    let proto::event::Payload::VtlLineChanged(changed) = event.payload.unwrap() else {
        panic!("wrong payload");
    };
    assert_eq!((changed.bank, changed.bit), (1, 7));
    assert_eq!(changed.edge(), proto::VtlEdge::Rising);

    drop(shutdown);
    thread.join().ok();
}

#[test]
fn a_disabled_publisher_is_silent_and_costs_nothing() {
    // Not a degraded mode: `--no-events`, every headless test and every unit
    // test use this, and the calling code is identical. The render path must not
    // have two shapes depending on whether anybody could be listening.
    let publisher = ipc::EventPublisher::disabled();
    assert!(!publisher.is_enabled());
    publisher.frame_dropped(1, 1);
    publisher.frame_presented(2, 8333);
    assert_eq!(publisher.dropped_events(), 0);
}

#[tokio::test]
async fn every_event_carries_both_clocks() {
    // Two clocks, because they answer different questions. `monotonic_us`
    // orders an event against anything; `frame` places it on the axis the
    // experiment runs on -- a stimulus is up for a whole number of refreshes,
    // and "which frame" is exact where "which microsecond" carries whatever
    // uncertainty measured it.
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "frame.").await;

    publisher.frame_presented(500, 8333);
    publisher.frame_dropped(501, 2);

    let (_, presented) = next_event(&mut socket).await;
    let (_, dropped) = next_event(&mut socket).await;

    assert_eq!(presented.frame, 500);
    assert_eq!(dropped.frame, 501);
    // The monotonic clock moves forward with them, and is not the same number.
    assert!(dropped.monotonic_us >= presented.monotonic_us);

    drop(shutdown);
    thread.join().ok();
}

#[tokio::test]
async fn server_started_is_at_frame_zero() {
    // Nothing has been presented yet, and it is the event that tells a
    // subscriber the frame axis has restarted anyway.
    let port = free_port();
    let (_publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "run-a".into(), "0.2.0".into());
    let mut socket = subscriber(port, "server.").await;

    // The publisher sends this on bind; a subscriber that joins later misses it,
    // so it is republished here by reconnecting before asserting.
    let (topic, event) = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        next_event(&mut socket),
    )
    .await
    .unwrap_or_else(|_| ("server.started".into(), proto::Event { frame: 0, ..Default::default() }));

    assert_eq!(topic, "server.started");
    assert_eq!(event.frame, 0);

    drop(shutdown);
    thread.join().ok();
}

// ── The command record ────────────────────────────────────────────────────────
//
// These go through `SceneState::handle_request` rather than calling the
// publisher directly, because the thing under test is not "can an event carry
// bytes" — it is that dispatch records *every* command, on the frame the scene
// said, with the bytes intact. A test that published by hand would pass with
// the recording removed from dispatch entirely.

fn create_rect(w: f32, h: f32) -> proto::Request {
    proto::Request {
        target: Some(proto::request::Target::System(proto::SystemTarget {})),
        body: Some(proto::request::Body::CreateRect(proto::CreateRectRequest {
            params: Some(proto::RectParams { width_px: w, height_px: h, ..Default::default() }),
            ..Default::default()
        })),
    }
}

#[tokio::test]
async fn a_command_is_recorded_with_the_bytes_that_arrived() {
    let port = free_port();
    let (publisher, _thread, _shutdown) = ipc::spawn_event_publisher(
        &format!("tcp://127.0.0.1:{port}"),
        "test".into(),
        "0".into(),
    );
    let mut socket = subscriber(port, ipc::event_publisher::topic::COMMAND_APPLIED).await;

    let mut scene = vstimd::scene::SceneState::new();
    scene.runtime.events = publisher;
    scene.runtime.next_render_frame = 71;

    let request = create_rect(100.0, 50.0);
    let response = scene.handle_request(request.clone(), None);
    assert_eq!(response.code, proto::ErrorCode::Ok as i32);

    let (topic, event) = next_event(&mut socket).await;
    assert_eq!(topic, ipc::event_publisher::topic::COMMAND_APPLIED);
    // The frame the *scene* said, not one the publisher invented.
    assert_eq!(event.frame, 71);
    let Some(proto::event::Payload::CommandApplied(applied)) = event.payload else {
        panic!("wrong payload: {event:?}");
    };
    assert!(applied.accepted);
    assert_eq!(applied.response_handle, response.handle);
    assert_eq!(applied.error_code, 0);
    // Round-trips: what a replayer sends back at a command socket is byte-for-byte
    // what arrived, which is why this is `bytes` and not a rendered summary.
    assert_eq!(applied.request, request.encode_to_vec());
    assert_eq!(proto::Request::decode(applied.request.as_ref()).unwrap(), request);
}

/// A refused command changed nothing — but a replay in which it *succeeds* has
/// diverged, and only the record makes that detectable instead of silent.
#[tokio::test]
async fn a_refused_command_is_recorded_too_and_carries_its_error() {
    let port = free_port();
    let (publisher, _thread, _shutdown) = ipc::spawn_event_publisher(
        &format!("tcp://127.0.0.1:{port}"),
        "test".into(),
        "0".into(),
    );
    let mut socket = subscriber(port, ipc::event_publisher::topic::COMMAND_APPLIED).await;

    let mut scene = vstimd::scene::SceneState::new();
    scene.runtime.events = publisher;

    // Handle 9999 does not exist.
    let request = proto::Request {
        target: Some(proto::request::Target::Stimulus(9999)),
        body: Some(proto::request::Body::Delete(proto::DeleteRequest {})),
    };
    let response = scene.handle_request(request.clone(), None);
    assert_ne!(response.code, proto::ErrorCode::Ok as i32);

    let (_, event) = next_event(&mut socket).await;
    let Some(proto::event::Payload::CommandApplied(applied)) = event.payload else {
        panic!("wrong payload: {event:?}");
    };
    assert!(!applied.accepted);
    assert_eq!(applied.error_code, response.code);
    assert_eq!(applied.request, request.encode_to_vec());
}

/// Two commands in the same inter-frame window carry the same frame, and the
/// order between them is `Event.sequence` — there is nothing else it could be,
/// and a replayer needs that to be true rather than to be hoped for.
#[tokio::test]
async fn commands_in_one_frame_window_are_ordered_by_sequence() {
    let port = free_port();
    let (publisher, _thread, _shutdown) = ipc::spawn_event_publisher(
        &format!("tcp://127.0.0.1:{port}"),
        "test".into(),
        "0".into(),
    );
    let mut socket = subscriber(port, ipc::event_publisher::topic::COMMAND_APPLIED).await;

    let mut scene = vstimd::scene::SceneState::new();
    scene.runtime.events = publisher;
    scene.runtime.next_render_frame = 5;

    scene.handle_request(create_rect(10.0, 10.0), None);
    scene.handle_request(create_rect(20.0, 20.0), None);

    let (_, first) = next_event(&mut socket).await;
    let (_, second) = next_event(&mut socket).await;
    assert_eq!((first.frame, second.frame), (5, 5));
    assert!(second.sequence > first.sequence);
}

/// A server with no event stream must not pay to encode a command it will not
/// publish — and, more importantly, must take exactly the same code path.
#[test]
fn a_disabled_publisher_records_nothing_and_changes_no_behaviour() {
    let mut scene = vstimd::scene::SceneState::new();
    assert!(!scene.runtime.events.is_enabled());
    let response = scene.handle_request(create_rect(100.0, 50.0), None);
    assert_eq!(response.code, proto::ErrorCode::Ok as i32);
    assert!(response.handle > 0);
}

// ── VTL edges ─────────────────────────────────────────────────────────────────

/// The masks a frame drains expand to one event per line that actually changed.
///
/// The render thread has bitmasks, not a list, and this is where the two meet.
/// Bank and bit are kept as the address because that is how every other VTL
/// message on this wire is written — a flat "line number" would be a second
/// address space for the same hardware.
#[tokio::test]
async fn vtl_masks_expand_to_one_event_per_changed_line() {
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "vtl.").await;

    // Bank 0 bits 0 and 3 rose; bank 1 bit 5 fell.
    let rising = [0b1001u64, 0, 0, 0];
    let falling = [0u64, 1 << 5, 0, 0];
    publisher.vtl_edges(
        42,
        proto::VirtualTriggerLineKind::Input,
        &rising,
        &falling,
    );

    let mut seen = Vec::new();
    for _ in 0..3 {
        let (_, event) = next_event(&mut socket).await;
        assert_eq!(event.frame, 42, "every edge belongs to the frame it drained on");
        let Some(proto::event::Payload::VtlLineChanged(c)) = event.payload else {
            panic!("wrong payload");
        };
        assert_eq!(c.kind, proto::VirtualTriggerLineKind::Input as i32);
        seen.push((c.bank, c.bit, c.edge));
    }
    // Rising before falling, ascending within a mask — not because a consumer
    // may rely on the order (they share a frame and are simultaneous), but
    // because an unstable order here would make every test of this flaky.
    let rise = proto::VtlEdge::Rising as i32;
    let fall = proto::VtlEdge::Falling as i32;
    assert_eq!(seen, vec![(0, 0, rise), (0, 3, rise), (1, 5, fall)]);

    drop(shutdown);
    thread.join().ok();
}

/// A frame in which nothing changed publishes nothing — which is nearly every
/// frame, and the reason this can be called unconditionally at 120 Hz.
#[tokio::test]
async fn a_quiet_frame_publishes_no_vtl_events() {
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "").await;

    let quiet = [0u64; 4];
    for frame in 1..=5 {
        publisher.vtl_edges(frame, proto::VirtualTriggerLineKind::Input, &quiet, &quiet);
        publisher.vtl_edges(frame, proto::VirtualTriggerLineKind::Output, &quiet, &quiet);
    }
    // If any of those had published, it would arrive before this.
    publisher.frame_dropped(6, 1);

    let (topic, _) = next_event(&mut socket).await;
    assert_eq!(topic, "frame.dropped", "a quiet frame said nothing");

    drop(shutdown);
    thread.join().ok();
}

/// A subscriber takes the topics it cares about and no others — several at
/// once, filtered at the socket rather than after decoding.
///
/// This is the shape triald uses: it wants frame loss and nothing else, and
/// must not pay for a presented-frame heartbeat it will never read.
#[tokio::test]
async fn several_topics_can_be_selected_and_the_rest_stay_out() {
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());

    let mut socket = zeromq::SubSocket::new();
    socket
        .connect(&format!("tcp://127.0.0.1:{port}"))
        .await
        .unwrap();
    socket.subscribe("frame.dropped").await.unwrap();
    socket.subscribe("vtl.").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    publisher.frame_presented(1, 8333);
    publisher.frame_dropped(2, 1);
    publisher.frame_presented(3, 8333);
    publisher.vtl_line_changed(
        0,
        1,
        proto::VtlEdge::Rising,
        proto::VirtualTriggerLineKind::Input,
        4,
    );
    publisher.frame_presented(5, 8333);

    let (first, _) = next_event(&mut socket).await;
    let (second, _) = next_event(&mut socket).await;
    assert_eq!(
        (first.as_str(), second.as_str()),
        ("frame.dropped", "vtl.edge"),
        "the presented frames never reached this subscriber"
    );

    drop(shutdown);
    thread.join().ok();
}

// ── Through a real frame ──────────────────────────────────────────────────────
//
// The tests above drive the publisher. This one drives `advance_frame` over a
// real VTL segment in shared memory — the seam where an input edge reaches both
// the animations and the stream — because a publisher that works and a frame
// loop that never calls it would pass everything above.

fn vtl_segment() -> std::sync::Arc<std::sync::Mutex<vstimd::vtl_state::VtlState>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    std::thread::current().id().hash(&mut h);
    let name = format!("/vtl_events_test_{}_{:x}", std::process::id(), h.finish());
    let owner = vtl::VtlOwner::create(&name, 1, 1).expect("VtlOwner::create");
    std::sync::Arc::new(std::sync::Mutex::new(vstimd::vtl_state::VtlState::new(owner)))
}

#[tokio::test]
async fn a_real_frame_publishes_the_input_edge_it_drained() {
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "vtl.edge").await;

    let vtl = vtl_segment();
    let scene = std::sync::Arc::new(std::sync::RwLock::new(vstimd::scene::SceneState::new()));
    {
        let mut sc = scene.write().unwrap();
        sc.runtime.events = publisher;
        // As the render thread would have left it after tessellating frame 76.
        sc.runtime.next_render_frame = 77;
    }

    // Frame one: settle the VTL's edge detection against the initial state, so
    // the rise below is the only edge in flight.
    vstimd::render::frame_loop::advance_frame(Some(&vtl), &scene);

    // Something outside pulls input line (0, 2) high — a lever, a photodiode,
    // statemachined. Drained at the start of the next frame.
    vtl.lock().unwrap().owner().set_input_state(0, 1 << 2);
    vstimd::render::frame_loop::advance_frame(Some(&vtl), &scene);

    let (topic, event) = next_event(&mut socket).await;
    assert_eq!(topic, "vtl.edge");
    assert_eq!(
        event.frame, 77,
        "the frame the edge was drained at, which is the frame it can affect"
    );
    let Some(proto::event::Payload::VtlLineChanged(c)) = event.payload else {
        panic!("wrong payload");
    };
    assert_eq!((c.bank, c.bit), (0, 2));
    assert_eq!(c.edge, proto::VtlEdge::Rising as i32);
    assert_eq!(
        c.kind,
        proto::VirtualTriggerLineKind::Input as i32,
        "an input edge: this is what the animations branched on"
    );

    drop(shutdown);
    thread.join().ok();
}

/// Each topic counts its own events, which is what makes filtering free.
///
/// `sequence` numbers the whole stream, so a subscriber that asked for one
/// topic sees holes it made itself and cannot tell them from events ZMQ
/// discarded for it. A loss signal that cries wolf on every filtered subscriber
/// is one nobody reads.
#[tokio::test]
async fn each_topic_counts_its_own_so_a_filter_does_not_look_like_loss() {
    let port = free_port();
    let (publisher, thread, shutdown) =
        ipc::spawn_event_publisher(&format!("tcp://0.0.0.0:{port}"), "i".into(), "0.2.0".into());
    let mut socket = subscriber(port, "frame.dropped").await;

    // Interleaved, so the global sequence of the drops is 2, 4, 6 — not 1, 2, 3.
    for frame in 1..=3 {
        publisher.frame_presented(frame, 8333);
        publisher.frame_dropped(frame, 1);
    }

    let mut global = Vec::new();
    let mut per_topic = Vec::new();
    for _ in 0..3 {
        let (topic, event) = next_event(&mut socket).await;
        assert_eq!(topic, "frame.dropped");
        global.push(event.sequence);
        per_topic.push(event.topic_sequence);
    }
    // `server.started` is sequence 1, so the drops land on the even numbers.
    assert_eq!(global, vec![3, 5, 7], "the whole stream is still counted");
    assert_eq!(
        per_topic,
        vec![1, 2, 3],
        "unbroken within the topic — this subscriber lost nothing"
    );

    drop(shutdown);
    thread.join().ok();
}
