#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-collection-page-actions.py")
text = SOURCE.read_text(encoding="utf-8")

old = '''        let sender = event_tx.clone();
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
new = '''        actions.append(&button);'''

count = text.count(old)
if count != 1:
    raise SystemExit(
        f"Expected one offline action compatibility block in {SOURCE}, found {count}."
    )

text = text.replace(old, new, 1)
namespace = {
    "__name__": "__main__",
    "__file__": str(SOURCE),
}
exec(compile(text, str(SOURCE), "exec"), namespace)
