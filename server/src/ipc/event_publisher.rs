//! The ZMQ PUB transport: broadcast what the renderer saw, and learn about nobody.
//!
//! Commands come in on the REP socket in [`super::zmq_server`]; observations go
//! out here. Two sockets because they are two different things: a request has an
//! addressee and a deadline, an observation has neither.
//!
//! # What this promises, which is very little on purpose
//!
//! Subscribing is connecting and unsubscribing is disconnecting. There is no
//! list of subscribers, no setting naming one, and no way for the server to know
//! whether any exists. A rig with nothing subscribed renders exactly the same —
//! which is what vstimd does most of the time, being always on for alignment,
//! display checks and luminance with no session running at all.
//!
//! That is the only thing a renderer can honestly promise. It cannot know
//! whether a consumer exists, or should, or is running a session, so **only a
//! consumer can tell "not yet" from "never"**, and the deadline belongs to
//! whoever is waiting. See the braemons/contracts repo, `INTERACTIONS.md` §2.
//!
//! # Why the render thread cannot touch this
//!
//! It publishes from its own thread, fed by a bounded channel, and **the render
//! side never blocks and never allocates for a subscriber**. Not approximately:
//! the topic crosses the channel as a `&'static str` and the protobuf `Event`
//! (whose `topic` is a `String`) is assembled on the publisher thread, so the
//! calling thread does an atomic increment, a clock read and a move. `try_send`
//! is synchronous and non-blocking even though the channel is `tokio`'s, which
//! is exactly why that channel was chosen; when it finds the channel full it
//! drops the event and counts it: a frame is due in 8 ms
//! and a socket is not a reason to miss it. PUB drops for a slow subscriber by
//! design and this extends the same rule inwards, so the worst a wedged
//! subscriber can cost is events, never a frame.
//!
//! Dropping silently would be the one unforgivable part, so it is not silent:
//! `Event.sequence` is assigned *before* the channel, so a drop leaves a hole a
//! subscriber can see. That is the whole reason the sequence number exists.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use prost::Message;

use crate::proto;

/// Default port for the event stream: the command port plus one.
///
/// Adjacent on purpose. A rig that has forwarded or firewalled 5555 has a
/// person who will guess 5556 correctly, and mDNS advertises both.
pub const DEFAULT_EVENT_PORT: u16 = 5556;

/// How many events may be waiting to go out before the render thread starts
/// dropping them.
///
/// Deep enough to ride out a scheduling hiccup on the publisher thread, and far
/// too shallow to hide a subscriber that has stopped reading. If this fills, the
/// answer is that something is wrong downstream — not a bigger buffer, which
/// would only move the same problem later and make the sequence gap larger when
/// it finally arrived.
const QUEUE_DEPTH: usize = 1024;

/// The topics, as a subscriber sets them with `ZMQ_SUBSCRIBE`.
///
/// Hierarchical and dot-separated, so a prefix means what it looks like:
/// `"frame."` takes both frame events, `""` takes everything.
pub mod topic {
    pub const FRAME_DROPPED: &str = "frame.dropped";
    pub const FRAME_PRESENTED: &str = "frame.presented";
    pub const VTL_EDGE: &str = "vtl.edge";
    pub const ANIMATION_STATE: &str = "animation.state";
    pub const SERVER_STARTED: &str = "server.started";
    pub const COMMAND_APPLIED: &str = "command.applied";
}

/// The render thread's handle on the event stream.
///
/// Cheap to clone and safe to hold anywhere. Every method is non-blocking and
/// none can fail in a way the caller must handle — publishing is best-effort by
/// construction, and a caller that had to decide what to do about a failed
/// publish would be a caller that had started caring about subscribers.
#[derive(Clone)]
pub struct EventPublisher {
    inner: Option<Arc<Inner>>,
}

/// What crosses the channel: the event's parts, not the encoded event.
///
/// The topic is a `&'static str` and stays one all the way to the socket, and
/// the protobuf `Event` is *built* on the publisher thread rather than by the
/// caller. Both exist so that the render thread allocates nothing at all —
/// `Event.topic` is a `String` and constructing it here would be two heap
/// allocations per event, taken on the one thread that must never make a
/// syscall it did not choose. The only call site today is a dropped frame,
/// which is precisely the moment the render thread is already late.
struct Outgoing {
    topic: &'static str,
    sequence: u64,
    monotonic_us: u64,
    frame: u64,
    payload: proto::event::Payload,
}

