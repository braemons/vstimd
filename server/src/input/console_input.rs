//! Keyboard input for the bare-console (no compositor) backends.
//!
//! Shared by `render::drm` and `render::evdi`: both grab the physical
//! keyboard through libinput and translate evdev key codes into app-level
//! actions plus egui events. Nothing here is DRM- or evdi-specific.
//!
//! Also home to the low-level VT primitives (`open_vt`, `active_vt`,
//! `vt_number_from_env`, `activate_vt`) that the tty guard needs;
//! `drm::drm_virtual_terminal` builds its full VT/KD_GRAPHICS guard on top
//! of these rather than the other way round, so this module stays free of
//! any backend dependency.

use input::event::keyboard::KeyboardEventTrait as _;
use std::os::fd::{AsRawFd as _, OwnedFd};
use std::path::Path;

use crate::input::AppKey;
use crate::render::overlay_ui::OverlayGroup;

// ── TTY keyboard suppression guard ───────────────────────────────────────────

/// Disables echo and canonical processing on a VT for the lifetime of a
/// bare-console session. Flushes any buffered input on drop so characters
/// typed during the session don't appear once the VT returns to text mode.
///
/// Which VT to target differs per backend, hence the argument: the DRM
/// backend *activates* its own VT (tty3 by default, `VSTIMD_TTY` override)
/// and guards that one; the evdi backend activates none and must guard
/// whichever VT happens to be active, since that is where stray keystrokes
/// would land.
///
/// Deliberately opens the target VT device directly rather than `/dev/tty`
/// (the calling process's controlling terminal) — same reasoning as
/// [`crate::render::drm`]'s VT guard. Over SSH (no `DISPLAY` → DRM
/// auto-detected), `/dev/tty` is the SSH pty, not the console VT; tweaking
/// its termios would affect the SSH session itself and had previously
/// swallowed Ctrl+C there.
///
/// Uses tcsetattr rather than KDSKBMODE: the latter only works on real VT
/// console nodes and requires CAP_SYS_TTY_CONFIG; tcsetattr works on any
/// tty type (VT or pts) without elevated permissions.
struct TtyKbdGuard {
    fd: libc::c_int,
    saved: libc::termios,
}

impl TtyKbdGuard {
    fn acquire(target_vt: u16) -> Option<Self> {
        let fd = open_vt(target_vt);
        if fd < 0 {
            log::warn!("vstimd: could not open /dev/tty{target_vt} — keys may echo to terminal");
            return None;
        }
        let mut saved: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut saved) } < 0 {
            unsafe { libc::close(fd) };
            return None;
        }
        let mut raw = saved;
        // Disable echo, canonical (line-buffered) mode, and signal generation
        // (Ctrl+C/Ctrl+\/Ctrl+Z) on the console VT. Real keyboard input is
        // grabbed exclusively via libinput and never reaches this tty, so
        // ISIG here is inert — but it's the console VT's own termios, not
        // whatever terminal launched the process, so it can't interfere with
        // signal delivery over SSH.
        raw.c_lflag &= !(libc::ECHO | libc::ECHOE | libc::ECHOK | libc::ECHONL | libc::ICANON | libc::ISIG);
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } < 0 {
            log::warn!("vstimd: tcsetattr failed — keys may echo to terminal");
            unsafe { libc::close(fd) };
            return None;
        }
        Some(Self { fd, saved })
    }
}

impl Drop for TtyKbdGuard {
    fn drop(&mut self) {
        unsafe {
            // Discard any keys buffered during DRM mode before restoring.
            libc::tcflush(self.fd, libc::TCIFLUSH);
            libc::tcsetattr(self.fd, libc::TCSANOW, &self.saved);
            libc::close(self.fd);
        }
    }
}

// ── libinput interface impl ───────────────────────────────────────────────────

