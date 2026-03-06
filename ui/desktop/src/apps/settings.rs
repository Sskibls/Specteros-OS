// Specteros Privacy Settings
// Security and privacy configuration UI

use gtk::prelude::*;
use gtk::{Box, Button, ComboBoxText, Label, Orientation, Switch};

pub struct PrivacySettings {
    container: Box,
}

impl PrivacySettings {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("privacy-settings")
            .spacing(16)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        // Security section
        container.append(&Self::create_section_header("🔐 Security"));
        container.append(&Self::create_security_settings());

        // Privacy section
        container.append(&Self::create_section_header("👁️ Privacy"));
        container.append(&Self::create_privacy_settings());

        // Network section
        container.append(&Self::create_section_header("🌐 Network"));
        container.append(&Self::create_network_settings());

        // Shard section
        container.append(&Self::create_section_header("🔒 Shards"));
        container.append(&Self::create_shard_settings());

        Self { container }
    }

    fn create_section_header(title: &str) -> Label {
        Label::builder()
            .label(title)
            .css_name("section-header")
            .halign(gtk::Align::Start)
            .build()
    }

    fn create_security_settings() -> Box {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("settings-group")
            .spacing(8)
            .build();

        // Secure Boot
        container.append(&Self::create_toggle_row(
            "Secure Boot Enforcement",
            "Require signed bootloaders and kernels",
            true,
        ));

        // TPM
        container.append(&Self::create_toggle_row(
            "TPM 2.0 Required",
            "Use TPM for key storage and measured boot",
            true,
        ));

        // Full Disk Encryption
        container.append(&Self::create_toggle_row(
            "Full Disk Encryption",
            "Encrypt all persistent storage",
            true,
        ));

        container
    }

    fn create_privacy_settings() -> Box {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("settings-group")
            .spacing(8)
            .build();

        // Screen privacy filter
        container.append(&Self::create_toggle_row(
            "Screen Privacy Filter",
            "Blur screen content when away",
            false,
        ));

        // Disable screenshots
        container.append(&Self::create_toggle_row(
            "Disable Screenshots",
            "Block screenshot functionality",
            true,
        ));

        // Disable clipboard history
        container.append(&Self::create_toggle_row(
            "Disable Clipboard History",
            "Clear clipboard after 60 seconds",
            true,
        ));

        // Camera indicator
        container.append(&Self::create_toggle_row(
            "Camera Activity Indicator",
            "Show LED when camera is active",
            true,
        ));

        // Microphone indicator
        container.append(&Self::create_toggle_row(
            "Microphone Activity Indicator",
            "Show icon when mic is recording",
            true,
        ));

        container
    }

    fn create_network_settings() -> Box {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("settings-group")
            .spacing(8)
            .build();

        // Default network route
        let route_row = Self::create_dropdown_row(
            "Default Network Route",
            "Network path for new shards",
            &["Direct", "Tor", "VPN", "Offline"],
        );
        container.append(&route_row);

        // DNS over HTTPS
        container.append(&Self::create_toggle_row(
            "DNS over HTTPS",
            "Encrypt DNS queries",
            true,
        ));

        // IPv6
        container.append(&Self::create_toggle_row(
            "Disable IPv6",
            "Prevent IPv6 leaks",
            true,
        ));

        // Kill switch
        container.append(&Self::create_toggle_row(
            "Network Kill Switch",
            "Block all network on VPN failure",
            true,
        ));

        container
    }

    fn create_shard_settings() -> Box {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("settings-group")
            .spacing(8)
            .build();

        // Auto-lock shards
        container.append(&Self::create_toggle_row(
            "Auto-lock Inactive Shards",
            "Lock shards after 5 minutes of inactivity",
            true,
        ));

        // Cross-shard clipboard
        container.append(&Self::create_toggle_row(
            "Disable Cross-Shard Clipboard",
            "Prevent clipboard sharing between shards",
            true,
        ));

        // Travel mode
        container.append(&Self::create_toggle_row(
            "Travel Mode",
            "Ephemeral sessions, no persistent storage",
            false,
        ));

        container
    }

    fn create_toggle_row(title: &str, subtitle: &str, active: bool) -> Box {
        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .margin_start(8)
            .build();

        let labels = Box::builder()
            .orientation(Orientation::Vertical)
            .hexpand(true)
            .build();

        let title_label = Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .build();

        let subtitle_label = Label::builder()
            .label(subtitle)
            .halign(gtk::Align::Start)
            .css_classes(vec!["subtitle"])
            .build();

        labels.append(&title_label);
        labels.append(&subtitle_label);

        let switch = Switch::builder()
            .active(active)
            .halign(gtk::Align::End)
            .build();

        container.append(&labels);
        container.append(&switch);

        container
    }

    fn create_dropdown_row(title: &str, subtitle: &str, options: &[&str]) -> Box {
        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .margin_start(8)
            .build();

        let labels = Box::builder()
            .orientation(Orientation::Vertical)
            .hexpand(true)
            .build();

        let title_label = Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .build();

        let subtitle_label = Label::builder()
            .label(subtitle)
            .halign(gtk::Align::Start)
            .css_classes(vec!["subtitle"])
            .build();

        labels.append(&title_label);
        labels.append(&subtitle_label);

        let dropdown = ComboBoxText::new();
        for option in options {
            dropdown.append(Some(option), option);
        }
        dropdown.set_active(Some(0));

        container.append(&labels);
        container.append(&dropdown);

        container
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}

/// Emergency mode controls
pub struct EmergencyControls {
    container: Box,
}

impl EmergencyControls {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("emergency-controls")
            .spacing(12)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        let header = Label::builder()
            .label("⚠️ Emergency Modes")
            .css_name("emergency-header")
            .build();

        let panic_btn = Button::builder()
            .label("🚨 PANIC MODE")
            .css_name("panic-button")
            .tooltip_text("Kill network, lock all shards, clear secrets")
            .build();

        let mask_btn = Button::builder()
            .label("🎭 MASK MODE")
            .css_name("mask-button")
            .tooltip_text("Switch to decoy desktop")
            .build();

        let travel_btn = Button::builder()
            .label("✈️ TRAVEL MODE")
            .css_name("travel-button")
            .tooltip_text("Enable ephemeral sessions")
            .build();

        container.append(&header);
        container.append(&panic_btn);
        container.append(&mask_btn);
        container.append(&travel_btn);

        Self { container }
    }

    pub fn connect_panic<F: Fn() + 'static>(&self, _callback: F) {
        // Connect panic button
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}
