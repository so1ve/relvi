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

fn decode_pixbuf(
    bytes: &[u8],
    max_size: Option<i32>,
) -> Result<(gdk_pixbuf::Pixbuf, gdk_pixbuf::PixbufFormat, i32, i32), Box<dyn Error>> {
    let loader = gdk_pixbuf::PixbufLoader::new();
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
            if let Some(max_size) = max_size
                && width.max(height) > max_size
            {
                let longest = width.max(height);
                loader.set_size(
                    (i64::from(width) * i64::from(max_size) / i64::from(longest)).max(1) as i32,
                    (i64::from(height) * i64::from(max_size) / i64::from(longest)).max(1) as i32,
                );
            }
        }
    });

    let decoded = loader.write(bytes);
    let closed = loader.close();
    let dimensions = dimensions.get().transpose()?;
    decoded?;
    closed?;
    let (width, height) = dimensions.unwrap();

    let pixbuf = loader.pixbuf().ok_or("image has no pixels")?;
    let pixbuf = pixbuf
        .apply_embedded_orientation()
        .ok_or("could not orient image")?;
    let format = loader.format().ok_or("unknown image format")?;

    Ok((pixbuf, format, width, height))
}

impl Image {
    pub fn decode(bytes: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        let (pixbuf, format, width, height) = decode_pixbuf(&bytes, Some(PREVIEW_SIZE))?;

        Ok(Self {
            bytes: glib::Bytes::from_owned(bytes),
            mime_type: format.mime_types()[0].to_string(),
            texture: gdk::Texture::for_pixbuf(&pixbuf),
            width,
            height,
        })
    }

    pub fn full_texture(&self) -> Result<gdk::Texture, Box<dyn Error>> {
        if self.width.max(self.height) <= PREVIEW_SIZE {
            return Ok(self.texture.clone());
        }

        let (pixbuf, ..) = decode_pixbuf(&self.bytes, None)?;

        Ok(gdk::Texture::for_pixbuf(&pixbuf))
    }

    pub fn memory_size(&self) -> usize {
        self.bytes.len() + self.texture.width() as usize * self.texture.height() as usize * 4
    }
}
