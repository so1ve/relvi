mod clipboard;
mod emoji;
mod launcher;

use gtk::Widget;

pub use self::clipboard::ClipboardPage;
pub use self::emoji::EmojiPage;
pub use self::launcher::LauncherPage;

pub trait Page {
    fn widget(&self) -> &Widget;
    fn width(&self) -> i32;
    fn present(&self);
}
