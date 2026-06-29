#!/usr/bin/env python3
from pathlib import Path


def replace(path: str, old: str, new: str, expected: int = 1) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} occurrence(s), found {count}: {old[:140]!r}"
        )
    file.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "Cargo.toml",
    "[dependencies]\n",
    '''[features]
default = ["assisted-login"]
assisted-login = ["dep:webkit6"]

[dependencies]
''',
)
replace(
    "Cargo.toml",
    'walkdir = "2.5"\n',
    'walkdir = "2.5"\nwebkit6 = { version = "0.6.1", optional = true }\n',
)

replace(
    "src/youtube/mod.rs",
    "mod backend;\n",
    "mod assisted_login;\nmod backend;\n",
)
replace(
    "src/youtube/mod.rs",
    "mod playback;\n",
    "mod login_policy;\nmod playback;\n",
)
replace(
    "src/youtube/mod.rs",
    "pub(crate) use collections::{resolve_youtube_collection_item, youtube_home_prefetch_candidates};\n",
    '''pub(crate) use assisted_login::present as present_assisted_login;
pub(crate) use collections::{resolve_youtube_collection_item, youtube_home_prefetch_candidates};
''',
)

Path("src/youtube/login_policy.rs").write_text(
    r'''use std::net::IpAddr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavigationDisposition {
    Allow,
    OpenExternal,
    Block,
}

const EMBEDDED_HOSTS: &[&str] = &[
    "accounts.google.com",
    "consent.google.com",
    "consent.youtube.com",
    "music.youtube.com",
    "myaccount.google.com",
    "www.youtube.com",
];

const EXTERNAL_HOSTS: &[&str] = &["policies.google.com", "support.google.com"];

fn https_host(uri: &str) -> Option<String> {
    let rest = uri.trim().strip_prefix("https://")?;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|value| !value.is_empty())?;
    if authority.contains('@') || authority.starts_with('[') || authority.ends_with(']') {
        return None;
    }

    let mut host = authority;
    if let Some((candidate, port)) = authority.rsplit_once(':') {
        if candidate.contains(':') || port != "443" {
            return None;
        }
        host = candidate;
    }

    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host.parse::<IpAddr>().is_ok()
        || host.split('.').any(|label| {
            label.is_empty()
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return None;
    }
    Some(host)
}

pub(crate) fn navigation_disposition(uri: &str) -> NavigationDisposition {
    let Some(host) = https_host(uri) else {
        return NavigationDisposition::Block;
    };
    if EMBEDDED_HOSTS.contains(&host.as_str()) {
        NavigationDisposition::Allow
    } else if EXTERNAL_HOSTS.contains(&host.as_str()) {
        NavigationDisposition::OpenExternal
    } else {
        NavigationDisposition::Block
    }
}

pub(crate) fn is_youtube_music_uri(uri: &str) -> bool {
    https_host(uri).as_deref() == Some("music.youtube.com")
}

#[cfg(test)]
mod tests {
    use super::{is_youtube_music_uri, navigation_disposition, NavigationDisposition};

    #[test]
    fn allows_only_exact_audited_https_hosts() {
        assert_eq!(
            navigation_disposition("https://accounts.google.com/v3/signin"),
            NavigationDisposition::Allow
        );
        assert_eq!(
            navigation_disposition("https://music.youtube.com/"),
            NavigationDisposition::Allow
        );
        assert_eq!(
            navigation_disposition("https://support.google.com/youtubemusic"),
            NavigationDisposition::OpenExternal
        );
    }

    #[test]
    fn blocks_lookalikes_credentials_ips_and_non_https_urls() {
        for uri in [
            "http://accounts.google.com/",
            "https://accounts.google.com.evil.example/",
            "https://user@accounts.google.com/",
            "https://127.0.0.1/",
            "https://[::1]/",
            "https://music.youtube.com:8443/",
            "data:text/html,hello",
            "javascript:alert(1)",
            "file:///tmp/session",
        ] {
            assert_eq!(navigation_disposition(uri), NavigationDisposition::Block, "{uri}");
        }
    }

    #[test]
    fn recognizes_only_the_exact_youtube_music_origin() {
        assert!(is_youtube_music_uri("https://music.youtube.com/"));
        assert!(is_youtube_music_uri("https://music.youtube.com/library"));
        assert!(!is_youtube_music_uri("https://www.youtube.com/"));
        assert!(!is_youtube_music_uri("https://music.youtube.com.evil.example/"));
    }
}
''',
    encoding="utf-8",
)

