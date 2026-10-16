//! Persistent launcher state (device only): which installed apps the user hid
//! and which apps they launched recently.
//!
//! Android may kill the process at any time, so anything the user changed must
//! survive it. The state is a small line-oriented text file in the app's
//! private data directory:
//!
//! ```text
//! launcher-lab-state/v1
//! hidden<TAB>com.example.notes<TAB>com.example.notes.Main<TAB>Notes
//! recent<TAB>com.android.chrome<TAB>com.android.chrome.Main<TAB>Chrome<TAB>2A6FF5
//! ```
//!
//! Writes go to a temporary file that is then renamed over the real one, so a
//! crash mid-write cannot leave a truncated file behind. Reads are validated:
//! a line with the wrong shape, an invalid package name, an unparsable colour
//! or a duplicate is dropped instead of trusted, and the lists are capped so a
//! hand-edited file cannot blow up the UI.
//!
//! On the desktop build — or if Android never handed us a data directory — the
//! store is inert: [`load`] yields the defaults and [`save`] does nothing.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::model::hidden::HiddenReal;
use crate::platform;

/// File name inside the app's private data directory.
const FILE_NAME: &str = "launcher-lab-state.txt";
/// First line of the file; changing the format means changing this tag.
const VERSION_TAG: &str = "launcher-lab-state/v1";
/// Upper bound on persisted rows (the UI lists are bounded the same way).
pub const MAX_HIDDEN: usize = 48;
/// Recents kept across restarts.
pub const MAX_RECENTS: usize = 8;

static DATA_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Record the app's private data directory. Called once from `android_main`.
pub fn set_dir(dir: Option<PathBuf>) {
    if let Ok(mut slot) = DATA_DIR.lock() {
        *slot = dir;
    }
}

/// The app's private data directory, if one was provided.
pub fn dir() -> Option<PathBuf> {
    DATA_DIR.lock().ok().and_then(|slot| slot.clone())
}

/// One recently launched app: the part of a recents task that outlives the
/// process (the demo-only task fields are rebuilt on load).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentApp {
    pub package: String,
    pub activity: String,
    pub label: String,
    pub accent: u32,
}

/// Everything that survives a process restart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub hidden: Vec<HiddenReal>,
    pub recents: Vec<RecentApp>,
}

/// Serialise to the on-disk format. Labels are flattened first: a tab or a
/// newline inside a display name must not be able to forge a column.
pub fn encode(state: &State) -> String {
    let mut out = String::with_capacity(64 + 96 * state.hidden.len() + 96 * state.recents.len());
    out.push_str(VERSION_TAG);
    out.push('\n');
    for hidden in state.hidden.iter().take(MAX_HIDDEN) {
        out.push_str("hidden\t");
        out.push_str(&sanitize_field(&hidden.package));
        out.push('\t');
        out.push_str(&sanitize_field(&hidden.activity));
        out.push('\t');
        out.push_str(&sanitize_field(&hidden.label));
        out.push('\n');
    }
    for recent in state.recents.iter().take(MAX_RECENTS) {
        out.push_str("recent\t");
        out.push_str(&sanitize_field(&recent.package));
        out.push('\t');
        out.push_str(&sanitize_field(&recent.activity));
        out.push('\t');
        out.push_str(&sanitize_field(&recent.label));
        out.push('\t');
        out.push_str(&format!("{:06X}", recent.accent));
        out.push('\n');
    }
    out
}

