mod catalog;
mod ui;

use std::cell::OnceCell;

use gtk::prelude::*;
use gtk::{Application, glib};

const APP_ID: &str = "io.so1ve.Relvi";

fn main() -> glib::ExitCode {
    let application = Application::builder().application_id(APP_ID).build();
    let ui = OnceCell::<ui::Ui>::new();

    application.connect_activate(move |application| {
        let instance = ui.get_or_init(|| ui::Ui::new(application));
        instance.present();
    });

    application.run()
}
