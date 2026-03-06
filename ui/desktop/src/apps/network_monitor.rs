// Specteros Network Monitor
// Real-time network traffic visualization and leak detection

use gtk::prelude::*;
use gtk::{Box, Button, Label, Orientation, ProgressBar};

pub struct NetworkMonitor {
    container: Box,
    status_label: Label,
    traffic_bar: ProgressBar,
    kill_switch_btn: Button,
    route_indicator: Label,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("network-monitor")
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        // Status header
        let status_label = Label::builder()
            .label("🌐 Network Status: SECURE")
            .css_name("status-label")
            .build();

        // Traffic visualization
        let traffic_bar = ProgressBar::builder()
            .fraction(0.0)
            .text("No active connections")
            .show_text(true)
            .build();

        // Route indicator
        let route_indicator = Label::builder()
            .label("📍 Route: Direct (eth0)")
            .css_name("route-indicator")
            .build();

        // Control buttons
        let button_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();

        let kill_switch_btn = Button::builder()
            .label("🚫 Kill Switch")
            .css_name("kill-switch-btn")
            .build();

        let tor_btn = Button::builder()
            .label("🧅 Tor Mode")
            .build();

        let refresh_btn = Button::builder()
            .label("🔄 Refresh")
            .build();

        button_box.append(&kill_switch_btn);
        button_box.append(&tor_btn);
        button_box.append(&refresh_btn);

        // Leak test button
        let leak_test_btn = Button::builder()
            .label("🔍 Test for Leaks")
            .tooltip_text("Check for DNS, WebRTC, and IP leaks")
            .build();

        container.append(&status_label);
        container.append(&route_indicator);
        container.append(&traffic_bar);
        container.append(&button_box);
        container.append(&leak_test_btn);

        Self {
            container,
            status_label,
            traffic_bar,
            kill_switch_btn,
            route_indicator,
        }
    }

    /// Update network status
    pub fn set_status(&self, status: NetworkStatus) {
        let (icon, text, class) = match status {
            NetworkStatus::Secure => ("🌐", "SECURE", "secure"),
            NetworkStatus::Tor => ("🧅", "TOR ACTIVE", "tor"),
            NetworkStatus::VPN => ("🔐", "VPN ACTIVE", "vpn"),
            NetworkStatus::KillSwitch => ("🚫", "KILL SWITCH", "danger"),
            NetworkStatus::LeakDetected => ("⚠️", "LEAK DETECTED", "warning"),
            NetworkStatus::Offline => ("✈️", "OFFLINE", "offline"),
        };
        self.status_label.set_text(&format!("{} Network Status: {}", icon, text));
        self.status_label.set_css_classes(&[class]);
    }

    /// Update traffic display
    pub fn update_traffic(&self, upload: f64, download: f64) {
        let total = upload + download;
        let fraction = (total / 100.0).min(1.0);
        self.traffic_bar.set_fraction(fraction);
        self.traffic_bar.set_text(Some(&format!(
            "↑ {:.1} KB/s  ↓ {:.1} KB/s",
            upload, download
        )));
    }

    /// Set current route
    pub fn set_route(&self, route: &str, interface: &str) {
        self.route_indicator.set_text(&format!(
            "📍 Route: {} ({})",
            route, interface
        ));
    }

    /// Set kill switch callback
    pub fn connect_kill_switch<F: Fn() + 'static>(&self, callback: F) {
        self.kill_switch_btn.connect_clicked(move |_| {
            callback();
        });
    }

    /// Enable kill switch visual state
    pub fn set_kill_switch_active(&self, active: bool) {
        if active {
            self.kill_switch_btn.set_label("✅ Kill Switch Active");
            self.kill_switch_btn.set_css_classes(&["active"]);
        } else {
            self.kill_switch_btn.set_label("🚫 Kill Switch");
            self.kill_switch_btn.set_css_classes(&[]);
        }
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NetworkStatus {
    Secure,
    Tor,
    VPN,
    KillSwitch,
    LeakDetected,
    Offline,
}

/// Per-shard network status widget
pub struct ShardNetworkStatus {
    container: Box,
}

impl ShardNetworkStatus {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("shard-network-status")
            .spacing(8)
            .build();

        Self { container }
    }

    /// Add shard network indicator
    pub fn add_shard_status(&self, shard: &str, status: &str) {
        let label = Label::builder()
            .label(&format!("{}: {}", shard, status))
            .halign(gtk::Align::Start)
            .build();
        self.container.append(&label);
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}

/// DNS leak test results
pub struct LeakTestResults {
    container: Box,
}

impl LeakTestResults {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("leak-test-results")
            .spacing(4)
            .build();

        Self { container }
    }

    pub fn show_results(&self, results: LeakTestReport) {
        // Display leak test results
        let _status = if results.all_clear {
            "✓ All tests passed - No leaks detected"
        } else {
            "⚠ Potential leaks detected"
        };
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}

pub struct LeakTestReport {
    pub all_clear: bool,
    pub dns_leak: bool,
    pub webrtc_leak: bool,
    pub ip_leak: bool,
    pub ipv6_leak: bool,
}
