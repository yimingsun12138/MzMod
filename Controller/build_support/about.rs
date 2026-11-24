use crate::region::Region;
use std::{fs, io, path::Path};

fn readme_section<'a>(readme: &'a str, heading: &str) -> io::Result<Vec<&'a str>> {
    let mut lines = readme.lines().map(str::trim);
    if !lines.any(|line| line == heading) {
        return Err(io::Error::other(format!(
            "missing README heading {heading:?}"
        )));
    }
    let section = lines
        .clone()
        .take_while(|line| !line.starts_with('#'))
        .collect();
    if lines.any(|line| line == heading) {
        return Err(io::Error::other(format!(
            "duplicate README heading {heading:?}"
        )));
    }
    Ok(section)
}

pub(super) fn write_about_copy(output: &Path, builds: &[String]) -> io::Result<()> {
    println!("cargo:rerun-if-changed=../README.md");
    println!("cargo:rerun-if-changed=Cargo.toml");
    let readme = fs::read_to_string("../README.md")?;
    let introduction = readme_section(&readme, "# MzMod")?;
    let mut introduction = introduction.into_iter().filter(|line| !line.is_empty());
    let description = introduction
        .next()
        .ok_or_else(|| io::Error::other("missing README introduction"))?;
    let info: Vec<_> = introduction
        .filter_map(|line| line.split_once('：'))
        .map(|(label, value)| (label.trim(), value.trim()))
        .collect();
    let field = |label| {
        let mut values = info
            .iter()
            .filter_map(|&(name, value)| (name == label).then_some(value));
        let value = values.next();
        if values.next().is_some() {
            return Err(io::Error::other(format!(
                "duplicate README field {label:?}"
            )));
        }
        value
            .filter(|value| !value.is_empty())
            .ok_or_else(|| io::Error::other(format!("missing README field {label:?}")))
    };
    if field("作者")? != env!("CARGO_PKG_AUTHORS") {
        return Err(io::Error::other(
            "README author must match the Cargo package author",
        ));
    }
    field("更新时间")?;
    let version = crate::RELEASE_VERSION;
    if field("当前版本")? != version {
        return Err(io::Error::other(format!(
            "README release version must match the Cargo package version {version}"
        )));
    }
    let manifest =
        toml_edit::Document::parse(fs::read_to_string("Cargo.toml")?).map_err(io::Error::other)?;
    let series = manifest["package"]["metadata"]["mzmod"]["game-series"]
        .as_str()
        .ok_or_else(|| io::Error::other("game-series must be a version string"))?;
    let compatibility = field("适配游戏版本")?;
    for (region, build) in Region::ALL.into_iter().zip(builds) {
        let name = match region {
            Region::Int => "国际服",
            Region::Cn => "国服",
        };
        let expected = format!("{series}.{build}");
        let actual = compatibility
            .split_once(name)
            .and_then(|(_, text)| text.strip_prefix('v'))
            .and_then(|number| {
                number
                    .split(|ch: char| !ch.is_ascii_digit() && ch != '.')
                    .next()
            });
        if actual != Some(expected.as_str()) {
            return Err(io::Error::other(format!(
                "README compatibility must include {name}v{expected}"
            )));
        }
    }
    let mut steps = Vec::new();
    for line in readme_section(&readme, "### 使用Windows设置器")? {
        if line.is_empty() {
            continue;
        }
        let prefix = format!("{}. ", steps.len() + 1);
        let text = line
            .strip_prefix(&prefix)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| {
                io::Error::other(format!("expected README installation step {prefix}"))
            })?;
        steps.push(text.replace('`', ""));
    }
    if steps.is_empty() {
        return Err(io::Error::other("missing README installation steps"));
    }
    let disclaimer_lines = readme_section(&readme, "## 免责声明")?;
    let mut paragraphs = disclaimer_lines
        .split(|line| line.is_empty())
        .filter(|paragraph| !paragraph.is_empty());
    let disclaimer = match (paragraphs.next(), paragraphs.next()) {
        (Some(text), None) => text.join(" "),
        _ => return Err(io::Error::other("expected one README disclaimer paragraph")),
    };
    fs::write(output.join("about_copy.rs"), format!(
        "const DESCRIPTION: &str = {description:?};\nconst MOD_INFO: &[(&str, &str)] = &{info:?};\nconst INSTALLATION: &[&str] = &{steps:?};\nconst DISCLAIMER: &str = {disclaimer:?};\n"
    ))
}