Path("src/youtube/assisted_login.rs").write_text(
    r'''use crate::config::AppLanguage;

#[cfg(feature = "assisted-login")]
mod implementation {
    use super::AppLanguage;
    use crate::youtube::login_policy::{
        is_youtube_music_uri, navigation_disposition, NavigationDisposition,
    };
    use adw::prelude::*;
    use gtk::{gio, glib};
    use std::{cell::{Cell, RefCell}, rc::Rc};
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

    struct Copy {
        title: &'static str,
        description: &'static str,
        loading: &'static str,
        waiting: &'static str,
        capturing: &'static str,
        missing_session: &'static str,
        cookie_error: &'static str,
        blocked: &'static str,
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
                cancel: "Cancelar",
            },
        }
    }

    fn finish_callback(
        callback: &Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
        cookie_header: String,
    ) {
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
                        status.set_text(text.blocked);
                    }
                }
                true
            });
        }

        let callback: Rc<RefCell<Option<Box<dyn Fn(String)>>>> =
            Rc::new(RefCell::new(Some(Box::new(on_session))));
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
        AppLanguage::Portuguese => "Esta compilação não inclui o login assistido. Use a importação manual de sessão.".to_string(),
        AppLanguage::English => "This build does not include assisted sign-in. Use manual session import instead.".to_string(),
        AppLanguage::Spanish => "Esta compilación no incluye el inicio asistido. Usa la importación manual de sesión.".to_string(),
    })
}
''',
    encoding="utf-8",
)

replace(
    "src/youtube/mod.rs",
    "    Connect(String),\n",
    "    AssistedLogin,\n    Connect(String),\n",
)
replace(
    "src/youtube/mod.rs",
    '''    connect_button: gtk::Button,
    disconnect_button: gtk::Button,
''',
    '''    connect_button: gtk::Button,
    manual_import_button: gtk::Button,
    disconnect_button: gtk::Button,
''',
)
replace(
    "src/youtube/mod.rs",
    '''        let subtitle = gtk::Label::new(Some(
            "Busque no catálogo ou conecte a sessão do navegador para acessar sua biblioteca.",
        ));
''',
    '''        let subtitle = gtk::Label::new(Some(
            "Busque no catálogo ou entre com o navegador para acessar sua biblioteca e playlists.",
        ));
''',
)
replace(
    "src/youtube/mod.rs",
    '''        let connect_button = gtk::Button::with_label("Conectar conta");
        connect_button.add_css_class("suggested-action");
        let disconnect_button = gtk::Button::with_label("Desconectar");
''',
    '''        let connect_button = gtk::Button::with_label("Entrar com o navegador");
        connect_button.add_css_class("suggested-action");
        let manual_import_button = gtk::Button::with_label("Importar sessão manualmente");
        manual_import_button.add_css_class("flat");
        let disconnect_button = gtk::Button::with_label("Desconectar");
''',
)
replace(
    "src/youtube/mod.rs",
    '''        account_row.append(&status);
        account_row.append(&connect_button);
        account_row.append(&disconnect_button);
''',
    '''        account_row.append(&status);
        account_row.append(&manual_import_button);
        account_row.append(&connect_button);
        account_row.append(&disconnect_button);
''',
)
replace(
    "src/youtube/mod.rs",
    '''        let auth_text = gtk::Label::new(Some(
            "Abra o YouTube Music no navegador do sistema, entre na conta e copie uma requisição bem-sucedida como cURL ou apenas o cabeçalho Cookie. O Nocky nunca solicita sua senha e guarda somente os cabeçalhos mínimos no Secret Service quando disponível.",
        ));
''',
    '''        let auth_text = gtk::Label::new(Some(
            "Alternativa avançada: abra o YouTube Music no navegador do sistema e cole uma requisição bem-sucedida como cURL ou apenas o cabeçalho Cookie. O Nocky guarda somente os cabeçalhos mínimos no Secret Service quando disponível.",
        ));
''',
)
replace(
    "src/youtube/mod.rs",
    '        let import_button = gtk::Button::with_label("Importar sessão");\n',
    '        let import_button = gtk::Button::with_label("Salvar sessão importada");\n',
)
replace(
    "src/youtube/mod.rs",
    '''            connect_button,
            disconnect_button,
''',
    '''            connect_button,
            manual_import_button,
            disconnect_button,
''',
)
replace(
    "src/youtube/mod.rs",
    '''        {
            let button = page.connect_button.clone();
            let weak = Rc::downgrade(&page);
            button.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.auth_revealer.set_reveal_child(true);
                }
            });
        }
''',
    '''        {
            let sender = page.event_tx.clone();
            page.connect_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::AssistedLogin);
            });
        }
        {
            let button = page.manual_import_button.clone();
            let weak = Rc::downgrade(&page);
            button.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.show_manual_import();
                }
            });
        }
''',
)
replace(
    "src/youtube/mod.rs",
    '''    pub fn root(&self) -> &gtk::Box {
        &self.root
    }

''',
    '''    pub fn root(&self) -> &gtk::Box {
        &self.root
    }

    pub fn show_manual_import(&self) {
        self.auth_revealer.set_reveal_child(true);
        self.auth_buffer.set_text("");
    }

    pub fn submit_assisted_session(&self, raw: String) {
        let _ = self.event_tx.send(YouTubePageEvent::Connect(raw));
    }

''',
)
replace(
    "src/youtube/mod.rs",
    '''            self.connect_button.set_visible(false);
            self.disconnect_button.set_visible(true);
''',
    '''            self.connect_button.set_visible(false);
            self.manual_import_button.set_visible(false);
            self.disconnect_button.set_visible(true);
''',
)
replace(
    "src/youtube/mod.rs",
    '''            self.connect_button.set_visible(true);
            self.disconnect_button.set_visible(false);
''',
    '''            self.connect_button.set_visible(true);
            self.manual_import_button.set_visible(true);
            self.disconnect_button.set_visible(false);
''',
)

