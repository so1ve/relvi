#[cfg(feature = "systemd")]
mod systemd;

use gtk::gio::prelude::*;
use gtk::{gio, glib};

pub async fn launch(
    app: &gio::AppInfo,
    context: &impl IsA<gio::AppLaunchContext>,
) -> Result<(), glib::Error> {
    context.unsetenv("NOTIFY_SOCKET");
    context.unsetenv("INVOCATION_ID");

    #[cfg(feature = "systemd")]
    if systemd::is_systemd() {
        return systemd::launch(app, context).await;
    }

    app.launch(&[], Some(context))
}
