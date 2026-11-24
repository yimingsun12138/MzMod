//! Identify package-relative files on Windows, including manifests read on Linux.

pub(crate) fn windows_path_key(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            '\\' => '/',
            c if c.is_ascii() => c.to_ascii_lowercase(),
            // File-name comparisons map individual characters without expanding their length.
            c => {
                let mut upper = c.to_uppercase();
                let first = upper.next().unwrap_or(c);
                if upper.next().is_none() {
                    first.to_ascii_lowercase()
                } else {
                    c
                }
            }
        })
        .collect()
}

/// Ancestors of a slash-separated game path, from the root down.
pub(crate) fn parent_paths(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/').map(|(end, _)| &path[..end])
}

pub(crate) fn is_valid_game_path(path: &str) -> bool {
    path.split(['/', '\\']).all(|part| {
        !part.is_empty()
            && !part.ends_with(['.', ' '])
            && !part.chars().any(|c| c < ' ' || "<>:\"|?*".contains(c))
            && !is_device_name(part)
    })
}

fn is_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    let stem = stem.to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    stem.strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}