replace(
    "src/app/controller/youtube.rs",
    '''impl AppController {
    pub(crate) fn refresh_youtube_status(&self) {
''',
    '''impl AppController {
    pub(crate) fn present_assisted_youtube_login(&self) {
        let page = self.youtube_page.clone();
        let language = self.config.borrow().language;
        if let Err(error) = youtube_domain::present_assisted_login(
            &self.window,
            language,
            move |raw| page.submit_assisted_session(raw),
        ) {
            self.youtube_page.show_manual_import();
            self.show_toast(&error);
        }
    }

    pub(crate) fn refresh_youtube_status(&self) {
''',
)
replace(
    "src/app/controller/youtube.rs",
    '''            match event {
                YouTubePageEvent::LoadHome {
''',
    '''            match event {
                YouTubePageEvent::AssistedLogin => {
                    self.present_assisted_youtube_login();
                }
                YouTubePageEvent::LoadHome {
''',
)

replace(
    "src/onboarding.rs",
    '''pub struct OnboardingChoices {
    pub startup_source: StartupSource,
''',
    '''pub struct OnboardingChoices {
    pub startup_source: StartupSource,
    pub suggest_youtube_login: bool,
''',
)
replace(
    "src/onboarding.rs",
    '''    youtube_title: &'static str,
    youtube_body: &'static str,
    experimental_title: &'static str,
''',
    '''    youtube_title: &'static str,
    youtube_body: &'static str,
    youtube_login_title: &'static str,
    youtube_login_body: &'static str,
    experimental_title: &'static str,
''',
)
replace(
    "src/onboarding.rs",
    '''    summary_source: &'static str,
    summary_learning: &'static str,
''',
    '''    summary_source: &'static str,
    summary_login: &'static str,
    summary_login_browser: &'static str,
    summary_login_not_needed: &'static str,
    summary_learning: &'static str,
''',
)
replace(
    "src/onboarding.rs",
    '''            youtube_title: "YouTube Music",
            youtube_body: "Use a busca pública e, opcionalmente, conecte sua conta para sincronizar biblioteca e playlists.",
            experimental_title: "Integração experimental",
''',
    '''            youtube_title: "YouTube Music",
            youtube_body: "Use o catálogo online e entre com o navegador para sincronizar biblioteca, curtidas e playlists.",
            youtube_login_title: "Login recomendado",
            youtube_login_body: "Ao concluir, o Nocky abrirá uma janela isolada para você entrar no YouTube Music. A importação manual continuará disponível como alternativa avançada.",
            experimental_title: "Integração experimental",
''',
)
replace(
    "src/onboarding.rs",
    '''            summary_source: "Fonte da Home",
            summary_learning: "Home personalizada",
''',
    '''            summary_source: "Fonte da Home",
            summary_login: "Próximo passo",
            summary_login_browser: "Entrar com o navegador",
            summary_login_not_needed: "Nenhum login necessário",
            summary_learning: "Home personalizada",
''',
)
replace(
    "src/onboarding.rs",
    '''            youtube_title: "YouTube Music",
            youtube_body: "Use public search and optionally connect an account to synchronize your library and playlists.",
            experimental_title: "Experimental integration",
''',
    '''            youtube_title: "YouTube Music",
            youtube_body: "Use the online catalog and sign in with the browser to synchronize your library, likes, and playlists.",
            youtube_login_title: "Recommended sign-in",
            youtube_login_body: "When setup finishes, Nocky will open an isolated window for YouTube Music sign-in. Manual session import remains available as an advanced alternative.",
            experimental_title: "Experimental integration",
''',
)
replace(
    "src/onboarding.rs",
    '''            summary_source: "Home source",
            summary_learning: "Personalized Home",
''',
    '''            summary_source: "Home source",
            summary_login: "Next step",
            summary_login_browser: "Sign in with browser",
            summary_login_not_needed: "No sign-in required",
            summary_learning: "Personalized Home",
''',
)
replace(
    "src/onboarding.rs",
    '''            youtube_title: "YouTube Music",
            youtube_body: "Usa la búsqueda pública y conecta una cuenta opcionalmente para sincronizar biblioteca y playlists.",
            experimental_title: "Integración experimental",
''',
    '''            youtube_title: "YouTube Music",
            youtube_body: "Usa el catálogo en línea e inicia sesión con el navegador para sincronizar biblioteca, favoritos y playlists.",
            youtube_login_title: "Inicio de sesión recomendado",
            youtube_login_body: "Al finalizar, Nocky abrirá una ventana aislada para iniciar sesión en YouTube Music. La importación manual seguirá disponible como alternativa avanzada.",
            experimental_title: "Integración experimental",
''',
)
replace(
    "src/onboarding.rs",
    '''            summary_source: "Fuente de Home",
            summary_learning: "Inicio personalizado",
''',
    '''            summary_source: "Fuente de Home",
            summary_login: "Siguiente paso",
            summary_login_browser: "Iniciar sesión con el navegador",
            summary_login_not_needed: "No se necesita iniciar sesión",
            summary_learning: "Inicio personalizado",
''',
)
replace(
    "src/onboarding.rs",
    '''    source_content.append(&option_card(
        text.youtube_title,
        text.youtube_body,
        &youtube_choice,
    ));

    let warning = gtk::Box::new(gtk::Orientation::Vertical, 5);
''',
    '''    source_content.append(&option_card(
        text.youtube_title,
        text.youtube_body,
        &youtube_choice,
    ));

    let login_recommendation = gtk::Box::new(gtk::Orientation::Vertical, 5);
    login_recommendation.add_css_class("onboarding-warning");
    let login_title = gtk::Label::new(Some(text.youtube_login_title));
    login_title.set_xalign(0.0);
    login_title.add_css_class("heading");
    let login_body = gtk::Label::new(Some(text.youtube_login_body));
    login_body.set_xalign(0.0);
    login_body.set_wrap(true);
    login_body.add_css_class("dim-label");
    login_recommendation.append(&login_title);
    login_recommendation.append(&login_body);
    login_recommendation.set_visible(youtube_choice.is_active());
    source_content.append(&login_recommendation);

    let warning = gtk::Box::new(gtk::Orientation::Vertical, 5);
''',
)
replace(
    "src/onboarding.rs",
    '''    {
        let warning = warning.clone();
        youtube_choice.connect_active_notify(move |choice| {
            warning.set_visible(choice.is_active());
        });
    }
''',
    '''    {
        let warning = warning.clone();
        let login_recommendation = login_recommendation.clone();
        youtube_choice.connect_active_notify(move |choice| {
            let active = choice.is_active();
            warning.set_visible(active);
            login_recommendation.set_visible(active);
        });
    }
''',
)
replace(
    "src/onboarding.rs",
    '''    let summary_source = gtk::Label::new(None);
    let summary_learning = gtk::Label::new(None);
''',
    '''    let summary_source = gtk::Label::new(None);
    let summary_login = gtk::Label::new(None);
    let summary_learning = gtk::Label::new(None);
''',
)
replace(
    "src/onboarding.rs",
    '''    summary_content.append(&summary_row(text.summary_source, &summary_source));
    summary_content.append(&summary_row(text.summary_learning, &summary_learning));
''',
    '''    summary_content.append(&summary_row(text.summary_source, &summary_source));
    summary_content.append(&summary_row(text.summary_login, &summary_login));
    summary_content.append(&summary_row(text.summary_learning, &summary_learning));
''',
)
replace(
    "src/onboarding.rs",
    '''        let summary_source = summary_source.clone();
        let summary_learning = summary_learning.clone();
''',
    '''        let summary_source = summary_source.clone();
        let summary_login = summary_login.clone();
        let summary_learning = summary_learning.clone();
''',
)
replace(
    "src/onboarding.rs",
    '''                summary_source.set_text(if local_choice.is_active() {
                    text.local_title
                } else {
                    text.youtube_title
                });
                summary_learning.set_text(if personalized_history.is_active() {
''',
    '''                summary_source.set_text(if local_choice.is_active() {
                    text.local_title
                } else {
                    text.youtube_title
                });
                summary_login.set_text(if local_choice.is_active() {
                    text.summary_login_not_needed
                } else {
                    text.summary_login_browser
                });
                summary_learning.set_text(if personalized_history.is_active() {
''',
)
replace(
    "src/onboarding.rs",
    '''            let choices = OnboardingChoices {
                startup_source: if local_choice.is_active() {
                    StartupSource::Local
                } else {
                    StartupSource::YouTube
                },
                show_personalized_home_history: personalized_history.is_active(),
''',
    '''            let youtube_selected = !local_choice.is_active();
            let choices = OnboardingChoices {
                startup_source: if youtube_selected {
                    StartupSource::YouTube
                } else {
                    StartupSource::Local
                },
                suggest_youtube_login: youtube_selected,
                show_personalized_home_history: personalized_history.is_active(),
''',
)

