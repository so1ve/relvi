use std::sync::Arc;
use std::thread;
use std::time::Duration;

use wl_clipboard_watch::{Config, Event, Transfer, Watcher};

use super::{Entry, MAX_IMAGE_BYTES};

pub fn watch() -> async_channel::Receiver<Result<Arc<Entry>, String>> {
    let (updates, events) = async_channel::bounded(8);

    thread::spawn(move || {
        let config = Config::new(MAX_IMAGE_BYTES, Duration::from_secs(5)).unwrap();
        let mut watcher = match Watcher::connect_with(config) {
            Ok(watcher) => watcher,
            Err(error) => {
                let _ = updates.send_blocking(Err(error.to_string()));

                return;
            }
        };

        while !updates.is_closed() {
            let selection = match watcher.next_event() {
                Ok(Event::Selection(selection)) => selection,
                Ok(Event::Cleared) => continue,
                Err(error) => {
                    let _ = updates.send_blocking(Err(error.to_string()));
                    break;
                }
            };

            if selection.offers("x-kde-passwordManagerHint") {
                continue;
            }

            let mime_type = [
                "image/png",
                "image/jpeg",
                "text/plain;charset=utf-8",
                "UTF8_STRING",
                "text/plain",
            ]
            .into_iter()
            .find_map(|preferred| {
                selection
                    .mime_types()
                    .iter()
                    .find(|mime| mime.eq_ignore_ascii_case(preferred))
                    .map(|offered| (preferred, offered))
            });
            let Some((format, mime_type)) = mime_type else {
                continue;
            };

            let bytes = match watcher.receive(&selection, mime_type) {
                Ok(Transfer::Complete(bytes)) => bytes,
                Ok(Transfer::Stale) => continue,
                Err(error) => {
                    eprintln!("Could not read clipboard content: {error}");
                    continue;
                }
            };
            let decoded = if format.starts_with("image/") {
                Entry::from_image(format, bytes)
            } else {
                String::from_utf8(bytes)
                    .map_err(Into::into)
                    .and_then(|text| Entry::from_text(text.into()))
            };
            let entry = match decoded {
                Ok(entry) => entry,
                Err(error) => {
                    eprintln!("Could not record clipboard content: {error}");
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
