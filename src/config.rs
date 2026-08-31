use std::{fs, path::Path};

pub const DEFAULT_INI: &str = include_str!("../EldenRingWindowedHDR.ini");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Observe,
    UnlockHdrMenu,
    EmulateHdrFullscreenState,
    EmulateHdrAndSetPq,
    #[default]
    WindowedHdr,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::UnlockHdrMenu => "unlock_hdr_menu",
            Self::EmulateHdrFullscreenState => "emulate_hdr_fullscreen_state",
            Self::EmulateHdrAndSetPq => "emulate_hdr_and_set_pq",
            Self::WindowedHdr => "windowed_hdr",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
    pub retired_force_pq_requested: bool,
}

impl Config {
    pub fn load_or_create(path: &Path) -> Result<(Self, bool), String> {
        if !path.exists() {
            fs::write(path, DEFAULT_INI)
                .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
            return Ok((Self::default(), true));
        }

        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Self::parse(&text).map(|config| (config, false))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut config = Self::default();
        let mut section = String::new();

        for (index, original_line) in text.lines().enumerate() {
            let line_number = index + 1;
            let line = original_line
                .trim_start_matches('\u{feff}')
                .split([';', '#'])
                .next()
                .unwrap_or_default()
                .trim();

            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_ascii_lowercase();
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("invalid INI syntax on line {line_number}"));
            };
            if section != "hdr" || !key.trim().eq_ignore_ascii_case("mode") {
                continue;
            }

            let requested_mode = value.trim().to_ascii_lowercase();
            config.mode = match requested_mode.as_str() {
                "observe" => Mode::Observe,
                "unlock_hdr_menu" => Mode::UnlockHdrMenu,
                "emulate_hdr_fullscreen_state" => Mode::EmulateHdrFullscreenState,
                "emulate_hdr_and_set_pq" => Mode::EmulateHdrAndSetPq,
                "windowed_hdr" => Mode::WindowedHdr,
                "force_pq_if_hdr10" => {
                    config.retired_force_pq_requested = true;
                    Mode::Observe
                }
                _ => {
                    return Err(format!(
                        "mode must be observe, unlock_hdr_menu, emulate_hdr_fullscreen_state, emulate_hdr_and_set_pq, or windowed_hdr (line {line_number})"
                    ));
                }
            };
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_windowed_hdr() {
        assert_eq!(Config::parse("").unwrap().mode, Mode::WindowedHdr);
    }

    #[test]
    fn enables_the_corrected_menu_unlock_experiment() {
        let config = Config::parse("\u{feff}[HDR]\nMODE = Unlock_HDR_Menu ; test").unwrap();
        assert_eq!(config.mode, Mode::UnlockHdrMenu);
        assert!(!config.retired_force_pq_requested);
    }

    #[test]
    fn enables_the_guarded_internal_fullscreen_state_experiment() {
        let config = Config::parse("[HDR]\nmode=emulate_hdr_fullscreen_state").unwrap();
        assert_eq!(config.mode, Mode::EmulateHdrFullscreenState);
        assert!(!config.retired_force_pq_requested);
    }

    #[test]
    fn enables_the_present_synchronized_pq_experiment() {
        let config = Config::parse("[HDR]\nmode=emulate_hdr_and_set_pq").unwrap();
        assert_eq!(config.mode, Mode::EmulateHdrAndSetPq);
        assert!(!config.retired_force_pq_requested);
    }

    #[test]
    fn enables_the_persistent_windowed_hdr_mode() {
        let config = Config::parse("[HDR]\nmode=windowed_hdr").unwrap();
        assert_eq!(config.mode, Mode::WindowedHdr);
        assert!(!config.retired_force_pq_requested);
    }

    #[test]
    fn retires_legacy_force_pq_mode_to_observation() {
        let config = Config::parse("[HDR]\nmode=force_pq_if_hdr10").unwrap();
        assert_eq!(config.mode, Mode::Observe);
        assert!(config.retired_force_pq_requested);
    }

    #[test]
    fn rejects_unknown_mode() {
        let error = Config::parse("[HDR]\nmode=force_everything").unwrap_err();
        assert!(error.contains("observe"));
    }

    #[test]
    fn ignores_unknown_sections_and_keys() {
        let config = Config::parse("[OTHER]\nmode=bad\n[HDR]\nfuture_key=42").unwrap();
        assert_eq!(config.mode, Mode::WindowedHdr);
        assert!(!config.retired_force_pq_requested);
    }
}
