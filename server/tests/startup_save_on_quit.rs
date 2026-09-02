//! Integration test for `[startup] save_on_quit`: the *actual* `vstimd` binary
//! must write the current scene to the last-session slot when it shuts down.
//!
//! Unlike `startup_config.rs` (which calls `SceneState::save_named_config`
//! directly), this spawns the real binary under `--null`, populates a scene
//! over ZMQ, sends SIGTERM, and checks the file the boot path would restore
//! from — exercising `main.rs`'s shutdown handling end to end.
//!
//! Linux-only: graceful save-on-quit relies on the SIGTERM handler installed in
//! `main.rs`, which is `#[cfg(target_os = "linux")]`.
#![cfg(target_os = "linux")]

use std::process::Command;
use std::time::{Duration, Instant};

use prost::Message;
use zeromq::{Socket, SocketRecv, SocketSend};

use vstimd::scene_config_file::{
    count_archive_configs, list_scene_config_names, parse_config_json, scene_config_path,
    SceneConfigRef, DEFAULT_PROJECT, LAST_SESSION_CONFIG, SESSION_PROJECT,
};
use vstimd::proto;
use vstimd::proto::request;

/// A unique scratch directory removed on drop (no `tempfile` dependency).
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("vstimd_save_on_quit_{tag}_{}_{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The last-session slot's on-disk path under a storage dir.
fn last_session_path(storage_dir: &std::path::Path) -> std::path::PathBuf {
    scene_config_path(
        storage_dir,
        &SceneConfigRef::parse(
            &format!("{SESSION_PROJECT}/{LAST_SESSION_CONFIG}"),
            DEFAULT_PROJECT,
        )
        .unwrap(),
    )
}

/// Spawn the real `vstimd` binary in null-render mode with the given storage dir
/// and rig-config, on a free ZMQ port. Returns the child and the port.
fn spawn_vstimd(storage_dir: &std::path::Path, rig_config: &std::path::Path) -> (std::process::Child, u16) {
    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_vstimd"))
        .args([
            "--null",
            "--no-web",
            "--rig-config",
            rig_config.to_str().unwrap(),
            "--storage-dir",
            storage_dir.to_str().unwrap(),
            "--zmq-port",
            &port.to_string(),
        ])
        .env("RUST_LOG", "warn")
        .spawn()
        .expect("spawn vstimd");
    (child, port)
}

/// Block until the ZMQ port accepts a TCP connection (server bound) or panic.
fn wait_for_port(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("vstimd ZMQ port {port} never became reachable");
}

/// Send SIGTERM (graceful shutdown, so the quit handler runs) and wait for exit.
fn terminate_and_wait(child: &mut std::process::Child) -> std::process::ExitStatus {
    let pid = child.id() as libc::pid_t;
    unsafe {
        assert_eq!(libc::kill(pid, libc::SIGTERM), 0, "kill(SIGTERM) failed");
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("vstimd did not exit within 15s of SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Create one rect over ZMQ so the saved scene is non-empty.
fn create_a_rect(port: u16) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let mut client = zeromq::ReqSocket::new();
        client
            .connect(&format!("tcp://127.0.0.1:{port}"))
            .await
            .expect("connect ZMQ");
        let req = proto::Request {
            target: Some(request::Target::System(proto::SystemTarget {})),
            body: Some(request::Body::CreateRect(proto::CreateRectRequest {
                                                     params: Some(proto::RectParams {
                                                         width_px: 200.0,
                                                         height_px: 100.0,
                                                         ..Default::default()
                                                     }),
                                                     ..Default::default()
                                                 })),
        };
        client.send(req.encode_to_vec().into()).await.unwrap();
        let msg = tokio::time::timeout(Duration::from_secs(5), client.recv())
            .await
            .expect("ZMQ recv timed out")
            .unwrap();
        let bytes = Vec::<u8>::try_from(msg).unwrap();
        let resp = proto::Response::decode(bytes.as_slice()).unwrap();
        assert_eq!(resp.code, proto::ErrorCode::Ok as i32, "create rect failed: {}", resp.error);
    });
}

#[test]
fn writes_last_session_on_quit_when_enabled() {
    let dir = TempDir::new("enabled");
    let rig = dir.path().join("rig.toml");
    std::fs::write(&rig, "[startup]\nsave_on_quit = true\n").unwrap();

    let (mut child, port) = spawn_vstimd(dir.path(), &rig);
    wait_for_port(port);
    create_a_rect(port);

    let status = terminate_and_wait(&mut child);
    assert!(status.success(), "vstimd exited with {status:?}");

    // The last-session slot must now exist and contain the rect we created.
    let saved = last_session_path(dir.path());
    assert!(saved.exists(), "expected last-session scene-config at {saved:?}");
    let json = std::fs::read_to_string(&saved).unwrap();
    let (scene, _io) = parse_config_json(&json).expect("saved config should parse");
    assert_eq!(scene.stimuli.len(), 1, "saved scene should contain the created rect");

    // …and a timestamped archive copy must also have been written.
    assert_eq!(
        count_archive_configs(dir.path()),
        1,
        "expected exactly one timestamped archive; got: {:?}",
        list_scene_config_names(dir.path(), SESSION_PROJECT).unwrap()
    );
}

#[test]
fn does_not_write_last_session_when_disabled() {
    let dir = TempDir::new("disabled");
    let rig = dir.path().join("rig.toml");
    // save_on_quit defaults to false; be explicit for clarity.
    std::fs::write(&rig, "[startup]\nsave_on_quit = false\n").unwrap();

    let (mut child, port) = spawn_vstimd(dir.path(), &rig);
    wait_for_port(port);

    let status = terminate_and_wait(&mut child);
    assert!(status.success(), "vstimd exited with {status:?}");

    let saved = last_session_path(dir.path());
    assert!(!saved.exists(), "no last-session scene-config should be written when disabled");
}