/// Replace the column separator and line breaks so a field can never forge a
/// new column or row.
fn sanitize_field(raw: &str) -> String {
    raw.chars()
        .map(|c| match c {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
}

/// Parse the on-disk format. Always returns a usable state: a truncated file,
/// an unknown version tag, malformed rows, invalid packages and duplicates are
/// silently skipped (the user's data is validated, never trusted).
pub fn decode(text: &str) -> State {
    let mut state = State::default();
    let mut lines = text.lines();
    match lines.next() {
        Some(tag) if tag.trim() == VERSION_TAG => {}
        _ => return state,
    }
    for line in lines {
        let mut cols = line.split('\t');
        match cols.next().unwrap_or("") {
            "hidden" => {
                let (Some(package), Some(activity), Some(label)) =
                    (cols.next(), cols.next(), cols.next())
                else {
                    continue;
                };
                if state.hidden.len() >= MAX_HIDDEN {
                    continue;
                }
                if platform::validate_component(package, activity).is_err() {
                    continue;
                }
                if state.hidden.iter().any(|row| row.package == package) {
                    continue;
                }
                state.hidden.push(HiddenReal {
                    package: package.to_owned(),
                    activity: activity.to_owned(),
                    label: platform::sanitize_label(label, package),
                });
            }
            "recent" => {
                let (Some(package), Some(activity), Some(label), Some(accent)) =
                    (cols.next(), cols.next(), cols.next(), cols.next())
                else {
                    continue;
                };
                if state.recents.len() >= MAX_RECENTS {
                    continue;
                }
                if platform::validate_component(package, activity).is_err() {
                    continue;
                }
                let Ok(accent) = u32::from_str_radix(accent.trim(), 16) else {
                    continue;
                };
                if state.recents.iter().any(|row| row.package == package) {
                    continue;
                }
                state.recents.push(RecentApp {
                    package: package.to_owned(),
                    activity: activity.to_owned(),
                    label: platform::sanitize_label(label, package),
                    accent,
                });
            }
            _ => {}
        }
    }
    state
}

/// Read the saved state. A missing file (first run) is not an error; a real
/// read problem is reported so the UI can say what happened.
pub fn load() -> Result<State, String> {
    let Some(dir) = dir() else {
        return Ok(State::default());
    };
    let path = dir.join(FILE_NAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(decode(&text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
        Err(error) => Err(format!("Could not read the saved launcher state: {error}")),
    }
}

/// Write the state file atomically (temporary file, then rename).
pub fn save(state: &State) -> Result<(), String> {
    let Some(dir) = dir() else {
        return Ok(());
    };
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create the launcher data directory: {error}"))?;
    let path = dir.join(FILE_NAME);
    let temporary = dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(&temporary, encode(state))
        .map_err(|error| format!("Could not save the launcher state: {error}"))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("Could not save the launcher state: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> State {
        State {
            hidden: vec![
                HiddenReal {
                    package: "com.example.notes".to_owned(),
                    activity: "com.example.notes.Main".to_owned(),
                    label: "Notes".to_owned(),
                },
                HiddenReal {
                    package: "org.demo.reader".to_owned(),
                    activity: "org.demo.reader.Main".to_owned(),
                    label: "Reader".to_owned(),
                },
            ],
            recents: vec![RecentApp {
                package: "com.android.chrome".to_owned(),
                activity: "com.android.chrome.Main".to_owned(),
                label: "Chrome".to_owned(),
                accent: 0x2A6FF5,
            }],
        }
    }

    #[test]
    fn round_trips_a_state() {
        let state = sample();
        assert_eq!(decode(&encode(&state)), state);
    }

    #[test]
    fn rejects_an_unknown_version() {
        let text = "some-other-format/v9\nhidden\tcom.a.b\tcom.a.b.Main\tB\n";
        assert_eq!(decode(text), State::default());
    }

    #[test]
    fn rejects_an_empty_file() {
        assert_eq!(decode(""), State::default());
    }

    #[test]
    fn skips_rows_that_do_not_validate() {
        let text = concat!(
            "launcher-lab-state/v1\n",
            "hidden\tnot a package\t\tBroken\n",
            "hidden\tcom.ok.app\tcom.ok.app.Main\tOk\n",
            "hidden\tcom.ok.app\tcom.ok.app.Main\tDuplicate\n",
            "hidden\tonly-one-column\n",
            "recent\tcom.ok.app\tcom.ok.app.Main\tOk\tZZZ\n",
            "recent\tcom.ok.app\tcom.ok.app.Main\tOk\t0099FF\n",
            "recent\tcom.other.app\tnope\tBad\t000001\n",
            "garbage\tline\n",
            "\n",
        );
        let state = decode(text);
        assert_eq!(state.hidden.len(), 1);
        assert_eq!(state.hidden[0].package, "com.ok.app");
        assert_eq!(state.hidden[0].label, "Ok");
        assert_eq!(state.recents.len(), 1);
        assert_eq!(state.recents[0].accent, 0x0099_FF);
    }

    #[test]
    fn flattens_control_characters_in_labels() {
        let state = State {
            hidden: vec![HiddenReal {
                package: "com.a.b".to_owned(),
                activity: "com.a.b.Main".to_owned(),
                label: "Line\nbreak\there".to_owned(),
            }],
            recents: Vec::new(),
        };
        let decoded = decode(&encode(&state));
        assert_eq!(decoded.hidden[0].label, "Line break here");
    }

    #[test]
    fn caps_the_row_counts() {
        let mut text = String::from("launcher-lab-state/v1\n");
        for index in 0..(MAX_HIDDEN + 5) {
            text.push_str(&format!(
                "hidden\tcom.example.p{index}\tcom.example.p{index}.Main\tP{index}\n"
            ));
        }
        for index in 0..(MAX_RECENTS + 3) {
            text.push_str(&format!(
                "recent\tcom.example.r{index}\tcom.example.r{index}.Main\tR{index}\t000001\n"
            ));
        }
        let state = decode(&text);
        assert_eq!(state.hidden.len(), MAX_HIDDEN);
        assert_eq!(state.recents.len(), MAX_RECENTS);
    }

    #[test]
    fn is_inert_without_a_data_directory() {
        if dir().is_some() {
            return;
        }
        assert!(save(&sample()).is_ok());
        assert_eq!(load().expect("load never fails without a directory"), State::default());
    }
}
