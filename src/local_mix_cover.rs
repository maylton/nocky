use gdk_pixbuf::{Colorspace, InterpType, Pixbuf};
use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const MIX_COVER_SIZE: i32 = 512;
const MAX_MIX_COVERS: usize = 4;

pub(crate) fn cover_for_mix<I>(paths: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let covers = distinct_existing_paths(paths);
    match covers.len() {
        0 => None,
        1 => covers.into_iter().next(),
        _ => composite_cover(&covers),
    }
}

fn distinct_existing_paths<I>(paths: I) -> Vec<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut covers: Vec<PathBuf> = Vec::new();

    for path in paths {
        if !path.is_file() || covers.iter().any(|known| same_path(known, &path)) {
            continue;
        }

        covers.push(path);
        if covers.len() == MAX_MIX_COVERS {
            break;
        }
    }

    covers
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn composite_cover(covers: &[PathBuf]) -> Option<PathBuf> {
    let output = cache_path(covers)?;
    if output.is_file() {
        return Some(output);
    }

    let canvas = Pixbuf::new(Colorspace::Rgb, true, 8, MIX_COVER_SIZE, MIX_COVER_SIZE)?;
    canvas.fill(0x18181bff);

    for (path, (x, y, width, height)) in covers.iter().zip(layout(covers.len())) {
        let source = Pixbuf::from_file(path).ok()?;
        let scaled = source.scale_simple(width, height, InterpType::Bilinear)?;
        scaled.copy_area(0, 0, width, height, &canvas, x, y);
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).ok()?;
    }

    canvas.savev(&output, "png", &[]).ok()?;
    Some(output)
}

fn layout(count: usize) -> Vec<(i32, i32, i32, i32)> {
    let half = MIX_COVER_SIZE / 2;

    match count {
        0 => Vec::new(),
        1 => vec![(0, 0, MIX_COVER_SIZE, MIX_COVER_SIZE)],
        2 => vec![
            (0, 0, half, MIX_COVER_SIZE),
            (half, 0, half, MIX_COVER_SIZE),
        ],
        3 => vec![
            (0, 0, half, MIX_COVER_SIZE),
            (half, 0, half, half),
            (half, half, half, half),
        ],
        _ => vec![
            (0, 0, half, half),
            (half, 0, half, half),
            (0, half, half, half),
            (half, half, half, half),
        ],
    }
}

fn cache_path(covers: &[PathBuf]) -> Option<PathBuf> {
    let cache_root = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;

    let mut hasher = DefaultHasher::new();
    "nocky-local-mix-cover-v1".hash(&mut hasher);

    for path in covers {
        path.hash(&mut hasher);
        if let Ok(metadata) = fs::metadata(path) {
            metadata.len().hash(&mut hasher);
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    duration.as_secs().hash(&mut hasher);
                    duration.subsec_nanos().hash(&mut hasher);
                }
            }
        }
    }

    Some(
        cache_root
            .join("nocky")
            .join("local-mix-covers")
            .join(format!("{:016x}.png", hasher.finish())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_fill_the_expected_number_of_slots() {
        assert_eq!(layout(0).len(), 0);
        assert_eq!(layout(1).len(), 1);
        assert_eq!(layout(2).len(), 2);
        assert_eq!(layout(3).len(), 3);
        assert_eq!(layout(4).len(), 4);
        assert_eq!(layout(8).len(), 4);
    }

    #[test]
    fn four_cover_layout_is_a_two_by_two_grid() {
        let half = MIX_COVER_SIZE / 2;
        assert_eq!(
            layout(4),
            vec![
                (0, 0, half, half),
                (half, 0, half, half),
                (0, half, half, half),
                (half, half, half, half),
            ]
        );
    }
}
