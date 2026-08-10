//! Minimal raw terminal + non-blocking stdin via `extern "C"` libc (no crates).
//!
//! macOS / Linux only. Restores attributes on Drop (RAII).

use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};

// ---------------------------------------------------------------------------
// libc FFI (termios + fcntl + poll) — zero crates
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod sys {
    use std::os::fd::RawFd;

    pub type Tcflag = std::ffi::c_ulong;
    pub type Cc = std::ffi::c_uchar;
    pub type Speed = std::ffi::c_ulong;

    pub const NCCS: usize = 20;
    pub const TCSANOW: i32 = 0;
    pub const VMIN: usize = 16;
    pub const VTIME: usize = 17;

    pub const ICANON: Tcflag = 0x0000_0100;
    pub const ECHO: Tcflag = 0x0000_0008;
    pub const ISIG: Tcflag = 0x0000_0080;
    pub const IXON: Tcflag = 0x0000_0200;

    pub const O_NONBLOCK: i32 = 0x0004;
    pub const F_GETFL: i32 = 3;
    pub const F_SETFL: i32 = 4;

    pub const POLLIN: i16 = 0x0001;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Termios {
        pub c_iflag: Tcflag,
        pub c_oflag: Tcflag,
        pub c_cflag: Tcflag,
        pub c_lflag: Tcflag,
        pub c_cc: [Cc; NCCS],
        pub c_ispeed: Speed,
        pub c_ospeed: Speed,
    }

    #[repr(C)]
    pub struct PollFd {
        pub fd: i32,
        pub events: i16,
        pub revents: i16,
    }

    extern "C" {
        pub fn isatty(fd: i32) -> i32;
        pub fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
        pub fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
        // Non-variadic form: always pass the third argument (0 when unused).
        pub fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
        pub fn poll(fds: *mut PollFd, nfds: u32, timeout: i32) -> i32;
    }

    pub fn make_raw(t: &mut Termios) {
        t.c_lflag &= !(ICANON | ECHO | ISIG);
        t.c_iflag &= !IXON;
        t.c_cc[VMIN] = 0;
        t.c_cc[VTIME] = 0;
    }

    #[allow(dead_code)]
    pub fn raw_fd_ok(_fd: RawFd) {}
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
mod sys {
    use std::os::fd::RawFd;

    // Linux glibc/musl termios
    pub type Tcflag = std::ffi::c_uint;
    pub type Cc = std::ffi::c_uchar;
    pub type Speed = std::ffi::c_uint;

    pub const NCCS: usize = 32;
    pub const TCSANOW: i32 = 0;
    pub const VMIN: usize = 6;
    pub const VTIME: usize = 5;

    pub const ICANON: Tcflag = 0x0002;
    pub const ECHO: Tcflag = 0x0008;
    pub const ISIG: Tcflag = 0x0001;
    pub const IXON: Tcflag = 0x0400;

    pub const O_NONBLOCK: i32 = 0x800;
    pub const F_GETFL: i32 = 3;
    pub const F_SETFL: i32 = 4;

    pub const POLLIN: i16 = 0x0001;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Termios {
        pub c_iflag: Tcflag,
        pub c_oflag: Tcflag,
        pub c_cflag: Tcflag,
        pub c_lflag: Tcflag,
        pub c_line: Cc,
        pub c_cc: [Cc; NCCS],
        pub c_ispeed: Speed,
        pub c_ospeed: Speed,
    }

    #[repr(C)]
    pub struct PollFd {
        pub fd: i32,
        pub events: i16,
        pub revents: i16,
    }

    extern "C" {
        pub fn isatty(fd: i32) -> i32;
        pub fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
        pub fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
        pub fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
        pub fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    }

    pub fn make_raw(t: &mut Termios) {
        t.c_lflag &= !(ICANON | ECHO | ISIG);
        t.c_iflag &= !IXON;
        t.c_cc[VMIN] = 0;
        t.c_cc[VTIME] = 0;
    }

    #[allow(dead_code)]
    pub fn raw_fd_ok(_fd: RawFd) {}
}

#[cfg(not(unix))]
mod sys {
    // Stub for non-Unix — interactive mode unavailable.
    pub fn unsupported() -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "interactive mode requires macOS or Linux",
        ))
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// True if stdin is a terminal.
pub fn stdin_is_tty() -> bool {
    #[cfg(unix)]
    {
        let fd = io::stdin().as_raw_fd();
        unsafe { sys::isatty(fd) == 1 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// RAII guard: puts stdin into raw + non-blocking mode; restores on drop.
pub struct RawMode {
    fd: RawFd,
    original: sys::Termios,
    original_flags: i32,
    active: bool,
}

impl RawMode {
    /// Enter raw non-blocking mode on stdin. Fails if not a TTY or on error.
    pub fn enter() -> io::Result<Self> {
        #[cfg(unix)]
        {
            let fd = io::stdin().as_raw_fd();
            if unsafe { sys::isatty(fd) } != 1 {
                return Err(io::Error::new(io::ErrorKind::Other, "stdin is not a TTY"));
            }

            let mut original = unsafe { std::mem::zeroed::<sys::Termios>() };
            if unsafe { sys::tcgetattr(fd, &mut original) } != 0 {
                return Err(io::Error::last_os_error());
            }

            let mut raw = original;
            sys::make_raw(&mut raw);
            if unsafe { sys::tcsetattr(fd, sys::TCSANOW, &raw) } != 0 {
                return Err(io::Error::last_os_error());
            }

            let original_flags = unsafe { sys::fcntl(fd, sys::F_GETFL, 0) };
            if original_flags < 0 {
                let _ = unsafe { sys::tcsetattr(fd, sys::TCSANOW, &original) };
                return Err(io::Error::last_os_error());
            }
            if unsafe { sys::fcntl(fd, sys::F_SETFL, original_flags | sys::O_NONBLOCK) } < 0 {
                let _ = unsafe { sys::tcsetattr(fd, sys::TCSANOW, &original) };
                return Err(io::Error::last_os_error());
            }

            Ok(Self {
                fd,
                original,
                original_flags,
                active: true,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = sys::unsupported()?;
            unreachable!()
        }
    }

    /// Explicit restore (also called by Drop).
    pub fn restore(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        #[cfg(unix)]
        {
            let _ = unsafe { sys::fcntl(self.fd, sys::F_SETFL, self.original_flags) };
            let _ = unsafe { sys::tcsetattr(self.fd, sys::TCSANOW, &self.original) };
            // silence unused on non-unix paths covered by cfg
            let _ = self.fd;
        }
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Keys we care about in interactive mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Space,
    Step,      // '.'
    Faster,    // '+' or '='
    Slower,    // '-'
    Quit,      // 'q' or 'Q'
    Reseed,    // 'r' or 'R'
    Theme,     // 't' or 'T' — cycle colour theme
    Age,       // 'a' or 'A' — toggle age heat-map
    Wrap,      // 'w' or 'W' — toggle toroidal wrap
    Save,      // 's' or 'S' — save snapshot.rle
    Sparkline, // 'p' or 'P' — print population sparkline
    Other(u8),
}

impl Key {
    pub fn from_byte(b: u8) -> Self {
        match b {
            b' ' => Key::Space,
            b'.' => Key::Step,
            b'+' | b'=' => Key::Faster,
            b'-' => Key::Slower,
            b'q' | b'Q' => Key::Quit,
            b'r' | b'R' => Key::Reseed,
            b't' | b'T' => Key::Theme,
            b'a' | b'A' => Key::Age,
            b'w' | b'W' => Key::Wrap,
            b's' | b'S' => Key::Save,
            b'p' | b'P' => Key::Sparkline,
            other => Key::Other(other),
        }
    }
}

/// Poll stdin for up to `timeout_ms` and return a single key if available.
///
/// Uses `poll(2)` then a non-blocking read. Returns `None` on timeout / no data.
pub fn poll_key(timeout_ms: i32) -> Option<Key> {
    #[cfg(unix)]
    {
        let fd = io::stdin().as_raw_fd();
        let mut pfd = sys::PollFd {
            fd,
            events: sys::POLLIN,
            revents: 0,
        };
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let n = unsafe { sys::poll(&mut pfd, 1u32, timeout_ms) };
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
        let n = unsafe { sys::poll(&mut pfd, 1u64, timeout_ms) };
        if n <= 0 {
            return None;
        }
        let mut buf = [0u8; 1];
        match io::stdin().read(&mut buf) {
            Ok(1) => Some(Key::from_byte(buf[0])),
            _ => None,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = timeout_ms;
        None
    }
}

/// Drain all pending keypresses; return the last meaningful one (or Quit if any quit).
pub fn drain_keys() -> Option<Key> {
    let mut last = None;
    while let Some(k) = poll_key(0) {
        if matches!(k, Key::Quit) {
            return Some(Key::Quit);
        }
        last = Some(k);
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_from_byte() {
        assert_eq!(Key::from_byte(b' '), Key::Space);
        assert_eq!(Key::from_byte(b'.'), Key::Step);
        assert_eq!(Key::from_byte(b'+'), Key::Faster);
        assert_eq!(Key::from_byte(b'='), Key::Faster);
        assert_eq!(Key::from_byte(b'-'), Key::Slower);
        assert_eq!(Key::from_byte(b'q'), Key::Quit);
        assert_eq!(Key::from_byte(b'R'), Key::Reseed);
        assert_eq!(Key::from_byte(b't'), Key::Theme);
        assert_eq!(Key::from_byte(b'a'), Key::Age);
        assert_eq!(Key::from_byte(b'w'), Key::Wrap);
        assert_eq!(Key::from_byte(b's'), Key::Save);
        assert_eq!(Key::from_byte(b'p'), Key::Sparkline);
        assert_eq!(Key::from_byte(b'x'), Key::Other(b'x'));
    }
}
