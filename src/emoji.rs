mod catalog;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emojis::{Emoji, Group, SkinTone};
use gtk::{gio, glib};

use self::catalog::Catalog;

pub const SKIN_TONES: [SkinTone; 6] = [
    SkinTone::Default,
    SkinTone::Light,
    SkinTone::MediumLight,
    SkinTone::Medium,
    SkinTone::MediumDark,
    SkinTone::Dark,
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category {
    All,
    Recent,
    Group(Group),
}

struct Query {
    input: String,
    category: Category,
}

type Results = Box<dyn Fn(Vec<&'static Emoji>)>;

pub struct Picker {
    catalog: RefCell<Option<Catalog>>,
    queries: async_channel::Sender<Query>,
    pending: Cell<bool>,
    results: RefCell<Option<Results>>,
}

impl Picker {
    pub fn new() -> Rc<Self> {
        let (queries, requests) = async_channel::bounded::<Query>(1);
        let picker = Rc::new(Self {
            catalog: RefCell::new(None),
            queries,
            pending: Cell::new(false),
            results: RefCell::new(None),
        });

        let weak = Rc::downgrade(&picker);
        glib::spawn_future_local(async move {
            let Ok(mut query) = requests.recv().await else {
                return;
            };
            let catalog = gio::spawn_blocking(Catalog::load).await.unwrap();
            if let Some(picker) = weak.upgrade() {
                picker.catalog.replace(Some(catalog));
            } else {
                return;
            }

            loop {
                while let Ok(latest) = requests.try_recv() {
                    query = latest;
                }

                let search = {
                    let Some(picker) = weak.upgrade() else {
                        break;
                    };

                    picker
                        .catalog
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .search(query.input, query.category)
                };
                let matches = search.await.unwrap();

                // Only the newest query may replace the visible results.
                if requests.is_empty() {
                    let Some(picker) = weak.upgrade() else {
                        break;
                    };
                    picker.pending.set(false);
                    picker.results.borrow().as_ref().unwrap()(matches);
                }

                let Ok(next) = requests.recv().await else {
                    break;
                };
                query = next;
            }
        });

        picker
    }

    pub fn search(&self, input: &str, category: Category) {
        self.pending.set(true);
        self.queries
            .force_send(Query {
                input: input.to_owned(),
                category,
            })
            .unwrap();
    }

    pub const fn is_pending(&self) -> bool {
        self.pending.get()
    }

    pub fn tone(&self) -> usize {
        self.catalog.borrow().as_ref().unwrap().tone()
    }

    pub fn cycle_tone(&self, offset: i32) {
        self.catalog
            .borrow_mut()
            .as_mut()
            .unwrap()
            .cycle_tone(offset);
    }

    pub fn record(&self, emoji: &Emoji) {
        self.catalog.borrow_mut().as_mut().unwrap().record(emoji);
    }

    pub fn connect_results(&self, results: impl Fn(Vec<&'static Emoji>) + 'static) {
        self.results.replace(Some(Box::new(results)));
    }
}
