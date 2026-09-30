use std::cell::Cell;
use std::error::Error;
use std::rc::Rc;

use gtk::gdk::prelude::*;
use gtk::{gdk, gdk_pixbuf, glib};

const MAX_IMAGE_PIXELS: i64 = 64 * 1024 * 1024;
const PREVIEW_SIZE: i32 = 1024;

pub struct Image {
    pub bytes: glib::Bytes,
    pub mime_type: String,
    pub texture: gdk::Texture,
    pub width: i32,
    pub height: i32,
}

impl Image {
    pub fn decode(mime_type: &str, bytes: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        if !matches!(mime_type, "image/png" | "image/jpeg") {
            return Err("unsupported image format".into());
        }

        let loader = gdk_pixbuf::PixbufLoader::with_mime_type(mime_type)?;
        let dimensions = Rc::new(Cell::new(None));
        loader.connect_size_prepared({
            let dimensions = Rc::clone(&dimensions);

            move |loader, width, height| {
                // PNG allocates the full image before scaling it. A zero size
                // stops PNG/JPEG decoding before that allocation.
                if i64::from(width) * i64::from(height) > MAX_IMAGE_PIXELS {
                    dimensions.set(Some(Err("image exceeds 64 megapixels")));
                    loader.set_size(0, 0);

                    return;
                }

                dimensions.set(Some(Ok((width, height))));
                let longest = width.max(height).max(PREVIEW_SIZE);
                loader.set_size(
                    (i64::from(width) * i64::from(PREVIEW_SIZE) / i64::from(longest)).max(1) as i32,
                    (i64::from(height) * i64::from(PREVIEW_SIZE) / i64::from(longest)).max(1)
                        as i32,
                );
            }
        });

        let decoded = loader.write(&bytes);
        let closed = loader.close();
        let dimensions = dimensions.get().transpose()?;
        decoded?;
        closed?;
        let (width, height) = dimensions.unwrap();

        let pixbuf = loader.pixbuf().ok_or("image has no pixels")?;
        let pixbuf = pixbuf
            .apply_embedded_orientation()
            .ok_or("could not orient image")?;

        Ok(Self {
            bytes: glib::Bytes::from_owned(bytes),
            mime_type: mime_type.to_owned(),
            texture: gdk::Texture::for_pixbuf(&pixbuf),
            width,
            height,
        })
    }

    pub fn memory_size(&self) -> usize {
        self.bytes.len() + self.texture.width() as usize * self.texture.height() as usize * 4
    }
}
