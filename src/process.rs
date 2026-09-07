//! Hidden child-process construction.
//!
//! On Windows every `Command` that runs an internal helper (`reg.exe`,
//! `java.exe` metadata probes, …) must carry `CREATE_NO_WINDOW`, otherwise a
//! GUI application with no console flashes a visible console window for each
//! probe. On other platforms this module is a thin pass-through over
//! [`std::process::Command::new`].
//!
//! All internal subprocesses in this crate must go through [`hidden_command`]
//! (or [`hide`]) so the flag cannot be forgotten on a new call site.

use std::ffi::OsStr;
use std::process::Command;

/// Windows `CREATE_NO_WINDOW` (`0x0800_0000`).
///
/// Kept public so tests and callers can assert which flag is applied without
/// re-typing the magic constant.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Windows `CREATE_NO_WINDOW` value on every platform.
///
/// Non-Windows builds never pass this to the OS; exposing the constant
/// unconditionally keeps unit tests portable.
#[cfg(not(windows))]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a child-process command that stays invisible on Windows.
///
/// Behaves exactly like [`Command::new`] on Unix; on Windows it additionally
/// sets `CREATE_NO_WINDOW` so the child never flashes a console window.
/// Captured `stdout`/`stderr` piping is unaffected — callers must keep using
/// `java.exe` (not `javaw.exe`) so output remains pipeable.
pub fn hidden_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide(&mut command);
    command
}

/// Apply the hidden-window flag to an existing [`Command`].
///
/// Idempotent: calling it twice sets the same flag twice, which Windows
/// treats as a single bit.
pub fn hide(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_command_targets_the_requested_program() {
        let command = hidden_command("reg");
        assert_eq!(command.get_program(), OsStr::new("reg"));
    }

    #[test]
    fn hide_is_idempotent_and_returns_the_same_command() {
        let mut command = Command::new("java");
        hide(&mut command);
        hide(&mut command);
        assert_eq!(command.get_program(), OsStr::new("java"));
    }

    #[test]
    fn create_no_window_matches_the_windows_sdk_value() {
        assert_eq!(CREATE_NO_WINDOW, 0x0800_0000);
    }
}
