use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::environment;
use crate::paths::HOME;
use crate::process::{chown_user, command_available, run_as_user};

pub const PREFERENCE: [VideoCodec; 3] = [VideoCodec::Av1, VideoCodec::Hevc, VideoCodec::H264];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodec {
    Av1,
    Hevc,
    H264,
}

impl std::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Av1 => write!(f, "AV1"),
            Self::Hevc => write!(f, "H.265/HEVC"),
            Self::H264 => write!(f, "H.264"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodecPolicy {
    pub render_node: Option<String>,
    pub hardware_codecs: Vec<VideoCodec>,
    pub preference: Vec<VideoCodec>,
}

impl CodecPolicy {
    pub fn detect(runtime_dir: &Path) -> Self {
        let mut best = Self {
            render_node: None,
            hardware_codecs: Vec::new(),
            preference: PREFERENCE.to_vec(),
        };
        if !command_available("vainfo") {
            eprintln!("steam-remote: vainfo is unavailable; Steam will use its own codec fallback");
            return best;
        }

        let env = environment::base(runtime_dir, None);
        for node in render_nodes() {
            let device = node.to_string_lossy();
            let Ok(output) =
                run_as_user(&["vainfo", "--display", "drm", "--device", &device], &env)
            else {
                continue;
            };
            if !output.status.success() {
                continue;
            }
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let codecs = parse_vainfo(&text);
            if capability_score(&codecs) > capability_score(&best.hardware_codecs) {
                best.render_node = Some(node.display().to_string());
                best.hardware_codecs = codecs;
            }
        }
        best
    }

    pub fn hardware_encoding_available(&self) -> bool {
        !self.hardware_codecs.is_empty()
    }

    pub fn log(&self) {
        let preference = display_codecs(&self.preference, " > ");
        let available = display_codecs(&self.hardware_codecs, ", ");
        eprintln!("steam-remote: codec preference: {preference}");
        eprintln!(
            "steam-remote: VA-API hardware codecs on {}: {}",
            self.render_node.as_deref().unwrap_or("no render node"),
            if available.is_empty() {
                "none (Steam software fallback remains available)"
            } else {
                &available
            }
        );
    }
}

pub fn display_codecs(codecs: &[VideoCodec], separator: &str) -> String {
    codecs
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(separator)
}

fn render_nodes() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("STEAM_REMOTE_RENDER_NODE") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return vec![path];
        }
        eprintln!(
            "steam-remote: ignoring missing STEAM_REMOTE_RENDER_NODE {}",
            path.display()
        );
    }
    let mut nodes: Vec<PathBuf> = fs::read_dir("/dev/dri")
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"))
        })
        .collect();
    nodes.sort();
    nodes
}

fn parse_vainfo(text: &str) -> Vec<VideoCodec> {
    let mut found = HashSet::new();
    for line in text.lines().map(str::to_ascii_lowercase) {
        if !line.contains("vaentrypointencslice") {
            continue;
        }
        if line.contains("vaprofileav1") {
            found.insert(VideoCodec::Av1);
        } else if line.contains("vaprofilehevc") {
            found.insert(VideoCodec::Hevc);
        } else if line.contains("vaprofileh264") {
            found.insert(VideoCodec::H264);
        }
    }
    PREFERENCE
        .iter()
        .copied()
        .filter(|codec| found.contains(codec))
        .collect()
}

fn capability_score(codecs: &[VideoCodec]) -> u8 {
    codecs.iter().fold(0, |score, codec| {
        score
            | match codec {
                VideoCodec::Av1 => 4,
                VideoCodec::Hevc => 2,
                VideoCodec::H264 => 1,
            }
    })
}

pub fn configure_steam_host(policy: &CodecPolicy) -> Result<usize> {
    let mut configured = 0;
    let mut seen = HashSet::new();
    for root in [
        Path::new(HOME).join(".steam/steam/userdata"),
        Path::new(HOME).join(".local/share/Steam/userdata"),
    ] {
        let Ok(accounts) = fs::read_dir(root) else {
            continue;
        };
        for account in accounts.flatten() {
            let path = account.path().join("config/localconfig.vdf");
            if !path.is_file() {
                continue;
            }
            let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if !seen.insert(canonical) {
                continue;
            }
            configure_file(&path, policy.hardware_encoding_available())?;
            configured += 1;
        }
    }
    Ok(configured)
}

fn configure_file(path: &Path, hardware_encoding: bool) -> Result<()> {
    let current = fs::read_to_string(path)
        .with_context(|| format!("reading Steam settings from {}", path.display()))?;
    let updated = update_streaming_v2(&current, hardware_encoding);
    if current == updated {
        return Ok(());
    }

    let metadata = fs::metadata(path)?;
    let temporary = path.with_extension("vdf.steam-remote.tmp");
    let mut file = fs::File::create(&temporary)
        .with_context(|| format!("creating temporary Steam settings {}", temporary.display()))?;
    file.write_all(updated.as_bytes())?;
    file.sync_all()?;
    fs::set_permissions(&temporary, metadata.permissions())?;
    fs::rename(&temporary, path)
        .with_context(|| format!("replacing Steam settings at {}", path.display()))?;
    chown_user(path);
    Ok(())
}

