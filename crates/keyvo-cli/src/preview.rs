//! `keyvo preview`: renders one page of a profile onto the Keypad's panel
//! and writes it as a PNG, without a device (spec §9). The panel is the
//! captured MX Creative Keypad geometry (spec §5.4).

use std::io::Write;
use std::path::Path;

use anyhow::{Context, bail};
use keyvo_core::layout;
use keyvo_core::profile::Profile;
use keyvo_core::render::{self, Renderer, icon};
use keyvo_core::stack::Stack;
use keyvo_hid::display;

/// Renders page `page` (counted from 1) of `profile` to the PNG `output`.
/// `profile` is a file path when it names an existing file or ends with
/// `.toml`, else a profile id looked up in `config_dir`. Unknown icons are
/// reported on `err`; the summary goes to `out`.
///
/// # Errors
///
/// An unreadable or invalid profile, an unknown id, a page the profile does
/// not have, and write errors.
pub fn preview(
    profile: &str,
    page: usize,
    output: &Path,
    config_dir: Option<&Path>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> anyhow::Result<()> {
    let profile = find(profile, config_dir)?;
    let panel = display::captured_geometry().panel();
    let keys = u8::try_from(panel.keys.len()).context("too many keys")?;
    let specs = render::page(&Stack::alone(&profile, page)?, keys);
    for (number, spec) in (1..).zip(&specs) {
        if let Some(Err(error)) = spec.icon.as_deref().map(icon::lookup) {
            writeln!(err, "key {number}: {error}; drawn as {}", icon::FALLBACK)?;
        }
    }
    let png = Renderer::new().panel(&specs, &panel)?.png()?;
    std::fs::write(output, png).with_context(|| format!("cannot write {}", output.display()))?;
    writeln!(
        out,
        "{} page {page} of {}: {}x{}, {} keys -> {}",
        profile.profile.id,
        profile.pages.len(),
        panel.size.width,
        panel.size.height,
        panel.keys.len(),
        output.display()
    )?;
    Ok(())
}

/// The profile `name`: a file, or an id in `config_dir`.
fn find(name: &str, config_dir: Option<&Path>) -> anyhow::Result<Profile> {
    let path = Path::new(name);
    if path.is_file()
        || path
            .extension()
            .is_some_and(|extension| extension == "toml")
    {
        let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {name}"))?;
        return Ok(Profile::from_toml(&text, path)?);
    }
    let Some(dir) = config_dir else {
        bail!("no configuration directory (neither XDG_CONFIG_HOME nor HOME is set)");
    };
    let loaded = layout::load(dir)?;
    if let Some(profile) = loaded.profiles.get(name) {
        return Ok(profile.clone());
    }
    let known: Vec<_> = loaded
        .profiles
        .iter()
        .map(|profile| profile.profile.id.as_str())
        .collect();
    bail!(
        "no profile `{name}` in {}; profiles: {}",
        dir.display(),
        known.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn demo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../keyvo-core/tests/fixtures/profiles/valid/demo.toml")
    }

    /// `(out, err)` of a preview, or its error.
    fn run(
        profile: &str,
        page: usize,
        dir: Option<&Path>,
    ) -> anyhow::Result<(String, String, Vec<u8>)> {
        let target = tempfile::tempdir()?;
        let output = target.path().join("grid.png");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        preview(profile, page, &output, dir, &mut out, &mut err)?;
        let text = String::from_utf8(out)?.replace(&output.display().to_string(), "grid.png");
        Ok((text, String::from_utf8(err)?, std::fs::read(&output)?))
    }

    #[test]
    fn a_profile_file_renders_to_a_png_of_the_panel() {
        let (out, err, png) = run(demo().to_str().unwrap(), 1, None).unwrap();
        assert_eq!(out, "demo page 1 of 2: 480x480, 9 keys -> grid.png\n");
        assert_eq!(
            err,
            "key 8: no bundled icon `lucide:no-such-icon`; drawn as lucide:circle-question-mark\n"
        );
        let picture = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
        assert_eq!((picture.width(), picture.height()), (480, 480));

        let profile =
            Profile::from_toml(&std::fs::read_to_string(demo()).unwrap(), &demo()).unwrap();
        let specs = render::page(&Stack::alone(&profile, 1).unwrap(), 9);
        let expected = Renderer::new()
            .panel(&specs, &display::captured_geometry().panel())
            .unwrap()
            .png()
            .unwrap();
        assert_eq!(png, expected);
    }

    #[test]
    fn a_profile_id_is_looked_up_in_the_config_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("profiles")).unwrap();
        std::fs::copy(demo(), dir.path().join("profiles/demo.toml")).unwrap();
        let (out, err, _) = run("demo", 2, Some(dir.path())).unwrap();
        assert_eq!(out, "demo page 2 of 2: 480x480, 9 keys -> grid.png\n");
        assert_eq!(err, "");

        let error = run("nope", 1, Some(dir.path())).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "no profile `nope` in {}; profiles: demo, global",
                dir.path().display()
            )
        );
        let error = run("nope", 1, None).unwrap_err();
        assert!(
            error.to_string().starts_with("no configuration directory"),
            "{error}"
        );
    }

    #[test]
    fn a_missing_page_or_file_is_an_error() {
        let error = run(demo().to_str().unwrap(), 3, None).unwrap_err();
        assert_eq!(error.to_string(), "profile `demo` has no page 3; it has 2");
        let error = run("/absent/x.toml", 1, None).unwrap_err();
        assert_eq!(error.to_string(), "cannot read /absent/x.toml");
    }
}
