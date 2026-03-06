// Specteros Desktop Top Panel
// System status, shard indicator, and quick controls

use gtk::prelude::*;
use gtk::{Box, Button, Label, Orientation, Widget};

pub struct TopPanel {
    container: Box,
    shard_label: Label,
    network_indicator: Label,
    privacy_status: Label,
    panic_button: Button,
}

impl TopPanel {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .css_name("top-panel")
            .spacing(6)
            .margin_top(0)
            .margin_start(0)
            .margin_end(0)
            .build();

        // Shard indicator
        let shard_label = Label::builder()
            .label("🔒 Shard: Work")
            .css_name("shard-indicator")
            .build();

        // Network status
        let network_indicator = Label::builder()
            .label("🌐 Direct")
            .css_name("network-status")
            .build();

        // Privacy status
        let privacy_status = Label::builder()
            .label("✓ Protected")
            .css_name("privacy-status")
            .build();

        // Panic button
        let panic_button = Button::builder()
            .label("⚠ PANIC")
            .css_name("panic-button")
            .tooltip_text("Activate emergency containment mode")
            .build();

        // Add widgets to panel
        container.append(&shard_label);
        container.append(&network_indicator);
        container.append(&privacy_status);
        container.append(&panic_button);

        Self {
            container,
            shard_label,
            network_indicator,
            privacy_status,
            panic_button,
        }
    }

    /// Update displayed shard
    pub fn set_shard(&self, shard: &str) {
        self.shard_label.set_text(&format!("🔒 Shard: {}", shard));
    }

    /// Update network status
    pub fn set_network_status(&self, status: NetworkStatus) {
        let (icon, text) = match status {
            NetworkStatus::Direct => ("🌐", "Direct"),
            NetworkStatus::Tor => ("🧅", "Tor"),
            NetworkStatus::VPN => ("🔐", "VPN"),
            NetworkStatus::Offline => ("✈️", "Offline"),
            NetworkStatus::KillSwitch => ("🚫", "KILL SWITCH"),
        };
        self.network_indicator.set_text(&format!("{} {}", icon, text));
    }

    /// Update privacy status
    pub fn set_privacy_status(&self, status: PrivacyStatus) {
        let (icon, text) = match status {
            PrivacyStatus::Protected => ("✓", "Protected"),
            PrivacyStatus::Warning => ("⚠", "Warning"),
            PrivacyStatus::Compromised => ("✗", "Risk Detected"),
        };
        self.privacy_status.set_text(&format!("{} {}", icon, text));
    }

    /// Set panic button callback
    pub fn connect_panic<F: Fn() + 'static>(&self, callback: F) {
        let panic_btn = self.panic_button.clone();
        self.panic_button.connect_clicked(move |_| {
            callback();
            panic_btn.set_label("⚠ ACTIVATED");
        });
    }

    /// Get the panel widget
    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Show privacy warning
    pub fn show_privacy_warning(&self, message: &str) {
        self.set_privacy_status(PrivacyStatus::Warning);
        self.privacy_status.set_tooltip_text(Some(message));
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NetworkStatus {
    Direct,
    Tor,
    VPN,
    Offline,
    KillSwitch,
}

#[derive(Debug, Clone, Copy)]
pub enum PrivacyStatus {
    Protected,
    Warning,
    Compromised,
}

/// System tray for Specteros services
pub struct SystemTray {
    container: Box,
}

impl SystemTray {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .css_name("system-tray")
            .spacing(4)
            .build();

        Self { container }
    }

    /// Add service indicator
    pub fn add_service(&self, name: &str, active: bool) {
        let icon = if active { "🟢" } else { "⚪" };
        let label = Label::builder()
            .label(&format!("{} {}", icon, name))
            .tooltip_text(&format!("{}: {}", name, if active { "Active" } else { "Inactive" }))
            .build();
        self.container.append(&label);
    }

    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }
}