/// `EVIOCGRAB` — `_IOW('E', 0x90, c_int)`. Evdev-specific, not part of POSIX,
/// so `libc` doesn't expose it; encoded by hand from `<linux/input.h>`'s
/// `_IOC` layout (direction=write, size=4, type='E', nr=0x90).
const EVIOCGRAB: libc::c_ulong = 0x4004_4590;

struct Interface;

impl input::LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        use std::os::unix::fs::OpenOptionsExt as _;
        std::fs::OpenOptions::new()
            .read(true)
            .write(flags & 0b11 != 0) // O_WRONLY=1, O_RDWR=2
            .custom_flags(flags)
            .open(path)
            .map(|f| {
                let fd: OwnedFd = f.into();
                // libinput's udev/seat backend does not itself grab devices
                // exclusively (confirmed via strace: it opens and queries
                // every device but never issues EVIOCGRAB) — without this,
                // the kernel's own VT keyboard handler keeps receiving the
                // same raw keys in parallel, so they leak to whatever
                // getty/shell is on the active VT even while the overlay
                // also responds normally.
                if unsafe { libc::ioctl(fd.as_raw_fd(), EVIOCGRAB, 1) } < 0 {
                    log::warn!(
                        "vstimd: EVIOCGRAB failed on {}: {} — input may leak to the console",
                        path.display(),
                        std::io::Error::last_os_error()
                    );
                }
                fd
            })
            .map_err(|e| e.raw_os_error().unwrap_or(-1))
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(fd);
    }
}

// ── InputState ───────────────────────────────────────────────────────────────

/// Wraps a libinput context and provides a simple per-frame key event drain.
pub struct InputState {
    ctx: input::Libinput,
    modifiers: egui::Modifiers,
    #[allow(dead_code)] // held for its Drop side-effect
    tty_kbd_guard: Option<TtyKbdGuard>,
}

impl InputState {
    /// `tty_guard_vt` is the VT whose echo/canonical mode gets suppressed for
    /// the session — see [`TtyKbdGuard`]. Use [`vt_number_from_env`] when the
    /// backend activates its own VT, [`active_vt`] when it does not.
    pub fn new(tty_guard_vt: u16) -> Self {
        let tty_kbd_guard = TtyKbdGuard::acquire(tty_guard_vt);
        let mut ctx = input::Libinput::new_with_udev(Interface);
        match ctx.udev_assign_seat("seat0") {
            Ok(()) => Self {
                ctx,
                modifiers: egui::Modifiers::default(),
                tty_kbd_guard,
            },
            Err(()) => {
                log::error!(
                    "vstimd: libinput could not open seat0 — \
                     add the current user to the 'input' group and log out/in:\n  \
                     sudo usermod -aG input $USER"
                );
                std::process::exit(1);
            }
        }
    }

    /// Suspend libinput, releasing the EVIOCGRAB on all input devices so the
    /// active VT's session can receive input while vstimd is in the background.
    pub fn suspend(&mut self) {
        self.ctx.suspend();
    }

    /// Resume libinput, re-acquiring EVIOCGRAB on all input devices.
    pub fn resume(&mut self) {
        if self.ctx.resume().is_err() {
            log::warn!("vstimd: libinput resume failed");
        }
    }

