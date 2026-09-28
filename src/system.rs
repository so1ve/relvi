use gtk::glib::variant::ToVariant;
use gtk::{gio, glib};

pub struct SystemAction {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub aliases: &'static [&'static str],
    pub confirmation: Option<&'static str>,
    method: &'static str,
}

impl SystemAction {
    pub async fn run(&self) -> Result<(), glib::Error> {
        let connection = gio::bus_get_future(gio::BusType::System).await?;

        connection
            .call_future(
                Some("org.freedesktop.login1"),
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
                self.method,
                Some(&(true,).to_variant()),
                None,
                gio::DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION,
                -1,
            )
            .await
            .map(|_| ())
            .map_err(|mut error| {
                gio::DBusError::strip_remote_error(&mut error);

                error
            })
    }
}

pub const ACTIONS: [SystemAction; 3] = [
    SystemAction {
        id: "relvi:reboot",
        title: "Restart",
        icon: "system-reboot-symbolic",
        aliases: &["reboot", "重启"],
        confirmation: Some("Restart this computer?"),
        method: "Reboot",
    },
    SystemAction {
        id: "relvi:poweroff",
        title: "Power off",
        icon: "system-shutdown-symbolic",
        aliases: &["poweroff", "shutdown", "关机"],
        confirmation: Some("Power off this computer?"),
        method: "PowerOff",
    },
    SystemAction {
        id: "relvi:suspend",
        title: "Suspend",
        icon: "system-suspend-symbolic",
        aliases: &["sleep", "挂起", "睡眠"],
        confirmation: None,
        method: "Suspend",
    },
];