replace(
    "src/app/controller/settings.rs",
    '''                let choose_local_folder = {
                    let mut config = controller.config.borrow_mut();
''',
    '''                let (choose_local_folder, suggest_youtube_login) = {
                    let mut config = controller.config.borrow_mut();
''',
)
replace(
    "src/app/controller/settings.rs",
    '''                    choices.startup_source == StartupSource::Local
                        && config.music_directory.is_none()
                };
''',
    '''                    (
                        choices.startup_source == StartupSource::Local
                            && config.music_directory.is_none(),
                        choices.suggest_youtube_login
                            && choices.startup_source == StartupSource::YouTube,
                    )
                };
''',
)
replace(
    "src/app/controller/settings.rs",
    '''                if choose_local_folder {
                    let controller = controller.clone();
                    glib::idle_add_local_once(move || {
                        controller.choose_library_folder();
                    });
                }
''',
    '''                if choose_local_folder {
                    let controller = controller.clone();
                    glib::idle_add_local_once(move || {
                        controller.choose_library_folder();
                    });
                } else if suggest_youtube_login {
                    let controller = controller.clone();
                    glib::idle_add_local_once(move || {
                        if !controller.youtube_library.borrow().connected {
                            controller.present_assisted_youtube_login();
                        }
                    });
                }
''',
)

