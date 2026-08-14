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
use std::os::fd::{AsRawFd as _, OwnedFd, RawFd};
use std::path::Path;

use crate::render::AppKey;
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

/// `EVIOCGNAME(len)` — same `_IOC` layout as [`EVIOCGRAB`] but read-direction
/// (dir=2, nr=0x06) and with a caller-chosen buffer length in the size field,
/// so it has to be built at the call site rather than written as a constant.
const fn eviocgname(len: u32) -> libc::c_ulong {
    ((2 << 30) | (len << 16) | (0x45 << 8) | 0x06) as libc::c_ulong
}

/// `EVIOCGBIT(EV_KEY, len)` — reads the device's key capability bitmap.
/// `nr` is `0x20 + ev_type`, and `EV_KEY` is 1.
const fn eviocgbit_key(len: u32) -> libc::c_ulong {
    ((2 << 30) | (len << 16) | (0x45 << 8) | 0x21) as libc::c_ulong
}

/// Bytes needed for a full evdev key bitmap (`KEY_CNT` = 768 bits).
const KEY_BITMAP_BYTES: usize = 96;

fn has_key(bitmap: &[u8; KEY_BITMAP_BYTES], code: usize) -> bool {
    bitmap[code / 8] & (1 << (code % 8)) != 0
}

/// Human-readable evdev device name, for logs only.
fn device_name(fd: RawFd) -> String {
    let mut buf = [0u8; 256];
    if unsafe { libc::ioctl(fd, eviocgname(buf.len() as u32), buf.as_mut_ptr()) } < 0 {
        return "<unnamed>".to_string();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// True when the device is a bare power/sleep switch rather than something a
/// person types or points with.
///
/// Matches the Pi 5's `gpio-keys` power button and the separate
/// "system control" nodes many USB keyboards expose, but not the keyboard
/// itself: the test is "advertises a power key *and* has no ordinary typing
/// keys". A keyboard that happened to carry `KEY_POWER` on its main node
/// still gets grabbed, which is the safe way round — a missed grab there
/// would leak every keystroke to the console.
fn is_power_switch(fd: RawFd) -> bool {
    let mut keys = [0u8; KEY_BITMAP_BYTES];
    if unsafe { libc::ioctl(fd, eviocgbit_key(KEY_BITMAP_BYTES as u32), keys.as_mut_ptr()) } < 0 {
        // Can't tell what this is — grab it, same as before.
        return false;
    }
    is_power_switch_bitmap(&keys)
}

/// The capability test behind [`is_power_switch`], split out so it can be
/// exercised without a real device.
fn is_power_switch_bitmap(keys: &[u8; KEY_BITMAP_BYTES]) -> bool {
    /// `KEY_POWER`, `KEY_SLEEP`, `KEY_SUSPEND`, `KEY_POWER2`.
    const POWER_KEYS: [usize; 4] = [116, 142, 205, 356];
    // KEY_1 (2) through KEY_SPACE (57) spans every digit and letter.
    const TYPING_KEYS: std::ops::RangeInclusive<usize> = 2..=57;

    POWER_KEYS.iter().any(|&k| has_key(keys, k)) && !TYPING_KEYS.clone().any(|k| has_key(keys, k))
}

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
                // The power button is an input device like any other (on the
                // Pi 5 a `gpio-keys` node emitting KEY_POWER), so libinput
                // opens it along with the keyboard. Grabbing it would swallow
                // the press before systemd-logind ever saw it, leaving no way
                // to shut the machine down while vstimd is running. Leave
                // power switches ungrabbed: we don't act on those keys, and
                // logind reads them in parallel — it never needs the grab.
                if is_power_switch(fd.as_raw_fd()) {
                    log::info!(
                        "vstimd: leaving power switch ungrabbed: {} ({}) — logind handles it",
                        path.display(),
                        device_name(fd.as_raw_fd())
                    );
                    return fd;
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a key bitmap advertising exactly `codes`.
    fn bitmap(codes: &[usize]) -> [u8; KEY_BITMAP_BYTES] {
        let mut b = [0u8; KEY_BITMAP_BYTES];
        for &c in codes {
            b[c / 8] |= 1 << (c % 8);
        }
        b
    }

    /// The Pi 5 / ACPI case this exists for: a `gpio-keys` node whose only
    /// capability is KEY_POWER must stay ungrabbed so logind sees the press.
    #[test]
    fn bare_power_button_is_left_ungrabbed() {
        assert!(is_power_switch_bitmap(&bitmap(&[116])));
    }

    /// A keyboard that also advertises KEY_POWER on its main node must still
    /// be grabbed — otherwise every keystroke leaks to the console. Observed
    /// on real hardware (Keychron Q11), so it is not a hypothetical.
    #[test]
    fn keyboard_carrying_power_key_is_still_grabbed() {
        // KEY_A, KEY_Z, KEY_SPACE alongside KEY_POWER.
        assert!(!is_power_switch_bitmap(&bitmap(&[30, 44, 57, 116])));
    }

    /// A keyboard's separate "system control" node (power/sleep only, no
    /// typing keys) is a power switch by the same rule.
    #[test]
    fn system_control_node_is_left_ungrabbed() {
        assert!(is_power_switch_bitmap(&bitmap(&[116, 142, 205])));
    }

    /// Mice and everything else without a power key are unaffected.
    #[test]
    fn mouse_is_grabbed() {
        assert!(!is_power_switch_bitmap(&bitmap(&[272, 273, 274]))); // BTN_LEFT/RIGHT/MIDDLE
        assert!(!is_power_switch_bitmap(&bitmap(&[])));
    }

    /// KEY_POWER2 (356) lives past the first 256 bits — a narrower bitmap
    /// buffer would silently miss it.
    #[test]
    fn power2_is_within_the_bitmap() {
        const { assert!(356 / 8 < KEY_BITMAP_BYTES) };
        assert!(is_power_switch_bitmap(&bitmap(&[356])));
    }

    /// The read-direction `_IOC` helpers are hand-encoded; check them against
    /// the independently known-good `EVIOCGRAB` layout.
    #[test]
    fn ioctl_encodings_match_the_ioc_layout() {
        let ioc = |dir: u32, ty: u32, nr: u32, size: u32| {
            ((dir << 30) | (size << 16) | (ty << 8) | nr) as libc::c_ulong
        };
        assert_eq!(EVIOCGRAB, ioc(1, 0x45, 0x90, 4));
        assert_eq!(eviocgname(256), ioc(2, 0x45, 0x06, 256));
        assert_eq!(eviocgbit_key(96), ioc(2, 0x45, 0x21, 96));
    }
}
