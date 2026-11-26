use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use keyvalues_parser::{Obj, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Library {
    pub path: PathBuf,
    pub client: Client,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Client {
    Native,
    Flatpak,
}

impl Client {
    pub fn name(self) -> &'static str {
        match self {
            Self::Native => "原生 Steam",
            Self::Flatpak => "Flatpak Steam",
        }
    }

    pub fn install(self, app_id: u32) -> Result<(), String> {
        let mut command = match self {
            Self::Native => Command::new("steam"),
            Self::Flatpak => {
                let mut command = Command::new("flatpak");
                command.args(["run", "com.valvesoftware.Steam"]);
                command
            }
        };
        let mut child = command
            .arg(format!("steam://install/{app_id}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("无法打开{}（{error}）。", self.name()))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SteamAccount {
    pub client: Client,
    pub steam_id: u32,
    pub config_dir: PathBuf,
}

pub fn discover() -> (Vec<Library>, Vec<SteamAccount>) {
    let Some(home) = env::home_dir() else {
        return (Vec::new(), Vec::new());
    };
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"));
    discover_at(&home, &data)
}

fn discover_at(home: &Path, data: &Path) -> (Vec<Library>, Vec<SteamAccount>) {
    let roots = [
        (Client::Native, data.join("Steam")),
        (Client::Native, home.join(".local/share/Steam")),
        (Client::Native, home.join(".steam/steam")),
        (Client::Native, home.join(".steam/root")),
        (Client::Native, home.join(".steam/debian-installation")),
        (
            Client::Flatpak,
            home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
        ),
        (
            Client::Flatpak,
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ),
        (
            Client::Flatpak,
            home.join(".var/app/com.valvesoftware.Steam/.steam/steam"),
        ),
    ];
    let mut seen = Vec::new();
    let mut libraries = Vec::new();
    let mut accounts = Vec::new();
    for (client, root) in roots {
        let Ok(root) = root.canonicalize() else {
            continue;
        };
        if seen.contains(&(client, root.clone())) {
            continue;
        }
        seen.push((client, root.clone()));

        let paths = read_vdf(&root.join("steamapps/libraryfolders.vdf"), "libraryfolders")
            .map(|vdf| library_paths(&vdf))
            .unwrap_or_default();
        for path in std::iter::once(root.clone()).chain(paths) {
            let Ok(path) = path.canonicalize() else {
                continue;
            };
            let library = Library { path, client };
            if library.path.join("steamapps").is_dir() && !libraries.contains(&library) {
                libraries.push(library);
            }
        }

        if let Ok(entries) = fs::read_dir(root.join("userdata")) {
            for entry in entries.flatten() {
                let Ok(steam_id) = entry.file_name().to_string_lossy().parse::<u32>() else {
                    continue;
                };
                let config_dir = entry.path().join("config");
                if steam_id > 0 && config_dir.is_dir() {
                    accounts.push(SteamAccount {
                        client,
                        steam_id,
                        config_dir,
                    });
                }
            }
        }
    }
    accounts.sort_by(|a, b| a.config_dir.cmp(&b.config_dir));
    (libraries, accounts)
}

pub fn installed_location(library: &Library, app_id: u32) -> Option<PathBuf> {
    let steamapps = library.path.join("steamapps");
    let vdf = read_vdf(
        &steamapps.join(format!("appmanifest_{app_id}.acf")),
        "AppState",
    )?;
    let app = vdf.get_obj()?;
    if field(app, "appid")?.parse::<u32>().ok()? != app_id
        || field(app, "StateFlags")?.parse::<u32>().ok()? & 4 == 0
    {
        return None;
    }
    let folder = Path::new(field(app, "installdir")?);
    let mut components = folder.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return None;
    }
    let path = steamapps.join("common").join(folder);
    path.is_dir().then_some(path)
}

fn read_vdf(path: &Path, root: &str) -> Option<Value<'static>> {
    let text = fs::read_to_string(path).ok()?;
    let vdf = keyvalues_parser::parse(text.trim_start_matches('\u{feff}')).ok()?;
    vdf.key
        .eq_ignore_ascii_case(root)
        .then(|| vdf.into_vdf().into_owned().value)
}

fn field<'a>(object: &'a Obj<'_>, name: &str) -> Option<&'a str> {
    object
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))?
        .1
        .first()?
        .get_str()
}

fn library_paths(value: &Value<'_>) -> Vec<PathBuf> {
    let Some(libraries) = value.get_obj() else {
        return Vec::new();
    };
    libraries
        .iter()
        .filter(|(key, _)| key.parse::<u32>().is_ok())
        .flat_map(|(_, values)| values)
        .filter_map(|value| {
            value
                .get_str()
                .or_else(|| field(value.get_obj()?, "path"))
                .map(PathBuf::from)
        })
        .filter(|path| path.is_absolute())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_native_and_flatpak_and_reads_installed_app() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path();
        let native = root.join(".local/share/Steam");
        let flatpak = root.join(".var/app/com.valvesoftware.Steam/data/Steam");
        let extra = root.join("External Steam Library");
        fs::create_dir_all(native.join("steamapps")).unwrap();
        fs::create_dir_all(flatpak.join("steamapps")).unwrap();
        fs::create_dir_all(flatpak.join("userdata/42/config")).unwrap();
        let game = extra.join("steamapps/common/Test Game");
        fs::create_dir_all(&game).unwrap();
        fs::write(
            native.join("steamapps/libraryfolders.vdf"),
            format!(
                r#""libraryfolders" {{ "1" {{ "path" "{}" }} }}"#,
                extra.display()
            ),
        )
        .unwrap();
        fs::write(
            extra.join("steamapps/appmanifest_440.acf"),
            r#""AppState" { "appid" "440" "StateFlags" "4" "installdir" "Test Game" }"#,
        )
        .unwrap();
        let (libraries, accounts) = discover_at(root, &root.join(".local/share"));
        assert_eq!(libraries.len(), 3);
        assert_eq!(accounts[0].client, Client::Flatpak);
        assert_eq!(
            libraries
                .iter()
                .find_map(|library| installed_location(library, 440)),
            Some(game)
        );
    }
}
