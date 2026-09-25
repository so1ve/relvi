use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

use crate::catalog::{Catalog, Entry};
use crate::history::History;

pub struct Applications {
    catalog: Catalog,
    history: History,
}

impl Applications {
    pub fn new() -> Self {
        Self {
            catalog: Catalog::load(),
            history: History::load(),
        }
    }

    pub fn search(&self, query: &str) -> Vec<Rc<Entry>> {
        let mut entries = self.catalog.search(query);
        self.history.sort(&mut entries, query);
        entries
    }

    pub fn refresh(&mut self) {
        self.catalog = Catalog::load();
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn launch(
        &mut self,
        entry: &Entry,
        query: &str,
        context: &impl IsA<gio::AppLaunchContext>,
    ) -> Result<(), String> {
        entry
            .launch(context)
            .map_err(|reason| format!("Could not launch {}: {reason}", entry.title()))?;

        if let Some(id) = entry.id() {
            self.history.record(id, query);
        }

        Ok(())
    }
}
