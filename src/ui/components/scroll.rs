use adw::prelude::*;
use adw::{
    AnimationState, CallbackAnimationTarget, Easing, PropertyAnimationTarget, TimedAnimation,
};
use gtk::{
    Adjustment, EventControllerScroll, EventControllerScrollFlags, GestureClick, Orientation,
    PropagationPhase, ScrolledWindow, gdk, glib,
};

#[derive(Clone)]
pub struct SmoothScroll {
    adjustment: Adjustment,
    animation: TimedAnimation,
}

impl SmoothScroll {
    pub fn attach(frame: &ScrolledWindow, orientation: Orientation) -> Self {
        let horizontal = orientation == Orientation::Horizontal;
        let adjustment = if horizontal {
            frame.hadjustment()
        } else {
            frame.vadjustment()
        };
        let target = PropertyAnimationTarget::new(&adjustment, "value");
        let animation = TimedAnimation::new(frame, 0.0, 0.0, 120, target.clone());
        animation.set_easing(Easing::EaseOutCubic);

        let scroll = Self {
            adjustment,
            animation,
        };

        let press = GestureClick::new();
        press.set_propagation_phase(PropagationPhase::Capture);
        press.connect_pressed(glib::clone!(
            #[strong]
            scroll,
            move |_, _, _, _| scroll.stop()
        ));
        frame.add_controller(press);

        let controller = EventControllerScroll::new(EventControllerScrollFlags::BOTH_AXES);
        controller.connect_scroll(glib::clone!(
            #[strong]
            scroll,
            #[strong]
            target,
            move |controller, dx, dy| {
                scroll.animation.set_target(&target);

                if controller.unit() != gdk::ScrollUnit::Wheel {
                    scroll.stop();

                    if horizontal && dx == 0.0 {
                        scroll.adjustment.set_value(scroll.adjustment.value() + dy);

                        return glib::Propagation::Stop;
                    }

                    return glib::Propagation::Proceed;
                }

                let delta = if horizontal && dx != 0.0 { dx } else { dy };
                if delta == 0.0 {
                    return glib::Propagation::Proceed;
                }

                // Use GTK's wheel distance and retain unconsumed motion.
                let distance = delta * scroll.adjustment.page_size().powf(2.0 / 3.0);
                scroll.scroll_by(distance);

                glib::Propagation::Stop
            }
        ));
        frame.add_controller(controller);

        scroll
    }

    pub fn scroll_pages(&self, pages: f64, scrolled: impl Fn(&Adjustment) + 'static) {
        scrolled(&self.adjustment);

        let target = CallbackAnimationTarget::new(glib::clone!(
            #[strong(rename_to = adjustment)]
            self.adjustment,
            move |value| {
                adjustment.set_value(value);
                scrolled(&adjustment);
            }
        ));
        self.animation.set_target(&target);
        self.scroll_by(pages * self.adjustment.page_size());
    }

    pub fn stop(&self) {
        self.animation.pause();
    }

    fn scroll_by(&self, distance: f64) {
        let current = self.adjustment.value();
        let pending = self.animation.value_to() - current;
        let origin = if self.animation.state() == AnimationState::Playing
            && pending.signum() == distance.signum()
        {
            self.animation.value_to()
        } else {
            current
        };
        let target = (origin + distance).clamp(
            self.adjustment.lower(),
            (self.adjustment.upper() - self.adjustment.page_size()).max(self.adjustment.lower()),
        );

        self.animation.pause();
        self.animation.set_value_from(current);
        self.animation.set_value_to(target);
        self.animation.play();
    }
}
