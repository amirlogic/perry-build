use gtk4::prelude::*;
use gtk4::Orientation;

/// Create a GtkBox with horizontal orientation.
pub fn create(spacing: f64) -> i64 {
    crate::app::ensure_gtk_init();
    let hbox = gtk4::Box::new(Orientation::Horizontal, spacing as i32);
    super::register_widget(hbox.upcast())
}

/// Create a GtkBox with horizontal orientation and custom edge insets.
pub fn create_with_insets(spacing: f64, top: f64, left: f64, bottom: f64, right: f64) -> i64 {
    crate::app::ensure_gtk_init();
    let hbox = gtk4::Box::new(Orientation::Horizontal, spacing as i32);
    hbox.set_margin_top(top as i32);
    hbox.set_margin_bottom(bottom as i32);
    hbox.set_margin_start(left as i32);
    hbox.set_margin_end(right as i32);
    super::register_widget(hbox.upcast())
}
