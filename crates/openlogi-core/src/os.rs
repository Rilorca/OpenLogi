//! Operating system and desktop environment detection.
//!
//! Provides runtime classification of the host operating system, Linux desktop
//! environment (GNOME, KDE Plasma, XFCE, Cinnamon, etc.), and display server
//! (Wayland, X11) to adapt system actions and shortcuts dynamically.

use std::env;
use std::fmt;

use serde::{Deserialize, Serialize};

/// The host operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatingSystem {
    /// Linux operating system.
    Linux,
    /// Apple macOS.
    MacOS,
    /// Microsoft Windows.
    Windows,
    /// Other or unclassified operating system.
    Unknown,
}

impl OperatingSystem {
    /// Detect the current operating system from compile target / runtime.
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOS
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self::Unknown
        }
    }

    /// User-facing label for the operating system.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Linux => "Linux",
            Self::MacOS => "macOS",
            Self::Windows => "Windows",
            Self::Unknown => "Unknown OS",
        }
    }

    /// Name this OS gives the primary "command" modifier — the key
    /// [`KeyCombo::has_command`](crate::binding::KeyCombo::has_command) models.
    ///
    /// The canonical serialized spelling stays `Cmd` on every platform (see
    /// [`KeyCombo::rendered_label`](crate::binding::KeyCombo::rendered_label));
    /// this is presentation only, so a Linux user reads `Super+D` for the chord
    /// their desktop calls `Super+D`.
    #[must_use]
    pub const fn command_modifier_label(self) -> &'static str {
        match self {
            Self::Linux => "Super",
            Self::MacOS => "Cmd",
            Self::Windows => "Win",
            Self::Unknown => "Meta",
        }
    }
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Known Linux desktop environments and compositors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinuxDesktop {
    /// GNOME Shell (Ubuntu, Fedora default).
    Gnome,
    /// KDE Plasma.
    Kde,
    /// XFCE desktop environment.
    Xfce,
    /// Cinnamon (Linux Mint default).
    Cinnamon,
    /// MATE desktop environment.
    Mate,
    /// System76 COSMIC desktop environment.
    Cosmic,
    /// Hyprland Wayland compositor.
    Hyprland,
    /// Sway Wayland compositor.
    Sway,
    /// LXQt lightweight desktop environment.
    Lxqt,
    /// Another desktop environment identified by name.
    Other(String),
    /// Desktop environment could not be identified or is running in headless mode.
    Unknown,
}

impl LinuxDesktop {
    /// Detect the running Linux desktop environment from environment variables.
    #[must_use]
    pub fn current() -> Self {
        if let Ok(desktop) = env::var("XDG_CURRENT_DESKTOP") {
            Self::from_desktop_name(&desktop)
        } else if let Ok(session) = env::var("DESKTOP_SESSION") {
            Self::from_desktop_name(&session)
        } else {
            Self::Unknown
        }
    }

    /// Parse a desktop name string from `XDG_CURRENT_DESKTOP` or `DESKTOP_SESSION`.
    #[must_use]
    pub fn from_desktop_name(name: &str) -> Self {
        // XDG_CURRENT_DESKTOP can be colon-separated (e.g. "ubuntu:GNOME", "pop:GNOME")
        let lower = name.to_ascii_lowercase();
        for part in lower.split(':') {
            let trimmed = part.trim();
            match trimmed {
                "gnome" | "unity" | "ubuntu" | "pop" => return Self::Gnome,
                "kde" | "plasma" => return Self::Kde,
                "xfce" | "x-cinnamon" => return Self::Xfce,
                "cinnamon" => return Self::Cinnamon,
                "mate" => return Self::Mate,
                "cosmic" => return Self::Cosmic,
                "hyprland" => return Self::Hyprland,
                "sway" => return Self::Sway,
                "lxqt" => return Self::Lxqt,
                // Empty segments and unknown names alike fall through to the
                // next `:`-separated part of XDG_CURRENT_DESKTOP.
                _ => {}
            }
        }
        if name.trim().is_empty() {
            Self::Unknown
        } else {
            Self::Other(name.trim().to_string())
        }
    }

