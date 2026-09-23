mod catalog;
mod history;
mod ui;

use std::cell::OnceCell;

use gtk::prelude::*;
use gtk::{Application, glib};

const APP_ID: &str = "io.so1ve.Relvi";

fn main() -> glib::ExitCode {
    let application = Application::builder().application_id(APP_ID).build();
    let ui = OnceCell::<ui::Ui>::new();

    let activate = application.connect_activate(move |application| {
        let instance = ui.get_or_init(|| ui::Ui::new(application));
        instance.present();
    });

    let exit_code = application.run();

    // Release the retained UI and finish pending history writes on shutdown.
    application.disconnect(activate);

    exit_code
}
