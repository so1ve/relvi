use gtk::SearchEntry;

pub fn search_field(placeholder: &str) -> SearchEntry {
    SearchEntry::builder()
        .placeholder_text(placeholder)
        .search_delay(0)
        .hexpand(true)
        .css_classes(["search-field"])
        .build()
}
