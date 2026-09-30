mod clipboard;
mod writer;

use std::error::Error;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

pub use self::clipboard::ClipboardStore;
use self::writer::Writer;

fn write_file(path: &Path, contents: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(directory) = path.parent() {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
    }

    glib::file_set_contents_full(
        path,
        contents,
        glib::FileSetContentsFlags::CONSISTENT,
        0o600,
    )?;

    Ok(())
}

fn load_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map(Some)
            .map_err(Into::into),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    write_file(path, &serde_json::to_vec(value)?)
}

pub struct JsonStore<T> {
    path: PathBuf,
    writer: Writer<T>,
}

impl<T: Serialize + Send + 'static> JsonStore<T> {
    pub fn new(path: PathBuf) -> Self {
        Self {
            writer: Writer::new(path.clone(), save_json),
            path,
        }
    }

    pub fn load(&self) -> io::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        load_json(&self.path)
    }

    pub fn save(&mut self, value: T) {
        self.writer.save(value);
    }
}
