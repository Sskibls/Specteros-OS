// Specteros Desktop Wallpaper
// Dynamic, shard-aware wallpaper with privacy indicators

use gtk::prelude::*;
use gtk::{DrawingArea, Widget};

pub struct Wallpaper {
    area: DrawingArea,
    current_shard: String,
}

impl Wallpaper {
    pub fn new() -> Self {
        let area = DrawingArea::builder()
            .css_name("wallpaper")
            .build();

        area.set_draw_func(|_, cr, width, height| {
            draw_wallpaper(cr, width, height);
        });

        Self {
            area,
            current_shard: "work".to_string(),
        }
    }

    pub fn set_shard(&mut self, shard: &str) {
        self.current_shard = shard.to_string();
        self.area.queue_draw();
    }

    pub fn widget(&self) -> &Widget {
        self.area.upcast_ref()
    }
}

fn draw_wallpaper(cr: &gtk::cairo::Context, width: i32, height: i32) {
    // Draw gradient background based on shard
    let colors = get_shard_colors("work");

    // Linear gradient
    let gradient = gtk::cairo::LinearGradient::new(0.0, 0.0, width as f64, height as f64);
    gradient.add_color_stop_rgb(0.0, colors.0, colors.1, colors.2);
    gradient.add_color_stop_rgb(1.0, colors.3, colors.4, colors.5);

    cr.set_source(&gradient);
    cr.paint().unwrap();

    // Draw Specteros logo watermark
    draw_watermark(cr, width, height);
}

fn get_shard_colors(shard: &str) -> (f64, f64, f64, f64, f64, f64) {
    match shard {
        "work" => (0.1, 0.1, 0.18, 0.09, 0.2, 0.37),      // Dark blue
        "anon" => (0.05, 0.1, 0.16, 0.03, 0.07, 0.1),     // Darker blue
        "burner" => (0.1, 0.05, 0.05, 0.07, 0.03, 0.03),  // Dark red
        "lab" => (0.05, 0.1, 0.1, 0.03, 0.07, 0.07),      // Dark cyan
        _ => (0.1, 0.1, 0.18, 0.09, 0.2, 0.37),
    }
}

fn draw_watermark(cr: &gtk::cairo::Context, width: i32, height: i32) {
    // Draw subtle Specteros watermark
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.03);

    // Ghost icon placeholder
    let center_x = width as f64 / 2.0;
    let center_y = height as f64 / 2.0;

    cr.arc(center_x, center_y, 100.0, 0.0, 2.0 * std::f64::consts::PI);
    cr.fill().unwrap();
}

/// Privacy filter overlay - blurs screen content
pub struct PrivacyFilter {
    overlay: gtk::Overlay,
    blur_widget: gtk::Box,
}

impl PrivacyFilter {
    pub fn new() -> Self {
        let overlay = gtk::Overlay::new();

        let blur_widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_name("privacy-filter")
            .build();

        blur_widget.set_visible(false);
        overlay.add_overlay(&blur_widget);

        Self {
            overlay,
            blur_widget,
        }
    }

    pub fn enable(&self) {
        self.blur_widget.set_visible(true);
    }

    pub fn disable(&self) {
        self.blur_widget.set_visible(false);
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.overlay.upcast_ref()
    }
}
