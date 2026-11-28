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
        self.open("install", app_id)
    }

    pub fn verify(self, app_id: u32) -> Result<(), String> {
        self.open("validate", app_id)
    }

    pub fn run(self, app_id: u32) -> Result<(), String> {
        self.open("rungameid", app_id)
    }

    fn open(self, action: &str, app_id: u32) -> Result<(), String> {
        let mut command = match self {
            Self::Native => Command::new("steam"),
            Self::Flatpak => {
                let mut command = Command::new("flatpak");
                command.args(["run", "com.valvesoftware.Steam"]);
                command
            }
        };
        let mut child = command
            .arg(format!("steam://{action}/{app_id}"))
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

pub fn discover() -> Vec<Library> {
    let Some(home) = env::home_dir() else {
        return Vec::new();
    };
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"));
    discover_at(&home, &data)
}

fn discover_at(home: &Path, data: &Path) -> Vec<Library> {
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
    }
    libraries
}

pub fn installed_location(library: &Library, app_id: u32) -> Option<PathBuf> {
    let (folder, flags) = app_record(library, app_id)?;
    let path = library.path.join("steamapps/common").join(folder);
    (flags & 4 != 0 && path.is_dir()).then_some(path)
}

pub fn app_record(library: &Library, app_id: u32) -> Option<(String, u32)> {
    let steamapps = library.path.join("steamapps");
    let vdf = read_vdf(
        &steamapps.join(format!("appmanifest_{app_id}.acf")),
        "AppState",
    )?;
    let app = vdf.get_obj()?;
    if field(app, "appid")?.parse::<u32>().ok()? != app_id {
        return None;
    }
    let folder = field(app, "installdir")?;
    let flags = field(app, "StateFlags")?.parse().ok()?;
    valid_folder(folder).then(|| (folder.to_owned(), flags))
}

/// 一个已安装或部分安装的 Steam 应用（来自 appmanifest）。
#[derive(Clone, Debug)]
#[allow(dead_code)] // installdir / build_id 留给 ACF 修复与版本核对。
pub struct InstalledApp {
    pub app_id: u32,
    pub name: String,
    pub installdir: String,
    pub size_on_disk: u64,
    pub build_id: String,
    pub state_flags: u32,
}

/// 读取一个库下所有 appmanifest；读不到的条目跳过。
pub fn installed_apps(library: &Library) -> Vec<InstalledApp> {
    let steamapps = library.path.join("steamapps");
    let Ok(entries) = fs::read_dir(&steamapps) else {
        return Vec::new();
    };
    let mut apps: Vec<InstalledApp> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let app_id = name
                .strip_prefix("appmanifest_")?
                .strip_suffix(".acf")?
                .parse::<u32>()
                .ok()?;
            let vdf = read_vdf(&entry.path(), "AppState")?;
            let object = vdf.get_obj()?;
            if field(object, "appid")?.parse::<u32>().ok()? != app_id {
                return None;
            }
            Some(InstalledApp {
                app_id,
                name: field(object, "name").unwrap_or("").trim().to_owned(),
                installdir: field(object, "installdir").unwrap_or("").to_owned(),
                size_on_disk: field(object, "SizeOnDisk")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
                build_id: field(object, "buildid").unwrap_or("").to_owned(),
                state_flags: field(object, "StateFlags")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
            })
        })
        .collect();
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.app_id.cmp(&b.app_id))
    });
    apps
}

pub fn valid_folder(folder: &str) -> bool {
    !folder.contains(['/', '\\'])
        && !folder.contains(char::is_control)
        && matches!(
            Path::new(folder).components().next(),
            Some(Component::Normal(_))
        )
}

pub fn check_import_target(library: &Library, app_id: u32, folder: &str) -> Result<(), String> {
    if let Some((registered, _)) = app_record(library, app_id)
        && folder != registered
    {
        return Err(format!("这个 AppID 的 Steam 安装目录是 {registered}。"));
    }
    for entry in library
        .path
        .join("steamapps")
        .read_dir()
        .map_err(|err| err.to_string())?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(|name| {
            name.strip_prefix("appmanifest_")?
                .strip_suffix(".acf")?
                .parse::<u32>()
                .ok()
        }) else {
            continue;
        };
        if id != app_id
            && app_record(library, id).is_some_and(|(registered, _)| registered == folder)
        {
            return Err(format!("该目录已由 Steam AppID {id} 使用。"));
        }
    }
    Ok(())
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
    fn lists_installed_apps_from_manifests() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path();
        let native = root.join(".local/share/Steam");
        let steamapps = native.join("steamapps");
        fs::create_dir_all(native.join("steamapps/common/A")).unwrap();
        fs::write(
            steamapps.join("appmanifest_10.acf"),
            r#""AppState" { "appid" "10" "name" "Alpha" "installdir" "A" "SizeOnDisk" "1024" "buildid" "7" "StateFlags" "4" }"#,
        )
        .unwrap();
        fs::write(
            steamapps.join("appmanifest_20.acf"),
            r#""AppState" { "appid" "20" "name" "Beta" "installdir" "B" "SizeOnDisk" "0" "buildid" "0" "StateFlags" "1026" }"#,
        )
        .unwrap();
        fs::write(steamapps.join("appmanifest_bad.acf"), "not a manifest").unwrap();
        let libraries = discover_at(root, &root.join(".local/share"));
        let library = libraries
            .iter()
            .find(|library| library.client == Client::Native)
            .unwrap();
        let apps = installed_apps(library);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].name, "Alpha");
        assert_eq!(apps[0].size_on_disk, 1024);
        assert_eq!(apps[1].app_id, 20);
        assert_eq!(apps[1].state_flags, 1026);
    }

    #[test]
    fn discovers_native_and_flatpak_and_reads_installed_app() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path();
        let native = root.join(".local/share/Steam");
        let flatpak = root.join(".var/app/com.valvesoftware.Steam/data/Steam");
        let extra = root.join("External Steam Library");
        fs::create_dir_all(native.join("steamapps")).unwrap();
        fs::create_dir_all(flatpak.join("steamapps")).unwrap();
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
        let libraries = discover_at(root, &root.join(".local/share"));
        assert_eq!(libraries.len(), 3);
        assert!(
            libraries
                .iter()
                .any(|library| library.client == Client::Flatpak)
        );
        assert_eq!(
            libraries
                .iter()
                .find_map(|library| installed_location(library, 440)),
            Some(game)
        );
    }
}
