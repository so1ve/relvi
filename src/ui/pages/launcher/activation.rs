use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GtkBox, Label, Orientation, glib};
use tracing::error;

use super::LauncherPage;
use crate::launcher::Entry;
use crate::ui::components::button;

impl LauncherPage {
    pub(super) fn activate(self: &Rc<Self>, entry: Rc<Entry>) {
        self.error.set_visible(false);

        let Some(question) = entry.confirmation() else {
            self.run(entry);

            return;
        };

        let label = Label::new(Some(question));
        label.set_xalign(0.0);
        label.set_hexpand(true);

        let cancel = button::text("Cancel").build();
        let accept = button::text(&entry.title).build();
        accept.add_css_class("destructive-action");

        let confirmation = GtkBox::new(Orientation::Horizontal, 8);
        confirmation.add_css_class("confirmation");
        confirmation.append(&label);
        confirmation.append(&cancel);
        confirmation.append(&accept);

        cancel.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| {
                page.cancel_confirmation();
                page.search.grab_focus();
            }
        ));
        accept.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| {
                page.cancel_confirmation();
                page.run(Rc::clone(&entry));
            }
        ));

        self.results.widget().set_visible(false);
        self.search.set_sensitive(false);
        self.categories.widget().set_sensitive(false);
        self.root.append(&confirmation);
        self.confirmation.replace(Some(confirmation));
        cancel.grab_focus();
    }

    pub(super) fn cancel_confirmation(&self) -> bool {
        let Some(confirmation) = self.confirmation.take() else {
            return false;
        };

        self.root.remove(&confirmation);
        self.results.widget().set_visible(true);
        self.search.set_sensitive(true);
        self.categories.widget().set_sensitive(true);

        true
    }

    fn run(self: &Rc<Self>, entry: Rc<Entry>) {
        let context = self.root.display().app_launch_context();
        let query = self.search.text();

        // Release the layer surface's keyboard grab before polkit can ask for
        // authentication.
        self.root.set_sensitive(false);
        self.root.activate_action("win.hide", None).unwrap();

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = page)]
            self,
            async move {
                let result = page.launcher.launch(&entry, &query, &context).await;
                page.root.set_sensitive(true);

                if let Err(reason) = result {
                    error!(error = %reason, entry = %entry.title, "Could not launch entry");
                    page.error
                        .set_text(&format!("Could not run {}: {reason}", entry.title));
                    page.error.set_visible(true);
                    page.root.activate_action("win.show", None).unwrap();
                    page.search.grab_focus();
                }
            }
        ));
    }
}
