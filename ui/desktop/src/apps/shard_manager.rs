// Specteros Shard Manager
// Visual shard lifecycle management and isolation controls

use gtk::prelude::*;
use gtk::{Box, Button, Grid, Label, Orientation};

pub struct ShardManagerApp {
    container: Box,
    shards: Vec<ShardCard>,
}

struct ShardCard {
    name: String,
    card: Box,
    status_label: Label,
    start_btn: Button,
    stop_btn: Button,
}

impl ShardManagerApp {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("shard-manager")
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let header = Label::builder()
            .label("🔒 Persona Shards")
            .css_name("header")
            .build();

        let grid = Grid::builder()
            .column_spacing(12)
            .row_spacing(12)
            .build();

        // Create shard cards
        let shard_configs = vec![
            ("work", "💼", "Work identity and files"),
            ("anon", "👤", "Anonymous browsing and activities"),
            ("burner", "🔥", "Temporary, disposable sessions"),
            ("lab", "🧪", "Security testing and research"),
        ];

        let mut shards = Vec::new();
        for (i, (name, icon, desc)) in shard_configs.iter().enumerate() {
            let card = Self::create_shard_card(name, icon, desc);
            let col = (i % 2) as i32;
            let row = (i / 2) as i32;
            grid.attach(&card.card, col, row, 1, 1);
            shards.push(card);
        }

        container.append(&header);
        container.append(&grid);

        Self { container, shards }
    }

    fn create_shard_card(name: &str, icon: &str, desc: &str) -> ShardCard {
        let card = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name(&format!("shard-card shard-{}", name))
            .spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let title = Label::builder()
            .label(&format!("{} {}", icon, name.to_uppercase()))
            .css_name("shard-title")
            .build();

        let description = Label::builder()
            .label(desc)
            .wrap(true)
            .build();

        let status_label = Label::builder()
            .label("⚪ Stopped")
            .css_name("shard-status")
            .build();

        let button_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();

        let start_btn = Button::builder()
            .label("▶ Start")
            .css_name("start-btn")
            .build();

        let stop_btn = Button::builder()
            .label("⏹ Stop")
            .css_name("stop-btn")
            .sensitive(false)
            .build();

        button_box.append(&start_btn);
        button_box.append(&stop_btn);

        card.append(&title);
        card.append(&description);
        card.append(&status_label);
        card.append(&button_box);

        ShardCard {
            name: name.to_string(),
            card,
            status_label,
            start_btn,
            stop_btn,
        }
    }

    /// Update shard status
    pub fn set_shard_status(&self, name: &str, status: ShardStatus) {
        if let Some(card) = self.shards.iter().find(|s| s.name == name) {
            let (icon, text) = match status {
                ShardStatus::Running => ("🟢", "Running"),
                ShardStatus::Stopped => ("⚪", "Stopped"),
                ShardStatus::Locked => ("🔒", "Locked"),
                ShardStatus::Error => ("❌", "Error"),
            };
            card.status_label.set_text(&format!("{} {}", icon, text));
            card.start_btn.set_sensitive(status != ShardStatus::Running);
            card.stop_btn.set_sensitive(status == ShardStatus::Running);
        }
    }

    /// Connect shard start callback
    pub fn connect_start<F: Fn(&str) + 'static + Clone>(&self, callback: F) {
        for card in &self.shards {
            let name = card.name.clone();
            let cb = callback.clone();
            card.start_btn.connect_clicked(move |_| {
                cb(&name);
            });
        }
    }

    /// Connect shard stop callback
    pub fn connect_stop<F: Fn(&str) + 'static + Clone>(&self, callback: F) {
        for card in &self.shards {
            let name = card.name.clone();
            let cb = callback.clone();
            card.stop_btn.connect_clicked(move |_| {
                cb(&name);
            });
        }
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShardStatus {
    Running,
    Stopped,
    Locked,
    Error,
}

/// Cross-shard access policy viewer
pub struct AccessPolicyViewer {
    container: Box,
}

impl AccessPolicyViewer {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("access-policy-viewer")
            .build();

        Self { container }
    }

    pub fn show_policies(&self, _policies: Vec<AccessPolicy>) {
        // Display access policies between shards
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}

pub struct AccessPolicy {
    pub from_shard: String,
    pub to_shard: String,
    pub resource: String,
    pub allowed: bool,
}
