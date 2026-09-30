macro_rules! keybindings {
    (
        $key:expr, $modifiers:expr;
        $(
            $($first:ident $(+ $rest:ident)*)|+ $(if $condition:expr)? => $action:expr,
        )*
        _ => $fallback:expr $(,)?
    ) => {{
        let mut modifiers = $modifiers & gtk::accelerator_get_default_mod_mask();
        let key = match $key {
            gtk::gdk::Key::ISO_Left_Tab => {
                modifiers |= gtk::gdk::ModifierType::SHIFT_MASK;

                gtk::gdk::Key::Tab
            }
            key => key.to_lower(),
        };

        match () {
            $(
                () if ($(
                    key == $crate::ui::keybindings::keybindings!(@key $first $(+ $rest)*)
                        && modifiers == $crate::ui::keybindings::keybindings!(@modifiers $first $(+ $rest)*)
                )||+) $(&& $condition)? => $action,
            )*
            () => $fallback,
        }
    }};
    (@key $key:ident) => {
        gtk::gdk::Key::$key.to_lower()
    };
    (@key $modifier:ident + $($rest:tt)+) => {
        $crate::ui::keybindings::keybindings!(@key $($rest)+)
    };
    (@modifiers $key:ident) => {
        gtk::gdk::ModifierType::empty()
    };
    (@modifiers Ctrl + $($rest:tt)+) => {
        gtk::gdk::ModifierType::CONTROL_MASK
            | $crate::ui::keybindings::keybindings!(@modifiers $($rest)+)
    };
    (@modifiers Shift + $($rest:tt)+) => {
        gtk::gdk::ModifierType::SHIFT_MASK
            | $crate::ui::keybindings::keybindings!(@modifiers $($rest)+)
    };
    (@modifiers Alt + $($rest:tt)+) => {
        gtk::gdk::ModifierType::ALT_MASK
            | $crate::ui::keybindings::keybindings!(@modifiers $($rest)+)
    };
    (@modifiers Super + $($rest:tt)+) => {
        gtk::gdk::ModifierType::SUPER_MASK
            | $crate::ui::keybindings::keybindings!(@modifiers $($rest)+)
    };
}

pub(super) use keybindings;
