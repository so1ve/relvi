use std::cell::{Ref, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::{gio, glib};

use super::{Entry, History, watch};

enum State {
    Loading,
    Cleared,
    Ready(History),
}

pub enum Change {
    Entries,
    Error(String),
}

type Changed = Box<dyn Fn(Change)>;

pub struct Session {
    state: RefCell<State>,
    changed: RefCell<Option<Changed>>,
}

impl Session {
    pub fn new() -> Rc<Self> {
        let session = Rc::new(Self {
            state: RefCell::new(State::Loading),
            changed: RefCell::new(None),
        });
        let weak = Rc::downgrade(&session);
        glib::spawn_future_local(async move {
            let mut history = gio::spawn_blocking(History::load).await.unwrap();
            {
                let Some(session) = weak.upgrade() else {
                    return;
                };
                if matches!(*session.state.borrow(), State::Cleared) {
                    history.clear();
                }
                session.state.replace(State::Ready(history));
                session.changed.borrow().as_ref().unwrap()(Change::Entries);
            }

            let events = watch::watch();
            while let Ok(event) = events.recv().await {
                let Some(session) = weak.upgrade() else {
                    break;
                };

                match event {
                    Ok(entry) => session.record(entry),
                    Err(error) => session.changed.borrow().as_ref().unwrap()(Change::Error(error)),
                }
            }
        });

        session
    }

    pub fn search(&self, query: &str) -> (usize, impl Iterator<Item = Arc<Entry>> + '_) {
        let entries = Ref::map(self.state.borrow(), |state| match state {
            State::Ready(history) => history.entries(),
            State::Loading | State::Cleared => &[],
        });
        let total = entries.len();
        let query = query.to_lowercase();
        let matches = (0..total).filter_map(move |index| {
            let entry = &entries[index];

            query
                .split_whitespace()
                .all(|term| entry.searchable.contains(term))
                .then(|| Arc::clone(entry))
        });

        (total, matches)
    }

    pub fn record(&self, entry: Arc<Entry>) {
        let changed = {
            let mut state = self.state.borrow_mut();
            let State::Ready(history) = &mut *state else {
                unreachable!("clipboard entries are only available after loading");
            };

            history.record(entry)
        };
        if changed {
            self.changed.borrow().as_ref().unwrap()(Change::Entries);
        }
    }

    pub fn remove(&self, id: &str) {
        {
            let mut state = self.state.borrow_mut();
            let State::Ready(history) = &mut *state else {
                unreachable!("clipboard entries are only available after loading");
            };
            history.remove(id);
        }
        self.changed.borrow().as_ref().unwrap()(Change::Entries);
    }

    pub fn clear(&self) {
        {
            let mut state = self.state.borrow_mut();
            match &mut *state {
                State::Ready(history) => history.clear(),
                State::Loading | State::Cleared => *state = State::Cleared,
            }
        }
        self.changed.borrow().as_ref().unwrap()(Change::Entries);
    }

    pub fn connect_changed(&self, changed: impl Fn(Change) + 'static) {
        self.changed.replace(Some(Box::new(changed)));
    }
}
