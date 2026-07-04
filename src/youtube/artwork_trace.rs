use super::YouTubeItem;
use std::{env, fmt::Write as _, fs, path::Path};

const TRACE_PREFIX: &str = "[YT_ARTWORK_TRACE]";
const TRACE_TARGETS: [&str; 3] = ["rbd", "dynamic anime soundtracks", "ser o parecer"];

pub(super) fn enabled() -> bool {
    match env::var("NOCKY_YOUTUBE_ARTWORK_TRACE") {
        Ok(value) => {
            let value = value.trim().to_ascii_lowercase();
            !value.is_empty() && value != "0" && value != "false" && value != "off"
        }
        Err(_) => false,
    }
}

pub(super) fn trace_item(
    phase: &str,
    section_title: &str,
    item: &YouTubeItem,
    explicit_cover_path: Option<&Path>,
    note: &str,
) {
    if !enabled() || !matches_item(item) {
        return;
    }

    let cover_path = explicit_cover_path
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| item.cover_path.clone());
    let cover_path_ref = (!cover_path.trim().is_empty()).then(|| Path::new(&cover_path));
    let cover = cover_path_ref.map(cover_metadata).unwrap_or_default();

    eprintln!(
        "{TRACE_PREFIX} phase={} section={} type={} title={} artist={} subtitle={} browse_id={} video_id={} thumbnail_url={} cover_path={} cover_exists={} cover_file={} cover_len={} cover_sha256={} note={}",
        field(phase),
        field(section_title),
        field(&item.result_type),
        field(&item.title),
        field(&item.artist),
        field(&item.subtitle),
        field(&item.browse_id),
        field(&item.video_id),
        field(&item.thumbnail_url),
        field(&cover_path),
        cover.exists,
        field(&cover.file_name),
        cover.len,
        field(&cover.sha256),
        field(note),
    );
}

pub(super) fn trace_delta_compare(
    phase: &str,
    section_title: &str,
    existing: &YouTubeItem,
    incoming: &YouTubeItem,
) {
    if !enabled() || !(matches_item(existing) || matches_item(incoming)) {
        return;
    }

    trace_item(
        phase,
        section_title,
        existing,
        None,
        "delta_existing_before_update",
    );
    trace_item(
        phase,
        section_title,
        incoming,
        None,
        "delta_incoming_candidate",
    );
}

pub(super) fn trace_delta_update(
    phase: &str,
    section_title: &str,
    field_name: &str,
    existing: &YouTubeItem,
    incoming: &YouTubeItem,
    old_value: &str,
    new_value: &str,
) {
    if !enabled() || !(matches_item(existing) || matches_item(incoming)) {
        return;
    }

    let note = format!(
        "field={} old={} new={}",
        compact(field_name),
        compact(old_value),
        compact(new_value)
    );
    trace_item(phase, section_title, existing, None, &note);
    trace_item(
        phase,
        section_title,
        incoming,
        None,
        &format!("incoming_for_{note}"),
    );
}

fn matches_item(item: &YouTubeItem) -> bool {
    let haystack = compact(&format!(
        "{} {} {} {} {} {} {}",
        item.title,
        item.artist,
        item.subtitle,
        item.browse_id,
        item.video_id,
        item.thumbnail_url,
        item.cover_path
    ))
    .to_ascii_lowercase();

    TRACE_TARGETS
        .iter()
        .any(|target| haystack.contains(target))
}

#[derive(Default)]
struct CoverMetadata {
    exists: bool,
    file_name: String,
    len: u64,
    sha256: String,
}

fn cover_metadata(path: &Path) -> CoverMetadata {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Ok(bytes) = fs::read(path) else {
        return CoverMetadata {
            file_name,
            ..CoverMetadata::default()
        };
    };

    CoverMetadata {
        exists: true,
        file_name,
        len: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
    }
}

fn field(value: &str) -> String {
    format!("\"{}\"", compact(value))
}

fn compact(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\n' | '\r' | '\t' => ' ',
            _ => character,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sha256_hex(bytes: &[u8]) -> String {
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
        0x1f83d9ab, 0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
        0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
        0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h = H0;
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    let mut data = bytes.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in data.chunks_exact(64) {
        let mut w = [0_u32; 64];
        for (index, word) in w.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut output = String::with_capacity(64);
    for word in h {
        let _ = write!(&mut output, "{word:08x}");
    }
    output
}
