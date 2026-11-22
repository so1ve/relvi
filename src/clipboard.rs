mod history;
mod session;
mod watch;

use std::error::Error;
use std::sync::Arc;

use gtk::prelude::*;
use gtk::{gdk, glib};

pub use self::history::History;
pub use self::session::{Change, Session};
use crate::image::Image;

pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const INTERNAL_MIME_TYPE: &str = "application/x-relvi-internal";

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

        let preview = text.trim().chars().take(160).collect();
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

    pub fn from_image(bytes: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("clipboard image exceeds 16 MiB".into());
        }

        let image = Image::decode(bytes)?;
        let id = glib::compute_checksum_for_bytes(glib::ChecksumType::Sha256, &image.bytes)
            .unwrap()
            .to_string();
        let preview = format!("Image · {} × {}", image.width, image.height);
        let searchable = format!("{preview} {}", image.mime_type).to_lowercase();

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

fn internal_marker() -> gdk::ContentProvider {
    gdk::ContentProvider::for_bytes(INTERNAL_MIME_TYPE, &glib::Bytes::from_static(&[]))
}

fn content_provider(content: &Content) -> Result<gdk::ContentProvider, Box<dyn Error>> {
    Ok(match content {
        Content::Text(text) => gdk::ContentProvider::new_union(&[
            gdk::ContentProvider::for_value(&text.as_ref().to_value()),
            internal_marker(),
        ]),
        Content::Image(image) => {
            let raw = gdk::ContentProvider::for_bytes(&image.mime_type, &image.bytes);
            if image.mime_type.eq_ignore_ascii_case("image/png") {
                gdk::ContentProvider::new_union(&[raw, internal_marker()])
            } else {
                let texture = image.full_texture()?;
                gdk::ContentProvider::new_union(&[
                    raw,
                    gdk::ContentProvider::for_value(&texture.to_value()),
                    internal_marker(),
                ])
            }
        }
    })
}

pub fn copy(content: &Content, clipboard: &gdk::Clipboard) -> Result<(), Box<dyn Error>> {
    let provider = content_provider(content)?;

    clipboard.set_content(Some(&provider))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
    use gtk::gio;

    use super::*;

    #[test]
    fn jpeg_provider_offers_original_and_full_res_png() {
        let pixbuf = Pixbuf::new(Colorspace::Rgb, false, 8, 1200, 800).unwrap();
        pixbuf.fill(0x3366_99ff);
        let jpeg = pixbuf.save_to_bufferv("jpeg", &[]).unwrap();

        let image = Image::decode(jpeg.clone()).unwrap();
        assert_eq!(image.mime_type, "image/jpeg");
        assert_eq!((image.width, image.height), (1200, 800));
        assert_eq!((image.texture.width(), image.texture.height()), (1024, 682));

        let provider = content_provider(&Content::Image(image)).unwrap();
        let formats = provider.formats();
        assert!(formats.contain_mime_type("image/jpeg"));
        assert!(formats.contain_mime_type(INTERNAL_MIME_TYPE));
        assert!(formats.contains_type(gdk::Texture::static_type()));

        let context = glib::MainContext::default();
        let jpeg_out = gio::MemoryOutputStream::new_resizable();
        context
            .block_on(provider.write_mime_type_future(
                "image/jpeg",
                &jpeg_out,
                glib::Priority::DEFAULT,
            ))
            .unwrap();
        jpeg_out.close(gio::Cancellable::NONE).unwrap();
        assert_eq!(jpeg_out.steal_as_bytes().as_ref(), jpeg.as_slice());

        let value = provider.value(gdk::Texture::static_type()).unwrap();
        let texture: gdk::Texture = value.get().unwrap();
        assert_eq!((texture.width(), texture.height()), (1200, 800));

        let png_bytes = texture.save_to_png_bytes();
        assert!(png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let decoded_png = Image::decode(png_bytes.to_vec()).unwrap();
        assert_eq!((decoded_png.width, decoded_png.height), (1200, 800));
    }

    #[test]
    fn png_and_text_providers_include_internal_marker() {
        let pixbuf = Pixbuf::new(Colorspace::Rgb, false, 8, 16, 16).unwrap();
        pixbuf.fill(0xff00_00ff);
        let png = pixbuf.save_to_bufferv("png", &[]).unwrap();

        let image = Image::decode(png).unwrap();
        let image_formats = content_provider(&Content::Image(image)).unwrap().formats();
        assert!(image_formats.contain_mime_type("image/png"));
        assert!(image_formats.contain_mime_type(INTERNAL_MIME_TYPE));
        assert!(!image_formats.contains_type(gdk::Texture::static_type()));

        let text_formats = content_provider(&Content::Text("hello".into()))
            .unwrap()
            .formats();
        assert!(text_formats.contains_type(String::static_type()));
        assert!(text_formats.contain_mime_type(INTERNAL_MIME_TYPE));
    }
}
