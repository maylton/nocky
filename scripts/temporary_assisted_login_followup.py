#!/usr/bin/env python3
from pathlib import Path


def replace(path: str, old: str, new: str, expected: int = 1) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} occurrence(s), found {count}: {old[:120]!r}"
        )
    file.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "src/youtube/mod.rs",
    "mod login_policy;\n",
    '#[cfg(feature = "assisted-login")]\nmod login_policy;\n',
)

replace(
    "src/youtube/assisted_login.rs",
    '''    const SAPISID_COOKIE_NAMES: &[&str] = &[
        "__Secure-3PAPISID",
        "SAPISID",
        "__Secure-1PAPISID",
        "APISID",
    ];

    struct Copy {
''',
    '''    const SAPISID_COOKIE_NAMES: &[&str] = &[
        "__Secure-3PAPISID",
        "SAPISID",
        "__Secure-1PAPISID",
        "APISID",
    ];

    type SessionCallback = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

    struct Copy {
''',
)

replace(
    "src/youtube/assisted_login.rs",
    '''    fn finish_callback(
        callback: &Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
        cookie_header: String,
    ) {
''',
    '''    fn finish_callback(callback: &SessionCallback, cookie_header: String) {
''',
)

replace(
    "src/youtube/assisted_login.rs",
    '''        let callback: Rc<RefCell<Option<Box<dyn Fn(String)>>>> =
            Rc::new(RefCell::new(Some(Box::new(on_session))));
''',
    '''        let callback: SessionCallback = Rc::new(RefCell::new(Some(Box::new(on_session))));
''',
)
