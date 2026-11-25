//! Icons of applications by name, as the freedesktop icon theme spec has
//! them: the theme GTK is set to, the themes it inherits, then hicolor; in
//! each, the directory of the size nearest to the one asked, a scalable one
//! best of all. Each name is looked up once.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// Where icon themes live, the user's first.
fn bases() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|home| home.join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let mut bases: Vec<PathBuf> = home.iter().map(|home| home.join(".icons")).collect();
    bases.extend(data_home.map(|dir| dir.join("icons")));
    bases.extend(
        data_dirs
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(|dir| Path::new(dir).join("icons")),
    );
    bases
}

/// The theme GTK is set to, if any.
fn gtk_theme() -> Option<String> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"]
        .iter()
        .find_map(|file| {
            let text = std::fs::read_to_string(config.join(file)).ok()?;
            text.lines().find_map(|line| {
                let (key, value) = line.split_once('=')?;
                (key.trim() == "gtk-icon-theme-name").then(|| value.trim().trim_matches('"').to_owned())
            })
        })
}

/// One directory of a theme: where it is, and the sizes it holds.
#[derive(Clone, Debug)]
struct Dir {
    path: PathBuf,
    size: u32,
    min: u32,
    max: u32,
}

impl Dir {
    /// How far its icons are from `size`: 0 for those that fit.
    fn distance(&self, size: u32) -> u32 {
        if size < self.min {
            self.min - size
        } else {
            size.saturating_sub(self.max)
        }
    }
}

/// A theme's index: its directories, and what it inherits.
#[derive(Clone, Debug, Default)]
struct Theme {
    dirs: Vec<Dir>,
    inherits: Vec<String>,
}

/// The index of `name` in `bases`: its directories, wherever they are.
fn theme(name: &str, bases: &[PathBuf]) -> Option<Theme> {
    let roots: Vec<PathBuf> = bases
        .iter()
        .map(|base| base.join(name))
        .filter(|root| root.is_dir())
        .collect();
    let index = roots
        .iter()
        .find_map(|root| std::fs::read_to_string(root.join("index.theme")).ok())?;
    let mut theme = Theme::default();
    let mut section = String::new();
    let mut fields: HashMap<String, HashMap<String, String>> = HashMap::new();
    for line in index.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.to_owned();
        } else if let Some((key, value)) = line.split_once('=') {
            fields
                .entry(section.clone())
                .or_default()
                .insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    let head = fields.get("Icon Theme").cloned().unwrap_or_default();
    theme.inherits = head
        .get("Inherits")
        .map(|all| all.split(',').map(|s| s.trim().to_owned()).collect())
        .unwrap_or_default();
    let listed = [head.get("Directories"), head.get("ScaledDirectories")];
    for dir in listed.into_iter().flatten().flat_map(|all| all.split(',')) {
        let dir = dir.trim();
        let Some(info) = fields.get(dir) else { continue };
        let number = |key: &str| info.get(key).and_then(|v| v.parse::<u32>().ok());
        let Some(size) = number("Size") else { continue };
        let scale = number("Scale").unwrap_or(1).max(1);
        let (min, max) = match info.get("Type").map(String::as_str) {
            Some("Scalable") => (number("MinSize").unwrap_or(size), number("MaxSize").unwrap_or(size)),
            Some("Fixed") => (size, size),
            _ => {
                let threshold = number("Threshold").unwrap_or(2);
                (size.saturating_sub(threshold), size + threshold)
            }
        };
        for root in &roots {
            theme.dirs.push(Dir {
                path: root.join(dir),
                size: size * scale,
                min: min * scale,
                max: max * scale,
            });
        }
    }
    Some(theme)
}

/// Finds icons by name; what it found is kept.
pub struct Lookup {
    bases: Vec<PathBuf>,
    chain: Vec<Theme>,
    found: HashMap<(String, u32, Option<PathBuf>), Option<PathBuf>>,
}

impl Lookup {
    /// The themes in `bases`, from `first` (GTK's when none) through what it
    /// inherits to hicolor.
    pub fn new(bases: Vec<PathBuf>, first: Option<String>) -> Self {
        let mut chain = Vec::new();
        let mut names: Vec<String> = first.or_else(gtk_theme).into_iter().collect();
        let mut seen = Vec::new();
        while let Some(name) = names.first().cloned() {
            names.remove(0);
            if seen.contains(&name) {
                continue;
            }
            seen.push(name.clone());
            if let Some(theme) = theme(&name, &bases) {
                names.extend(theme.inherits.clone());
                chain.push(theme);
            }
        }
        if !seen.iter().any(|name| name == "hicolor")
            && let Some(theme) = theme("hicolor", &bases)
        {
            chain.push(theme);
        }
        Self {
            bases,
            chain,
            found: HashMap::new(),
        }
    }

