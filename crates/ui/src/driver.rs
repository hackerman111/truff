use truff_core::vim::Key;

use crate::shell::{Shell, ShellAction};

/// Converts winit key events and characters to platform-independent Truff `Key`.
pub fn map_key_code(c: char, ctrl: bool, alt: bool) -> Key {
    if ctrl {
        Key::Ctrl(c)
    } else if alt {
        Key::Alt(c)
    } else {
        Key::Char(c)
    }
}

/// Headless or windowed UI driver directing event dispatch.
pub struct UiDriver {
    shell: Shell,
}

impl Default for UiDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl UiDriver {
    pub fn new() -> Self {
        Self {
            shell: Shell::new(),
        }
    }

    pub fn shell(&self) -> &Shell {
        &self.shell
    }

    pub fn shell_mut(&mut self) -> &mut Shell {
        &mut self.shell
    }

    pub fn dispatch_key(&mut self, key: Key) -> ShellAction {
        self.shell.feed_key(key)
    }
}