replace(
    "install.sh",
    '''      gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav \
      desktop-file-utils hicolor-icon-theme libglib2.0-bin
''',
    '''      gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav \
      desktop-file-utils hicolor-icon-theme libglib2.0-bin
''',
)
replace(
    "install.sh",
    '''        python3 python3-venv python3-pip python3-gi \
        gir1.2-secret-1 libsecret-1-0 curl unzip
''',
    '''        python3 python3-venv python3-pip python3-gi \
        gir1.2-secret-1 libsecret-1-0 libwebkitgtk-6.0-dev curl unzip
''',
)
replace(
    "install.sh",
    '''        python3 python3-pip python3-gobject libsecret curl unzip
''',
    '''        python3 python3-pip python3-gobject libsecret webkitgtk6.0-devel curl unzip
''',
    expected=2,
)
replace(
    "install.sh",
    '''        python3 python3-pip python3-gobject libsecret-1-0 curl unzip
''',
    '''        python3 python3-pip python3-gobject libsecret-1-0 'pkgconfig(webkitgtk-6.0)' curl unzip
''',
)
replace(
    "install.sh",
    '''        python python-pip python-gobject libsecret curl unzip
''',
    '''        python python-pip python-gobject libsecret webkitgtk-6.0 curl unzip
''',
)
replace(
    "install.sh",
    '''for package in gtk4 libadwaita-1 gstreamer-1.0; do
''',
    '''required_packages=(gtk4 libadwaita-1 gstreamer-1.0)
$INSTALL_YOUTUBE && required_packages+=(webkitgtk-6.0)
for package in "${required_packages[@]}"; do
''',
)
replace(
    "install.sh",
    '''cd "$ROOT_DIR"
echo "Building ${APP_NAME} ${VERSION} in release mode..."
cargo build --release --locked
''',
    '''cd "$ROOT_DIR"
echo "Building ${APP_NAME} ${VERSION} in release mode..."
if $INSTALL_YOUTUBE; then
  cargo build --release --locked
else
  cargo build --release --locked --no-default-features
fi
''',
)
