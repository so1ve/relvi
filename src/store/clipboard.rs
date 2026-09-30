use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{Writer, load_json, save_json, write_file};
use crate::clipboard::{Content, Entry, MAX_IMAGE_BYTES};

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum StoredEntry {
    Text(Arc<str>),
    Image { image: String, mime_type: String },
}

fn is_image_name(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn read_image(directory: &Path, name: &str, mime_type: &str) -> Result<Entry, Box<dyn Error>> {
    if !is_image_name(name) {
        return Err("invalid clipboard image filename".into());
    }

    let mut bytes = Vec::new();
    File::open(directory.join(name))?
        .take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;

    Entry::from_image(mime_type, bytes)
}

fn save_entries(path: &Path, entries: &[Arc<Entry>]) -> Result<(), Box<dyn Error>> {
    let directory = path.with_extension("");
    let mut saved = Vec::with_capacity(entries.len());
    let mut images = HashSet::new();

    for entry in entries {
        saved.push(match &entry.content {
            Content::Text(text) => StoredEntry::Text(Arc::clone(text)),
            Content::Image(image) => {
                let target = directory.join(&entry.id);
                if !target.try_exists()? {
                    write_file(&target, &image.bytes)?;
                }
                images.insert(entry.id.as_str());

                StoredEntry::Image {
                    image: entry.id.clone(),
                    mime_type: image.mime_type.clone(),
                }
            }
        });
    }

    // Commit references only after their files exist; delete unused files
    // last.
    save_json(path, &saved)?;
    let files = match fs::read_dir(&directory) {
        Ok(files) => files,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for file in files {
        let file = file?;
        let name = file.file_name();
        let name = name.to_string_lossy();
        if is_image_name(&name) && !images.contains(name.as_ref()) && file.file_type()?.is_file() {
            fs::remove_file(file.path())?;
        }
    }

    Ok(())
}

pub struct ClipboardStore {
    path: PathBuf,
    writer: Writer<Vec<Arc<Entry>>>,
}

impl ClipboardStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            writer: Writer::new(path.clone(), |path, entries| save_entries(path, entries)),
            path,
        }
    }

    pub fn load(&self) -> io::Result<impl Iterator<Item = Arc<Entry>>> {
        let saved = load_json::<Vec<StoredEntry>>(&self.path)?.unwrap_or_default();
        let directory = self.path.with_extension("");

        Ok(saved.into_iter().filter_map(move |entry| {
            let entry = match entry {
                StoredEntry::Text(text) => Entry::from_text(text),
                StoredEntry::Image { image, mime_type } => {
                    read_image(&directory, &image, &mime_type)
                }
            };

            match entry {
                Ok(entry) => Some(Arc::new(entry)),
                Err(error) => {
                    eprintln!("Could not load clipboard entry: {error}");

                    None
                }
            }
        }))
    }

    pub fn save(&mut self, entries: Vec<Arc<Entry>>) {
        self.writer.save(entries);
    }
}
