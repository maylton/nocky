use super::{page::SettingsPage as BaseSettingsPage, stream_sources};
use crate::{config::AppConfig, dialogs::SettingsEvent};
use adw::prelude::*;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
};

pub(crate) struct SettingsPage {
    root: gtk::Box,
    base: Rc<BaseSettingsPage>,
    config: RefCell<AppConfig>,
    sender: Sender<SettingsEvent>,
    receiver: Receiver<SettingsEvent>,
    stream_summary: gtk::Label,
}

impl SettingsPage {
    pub(crate) fn new(initial: &AppConfig, noctalia_available: bool) -> Rc<Self> {
        let base = BaseSettingsPage::new(initial, noctalia_available);
        let (sender, receiver) = mpsc::channel();
        let (entry, button, stream_summary) = stream_sources::entry_row(
            &initial.youtube_stream_sources,
            initial.language,
        );

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_vexpand(true);
        root.set_hexpand(true);
        root.append(&entry);
        root.append(base.root());

        let page = Rc::new(Self {
            root,
            base,
            config: RefCell::new(initial.clone()),
            sender,
            receiver,
            stream_summary,
        });

        {
            let weak = Rc::downgrade(&page);
            button.connect_clicked(move |button| {
                let Some(page) = weak.upgrade() else {
                    return;
                };
                let Some(root) = button.root() else {
                    return;
                };
                let Ok(parent) = root.downcast::<adw::ApplicationWindow>() else {
                    return;
                };
                let config = page.config.borrow().clone();
                stream_sources::present_dialog(
                    &parent,
                    config.youtube_stream_sources,
                    config.language,
                    page.sender.clone(),
                );
            });
        }

        page
    }

    pub(crate) fn root(&self) -> &gtk::Box {
        &self.root
    }

    pub(crate) fn try_recv(&self) -> Option<SettingsEvent> {
        self.base
            .try_recv()
            .or_else(|| self.receiver.try_recv().ok())
    }

    pub(crate) fn rebuild(&self, initial: &AppConfig, noctalia_available: bool) {
        self.config.replace(initial.clone());
        self.stream_summary
            .set_text(&initial.youtube_stream_sources.effective_order_csv());
        self.base.rebuild(initial, noctalia_available);
    }
}
