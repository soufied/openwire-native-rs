use crate::playback::{is_audio_file, sound_name, Sampler};
use anyhow::{anyhow, bail, Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

const SCHEME: &str = "soundpad://";
const SOUND_PREFIX: &str = "sound/url/";
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(120);
const MAX_NAME_ATTEMPTS: usize = 10_000;

pub fn is_soundpad_link(raw: &str) -> bool {
    strip_prefix_ci(raw.trim(), SCHEME).is_some()
}

pub fn parse_soundpad_link(raw: &str) -> Option<String> {
    let rest = strip_prefix_ci(raw.trim(), SCHEME)?;
    let rest = strip_prefix_ci(rest, SOUND_PREFIX)?;
    let url = if starts_with_http(rest) {
        rest.to_string()
    } else {
        percent_decode(rest)
    };
    starts_with_http(&url).then_some(url)
}

pub fn file_name_from_url(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let last = path.rsplit('/').next().unwrap_or_default();
    let sanitized: String = percent_decode(last)
        .chars()
        .map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let name = sanitized.trim().trim_start_matches('.').trim();
    if name.is_empty() {
        "sound".to_string()
    } else {
        name.to_string()
    }
}

pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                out.push(high * 16 + low);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn download(url: &str, dir: &Path) -> Result<PathBuf> {
    let name = file_name_from_url(url);
    if !is_audio_file(Path::new(&name)) {
        bail!("{name} is not an audio file");
    }
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let (path, mut file) = reserve(dir, &name)?;
    let written = write_response(url, &mut file);
    drop(file);
    match written {
        Ok(()) => Ok(path),
        Err(err) => {
            let _ = std::fs::remove_file(&path);
            Err(err)
        }
    }
}

pub fn fetch_into_group(sampler: &Sampler, bank: usize, url: &str, folder: &Path) -> Result<String> {
    let path = download(url, folder)?;
    let done = AtomicUsize::new(0);
    match sampler.import_many(bank, vec![path.clone()], &done) {
        Ok(report) if report.added > 0 => Ok(sound_name(&path)),
        Ok(report) => {
            let _ = std::fs::remove_file(&path);
            Err(anyhow!(report
                .failed
                .first()
                .cloned()
                .unwrap_or_else(|| "file is not a supported audio file".to_string())))
        }
        Err(err) => {
            let _ = std::fs::remove_file(&path);
            Err(err)
        }
    }
}

fn write_response(url: &str, file: &mut File) -> Result<()> {
    let request_url = url.replace(' ', "%20");
    let response = ureq::get(&request_url)
        .timeout(TIMEOUT)
        .set("User-Agent", concat!("OpenWire/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|err| anyhow!("download failed: {err}"))?;
    let mut reader = response.into_reader().take(MAX_DOWNLOAD_BYTES + 1);
    let copied = std::io::copy(&mut reader, file).context("download interrupted")?;
    if copied > MAX_DOWNLOAD_BYTES {
        bail!("file is larger than {} MiB", MAX_DOWNLOAD_BYTES / (1024 * 1024));
    }
    Ok(())
}

fn reserve(dir: &Path, name: &str) -> Result<(PathBuf, File)> {
    let (stem, ext) = split_name(name);
    for index in 1..=MAX_NAME_ATTEMPTS {
        let candidate = if index == 1 {
            name.to_string()
        } else {
            format!("{stem} ({index}){ext}")
        };
        let path = dir.join(&candidate);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err).with_context(|| format!("cannot create {}", path.display())),
        }
    }
    bail!("no free file name for {name}")
}

fn split_name(name: &str) -> (String, String) {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (stem, ext)
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        Some(&text[prefix.len()..])
    } else {
        None
    }
}

fn starts_with_http(text: &str) -> bool {
    text.starts_with("https://") || text.starts_with("http://")
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_soundpad_link_with_plain_url() {
        let raw = "soundpad://sound/url/https://uwupad.me/soundpad/111023/Zima%20ne%20leto%20arbuz%20ne%20kotleta.mp3";
        assert_eq!(
            parse_soundpad_link(raw).as_deref(),
            Some("https://uwupad.me/soundpad/111023/Zima%20ne%20leto%20arbuz%20ne%20kotleta.mp3")
        );
    }

    #[test]
    fn parses_soundpad_link_with_encoded_url_and_upper_case_scheme() {
        let raw = "SoundPad://sound/url/https%3A%2F%2Fexample.com%2Fa.wav";
        assert_eq!(parse_soundpad_link(raw).as_deref(), Some("https://example.com/a.wav"));
    }

    #[test]
    fn rejects_other_links() {
        assert!(parse_soundpad_link("https://uwupad.me/download/111023").is_none());
        assert!(parse_soundpad_link("soundpad://sound/file/horn.wav").is_none());
        assert!(parse_soundpad_link("soundpad://sound/url/ftp://example.com/a.mp3").is_none());
        assert!(parse_soundpad_link("soundpad://").is_none());
        assert!(!is_soundpad_link("sound"));
        assert!(is_soundpad_link("soundpad://sound/url/https://example.com/a.mp3"));
    }

    #[test]
    fn decodes_percent_escapes_and_keeps_invalid_ones() {
        assert_eq!(percent_decode("Zima%20ne%20leto.mp3"), "Zima ne leto.mp3");
        assert_eq!(percent_decode("100%.mp3"), "100%.mp3");
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
    }

    #[test]
    fn names_file_from_url_path() {
        assert_eq!(
            file_name_from_url("https://uwupad.me/soundpad/111023/Zima%20ne%20leto%20arbuz%20ne%20kotleta.mp3?x=1"),
            "Zima ne leto arbuz ne kotleta.mp3"
        );
        assert_eq!(file_name_from_url("https://example.com/a/b%2Fc.mp3"), "b_c.mp3");
        assert_eq!(file_name_from_url("https://example.com/"), "sound");
        assert_eq!(file_name_from_url("https://example.com/..%2F..%2Fetc"), "_.._etc");
    }

    #[test]
    fn splits_name_for_unique_suffixes() {
        assert_eq!(split_name("horn.mp3"), ("horn".to_string(), ".mp3".to_string()));
        assert_eq!(split_name("a.b.wav"), ("a.b".to_string(), ".wav".to_string()));
        assert_eq!(split_name("noext"), ("noext".to_string(), String::new()));
    }
}
