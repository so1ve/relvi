use std::cell::RefCell;
use std::fmt::Write;
use std::path::Path;
use std::rc::Rc;

use gtk::gio::prelude::*;
use gtk::glib::variant::ToVariant;
use gtk::{gio, glib};
use tracing::warn;

pub fn is_systemd() -> bool {
    std::fs::read_to_string("/proc/self/cgroup").is_ok_and(|cgroup| {
        cgroup.lines().any(|line| {
            line.split('/')
                .any(|part| part.starts_with("user@") && part.ends_with(".service"))
        })
    })
}

pub async fn launch(
    app: &gio::AppInfo,
    context: &impl IsA<gio::AppLaunchContext>,
) -> Result<(), glib::Error> {
    let pids = Rc::new(RefCell::new(Vec::new()));
    let handler = context.connect_launched(glib::clone!(
        #[strong]
        pids,
        move |_, _, platform_data| {
            if let Ok(Some(pid)) = glib::VariantDict::new(Some(platform_data)).lookup::<i32>("pid")
                && pid > 0
            {
                pids.borrow_mut().push(pid.cast_unsigned());
            }
        }
    ));

    let result = app.launch(&[], Some(context));
    context.disconnect(handler);
    result?;

    let id = app.id().map_or_else(
        || {
            Path::new(&app.executable())
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        },
        |id| id.trim_end_matches(".desktop").to_owned(),
    );

    for pid in pids.take() {
        if let Err(error) = start_scope(scope_name(&id, pid), &app.display_name(), pid).await {
            warn!(%error, pid, "Could not move application into its own scope");
        }
    }

    Ok(())
}

async fn start_scope(unit: String, description: &str, pid: u32) -> Result<(), glib::Error> {
    let connection = gio::bus_get_future(gio::BusType::Session).await?;

    let properties = vec![
        ("Description", description.to_variant()),
        ("PIDs", vec![pid].to_variant()),
        ("Slice", "app.slice".to_variant()),
        ("CollectMode", "inactive-or-failed".to_variant()),
    ];
    let auxiliary: Vec<(&str, Vec<(&str, glib::Variant)>)> = Vec::new();

    connection
        .call_future(
            Some("org.freedesktop.systemd1"),
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "StartTransientUnit",
            Some(&(unit, "fail", properties, auxiliary).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
        )
        .await
        .map(|_| ())
        .map_err(|mut error| {
            gio::DBusError::strip_remote_error(&mut error);

            error
        })
}

fn scope_name(app_id: &str, pid: u32) -> String {
    let mut name = String::from("app-relvi-");

    for byte in app_id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'.') {
            name.push(char::from(byte));
        } else {
            write!(name, "\\x{byte:02x}").unwrap();
        }
    }

    write!(name, "-{pid}.scope").unwrap();

    name
}