    /// Drain pending events.  Returns app-level key actions and egui keyboard
    /// events for overlay navigation (Tab, arrows, Enter, Space, etc.).
    /// Non-blocking — returns immediately if there are no events.
    pub fn poll(&mut self) -> (Vec<AppKey>, Vec<egui::Event>) {
        if self.ctx.dispatch().is_err() {
            return (vec![], vec![]);
        }

        let mut app_keys = Vec::new();
        let mut egui_events = Vec::new();

        for event in self.ctx.by_ref() {
            let input::Event::Keyboard(kb) = event else {
                continue;
            };
            let pressed = kb.key_state() == input::event::keyboard::KeyState::Pressed;
            let code = kb.key();

            // Modifier tracking (press + release) — no separate egui event;
            // modifier state is embedded in subsequent key events.
            match code {
                42 | 54 => { self.modifiers.shift = pressed; continue; } // L/R SHIFT
                29 | 97 => { self.modifiers.ctrl  = pressed; continue; } // L/R CTRL
                56 | 100 => { self.modifiers.alt  = pressed; continue; } // L/R ALT
                _ => {}
            }

            // Ctrl+Q → quit. DRM mode has no window manager to send a close
            // request (unlike winit's Alt+F4/CloseRequested), so this is the
            // only in-session hotkey; SIGINT/SIGTERM still work too.
            if pressed && self.modifiers.ctrl && !self.modifiers.alt && code == 16 {
                app_keys.push(AppKey::Quit);
                continue;
            }

            // Ctrl+Alt+F1–F12 → VT switch (libinput grabs input exclusively, so the
            // kernel never sees these; we forward them ourselves). Takes priority
            // over plain-Fn group selection.
            if pressed && self.modifiers.ctrl && self.modifiers.alt {
                let vt = match code {
                    59..=68 => Some((code - 58) as u16), // F1→1 … F10→10
                    87 => Some(11),                      // F11
                    88 => Some(12),                      // F12
                    _ => None,
                };
                if let Some(n) = vt {
                    app_keys.push(AppKey::SwitchVt(n));
                    continue;
                }
            }

            // App-level keys (press only). F-keys and Esc never type, so they
            // `continue`; KEY_D falls through so 'd' can also reach text fields.
            match code {
                1 if pressed => { app_keys.push(AppKey::Escape); continue; } // KEY_ESC
                41 if pressed => { app_keys.push(AppKey::ToggleOverlay); continue; } // KEY_GRAVE
                // KEY_F12 / KEY_SYSRQ (PrintScreen). Before the F-key arm
                // below, which would otherwise swallow F12 as a group that
                // does not exist. Ctrl+Alt+F12 is a VT switch and already
                // `continue`d above, so it cannot reach here.
                88 | 99 if pressed => { app_keys.push(AppKey::Screenshot); continue; }
                59..=68 | 87 | 88 if pressed => {
                    let n = match code { 87 => 11, 88 => 12, _ => (code - 58) as u8 };
                    if let Some(group) = OverlayGroup::from_fkey(n) {
                        if self.modifiers.shift {
                            app_keys.push(AppKey::HideGroup(group));
                        } else {
                            app_keys.push(AppKey::ShowGroup(group));
                        }
                    }
                    continue;
                }
                32 if pressed => app_keys.push(AppKey::D), // KEY_D — also types below
                _ => {}
            }

            // Navigation keys → egui events (press + release).
            if let Some(key) = evdev_to_egui_key(code) {
                egui_events.push(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: self.modifiers,
                });
            }
            // Printable characters → egui text input (press only). Without this,
            // dialog text fields cannot be typed into in DRM mode.
            if pressed && let Some(ch) = evdev_to_char(code, self.modifiers.shift) {
                egui_events.push(egui::Event::Text(ch.to_string()));
            }
        }

        (app_keys, egui_events)
    }
}

// ── VT primitives ────────────────────────────────────────────────────────────

/// Open the TTY device for `target_vt`.
///
/// When systemd has already opened the device via `TTYPath=` + `StandardInput=tty`,
/// stdin (fd 0) *is* `/dev/tty{target_vt}`. Dup-ing it avoids needing the
/// vstimd user to have direct open permission on the device node (which is
/// `crw-------` / root-only when no login session owns it).
pub(crate) fn open_vt(target_vt: u16) -> libc::c_int {
    let expected = format!("/dev/tty{target_vt}");
    if ttyname_of(0).as_deref() == Some(&expected) {
        let fd = unsafe { libc::fcntl(0, libc::F_DUPFD_CLOEXEC, 0) };
        if fd >= 0 {
            return fd;
        }
    }
    // Fall back to a direct open (works when run with sufficient permissions,
    // e.g. during development or with a udev rule granting group access).
    let path = format!("{expected}\0");
    unsafe {
        libc::open(
            path.as_ptr() as *const libc::c_char,
            libc::O_WRONLY | libc::O_CLOEXEC,
        )
    }
}