struct Inner {
    sender: tokio::sync::mpsc::Sender<Outgoing>,
    sequence: AtomicU64,
    dropped_events: AtomicU64,
    started: std::time::Instant,
    total_frame_drops: AtomicU64,
}

impl EventPublisher {
    /// A publisher that goes nowhere.
    ///
    /// **Not a degraded mode.** `--no-events`, a headless test, and every unit
    /// test that builds a scene use this, and the calling code is identical: the
    /// render path must not have two shapes depending on whether anybody could
    /// be listening.
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    /// Whether anything is being published at all. Not whether anybody is
    /// listening — that is unknowable and is the point.
    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Events the render thread had to drop because the publisher fell behind.
    ///
    /// Worth logging at shutdown: a non-zero count means a subscriber wedged
    /// hard enough to fill a 1024-deep queue, and the gaps are in every
    /// subscriber's sequence numbers, not just the slow one's.
    pub fn dropped_events(&self) -> u64 {
        self.inner
            .as_ref()
            .map_or(0, |i| i.dropped_events.load(Ordering::Relaxed))
    }

    /// Publish one event, stamped with both clocks.
    ///
    /// `frame` is the display's frame index, and it is a *clock* rather than a
    /// detail of any one payload: microseconds are continuous and a display is
    /// not, so "which frame" is exact where "which microsecond" carries the
    /// uncertainty of whatever measured it. It is also the join key a consumer
    /// uses to attribute an event to a trial, which is what lets this server
    /// have no trial concept at all.
    fn publish(&self, topic: &'static str, frame: u64, payload: proto::event::Payload) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        // Assigned before the queue, so an event dropped here leaves a hole a
        // subscriber can see. A sequence that only counted what got out would
        // make the loss invisible, which is the one thing PUB must not be.
        let sequence = inner.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let outgoing = Outgoing {
            topic,
            sequence,
            // `Instant::elapsed` is a vDSO read, not a syscall, and the only
            // clock the render thread is allowed to touch here.
            monotonic_us: inner.started.elapsed().as_micros() as u64,
            frame,
            payload,
        };
        if inner.sender.try_send(outgoing).is_err() {
            inner.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The GPU missed a deadline before `frame`.
    ///
    /// Keeps the running total itself so the message can carry an absolute
    /// number: a subscriber differencing `total_since_start` is correct across a
    /// gap in the sequence, and one counting events is not.
    pub fn frame_dropped(&self, frame: u64, count: u32) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        let total = inner
            .total_frame_drops
            .fetch_add(u64::from(count), Ordering::Relaxed)
            + u64::from(count);
        self.publish(
            topic::FRAME_DROPPED,
            frame,
            proto::event::Payload::FrameDropped(proto::FrameDropped {
                count,
                total_since_start: total,
            }),
        );
    }

    /// A frame reached the screen.
    pub fn frame_presented(&self, frame: u64, since_previous_us: u32) {
        self.publish(
            topic::FRAME_PRESENTED,
            frame,
            proto::event::Payload::FramePresented(proto::FramePresented { since_previous_us }),
        );
    }

    /// A virtual trigger line changed, as seen at frame start.
    ///
    /// The record of something that already happened on the fast bus. Nothing
    /// may wait on this: the TTL edge carries the timing, this carries the
    /// account of it.
    pub fn vtl_line_changed(
        &self,
        line: u32,
        edge: proto::VtlEdge,
        kind: proto::VirtualTriggerLineKind,
        frame: u64,
    ) {
        self.publish(
            topic::VTL_EDGE,
            frame,
            proto::event::Payload::VtlLineChanged(proto::VtlLineChanged {
                line,
                edge: edge as i32,
                kind: kind as i32,
            }),
        );
    }

    /// An armed animation changed state.
    pub fn animation_state(
        &self,
        handle: u64,
        state: proto::animation_state_changed::State,
        frame: u64,
    ) {
        self.publish(
            topic::ANIMATION_STATE,
            frame,
            proto::event::Payload::AnimationStateChanged(proto::AnimationStateChanged {
                handle,
                state: state as i32,
            }),
        );
    }

    /// A command was dispatched against the scene at `frame`.
    ///
    /// **Not published from the render thread**, unlike everything else here:
    /// this is called from the ZMQ or web thread while it holds the scene write
    /// lock, which is exactly the thread that already owns the allocation of
    /// decoding the request. Encoding it again to record it costs that thread,
    /// never the frame clock — and the caller checks [`Self::is_enabled`] first
    /// so a server with no event stream does not pay for the encode at all.
    ///
    /// `frame` is the first frame that will render with this applied; see
    /// `SceneRuntimeState::next_render_frame` for why only the applying thread
    /// can know it.
    pub fn command_applied(
        &self,
        frame: u64,
        request: Vec<u8>,
        accepted: bool,
        response_handle: i32,
        error_code: i32,
    ) {
        self.publish(
            topic::COMMAND_APPLIED,
            frame,
            proto::event::Payload::CommandApplied(proto::CommandApplied {
                request,
                accepted,
                response_handle,
                error_code,
            }),
        );
    }

    fn server_started(&self, instance_id: String, version: String) {
        // Frame 0: nothing has been presented yet, and this is the event that
        // tells a subscriber the frame axis has restarted anyway.
        self.publish(
            topic::SERVER_STARTED,
            0,
            proto::event::Payload::ServerStarted(proto::ServerStarted {
                instance_id,
                version,
            }),
        );
    }
}

/// Bind a PUB socket and start publishing.
///
/// Returns the handle the render thread holds, the thread, and a shutdown
/// sender — drop it to stop the loop cleanly.
///
/// # Bind address
///
/// Same rule as the REP socket: a concrete IP, not a wildcard hostname. The
/// `zeromq` crate resolves the host part as DNS, so `tcp://*:5556` fails with a
/// lookup error. Use `tcp://0.0.0.0:5556`.
pub fn spawn_event_publisher(
    bind_addr: &str,
    instance_id: String,
    version: String,
) -> (
    EventPublisher,
    std::thread::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (sender, receiver) = tokio::sync::mpsc::channel(QUEUE_DEPTH);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let addr = bind_addr.to_owned();

    let publisher = EventPublisher {
        inner: Some(Arc::new(Inner {
            sender,
            sequence: AtomicU64::new(0),
            dropped_events: AtomicU64::new(0),
            started: std::time::Instant::now(),
            total_frame_drops: AtomicU64::new(0),
        })),
    };

    let handle = std::thread::Builder::new()
        .name("zmq-events".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for the event thread");
            rt.block_on(publish_loop(&addr, receiver, shutdown_rx));
        })
        .expect("failed to spawn the event publisher thread");