    /// As the desktop is set up.
    pub fn desktop() -> Self {
        Self::new(bases(), None)
    }

    /// The file of the icon `name` nearest to `size` pixels, looked in
    /// `extra` first (an app's own icon path), then in the themes, then
    /// among the loose pixmaps.
    pub fn find(&mut self, name: &str, size: u32, extra: Option<&Path>) -> Option<PathBuf> {
        if name.is_empty() {
            return None;
        }
        // A path given whole.
        if name.starts_with('/') {
            return Path::new(name).is_file().then(|| PathBuf::from(name));
        }
        let key = (name.to_owned(), size, extra.map(Path::to_path_buf));
        if let Some(found) = self.found.get(&key) {
            return found.clone();
        }
        let found = self.search(name, size, extra);
        self.found.insert(key, found.clone());
        found
    }

    fn search(&self, name: &str, size: u32, extra: Option<&Path>) -> Option<PathBuf> {
        let file = |dir: &Path| {
            ["svg", "png"]
                .iter()
                .map(|ext| dir.join(format!("{name}.{ext}")))
                .find(|path| path.is_file())
        };
        if let Some(extra) = extra.filter(|dir| dir.is_dir()) {
            if let Some(path) = file(extra) {
                return Some(path);
            }
            // An app's own little theme: any size in it.
            if let Some(theme) = extra
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| theme(n, &[extra.parent()?.to_path_buf()]))
                && let Some(path) = nearest(&theme, size, &file)
            {
                return Some(path);
            }
        }
        for theme in &self.chain {
            if let Some(path) = nearest(theme, size, &file) {
                return Some(path);
            }
        }
        self.bases
            .iter()
            .filter_map(|base| base.parent())
            .chain([Path::new("/usr/share")])
            .find_map(|share| file(&share.join("pixmaps")))
    }
}

/// In `theme`, the directory nearest to `size` that has the file.
fn nearest(theme: &Theme, size: u32, file: &impl Fn(&Path) -> Option<PathBuf>) -> Option<PathBuf> {
    let mut dirs: Vec<&Dir> = theme.dirs.iter().collect();
    dirs.sort_by_key(|dir| (dir.distance(size), u32::MAX - dir.size));
    dirs.into_iter().find_map(|dir| file(&dir.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn icons_come_from_the_nearest_size_then_what_the_theme_inherits() {
        let root = std::env::temp_dir().join(format!("yokoso-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let icons = root.join("icons");
        write(
            &icons.join("Mine/index.theme"),
            "[Icon Theme]\nInherits=hicolor\nDirectories=16x16/apps,48x48/apps,scalable/apps\n\
             [16x16/apps]\nSize=16\nType=Fixed\n[48x48/apps]\nSize=48\nType=Fixed\n\
             [scalable/apps]\nSize=48\nMinSize=8\nMaxSize=512\nType=Scalable\n",
        );
        write(&icons.join("Mine/16x16/apps/small.png"), "png");
        write(&icons.join("Mine/48x48/apps/small.png"), "png");
        write(&icons.join("Mine/scalable/apps/vector.svg"), "svg");
        write(
            &icons.join("hicolor/index.theme"),
            "[Icon Theme]\nDirectories=32x32/apps\n[32x32/apps]\nSize=32\nType=Threshold\n",
        );
        write(&icons.join("hicolor/32x32/apps/inherited.png"), "png");
        let mut lookup = Lookup::new(vec![icons.clone()], Some("Mine".into()));
        let found = |lookup: &mut Lookup, name, size| {
            lookup
                .find(name, size, None)
                .map(|p| p.strip_prefix(&icons).unwrap().display().to_string())
        };
        assert_eq!(
            found(&mut lookup, "small", 20).as_deref(),
            Some("Mine/16x16/apps/small.png")
        );
        assert_eq!(
            found(&mut lookup, "small", 40).as_deref(),
            Some("Mine/48x48/apps/small.png")
        );
        assert_eq!(
            found(&mut lookup, "vector", 22).as_deref(),
            Some("Mine/scalable/apps/vector.svg")
        );
        assert_eq!(
            found(&mut lookup, "inherited", 22).as_deref(),
            Some("hicolor/32x32/apps/inherited.png")
        );
        assert_eq!(found(&mut lookup, "nowhere", 22), None);
        // An app's own icon path comes first.
        write(&root.join("own/small.svg"), "svg");
        assert_eq!(
            lookup.find("small", 20, Some(&root.join("own"))),
            Some(root.join("own/small.svg"))
        );
    }
}
