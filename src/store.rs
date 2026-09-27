use std::error::Error;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use serde::Serialize;
use serde::de::DeserializeOwned;

pub struct JsonStore<T> {
    path: PathBuf,
    writer: Option<(Sender<T>, JoinHandle<()>)>,
}

impl<T> JsonStore<T> {
    pub const fn new(path: PathBuf) -> Self {
        Self { path, writer: None }
    }

    pub fn load(&self) -> io::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        match fs::read(&self.path) {
            Ok(contents) => serde_json::from_slice(&contents)
                .map(Some)
                .map_err(Into::into),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn save(&mut self, value: T)
    where
        T: Serialize + Send + 'static,
    {
        let (updates, _) = self.writer.get_or_insert_with(|| {
            let path = self.path.clone();
            let (updates, pending) = mpsc::channel();
            let writer = thread::spawn(move || {
                while let Ok(mut value) = pending.recv() {
                    while let Ok(newer) = pending.try_recv() {
                        value = newer;
                    }

                    if let Err(error) = write_json(&path, &value) {
                        eprintln!("Could not save {}: {error}", path.display());
                    }
                }
            });

            (updates, writer)
        });
        updates.send(value).unwrap();
    }
}

impl<T> Drop for JsonStore<T> {
    fn drop(&mut self) {
        if let Some((updates, writer)) = self.writer.take() {
            drop(updates);
            // Closing the queue drains pending writes before exit
            writer.join().unwrap();
        }
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    let contents = serde_json::to_vec(value)?;

    if let Some(directory) = path.parent() {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
    }

    glib::file_set_contents_full(
        path,
        &contents,
        glib::FileSetContentsFlags::CONSISTENT,
        0o600,
    )?;

    Ok(())
}
