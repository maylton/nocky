use crate::{
    config::{AppLanguage, BlurMode, FooterMode, StartupSource, VisualTheme},
    i18n::{self, Message},
};
use adw::prelude::*;
use std::rc::Rc;
fn inherit_visual_theme(parent: &adw::ApplicationWindow, widget: &impl IsA<gtk::Widget>) {
    widget.remove_css_class("theme-noctalia");
    widget.remove_css_class("theme-material-expressive");
    widget.remove_css_class("theme-frosted-glass");

    if parent.has_css_class("theme-frosted-glass") {
        widget.add_css_class("theme-material-expressive");
        widget.add_css_class("theme-frosted-glass");
    } else if parent.has_css_class("theme-material-expressive") {
        widget.add_css_class("theme-material-expressive");
    } else {
        widget.add_css_class("theme-noctalia");
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum SettingsEvent {
    Language(AppLanguage),
    StartupSource(StartupSource),
    BlurMode(BlurMode),
    BlurOpacityPreview(f64),
    BlurOpacityCommit(f64),
    ShowHomeVisualizer(bool),
    ShowHomeLyrics(bool),
    ShowPersonalizedHomeHistory(bool),
    CollectListeningHistory(bool),
    ClearListeningHistory,
    VisualTheme(VisualTheme),
    FooterMode(FooterMode),
    ExpressiveTransportEffects(bool),
    ExpressiveHomeCardEffects(bool),
    AutoDownloadLyrics(bool),
    ResumePlaybackOnStartup(bool),
    YouTubeAutoSync(bool),
    OfflineCollectionAutoSync(bool),
    NoctaliaThemeSync(bool),
    ManageYouTube,
    OpenOfflineFolder,
    CleanOfflinePartials,
    ClearOfflineDownloads,
}

pub(crate) fn present_youtube_settings<W>(parent: &adw::ApplicationWindow, root: &W) -> adw::Dialog
where
    W: IsA<gtk::Widget> + Clone + 'static,
{
    let dialog = adw::Dialog::builder()
        .title("YouTube Music")
        .content_width(760)
        .content_height(620)
        .build();
    dialog.add_css_class("youtube-settings-dialog");
    inherit_visual_theme(parent, &dialog);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_css_class("material-dialog-toolbar");
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.add_css_class("youtube-settings-host");
    host.append(root);
    toolbar.set_content(Some(&host));
    dialog.set_child(Some(&toolbar));

    let youtube_root = root.clone();
    dialog.connect_closed(move |_| {
        host.remove(&youtube_root);
    });
    dialog.present(Some(parent));
    dialog
}

pub(crate) fn present_startup_source<F>(
    parent: &adw::ApplicationWindow,
    language: AppLanguage,
    first_run: bool,
    on_select: F,
) where
    F: Fn(StartupSource) + 'static,
{
    let tr = |message| i18n::text(language, message);
    let on_select: Rc<dyn Fn(StartupSource)> = Rc::new(on_select);

    let dialog = adw::Dialog::builder()
        .title(if first_run {
            tr(Message::StartupWelcome)
        } else {
            tr(Message::StartupSourceTitle)
        })
        .content_width(480)
        .build();
    dialog.add_css_class("startup-dialog");
    inherit_visual_theme(parent, &dialog);
    dialog.set_can_close(!first_run);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.add_css_class("startup-dialog-content");
    dialog.set_child(Some(&content));
    content.set_margin_top(22);
    content.set_margin_bottom(22);
    content.set_margin_start(22);
    content.set_margin_end(22);

    let title = gtk::Label::new(Some(if first_run {
        tr(Message::StartupQuestion)
    } else {
        tr(Message::StartupChoose)
    }));
    title.set_wrap(true);
    title.set_xalign(0.0);
    title.add_css_class("title-2");
    title.add_css_class("startup-dialog-title");

    let description = gtk::Label::new(Some(tr(Message::StartupDescription)));
    description.set_wrap(true);
    description.set_xalign(0.0);
    description.add_css_class("dim-label");
    description.add_css_class("startup-dialog-description");

    let local_button = gtk::Button::with_label(tr(Message::UseLocalLibrary));
    local_button.set_tooltip_text(Some(tr(Message::UseLocalLibraryTooltip)));
    local_button.add_css_class("source-choice-button");

    let youtube_button = gtk::Button::with_label(tr(Message::UseYoutubeMusic));
    youtube_button.set_tooltip_text(Some(tr(Message::UseYoutubeMusicTooltip)));
    youtube_button.add_css_class("source-choice-button");
    youtube_button.add_css_class("suggested-action");

    let choices = gtk::Box::new(gtk::Orientation::Vertical, 10);
    choices.add_css_class("startup-choice-group");
    choices.append(&local_button);
    choices.append(&youtube_button);

    content.append(&title);
    content.append(&description);
    content.append(&choices);

    if !first_run {
        let cancel_button = gtk::Button::with_label(tr(Message::Cancel));
        cancel_button.set_halign(gtk::Align::End);
        cancel_button.add_css_class("startup-cancel-action");
        content.append(&cancel_button);

        let dialog = dialog.clone();
        cancel_button.connect_clicked(move |_| {
            dialog.close();
        });
    }

    {
        let on_select = on_select.clone();
        let dialog = dialog.clone();
        local_button.connect_clicked(move |_| {
            on_select(StartupSource::Local);
            dialog.close();
        });
    }

    {
        let on_select = on_select.clone();
        let dialog = dialog.clone();
        youtube_button.connect_clicked(move |_| {
            on_select(StartupSource::YouTube);
            dialog.close();
        });
    }

    dialog.present(Some(parent));
}
