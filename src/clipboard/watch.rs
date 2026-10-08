use std::sync::Arc;
use std::thread;
use std::time::Duration;

use gtk::gdk_pixbuf::Pixbuf;
use tracing::{debug, error, warn};
use wl_clipboard_watch::{Config, Event, Transfer, Watcher};

use super::{Entry, MAX_IMAGE_BYTES};

pub fn watch() -> async_channel::Receiver<Result<Arc<Entry>, String>> {
    let (updates, events) = async_channel::bounded(8);

    thread::spawn(move || {
        let mut image_types: Vec<_> = Pixbuf::formats()
            .into_iter()
            .filter(|format| !format.is_disabled())
            .flat_map(|format| format.mime_types())
            .collect();
        image_types.sort_by_key(|mime| mime.as_str() != "image/png");

        let config = Config::new(MAX_IMAGE_BYTES, Duration::from_secs(5)).unwrap();
        let mut watcher = match Watcher::connect_with(config) {
            Ok(watcher) => watcher,
            Err(error) => {
                error!(%error, "Could not connect to clipboard");
                let _ = updates.send_blocking(Err(error.to_string()));

                return;
            }
        };

        debug!("Clipboard monitoring started");

        while !updates.is_closed() {
            let selection = match watcher.next_event() {
                Ok(Event::Selection(selection)) => selection,
                Ok(Event::Cleared) => continue,
                Err(error) => {
                    error!(%error, "Clipboard monitoring failed");
                    let _ = updates.send_blocking(Err(error.to_string()));

                    break;
                }
            };

            if selection.offers("x-kde-passwordManagerHint") {
                continue;
            }

            let offered = |preferred: &str| {
                selection
                    .mime_types()
                    .iter()
                    .find(|mime| mime.eq_ignore_ascii_case(preferred))
            };
            let image_type = image_types.iter().find_map(|mime| offered(mime));
            let mime_type = image_type.or_else(|| {
                ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"]
                    .into_iter()
                    .find_map(offered)
            });
            let Some(mime_type) = mime_type else {
                continue;
            };

            let bytes = match watcher.receive(&selection, mime_type) {
                Ok(Transfer::Complete(bytes)) => bytes,
                Ok(Transfer::Stale) => continue,
                Err(error) => {
                    warn!(%error, mime_type, "Could not read clipboard content");

                    continue;
                }
            };

            let decoded = if image_type.is_some() {
                Entry::from_image(bytes)
            } else {
                String::from_utf8(bytes)
                    .map_err(Into::into)
                    .and_then(|text| Entry::from_text(text.into()))
            };

            let entry = match decoded {
                Ok(entry) => entry,
                Err(error) => {
                    warn!(%error, mime_type, "Could not record clipboard content");

                    continue;
                }
            };

            if updates.send_blocking(Ok(Arc::new(entry))).is_err() {
                break;
            }
        }
    });

    events
}
