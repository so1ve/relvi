mod applications;
mod catalog;
mod history;
mod system;

use std::cell::{Ref, RefCell};
use std::rc::Rc;

use gtk::gio::prelude::*;
use gtk::{gio, glib};

use self::catalog::Catalog;
pub use self::history::History;
use self::system::SystemAction;

enum Target {
    Application(gio::AppInfo),
    SystemAction(&'static SystemAction),
}

pub struct Entry {
    target: Target,
    pub id: Option<glib::GString>,
    pub title: glib::GString,
    pub subtitle: Option<glib::GString>,
    pub icon: Option<gio::Icon>,
    categories: Vec<&'static str>,
}

impl Entry {
    pub const fn confirmation(&self) -> Option<&'static str> {
        match &self.target {
            Target::Application(_) => None,
            Target::SystemAction(action) => action.confirmation,
        }
    }
}

pub enum Change {
    Catalog,
    History,
}

type Changed = Box<dyn Fn(Change)>;

pub struct Launcher {
    catalog: RefCell<Catalog>,
    history: RefCell<History>,
    monitor: gio::AppInfoMonitor,
    changed: RefCell<Option<Changed>>,
}

impl Launcher {
    pub fn new() -> Rc<Self> {
        let launcher = Rc::new(Self {
            catalog: RefCell::new(Catalog::load()),
            history: RefCell::new(History::load()),
            monitor: gio::AppInfoMonitor::get(),
            changed: RefCell::new(None),
        });
        launcher.monitor.connect_changed(glib::clone!(
            #[weak]
            launcher,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    launcher,
                    move || {
                        // Scanning also rearms the desktop application monitor.
                        launcher.catalog.replace(Catalog::load());
                        launcher.changed.borrow().as_ref().unwrap()(Change::Catalog);
                    }
                ));
            }
        ));

        launcher
    }

    pub fn entries(&self) -> Ref<'_, [Rc<Entry>]> {
        Ref::map(self.catalog.borrow(), Catalog::entries)
    }

    pub fn categories(&self) -> Vec<&'static str> {
        self.catalog.borrow().categories()
    }

    pub fn search(&self, query: &str, category: Option<&str>) -> Vec<usize> {
        self.catalog
            .borrow()
            .search(query, category, &self.history.borrow())
    }

    pub async fn launch(
        &self,
        entry: &Entry,
        query: &str,
        context: &impl IsA<gio::AppLaunchContext>,
    ) -> Result<(), glib::Error> {
        match &entry.target {
            Target::Application(app) => app.launch(&[], Some(context)),
            Target::SystemAction(action) => action.run().await,
        }?;

        if let Some(id) = entry.id.as_deref() {
            self.history.borrow_mut().record(id, query);
        }

        Ok(())
    }

    pub fn clear_history(&self) {
        self.history.borrow_mut().clear();
        self.changed.borrow().as_ref().unwrap()(Change::History);
    }

    pub fn connect_changed(&self, changed: impl Fn(Change) + 'static) {
        self.changed.replace(Some(Box::new(changed)));
    }
}
