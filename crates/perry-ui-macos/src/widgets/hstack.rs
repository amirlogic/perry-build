use objc2::rc::Retained;
use objc2::{msg_send};
use objc2_app_kit::{NSStackView, NSView, NSUserInterfaceLayoutOrientation, NSLayoutAttribute};
use objc2_foundation::MainThreadMarker;

/// Set distribution to GravityAreas (-1) so children pack into gravity zones.
/// Combined with addView:inGravity:NSStackViewGravityLeading (in add_child),
/// this ensures children pack from the leading edge without stretching.
fn set_gravity_distribution(stack: &NSStackView) {
    unsafe {
        let _: () = msg_send![stack, setDistribution: -1i64]; // NSStackViewDistributionGravityAreas
    }
}

/// Create an NSStackView with horizontal orientation.
pub fn create(spacing: f64) -> i64 {
    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setSpacing(spacing);
    stack.setAlignment(NSLayoutAttribute::CenterY);
    set_gravity_distribution(&stack);
    let view: Retained<NSView> = unsafe { Retained::cast_unchecked(stack) };
    super::register_widget(view)
}

/// Create an NSStackView with horizontal orientation and custom edge insets.
pub fn create_with_insets(spacing: f64, top: f64, left: f64, bottom: f64, right: f64) -> i64 {
    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setSpacing(spacing);
    stack.setAlignment(NSLayoutAttribute::CenterY);
    set_gravity_distribution(&stack);
    unsafe {
        stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top, left, bottom, right,
        });
    }
    let view: Retained<NSView> = unsafe { Retained::cast_unchecked(stack) };
    super::register_widget(view)
}
