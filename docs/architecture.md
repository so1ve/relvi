# Relvi lifecycle

Relvi keeps one GTK application process and one launcher window alive. Activating
it again presents the existing window and focuses the search field; dismissing
the launcher hides the window instead of destroying it.

The first activation of the primary process builds `Catalog` from
`gio::AppInfo::all()`. Display metadata and the original Gio launch handles are
retained in an immutable in-memory snapshot owned by the UI, and polysearch builds
its index from the same snapshot. An empty Gio result remains empty and is
shown through the normal empty state. Search only traverses that snapshot, so
activation and keystrokes do not read desktop files or query Gio again. A second
process started through the same application ID is forwarded to the existing
process and never builds another snapshot.

A retained `gio::AppInfoMonitor` schedules an idle refresh when applications change.
`AppInfo::all()` rearms the monitor; the new snapshot replaces the old one on the
GTK main thread, including while the launcher is hidden. The current query is
reapplied and the selected desktop ID is preserved when it still matches. That
refresh is separate from activation and search; the hot path remains a presentation
plus an in-memory polysearch query.
Polysearch does not truncate candidates by default. Relvi requests up to the
snapshot's application count, so matching applications remain reachable by scrolling.

`src/ui` owns the launcher UI. `ui::Ui` owns the layer-shell
window and a `gtk::Stack`; the stack currently contains the applications palette
and is the extension point for future views. `Palette` connects input, activation,
and catalog notifications during construction. `Applications` owns result rendering
and selection. The row pool grows or shrinks
when a snapshot is replaced; query results only update those existing rows, so
typing does not allocate or destroy GTK widgets. The overlay fixes the panel width and uses
the initial full result height to place it at screen center, so filtering only
changes the bottom edge and leaves the search anchor in place.

The result viewport fits up to nine complete rows without limiting result count.
Names and descriptions wrap at the available width rather than ellipsizing. The
height limit sums the first nine visible rows' GTK measurements, including CSS
margins. It is recalculated when results or the viewport width change. GTK scrolls
selected rows into view; outer spacing keeps rows and the scrollbar clear of the
panel border.

Search indexes the display name, additional standard names, generic name, desktop
keywords, description, desktop ID, and executable basename with their respective
polysearch roles. Display names still follow Gio's locale selection. Empty and
duplicate fields are omitted; the generic UI description is not indexed.

SearchEntry activation (Enter after IME processing) and single-clicking a result
share the same launch path. Gio handles desktop Exec expansion, terminal requests,
and D-Bus activation. A GDK launch context supplies desktop activation information;
the launcher hides only after Gio accepts the launch. Errors stay in the palette
and clear on another query or activation. Rows hold references to entries in their
snapshot, so refreshing the catalog cannot redirect a pending launch to another app.