fn ttyname_of(fd: libc::c_int) -> Option<String> {
    let mut buf = [0u8; 64];
    let ret = unsafe { libc::ttyname_r(fd, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if ret != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Some(String::from_utf8_lossy(&buf[..end]).into_owned())
}

pub(crate) fn vt_number_from_env() -> u16 {
    match std::env::var("VSTIMD_TTY") {
        Ok(s) => match s.trim().parse::<u16>() {
            Ok(n) if n >= 1 => n,
            _ => {
                log::warn!("vstimd: VSTIMD_TTY={s:?} is not a valid VT number, using 3");
                3
            }
        },
        Err(_) => 3,
    }
}

pub(crate) fn active_vt() -> Option<u16> {
    let s = std::fs::read_to_string("/sys/class/tty/tty0/active").ok()?;
    s.trim().strip_prefix("tty")?.parse().ok()
}

/// Map evdev key codes to characters for text entry (US QWERTY layout).
/// Returns the shifted glyph when `shift` is held.
fn evdev_to_char(code: u32, shift: bool) -> Option<char> {
    let (lo, hi): (char, char) = match code {
        2 => ('1', '!'), 3 => ('2', '@'), 4 => ('3', '#'), 5 => ('4', '$'),
        6 => ('5', '%'), 7 => ('6', '^'), 8 => ('7', '&'), 9 => ('8', '*'),
        10 => ('9', '('), 11 => ('0', ')'),
        12 => ('-', '_'), 13 => ('=', '+'),
        16 => ('q', 'Q'), 17 => ('w', 'W'), 18 => ('e', 'E'), 19 => ('r', 'R'),
        20 => ('t', 'T'), 21 => ('y', 'Y'), 22 => ('u', 'U'), 23 => ('i', 'I'),
        24 => ('o', 'O'), 25 => ('p', 'P'), 26 => ('[', '{'), 27 => (']', '}'),
        30 => ('a', 'A'), 31 => ('s', 'S'), 32 => ('d', 'D'), 33 => ('f', 'F'),
        34 => ('g', 'G'), 35 => ('h', 'H'), 36 => ('j', 'J'), 37 => ('k', 'K'),
        38 => ('l', 'L'), 39 => (';', ':'), 40 => ('\'', '"'), 43 => ('\\', '|'),
        44 => ('z', 'Z'), 45 => ('x', 'X'), 46 => ('c', 'C'), 47 => ('v', 'V'),
        48 => ('b', 'B'), 49 => ('n', 'N'), 50 => ('m', 'M'),
        51 => (',', '<'), 52 => ('.', '>'), 53 => ('/', '?'),
        57 => (' ', ' '),
        _ => return None,
    };
    Some(if shift { hi } else { lo })
}

/// Map evdev key codes (linux/input-event-codes.h) to egui navigation keys.
fn evdev_to_egui_key(code: u32) -> Option<egui::Key> {
    Some(match code {
        14 => egui::Key::Backspace,
        15 => egui::Key::Tab,
        28 | 96 => egui::Key::Enter, // KEY_ENTER, KEY_KPENTER
        57 => egui::Key::Space,
        102 => egui::Key::Home,       // KEY_HOME
        103 => egui::Key::ArrowUp,    // KEY_UP
        104 => egui::Key::PageUp,     // KEY_PAGEUP
        105 => egui::Key::ArrowLeft,  // KEY_LEFT
        106 => egui::Key::ArrowRight, // KEY_RIGHT
        107 => egui::Key::End,        // KEY_END
        108 => egui::Key::ArrowDown,  // KEY_DOWN
        109 => egui::Key::PageDown,   // KEY_PAGEDOWN
        _ => return None,
    })
}