    /// Human-readable label for the desktop environment.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE Plasma",
            Self::Xfce => "XFCE",
            Self::Cinnamon => "Cinnamon",
            Self::Mate => "MATE",
            Self::Cosmic => "COSMIC",
            Self::Hyprland => "Hyprland",
            Self::Sway => "Sway",
            Self::Lxqt => "LXQt",
            Self::Other(name) => name.as_str(),
            Self::Unknown => "Generic Linux",
        }
    }
}

impl fmt::Display for LinuxDesktop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// The display server protocol in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayServer {
    /// Wayland display server.
    Wayland,
    /// X11 (X Window System).
    X11,
    /// Unknown or non-applicable display server.
    Unknown,
}

impl DisplayServer {
    /// Detect whether Wayland or X11 is running.
    #[must_use]
    pub fn current() -> Self {
        if env::var_os("WAYLAND_DISPLAY").is_some() {
            return Self::Wayland;
        }
        if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
            if session_type.eq_ignore_ascii_case("wayland") {
                return Self::Wayland;
            }
            if session_type.eq_ignore_ascii_case("x11") {
                return Self::X11;
            }
        }
        if env::var_os("DISPLAY").is_some() {
            Self::X11
        } else {
            Self::Unknown
        }
    }

    /// User-facing label for the display server.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for DisplayServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Full host system environment snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemEnvironment {
    /// Host operating system.
    pub os: OperatingSystem,
    /// Linux desktop environment if running on Linux.
    pub desktop: Option<LinuxDesktop>,
    /// Display server if applicable.
    pub display_server: Option<DisplayServer>,
}

impl SystemEnvironment {
    /// Inspect the environment and return the detected host environment.
    #[must_use]
    pub fn detect() -> Self {
        let os = OperatingSystem::current();
        let (desktop, display_server) = match os {
            OperatingSystem::Linux => (
                Some(LinuxDesktop::current()),
                Some(DisplayServer::current()),
            ),
            _ => (None, None),
        };
        Self {
            os,
            desktop,
            display_server,
        }
    }

    /// A friendly descriptive summary string of the system environment.
    #[must_use]
    pub fn summary(&self) -> String {
        match self.os {
            OperatingSystem::Linux => {
                let de = self.desktop.as_ref().map_or("Linux", LinuxDesktop::label);
                let ds = self
                    .display_server
                    .as_ref()
                    .map_or("", |server| match server {
                        DisplayServer::Wayland => " (Wayland)",
                        DisplayServer::X11 => " (X11)",
                        DisplayServer::Unknown => "",
                    });
                format!("{de}{ds}")
            }
            OperatingSystem::MacOS => "macOS".to_string(),
            OperatingSystem::Windows => "Windows".to_string(),
            OperatingSystem::Unknown => "Unknown OS".to_string(),
        }
    }
}

impl Default for SystemEnvironment {
    fn default() -> Self {
        Self::detect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_linux_desktops() {
        assert_eq!(
            LinuxDesktop::from_desktop_name("ubuntu:GNOME"),
            LinuxDesktop::Gnome
        );
        assert_eq!(LinuxDesktop::from_desktop_name("KDE"), LinuxDesktop::Kde);
        assert_eq!(LinuxDesktop::from_desktop_name("XFCE"), LinuxDesktop::Xfce);
        assert_eq!(
            LinuxDesktop::from_desktop_name("Cinnamon"),
            LinuxDesktop::Cinnamon
        );
        assert_eq!(LinuxDesktop::from_desktop_name("MATE"), LinuxDesktop::Mate);
        assert_eq!(
            LinuxDesktop::from_desktop_name("Cosmic"),
            LinuxDesktop::Cosmic
        );
        assert_eq!(
            LinuxDesktop::from_desktop_name("Hyprland"),
            LinuxDesktop::Hyprland
        );
        assert_eq!(LinuxDesktop::from_desktop_name("sway"), LinuxDesktop::Sway);
    }

    #[test]
    fn detects_operating_system() {
        let os = OperatingSystem::current();
        assert_ne!(os, OperatingSystem::Unknown);
    }

    #[test]
    fn system_environment_summary_is_non_empty() {
        let env = SystemEnvironment::detect();
        assert!(!env.summary().is_empty());
    }
}
