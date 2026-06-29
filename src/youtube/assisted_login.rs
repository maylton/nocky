use crate::config::AppLanguage;

#[cfg(feature = "assisted-login")]
mod implementation {
    use super::AppLanguage;
    use crate::youtube::login_policy::{
        is_youtube_music_uri, navigation_disposition, navigation_host, NavigationDisposition,
    };
    use adw::prelude::*;
    use gtk::{gio, glib};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use webkit6::{
        prelude::*, CookieAcceptPolicy, LoadEvent, NavigationPolicyDecision, NetworkSession,
        PolicyDecisionType, WebView,
    };

    const YOUTUBE_MUSIC_URI: &str = "https://music.youtube.com/";
    const SAPISID_COOKIE_NAMES: &[&str] = &[
        "__Secure-3PAPISID",
        "SAPISID",
        "__Secure-1PAPISID",
        "APISID",
    ];

    type SessionCallback = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

    struct Copy {
        title: &'static str,
        description: &'static str,
        loading: &'static str,
        waiting: &'static str,
        capturing: &'static str,
        missing_session: &'static str,
        cookie_error: &'static str,
        blocked: &'static str,
        blocked_host: &'static str,
        cancel: &'static str,
    }

    fn copy(language: AppLanguage) -> Copy {
        match language {
            AppLanguage::Portuguese => Copy {
                title: "Entrar no YouTube Music",
                description: "Entre na sua conta nesta janela isolada. O Nocky não lê sua senha nem o conteúdo da página; ele captura somente a sessão associada ao YouTube Music depois que o login termina.",
                loading: "Abrindo o login seguro…",
                waiting: "Conclua o login para continuar.",
                capturing: "Validando a sessão do YouTube Music…",
                missing_session: "O YouTube Music abriu, mas a sessão autenticada ainda não foi encontrada. Conclua o login ou escolha a conta correta.",
                cookie_error: "Não foi possível ler a sessão do YouTube Music.",
                blocked: "Este endereço não faz parte do fluxo de login permitido.",
                blocked_host: "Endereço bloqueado:",
                cancel: "Cancelar",
            },
            AppLanguage::English => Copy {
                title: "Sign in to YouTube Music",
                description: "Sign in inside this isolated window. Nocky never reads your password or page contents; it captures only the YouTube Music session after sign-in finishes.",
                loading: "Opening secure sign-in…",
                waiting: "Complete sign-in to continue.",
                capturing: "Validating the YouTube Music session…",
                missing_session: "YouTube Music opened, but an authenticated session was not found yet. Finish signing in or choose the correct account.",
                cookie_error: "The YouTube Music session could not be read.",
                blocked: "This address is outside the permitted sign-in flow.",
                blocked_host: "Blocked address:",
                cancel: "Cancel",
            },
            AppLanguage::Spanish => Copy {
                title: "Iniciar sesión en YouTube Music",
                description: "Inicia sesión dentro de esta ventana aislada. Nocky no lee tu contraseña ni el contenido de la página; solo captura la sesión de YouTube Music cuando finaliza el acceso.",
                loading: "Abriendo el acceso seguro…",
                waiting: "Completa el inicio de sesión para continuar.",
                capturing: "Validando la sesión de YouTube Music…",
                missing_session: "YouTube Music se abrió, pero todavía no se encontró una sesión autenticada. Finaliza el acceso o elige la cuenta correcta.",
                cookie_error: "No se pudo leer la sesión de YouTube Music.",
                blocked: "Esta dirección no forma parte del flujo de acceso permitido.",
                blocked_host: "Dirección bloqueada:",
                cancel: "Cancelar",
            },
        }
    }

    fn finish_callback(callback: &SessionCallback, cookie_header: String) {
        if let Some(callback) = callback.borrow_mut().take() {
            callback(cookie_header);
        }
    }