fn update_streaming_v2(input: &str, hardware_encoding: bool) -> String {
    let newline = if input.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing_newline = input.ends_with('\n');
    let mut lines: Vec<String> = input
        .lines()
        .map(|line| line.trim_end_matches('\r').into())
        .collect();
    let values = [
        ("EnableStreaming", "1"),
        ("ServerConfigEnabled", "1"),
        (
            "EnableHardwareEncoding",
            if hardware_encoding { "1" } else { "0" },
        ),
    ];

    if let Some(section_key) = lines
        .iter()
        .position(|line| vdf_key(line) == Some("streaming_v2"))
    {
        if let Some((open, close)) = block_bounds(&lines, section_key) {
            let child_indent = format!("{}\t", leading_whitespace(&lines[section_key]));
            let mut present = HashSet::new();
            let mut depth = 1_i32;
            for line in lines.iter_mut().take(close).skip(open + 1) {
                match line.trim() {
                    "{" => depth += 1,
                    "}" => depth -= 1,
                    _ if depth == 1 => {
                        if let Some((key, value)) =
                            values.iter().find(|(key, _)| vdf_key(line) == Some(*key))
                        {
                            *line = format!("{child_indent}\"{key}\"\t\t\"{value}\"");
                            present.insert(*key);
                        }
                    }
                    _ => {}
                }
            }
            let missing: Vec<String> = values
                .iter()
                .filter(|(key, _)| !present.contains(key))
                .map(|(key, value)| format!("{child_indent}\"{key}\"\t\t\"{value}\""))
                .collect();
            lines.splice(close..close, missing);
        }
    } else if let Some(root_close) = lines.iter().rposition(|line| line.trim() == "}") {
        lines.splice(
            root_close..root_close,
            [
                "\t\"streaming_v2\"".into(),
                "\t{".into(),
                "\t\t\"EnableStreaming\"\t\t\"1\"".into(),
                "\t\t\"ServerConfigEnabled\"\t\t\"1\"".into(),
                format!(
                    "\t\t\"EnableHardwareEncoding\"\t\t\"{}\"",
                    if hardware_encoding { "1" } else { "0" }
                ),
                "\t}".into(),
            ],
        );
    }

    let mut output = lines.join(newline);
    if trailing_newline {
        output.push_str(newline);
    }
    output
}

fn vdf_key(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix('"')?;
    rest.split_once('"').map(|(key, _)| key)
}

fn leading_whitespace(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn block_bounds(lines: &[String], key_index: usize) -> Option<(usize, usize)> {
    let open = (key_index + 1..lines.len()).find(|index| lines[*index].trim() == "{")?;
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate().skip(open) {
        match line.trim() {
            "{" => depth += 1,
            "}" => {
                depth -= 1;
                if depth == 0 {
                    return Some((open, index));
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encode_profiles_in_preference_order() {
        let output = r#"
            VAProfileH264High : VAEntrypointEncSlice
            VAProfileHEVCMain : VAEntrypointVLD
            VAProfileAV1Profile0 : VAEntrypointEncSliceLP
            VAProfileHEVCMain : VAEntrypointEncSlice
        "#;
        assert_eq!(
            parse_vainfo(output),
            vec![VideoCodec::Av1, VideoCodec::Hevc, VideoCodec::H264]
        );
    }

    #[test]
    fn enables_hardware_encoding_in_existing_streaming_section() {
        let input = "\"UserLocalConfigStore\"\n{\n\t\"streaming_v2\"\n\t{\n\t\t\"EnableStreaming\"\t\t\"0\"\n\t\t\"ServerConfigEnabled\"\t\t\"0\"\n\t}\n}\n";
        let updated = update_streaming_v2(input, true);
        assert!(updated.contains("\"EnableStreaming\"\t\t\"1\""));
        assert!(updated.contains("\"ServerConfigEnabled\"\t\t\"1\""));
        assert!(updated.contains("\"EnableHardwareEncoding\"\t\t\"1\""));
    }

    #[test]
    fn adds_missing_streaming_section_and_software_fallback() {
        let input = "\"UserLocalConfigStore\"\r\n{\r\n}\r\n";
        let updated = update_streaming_v2(input, false);
        assert!(updated.contains("\r\n\t\"streaming_v2\"\r\n"));
        assert!(updated.contains("\"EnableHardwareEncoding\"\t\t\"0\""));
    }
}
