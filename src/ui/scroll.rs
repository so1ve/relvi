use adw::prelude::*;
use adw::{AnimationState, Easing, PropertyAnimationTarget, TimedAnimation};
use gtk::{
    EventControllerScroll, EventControllerScrollFlags, GestureClick, Orientation, PropagationPhase,
    ScrolledWindow, gdk, glib,
};

pub fn smooth_scroll(frame: &ScrolledWindow, orientation: Orientation) -> TimedAnimation {
    let horizontal = orientation == Orientation::Horizontal;
    let adjustment = if horizontal {
        frame.hadjustment()
    } else {
        frame.vadjustment()
    };
    let target = PropertyAnimationTarget::new(&adjustment, "value");
    let animation = TimedAnimation::new(frame, 0.0, 0.0, 120, target);
    animation.set_easing(Easing::EaseOutCubic);

    let press = GestureClick::new();
    press.set_propagation_phase(PropagationPhase::Capture);
    press.connect_pressed(glib::clone!(
        #[strong]
        animation,
        move |_, _, _, _| animation.pause()
    ));
    frame.add_controller(press);

    let scroll = EventControllerScroll::new(EventControllerScrollFlags::BOTH_AXES);
    scroll.connect_scroll(glib::clone!(
        #[strong]
        animation,
        move |scroll, dx, dy| {
            if scroll.unit() != gdk::ScrollUnit::Wheel {
                animation.pause();

                if horizontal && dx == 0.0 {
                    adjustment.set_value(adjustment.value() + dy);

                    return glib::Propagation::Stop;
                }

                return glib::Propagation::Proceed;
            }

            let delta = if horizontal && dx != 0.0 { dx } else { dy };
            if delta == 0.0 {
                return glib::Propagation::Proceed;
            }

            let current = adjustment.value();
            let pending = animation.value_to() - current;
            let origin = if animation.state() == AnimationState::Playing
                && pending.signum() == delta.signum()
            {
                animation.value_to()
            } else {
                current
            };

            // Use GTK's wheel distance and retain unconsumed motion.
            let distance = delta * adjustment.page_size().powf(2.0 / 3.0);
            let target = (origin + distance).clamp(
                adjustment.lower(),
                (adjustment.upper() - adjustment.page_size()).max(adjustment.lower()),
            );

            animation.pause();
            animation.set_value_from(current);
            animation.set_value_to(target);
            animation.play();

            glib::Propagation::Stop
        }
    ));
    frame.add_controller(scroll);

    animation
}
