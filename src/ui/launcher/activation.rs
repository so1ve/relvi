use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Label, Orientation, glib};

use super::LauncherView;
use crate::catalog::{Entry, Target};

impl LauncherView {
    pub fn activate(self: &Rc<Self>, entry: Rc<Entry>) {
        self.error.set_visible(false);

        let question = match &entry.target {
            Target::Application(_) => None,
            Target::SystemAction(action) => action.confirmation,
        };
        let Some(question) = question else {
            self.run(entry);

            return;
        };

        let label = Label::new(Some(question));
        label.set_xalign(0.0);
        label.set_hexpand(true);

        let cancel = Button::with_label("Cancel");
        let accept = Button::with_label(&entry.title);
        accept.add_css_class("destructive-action");

        let confirmation = GtkBox::new(Orientation::Horizontal, 8);
        confirmation.add_css_class("confirmation");
        confirmation.append(&label);
        confirmation.append(&cancel);
        confirmation.append(&accept);

        cancel.connect_clicked(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_| {
                view.cancel_confirmation();
            }
        ));
        accept.connect_clicked(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_| {
                view.cancel_confirmation();
                view.run(Rc::clone(&entry));
            }
        ));

        self.results.widget().set_visible(false);
        self.search.set_sensitive(false);
        self.categories.widget().set_sensitive(false);
        self.root.append(&confirmation);
        self.confirmation.replace(Some(confirmation));
        cancel.grab_focus();
    }

    pub fn cancel_confirmation(&self) -> bool {
        let Some(confirmation) = self.confirmation.take() else {
            return false;
        };

        self.root.remove(&confirmation);
        self.results.widget().set_visible(true);
        self.search.set_sensitive(true);
        self.categories.widget().set_sensitive(true);
        self.search.grab_focus();

        true
    }

    fn run(self: &Rc<Self>, entry: Rc<Entry>) {
        let context = self.root.display().app_launch_context();
        let query = self.search.text();

        // Release the layer surface's keyboard grab before polkit can ask for
        // authentication.
        self.root.set_sensitive(false);
        self.window.set_visible(false);

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = view)]
            self,
            async move {
                let result = match &entry.target {
                    Target::Application(app) => app.launch(&[], Some(&context)),
                    Target::SystemAction(action) => action.run().await,
                };
                view.root.set_sensitive(true);

                match result {
                    Ok(()) => {
                        if let Some(id) = entry.id.as_deref() {
                            view.history.borrow_mut().record(id, &query);
                        }
                    }
                    Err(reason) => {
                        view.error
                            .set_text(&format!("Could not run {}: {reason}", entry.title));
                        view.error.set_visible(true);
                        view.window.present();
                        view.search.grab_focus();
                    }
                }
            }
        ));
    }
}
