use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use serde::Serialize;
use serde::de::DeserializeOwned;

pub struct JsonStore<T> {
    path: PathBuf,
    updates: Option<Sender<T>>,
    writer: Option<JoinHandle<()>>,
}

impl<T> JsonStore<T>
where
    T: Serialize + DeserializeOwned + Send + 'static,
{
    pub fn new(path: PathBuf) -> Self {
        let (updates, pending) = mpsc::channel();

        let writer = {
            let path = path.clone();

            thread::spawn(move || {
                while let Ok(mut value) = pending.recv() {
                    while let Ok(newer) = pending.try_recv() {
                        value = newer;
                    }

                    let contents = match serde_json::to_vec(&value) {
                        Ok(contents) => contents,
                        Err(error) => {
                            eprintln!("Could not serialize {}: {error}", path.display());

                            continue;
                        }
                    };

                    if let Some(directory) = path.parent()
                        && let Err(error) = DirBuilder::new()
                            .recursive(true)
                            .mode(0o700)
                            .create(directory)
                    {
                        eprintln!("Could not create {}: {error}", directory.display());

                        continue;
                    }

                    if let Err(error) = glib::file_set_contents_full(
                        &path,
                        &contents,
                        glib::FileSetContentsFlags::CONSISTENT,
                        0o600,
                    ) {
                        eprintln!("Could not save {}: {error}", path.display());
                    }
                }
            })
        };

        Self {
            path,
            updates: Some(updates),
            writer: Some(writer),
        }
    }

    pub fn load(&self) -> io::Result<Option<T>> {
        match fs::read(&self.path) {
            Ok(contents) => serde_json::from_slice(&contents)
                .map(Some)
                .map_err(Into::into),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn save(&mut self, value: T) {
        self.updates.as_ref().unwrap().send(value).unwrap();
    }
}

impl<T> Drop for JsonStore<T> {
    fn drop(&mut self) {
        drop(self.updates.take());

        if let Some(writer) = self.writer.take() {
            // ensure all pending updates are flushed before dropping the
            // writer thread
            let _ = writer.join();
        }
    }
}
