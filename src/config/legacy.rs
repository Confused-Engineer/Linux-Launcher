//! Import of the pre-2.0 `LL.ini` format.
//!
//! ```ini
//! [unique_id]
//! title=Task Manager
//! command=C:\path\to\app.exe   ; or "start" to open `arguments` as a URL/URI
//! arguments=
//! icon=https://example.com/icon.png
//! weight=3                       ; sort order, ascending
//! ```

use super::{AppEntry, Config};

#[derive(Debug)]
pub struct LegacyError(String);

impl std::fmt::Display for LegacyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid legacy INI: {}", self.0)
    }
}

impl std::error::Error for LegacyError {}

/// Converts the contents of a legacy `LL.ini` into a [`Config`].
///
/// Entries are ordered by their `weight`; entries without a command are skipped.
pub fn from_legacy_ini(text: &str) -> Result<Config, LegacyError> {
    let ini = ini::Ini::load_from_str_noescape(text).map_err(|e| LegacyError(e.to_string()))?;

    let mut weighted: Vec<(i64, AppEntry)> = Vec::new();
    for (section, props) in ini.iter() {
        let Some(id) = section else { continue };
        let get = |key: &str| props.get(key).map(str::trim).unwrap_or_default();

        let command = get("command");
        if command.is_empty() {
            continue;
        }
        let arguments = get("arguments");

        // `cmd /C start <url>` was the old way to open URLs and URIs.
        let (target, args) = if command.eq_ignore_ascii_case("start") && !arguments.is_empty() {
            (arguments, "")
        } else {
            (command, arguments)
        };

        let name = match get("title") {
            "" => id,
            title => title,
        };

        let weight = get("weight").parse::<i64>().unwrap_or(i64::MAX);
        weighted.push((
            weight,
            AppEntry {
                args: args.to_owned(),
                icon: get("icon").to_owned(),
                ..AppEntry::new(name, target)
            },
        ));
    }
    weighted.sort_by_key(|(w, _)| *w); // stable: ties keep file order

    let mut config = Config::default();
    for (_, app) in weighted {
        config.add_app(app);
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/legacy.ini");

    #[test]
    fn converts_fixture_in_weight_order() {
        let c = from_legacy_ini(FIXTURE).unwrap();
        let names: Vec<_> = c.apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Google", "Volume Mixer", "Task Manager"]);

        let google = &c.apps[0];
        assert_eq!(google.target, "https://www.google.com/");
        assert!(google.args.is_empty());

        let tm = &c.apps[2];
        assert!(tm.target.ends_with("Task Manager.lnk"));
        assert!(tm.icon.starts_with("https://"));
    }

    #[test]
    fn keeps_arguments_for_real_commands() {
        let c = from_legacy_ini("[x]\ntitle=X\ncommand=C:\\x.exe\narguments=-a -b\n").unwrap();
        assert_eq!(c.apps[0].target, r"C:\x.exe");
        assert_eq!(c.apps[0].args, "-a -b");
    }

    #[test]
    fn falls_back_to_section_name_and_skips_incomplete() {
        let c = from_legacy_ini("[noname]\ncommand=x.exe\n[nocmd]\ntitle=Y\n").unwrap();
        assert_eq!(c.apps.len(), 1);
        assert_eq!(c.apps[0].name, "noname");
    }
}
