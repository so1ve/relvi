use std::error::Error;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use async_channel::Sender;
use tracing::error;

type Write<T> = fn(&Path, &T) -> Result<(), Box<dyn Error>>;

pub struct Writer<T> {
    path: PathBuf,
    write: Write<T>,
    worker: Option<(Sender<T>, JoinHandle<()>)>,
}

impl<T: Send + 'static> Writer<T> {
    pub const fn new(path: PathBuf, write: Write<T>) -> Self {
        Self {
            path,
            write,
            worker: None,
        }
    }

    pub fn save(&mut self, value: T) {
        let (updates, _) = self.worker.get_or_insert_with(|| {
            let path = self.path.clone();
            let write = self.write;
            let (updates, pending) = async_channel::bounded(1);
            let worker = thread::spawn(move || {
                while let Ok(value) = pending.recv_blocking() {
                    if let Err(error) = write(&path, &value) {
                        error!(%error, path = %path.display(), "Could not save snapshot");
                    }
                }
            });

            (updates, worker)
        });

        // Each value is a complete snapshot. replace any older pending one
        updates.force_send(value).unwrap();
    }
}

impl<T> Drop for Writer<T> {
    fn drop(&mut self) {
        if let Some((updates, worker)) = self.worker.take() {
            drop(updates);
            // Closing the queue drains pending writes before exit.
            worker.join().unwrap();
        }
    }
}
