use std::cell::Cell;

use adw::prelude::*;
use adw::{AnimationState, Easing, PropertyAnimationTarget, TimedAnimation};
use gtk::{
    Adjustment, EventControllerScroll, EventControllerScrollFlags, GestureClick, Orientation,
    PropagationPhase, ScrolledWindow, SingleSelection, gdk, glib,
};

#[derive(Clone, Copy)]
struct SelectionAnchor {
    scroll_offset: f64,
    selected: u32,
    row_offset: f64,
}

impl SelectionAnchor {
    fn update(&mut self, adjustment: &Adjustment, selection: &SingleSelection) -> u32 {
        let previous_offset = self.scroll_offset;
        self.scroll_offset = adjustment.value().floor();

        let selected = selection.selected();
        let page_size = adjustment.page_size();
        let content_height = adjustment.upper() - adjustment.lower();

        if selected == gtk::INVALID_LIST_POSITION || content_height <= page_size {
            self.selected = selected;
            self.row_offset = 0.0;

            return selected;
        }

        let count = selection.n_items();
        let row_height = content_height / f64::from(count);
        let top = self.scroll_offset - adjustment.lower();
        let first = ((top / row_height).ceil() as u32).min(count - 1);
        let last = (((top + page_size) / row_height).floor() as u32)
            .saturating_sub(1)
            .max(first)
            .min(count - 1);

        if selected != self.selected {
            let position = f64::from(selected) * row_height;

            // Keep a selection made by keyboard navigation while GTK reveals
            // it.
            if position < previous_offset || position + row_height > previous_offset + page_size {
                if (first..=last).contains(&selected) {
                    self.selected = selected;
                    self.row_offset = position - top;
                }

                return selected;
            }

            self.row_offset = position - previous_offset;
        }

        self.selected = (((top + self.row_offset) / row_height).round() as u32).clamp(first, last);

        self.selected
    }
}

#[derive(Clone)]
pub struct SmoothScroll {
    adjustment: Adjustment,
    animation: TimedAnimation,
}

impl SmoothScroll {
    pub fn new(frame: &ScrolledWindow, orientation: Orientation) -> Self {
        let horizontal = orientation == Orientation::Horizontal;
        let adjustment = if horizontal {
            frame.hadjustment()
        } else {
            frame.vadjustment()
        };
        let target = PropertyAnimationTarget::new(&adjustment, "value");
        let animation = TimedAnimation::new(frame, 0.0, 0.0, 120, target);
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
            move |controller, dx, dy| {
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

    pub fn follow_selection(&self, selection: &SingleSelection) {
        let anchor = Cell::new(SelectionAnchor {
            scroll_offset: self.adjustment.value(),
            selected: selection.selected(),
            row_offset: 0.0,
        });
        self.adjustment.connect_value_changed(glib::clone!(
            #[weak]
            selection,
            move |adjustment| {
                let mut state = anchor.get();
                let selected = state.update(adjustment, &selection);
                anchor.set(state);
                selection.set_selected(selected);
            }
        ));
    }

    pub fn scroll_pages(&self, pages: f64) {
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
