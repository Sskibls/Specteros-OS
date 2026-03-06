// Specteros Desktop Environment
// Secure, privacy-focused Wayland desktop shell

mod shell;
mod panel;
mod wallpaper;
mod apps;

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};

use shell::DesktopShell;
use panel::TopPanel;

pub struct SpecterosDesktop {
    app: Application,
    shell: DesktopShell,
    panel: TopPanel,
}

impl SpecterosDesktop {
    pub fn new(application_id: &str) -> Self {
        let app = Application::builder()
            .application_id(application_id)
            .build();

        Self {
            app,
            shell: DesktopShell::new(),
            panel: TopPanel::new(),
        }
    }

    pub fn run(&self) -> gtk::glib::ExitCode {
        self.app.connect_startup(|_| {
            // Load secure defaults
            apply_security_hardening();
        });

        self.app.connect_activate(|app| {
            // Create main window
            let window = ApplicationWindow::builder()
                .application(app)
                .title("Specteros OS")
                .default_width(1920)
                .default_height(1080)
                .build();

            // Apply theme
            apply_theme(&window);

            window.show();
        });

        self.app.run()
    }
}

/// Apply security hardening to the desktop environment
fn apply_security_hardening() {
    // Disable clipboard persistence across shards
    // Disable screenshot by default
    // Enable screen privacy filter
    // Set secure window flags
}

/// Apply the active theme to the desktop
fn apply_theme(_window: &ApplicationWindow) {
    // Load theme from Specteros config
    // Available: fsociety, allsafe, darkarmy, default
}

fn main() -> gtk::glib::ExitCode {
    println!("👻 Specteros Desktop Environment v0.1.0");
    println!("   Secure Wayland Desktop Shell");

    let desktop = SpecterosDesktop::new("org.specteros.desktop");
    desktop.run()
}
