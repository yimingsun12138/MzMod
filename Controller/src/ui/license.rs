use std::{sync::Arc, thread, time::Duration};

use anyhow::{Context as _, Result};
use eframe::egui::{Context, Id};
use ureq::{
    http::{Response, StatusCode},
    Agent, Body,
};

const BUNDLED: &str = include_str!("../../../LICENSE");
const LICENSE_ID: &str = "project_license";
const MAX_BYTES: u64 = 64 * 1024;

pub(super) fn text(ctx: &Context) -> Arc<str> {
    ctx.data_mut(|data| {
        Arc::clone(
            data.get_temp_mut_or_insert_with(Id::new(LICENSE_ID), || Arc::<str>::from(BUNDLED)),
        )
    })
}

pub(super) fn synchronize(ctx: &Context) {
    let ctx = ctx.clone();
    let _ = thread::Builder::new()
        .name("project-license".into())
        .spawn(move || {
            if let Ok(text) = fetch() {
                ctx.data_mut(|data| data.insert_temp(Id::new(LICENSE_ID), Arc::<str>::from(text)));
                ctx.request_repaint();
            }
        });
}

fn fetch() -> Result<String> {
    let repository = env!("CARGO_PKG_REPOSITORY")
        .strip_prefix("https://github.com/")
        .context("expected a GitHub repository URL")?;
    let url = format!("https://raw.githubusercontent.com/{repository}/main/LICENSE");
    let agent = Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .new_agent();
    read_license(agent.get(url).call()?).context("expected a plain-text project license")
}

fn read_license(mut response: Response<Body>) -> Option<String> {
    if response.status() != StatusCode::OK || response.body().mime_type() != Some("text/plain") {
        return None;
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let body = text.trim();
    if body.starts_with('<')
        || !body.contains('\n')
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return None;
    }
    let lowercase = body.to_ascii_lowercase();
    ["license", "licence", "copyright"]
        .iter()
        .any(|word| lowercase.contains(word))
        .then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, mime_type: &str, bytes: &[u8]) -> Response<Body> {
        Response::builder()
            .status(status)
            .body(Body::builder().mime_type(mime_type).data(bytes))
            .unwrap()
    }

    #[test]
    fn the_bundled_license_is_available_before_any_network_response() {
        assert_eq!(text(&Context::default()).as_ref(), BUNDLED);
    }

    #[test]
    #[ignore = "requires access to the public GitHub repository"]
    fn github_serves_a_readable_project_license() -> Result<()> {
        fetch()?;
        Ok(())
    }

    #[test]
    fn accepts_updated_license_text_and_discards_invalid_replies() {
        let updated = BUNDLED.replace("2026", "2027");
        assert_eq!(
            read_license(response(200, "text/plain", updated.as_bytes())),
            Some(updated)
        );
        for invalid in [
            response(503, "text/plain", BUNDLED.as_bytes()),
            response(200, "text/html", BUNDLED.as_bytes()),
            response(200, "text/plain", b"<html>\nLicense error</html>"),
            response(200, "text/plain", b"Network error\nAccess denied"),
            response(200, "text/plain", b"MIT License\n\xff"),
            response(200, "text/plain", b"MIT License\n\x00"),
            response(200, "text/plain", &vec![b'L'; MAX_BYTES as usize + 1]),
        ] {
            assert_eq!(read_license(invalid), None);
        }
    }
}