    // First message on the socket, so a subscriber learns which run it is
    // attached to before anything else. It goes through the same queue as
    // everything else and is therefore sequence 1.
    publisher.server_started(instance_id, version);

    (publisher, handle, shutdown_tx)
}

async fn publish_loop(
    addr: &str,
    mut receiver: tokio::sync::mpsc::Receiver<Outgoing>,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) {
    use zeromq::{Socket, SocketSend};

    let mut socket = zeromq::PubSocket::new();
    if let Err(e) = socket.bind(addr).await {
        // Not fatal, unlike the command socket. A rig whose renderer refused to
        // start because nothing could listen to its diagnostics would be a rig
        // that stopped for the least important reason it has.
        log::error!("vstimd: event stream bind to {addr} failed, publishing nothing: {e}");
        return;
    }
    log::info!("vstimd: ZMQ PUB event stream on {addr}");

    loop {
        // **An async channel, not a blocking one, and this is not a style
        // choice.** The runtime here is current-thread, and `zeromq` drives its
        // peer handling as tasks on it -- so a synchronous `recv_timeout` would
        // park the only thread the socket has, and nothing would ever be
        // delivered to a subscriber. It compiles, it looks right, and the
        // stream is silent.
        let next = tokio::select! {
            biased;
            _ = &mut shutdown => {
                log::info!("vstimd: event stream shutting down");
                return;
            }
            next = receiver.recv() => next,
        };
        // Every sender is gone: the render loop has finished.
        let Some(out) = next else { return };

        // Built here, off the render thread: this is where the `String` and the
        // encode buffer are allowed to be allocated.
        let event = proto::Event {
            sequence: out.sequence,
            monotonic_us: out.monotonic_us,
            frame: out.frame,
            topic: out.topic.to_owned(),
            payload: Some(out.payload),
        };

        // Two frames: the topic is what SUB filters on without decoding.
        let mut message = zeromq::ZmqMessage::from(out.topic.as_bytes().to_vec());
        message.push_back(event.encode_to_vec().into());
        if let Err(e) = socket.send(message).await {
            log::warn!("vstimd: event publish failed: {e}");
        }
    }
}
