#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-collection-page-actions.py")
text = SOURCE.read_text(encoding="utf-8")

old_offline = '''        let sender = event_tx.clone();
        let popover_for_click = popover.clone();
        button.connect_clicked(move |button| {
            button.set_sensitive(false);
            button.add_css_class("material-card-menu-action-loading");
            set_home_offline_menu_content(
                button,
                "emblem-synchronizing-symbolic",
                match language {
                    AppLanguage::Portuguese => "Preparando download…",
                    AppLanguage::English => "Preparing download…",
                    AppLanguage::Spanish => "Preparando descarga…",
                },
            );
            popover_for_click.popdown();
            let _ = sender.send(offline_event.clone());
        });
        actions.append(&button);'''
new_offline = '''        actions.append(&button);'''

old_playlist_helper = '''fn playlist_row_with_actions(
    cover_path: Option<&Path>,'''
new_playlist_helper = '''#[expect(
    clippy::too_many_arguments,
    reason = "Playlist action rows keep navigation, playback and source state explicit"
)]
fn playlist_row_with_actions(
    cover_path: Option<&Path>,'''

for old, new, label in [
    (old_offline, new_offline, "offline single dispatch"),
    (old_playlist_helper, new_playlist_helper, "playlist helper Clippy contract"),
]:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"Expected one {label} block in {SOURCE}, found {count}.")
    text = text.replace(old, new, 1)

namespace = {
    "__name__": "__main__",
    "__file__": str(SOURCE),
}
exec(compile(text, str(SOURCE), "exec"), namespace)
