use gtk4::prelude::*;
use gtk4::Orientation;

/// Create a transparent spacer widget that expands to fill available space.
pub fn create() -> i64 {
    crate::app::ensure_gtk_init();
    let spacer = gtk4::Box::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    spacer.set_hexpand(true);
    super::register_widget(spacer.upcast())
}
