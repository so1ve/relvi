mod history;
mod watch;

use std::error::Error;
use std::sync::Arc;

use gtk::{gdk, glib};

pub use self::history::History;
pub use self::watch::watch;
use crate::image::Image;

pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;

pub enum Content {
    Text(Arc<str>),
    Image(Image),
}

pub struct Entry {
    pub id: String,
    pub content: Content,
    pub preview: String,
    searchable: String,
}

impl Entry {
    pub fn from_text(text: Arc<str>) -> Result<Self, Box<dyn Error>> {
        if text.len() > MAX_TEXT_BYTES || text.trim().is_empty() || text.contains('\0') {
            return Err("invalid clipboard text".into());
        }

        let preview = text
            .split_whitespace()
            .flat_map(|word| word.chars().chain(std::iter::once(' ')))
            .take(160)
            .collect::<String>();
        let searchable = text.to_lowercase();
        let id = glib::compute_checksum_for_data(glib::ChecksumType::Sha256, text.as_bytes())
            .unwrap()
            .to_string();

        Ok(Self {
            id,
            content: Content::Text(text),
            preview,
            searchable,
        })
    }

    pub fn from_image(mime_type: &str, bytes: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("clipboard image exceeds 16 MiB".into());
        }

        let image = Image::decode(mime_type, bytes)?;
        let id = glib::compute_checksum_for_bytes(glib::ChecksumType::Sha256, &image.bytes)
            .unwrap()
            .to_string();
        let preview = format!("Image · {} × {}", image.width, image.height);
        let searchable = format!("{preview} {mime_type}").to_lowercase();

        Ok(Self {
            id,
            content: Content::Image(image),
            preview,
            searchable,
        })
    }

    fn memory_size(&self) -> usize {
        self.preview.len()
            + self.searchable.len()
            + match &self.content {
                Content::Text(text) => text.len(),
                Content::Image(image) => image.memory_size(),
            }
    }
}

pub fn copy(content: &Content, clipboard: &gdk::Clipboard) -> Result<(), glib::BoolError> {
    match content {
        Content::Text(text) => {
            clipboard.set_text(text);

            Ok(())
        }
        Content::Image(image) => {
            let content = gdk::ContentProvider::for_bytes(&image.mime_type, &image.bytes);

            clipboard.set_content(Some(&content))
        }
    }
}