    pub(crate) fn present<F>(
        parent: &adw::ApplicationWindow,
        language: AppLanguage,
        on_session: F,
    ) -> Result<(), String>
    where
        F: Fn(String) + 'static,
    {
        let text = copy(language);
        let network_session = NetworkSession::new_ephemeral();
        network_session.set_persistent_credential_storage_enabled(false);
        network_session.set_itp_enabled(true);
        let cookie_manager = network_session
            .cookie_manager()
            .ok_or_else(|| text.cookie_error.to_string())?;
        cookie_manager.set_accept_policy(CookieAcceptPolicy::Always);
        network_session.connect_download_started(|_, download| download.cancel());

        let web_view = WebView::builder()
            .network_session(&network_session)
            .hexpand(true)
            .vexpand(true)
            .build();

        let window = adw::Window::builder()
            .title(text.title)
            .transient_for(parent)
            .modal(true)
            .default_width(920)
            .default_height(720)
            .build();
        window.add_css_class("youtube-assisted-login-window");

        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        toolbar.add_top_bar(&header);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(18);
        content.set_margin_end(18);
        content.set_vexpand(true);

        let description = gtk::Label::new(Some(text.description));
        description.set_xalign(0.0);
        description.set_wrap(true);
        description.add_css_class("dim-label");

        let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let spinner = gtk::Spinner::new();
        spinner.start();
        let status = gtk::Label::new(Some(text.loading));
        status.set_xalign(0.0);
        status.set_hexpand(true);
        status.add_css_class("dim-label");
        let cancel = gtk::Button::with_label(text.cancel);
        cancel.add_css_class("flat");
        status_row.append(&spinner);
        status_row.append(&status);
        status_row.append(&cancel);

        let web_scroll = gtk::ScrolledWindow::new();
        web_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        web_scroll.set_vexpand(true);
        web_scroll.set_hexpand(true);
        web_scroll.set_child(Some(&web_view));
        web_scroll.add_css_class("youtube-assisted-login-browser");

        content.append(&description);
        content.append(&status_row);
        content.append(&web_scroll);
        toolbar.set_content(Some(&content));
        window.set_content(Some(&toolbar));

        {
            let window = window.clone();
            cancel.connect_clicked(move |_| window.close());
        }

        web_view.connect_context_menu(|_, _, _| true);
        web_view.connect_create(|_, _| None);
        web_view.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        web_view.connect_run_file_chooser(|_, request| {
            request.cancel();
            true
        });

        {
            let status = status.clone();
            web_view.connect_decide_policy(move |_, decision, decision_type| {
                if !matches!(
                    decision_type,
                    PolicyDecisionType::NavigationAction | PolicyDecisionType::NewWindowAction
                ) {
                    return false;
                }
                let Some(navigation) = decision.downcast_ref::<NavigationPolicyDecision>() else {
                    decision.ignore();
                    return true;
                };
                let uri = navigation
                    .navigation_action()
                    .and_then(|action| action.request())
                    .and_then(|request| request.uri())
                    .map(|uri| uri.to_string())
                    .unwrap_or_default();
                match navigation_disposition(&uri) {
                    NavigationDisposition::Allow => {
                        decision.use_();
                    }
                    NavigationDisposition::OpenExternal => {
                        decision.ignore();
                        let _ = gio::AppInfo::launch_default_for_uri(
                            &uri,
                            None::<&gio::AppLaunchContext>,
                        );
                    }
                    NavigationDisposition::Block => {
                        decision.ignore();
                        if let Some(host) = navigation_host(&uri) {
                            status.set_text(&format!("{} {host}", text.blocked_host));
                        } else {
                            status.set_text(text.blocked);
                        }
                    }
                }
                true
            });
        }

        let callback: SessionCallback = Rc::new(RefCell::new(Some(Box::new(on_session))));
        let capturing = Rc::new(Cell::new(false));
        {
            let window = window.clone();
            let status = status.clone();
            let spinner = spinner.clone();
            let cookie_manager = cookie_manager.clone();
            let callback = callback.clone();
            let capturing = capturing.clone();
            web_view.connect_load_changed(move |web_view, event| {
                if event != LoadEvent::Finished {
                    return;
                }
                let uri = web_view
                    .uri()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                if !is_youtube_music_uri(&uri) || capturing.replace(true) {
                    status.set_text(text.waiting);
                    return;
                }

                status.set_text(text.capturing);
                spinner.start();
                let window = window.clone();
                let status = status.clone();
                let spinner = spinner.clone();
                let cookie_manager = cookie_manager.clone();
                let callback = callback.clone();
                let capturing = capturing.clone();
                glib::MainContext::default().spawn_local(async move {
                    match cookie_manager.cookies_future(YOUTUBE_MUSIC_URI).await {
                        Ok(cookies) => {
                            let mut pairs = Vec::new();
                            let mut has_sapisid = false;
                            for mut cookie in cookies {
                                let Some(name) = cookie.name() else {
                                    continue;
                                };
                                let Some(value) = cookie.value() else {
                                    continue;
                                };
                                if value.is_empty() {
                                    continue;
                                }
                                if SAPISID_COOKIE_NAMES
                                    .iter()
                                    .any(|candidate| name.eq_ignore_ascii_case(candidate))
                                {
                                    has_sapisid = true;
                                }
                                pairs.push(format!("{name}={value}"));
                            }
                            if has_sapisid && !pairs.is_empty() {
                                finish_callback(&callback, format!("Cookie: {}", pairs.join("; ")));
                                window.close();
                            } else {
                                capturing.set(false);
                                spinner.stop();
                                status.set_text(text.missing_session);
                            }
                        }
                        Err(_) => {
                            capturing.set(false);
                            spinner.stop();
                            status.set_text(text.cookie_error);
                        }
                    }
                });
            });
        }

        web_view.connect_load_failed({
            let status = status.clone();
            let spinner = spinner.clone();
            move |_, _, _, _| {
                spinner.stop();
                status.set_text(text.waiting);
                false
            }
        });

        window.present();
        web_view.load_uri(YOUTUBE_MUSIC_URI);
        Ok(())
    }
}

#[cfg(feature = "assisted-login")]
pub(crate) use implementation::present;

#[cfg(not(feature = "assisted-login"))]
pub(crate) fn present<F>(
    _parent: &adw::ApplicationWindow,
    language: AppLanguage,
    _on_session: F,
) -> Result<(), String>
where
    F: Fn(String) + 'static,
{
    Err(match language {
        AppLanguage::Portuguese => {
            "Esta compilação não inclui o login assistido. Use a importação manual de sessão."
                .to_string()
        }
        AppLanguage::English => {
            "This build does not include assisted sign-in. Use manual session import instead."
                .to_string()
        }
        AppLanguage::Spanish => {
            "Esta compilación no incluye el inicio asistido. Usa la importación manual de sesión."
                .to_string()
        }
    })
}
