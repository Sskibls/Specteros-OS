// Specteros Secure File Manager
// Shard-aware file browser with metadata sanitization

use gtk::prelude::*;
use gtk::{Box, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow};

pub struct SecureFileManager {
    container: Box,
    current_shard: String,
    path_label: Label,
    file_list: ListBox,
}

impl SecureFileManager {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("file-manager")
            .build();

        // Path bar
        let path_label = Label::builder()
            .label("~/work")
            .css_name("path-bar")
            .selectable(true)
            .build();

        // File list
        let file_list = ListBox::builder()
            .css_name("file-list")
            .build();

        let scrolled = ScrolledWindow::builder()
            .vexpand(true)
            .child(&file_list)
            .build();

        // Toolbar
        let toolbar = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();

        let sanitize_btn = Button::builder()
            .label("🧹 Sanitize Metadata")
            .tooltip_text("Remove metadata from selected files")
            .build();

        let airlock_btn = Button::builder()
            .label("🔓 Airlock Transfer")
            .tooltip_text("Transfer file to another shard")
            .build();

        toolbar.append(&sanitize_btn);
        toolbar.append(&airlock_btn);

        container.append(&path_label);
        container.append(&toolbar);
        container.append(&scrolled);

        Self {
            container,
            current_shard: "work".to_string(),
            path_label,
            file_list,
        }
    }

    /// Set current shard context
    pub fn set_shard(&mut self, shard: &str) {
        self.current_shard = shard.to_string();
        self.path_label.set_text(&format!("~/{}", shard));
        self.load_directory();
    }

    /// Load files for current shard
    fn load_directory(&self) {
        // Clear existing items
        while let Some(row) = self.file_list.row_at_index(0) {
            self.file_list.remove(&row);
        }

        // Sample files for demo
        let files = vec![
            ("📁", "Documents", "directory"),
            ("📁", "Downloads", "directory"),
            ("📄", "report.pdf", "file"),
            ("📊", "data.csv", "file"),
            ("🖼️", "image.png", "image"),
        ];

        for (icon, name, type_) in files {
            let row = ListBoxRow::builder().build();
            let row_box = Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(8)
                .margin_start(8)
                .margin_end(8)
                .margin_top(4)
                .margin_bottom(4)
                .build();

            let icon_label = Label::new(Some(icon));
            let name_label = Label::builder()
                .label(name)
                .halign(gtk::Align::Start)
                .hexpand(true)
                .build();

            let meta_indicator = if type_ == "file" || type_ == "image" {
                Label::new(Some("📋"))
            } else {
                Label::new(Some(""))
            };

            row_box.append(&icon_label);
            row_box.append(&name_label);
            row_box.append(&meta_indicator);

            row.set_child(Some(&row_box));
            self.file_list.append(&row);
        }
    }

    /// Sanitize metadata for selected files
    pub fn sanitize_selected(&self) {
        // Call gk-metadata-sanitizer
        println!("Sanitizing metadata for selected files...");
    }

    /// Open airlock transfer dialog
    pub fn airlock_transfer(&self) {
        // Show airlock transfer UI
        println!("Opening airlock transfer...");
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}

/// File metadata display
pub struct MetadataViewer {
    container: Box,
}

impl MetadataViewer {
    pub fn new() -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .css_name("metadata-viewer")
            .spacing(4)
            .build();

        container.append(&Label::new(Some("📋 File Metadata")));

        Self { container }
    }

    pub fn show_metadata(&self, _metadata: &std::collections::HashMap<String, String>) {
        // Display metadata
        // Option to strip before export
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.container.upcast_ref()
    }
}
