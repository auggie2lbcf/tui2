use std::{
    collections::{HashMap, HashSet},
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct LauncherItem {
    pub name: String,
    pub command: String,
    pub description: String,
    pub source: LauncherSource,
    pub launch_count: u32,
    pub is_favorite: bool,
}

impl LauncherItem {
    fn detected(name: &str, command: &str, description: &str) -> Self {
        Self::new(name, command, description, LauncherSource::Detected)
    }

    pub fn custom(name: &str, command: &str, description: &str) -> Self {
        Self::new(name, command, description, LauncherSource::Custom)
    }

    fn new(name: &str, command: &str, description: &str, source: LauncherSource) -> Self {
        Self {
            name: name.to_string(),
            command: command.to_string(),
            description: description.to_string(),
            source,
            launch_count: 0,
            is_favorite: false,
        }
    }
}

#[derive(Clone)]
pub enum LauncherSource {
    Detected,
    Custom,
}

impl LauncherSource {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Detected => "detected on this computer",
            Self::Custom => "added by you",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Detected => "auto",
            Self::Custom => "user",
        }
    }
}

pub fn build_launcher_items(custom_items: Vec<LauncherItem>) -> Vec<LauncherItem> {
    let stats = load_launcher_stats().unwrap_or_else(|_| HashMap::new());
    let mut used_commands = HashSet::new();
    let mut items = Vec::new();

    for mut item in custom_items {
        apply_stats(&mut item, &stats);
        used_commands.insert(command_name(&item.command).to_string());
        items.push(item);
    }

    for mut item in detected_launcher_items() {
        apply_stats(&mut item, &stats);
        let command = command_name(&item.command);
        if used_commands.insert(command.to_string()) {
            items.push(item);
        }
    }

    sort_by_priority(&mut items);
    items
}

pub fn toggle_favorite(command: &str) -> io::Result<bool> {
    let mut stats = load_launcher_stats()?;
    let command_key = command_name(command).to_string();
    let entry = stats.entry(command_key).or_default();
    entry.is_favorite = !entry.is_favorite;
    let is_favorite = entry.is_favorite;
    save_launcher_stats(&stats)?;
    Ok(is_favorite)
}

pub fn record_launch(command: &str) -> io::Result<()> {
    let mut stats = load_launcher_stats()?;
    let command_key = command_name(command).to_string();
    let entry = stats.entry(command_key).or_default();
    entry.launch_count = entry.launch_count.saturating_add(1);
    save_launcher_stats(&stats)
}

pub fn stats_file_path() -> PathBuf {
    config_directory().join("stats.txt")
}

pub fn load_custom_items() -> io::Result<Vec<LauncherItem>> {
    let path = config_file_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(path)?;
    let mut items = Vec::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() < 2 {
            continue;
        }

        let description = parts.get(2).copied().unwrap_or("Added from config file.");
        items.push(LauncherItem::custom(parts[0], parts[1], description));
    }

    Ok(items)
}

pub fn save_custom_items(items: &[LauncherItem]) -> io::Result<()> {
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut contents =
        String::from("# TUI launcher entries. Each line is: name<TAB>command<TAB>description\n");

    for item in items {
        contents.push_str(&item.name.replace('\t', " "));
        contents.push('\t');
        contents.push_str(&item.command.replace('\t', " "));
        contents.push('\t');
        contents.push_str(&item.description.replace('\t', " "));
        contents.push('\n');
    }

    fs::write(path, contents)
}

pub fn config_file_path() -> PathBuf {
    config_directory().join("launchers.txt")
}

fn config_directory() -> PathBuf {
    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("tui2");
    }

    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join(".config").join("tui2");
    }

    PathBuf::from(".")
}

#[derive(Default)]
struct LauncherStats {
    launch_count: u32,
    is_favorite: bool,
}

fn apply_stats(item: &mut LauncherItem, stats: &HashMap<String, LauncherStats>) {
    if let Some(saved_stats) = stats.get(command_name(&item.command)) {
        item.launch_count = saved_stats.launch_count;
        item.is_favorite = saved_stats.is_favorite;
    }
}

fn sort_by_priority(items: &mut [LauncherItem]) {
    items.sort_by(|left, right| {
        right
            .is_favorite
            .cmp(&left.is_favorite)
            .then(right.launch_count.cmp(&left.launch_count))
            .then(left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
}

fn load_launcher_stats() -> io::Result<HashMap<String, LauncherStats>> {
    let path = stats_file_path();
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let contents = fs::read_to_string(path)?;
    let mut stats = HashMap::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() < 3 {
            continue;
        }

        let launch_count = parts[1].parse().unwrap_or(0);
        let is_favorite = parts[2] == "true";
        stats.insert(
            parts[0].to_string(),
            LauncherStats {
                launch_count,
                is_favorite,
            },
        );
    }

    Ok(stats)
}

fn save_launcher_stats(stats: &HashMap<String, LauncherStats>) -> io::Result<()> {
    let path = stats_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut rows: Vec<(&String, &LauncherStats)> = stats.iter().collect();
    rows.sort_by(|left, right| left.0.cmp(right.0));

    let mut contents =
        String::from("# TUI usage stats. Each line is: command<TAB>launch_count<TAB>favorite\n");

    for (command, stats) in rows {
        contents.push_str(&command.replace('\t', " "));
        contents.push('\t');
        contents.push_str(&stats.launch_count.to_string());
        contents.push('\t');
        contents.push_str(if stats.is_favorite { "true" } else { "false" });
        contents.push('\n');
    }

    fs::write(path, contents)
}

fn detected_launcher_items() -> Vec<LauncherItem> {
    known_tui_programs()
        .into_iter()
        .filter(|program| should_show_program(program.command))
        .filter(|program| command_exists(program.command))
        .map(|program| LauncherItem::detected(program.name, program.command, program.description))
        .collect()
}

struct KnownTuiProgram {
    name: &'static str,
    command: &'static str,
    description: &'static str,
}

fn known_tui_programs() -> Vec<KnownTuiProgram> {
    vec![
        KnownTuiProgram {
            name: "Zellij",
            command: "zellij",
            description: "Open the Zellij terminal workspace and multiplexer.",
        },
        KnownTuiProgram {
            name: "Tmux",
            command: "tmux",
            description: "Open a tmux terminal multiplexer session.",
        },
        KnownTuiProgram {
            name: "Neovim",
            command: "nvim",
            description: "Open Neovim in the directory where this launcher was started.",
        },
        KnownTuiProgram {
            name: "Vim",
            command: "vim",
            description: "Open Vim in the directory where this launcher was started.",
        },
        KnownTuiProgram {
            name: "Helix",
            command: "hx",
            description: "Open the Helix editor in the current directory.",
        },
        KnownTuiProgram {
            name: "Micro",
            command: "micro",
            description: "Open the Micro terminal text editor.",
        },
        KnownTuiProgram {
            name: "Emacs",
            command: "emacs -nw",
            description: "Open Emacs in terminal mode.",
        },
        KnownTuiProgram {
            name: "Nano",
            command: "nano",
            description: "Open the Nano terminal text editor.",
        },
        KnownTuiProgram {
            name: "Lazygit",
            command: "lazygit",
            description: "Open a Git status and commit dashboard for the current directory.",
        },
        KnownTuiProgram {
            name: "GitUI",
            command: "gitui",
            description: "Open a keyboard-driven Git interface.",
        },
        KnownTuiProgram {
            name: "Tig",
            command: "tig",
            description: "Browse Git history from the terminal.",
        },
        KnownTuiProgram {
            name: "Lazy Docker",
            command: "lazydocker",
            description: "Open a terminal dashboard for Docker containers and services.",
        },
        KnownTuiProgram {
            name: "Docker TUI",
            command: "docui",
            description: "Open a terminal interface for Docker resources.",
        },
        KnownTuiProgram {
            name: "K9s",
            command: "k9s",
            description: "Open the K9s Kubernetes dashboard.",
        },
        KnownTuiProgram {
            name: "Kubectl TUI",
            command: "kui",
            description: "Open a terminal interface for Kubernetes workflows.",
        },
        KnownTuiProgram {
            name: "Stern",
            command: "stern",
            description: "Tail Kubernetes logs from multiple pods.",
        },
        KnownTuiProgram {
            name: "Yazi",
            command: "yazi",
            description: "Open the Yazi terminal file manager.",
        },
        KnownTuiProgram {
            name: "Ranger",
            command: "ranger",
            description: "Open the Ranger terminal file manager.",
        },
        KnownTuiProgram {
            name: "Lf",
            command: "lf",
            description: "Open the lf terminal file manager.",
        },
        KnownTuiProgram {
            name: "Nnn",
            command: "nnn",
            description: "Open the nnn terminal file manager.",
        },
        KnownTuiProgram {
            name: "Vifm",
            command: "vifm",
            description: "Open the Vifm dual-pane file manager.",
        },
        KnownTuiProgram {
            name: "Midnight Commander",
            command: "mc",
            description: "Open the Midnight Commander file manager.",
        },
        KnownTuiProgram {
            name: "Superfile",
            command: "spf",
            description: "Open the Superfile terminal file manager.",
        },
        KnownTuiProgram {
            name: "Btop",
            command: "btop",
            description: "Open the btop system monitor.",
        },
        KnownTuiProgram {
            name: "Bottom",
            command: "btm",
            description: "Open the Bottom system monitor.",
        },
        KnownTuiProgram {
            name: "Htop",
            command: "htop",
            description: "Open the htop process monitor.",
        },
        KnownTuiProgram {
            name: "Top",
            command: "top",
            description: "Open the standard process monitor.",
        },
        KnownTuiProgram {
            name: "Glances",
            command: "glances",
            description: "Open the Glances system monitor.",
        },
        KnownTuiProgram {
            name: "Atop",
            command: "atop",
            description: "Open the atop system and process monitor.",
        },
        KnownTuiProgram {
            name: "Iotop",
            command: "iotop",
            description: "Inspect disk I/O usage by process.",
        },
        KnownTuiProgram {
            name: "Bandwhich",
            command: "bandwhich",
            description: "Show current network usage by process and connection.",
        },
        KnownTuiProgram {
            name: "Nethogs",
            command: "nethogs",
            description: "Show network usage grouped by process.",
        },
        KnownTuiProgram {
            name: "Nload",
            command: "nload",
            description: "Monitor network throughput in the terminal.",
        },
        KnownTuiProgram {
            name: "Bluetuith",
            command: "bluetuith",
            description: "Manage Bluetooth devices from the terminal.",
        },
        KnownTuiProgram {
            name: "Impala",
            command: "impala",
            description: "Manage Wi-Fi connections from the terminal.",
        },
        KnownTuiProgram {
            name: "Glow",
            command: "glow",
            description: "Browse Markdown files in the terminal.",
        },
        KnownTuiProgram {
            name: "Broot",
            command: "broot",
            description: "Explore and search directory trees.",
        },
        KnownTuiProgram {
            name: "Dust",
            command: "dust",
            description: "Inspect disk usage with a terminal tree view.",
        },
        KnownTuiProgram {
            name: "Ncdu",
            command: "ncdu",
            description: "Inspect disk usage from the terminal.",
        },
        KnownTuiProgram {
            name: "Dua",
            command: "dua interactive",
            description: "Inspect disk usage with dua's interactive terminal view.",
        },
        KnownTuiProgram {
            name: "Gdu",
            command: "gdu",
            description: "Inspect disk usage with the gdu terminal analyzer.",
        },
        KnownTuiProgram {
            name: "Rmpc",
            command: "rmpc",
            description: "Open the rmpc terminal music client.",
        },
        KnownTuiProgram {
            name: "Newsboat",
            command: "newsboat",
            description: "Open the Newsboat RSS reader.",
        },
        KnownTuiProgram {
            name: "Aerc",
            command: "aerc",
            description: "Open the aerc terminal email client.",
        },
        KnownTuiProgram {
            name: "Neomutt",
            command: "neomutt",
            description: "Open the NeoMutt terminal email client.",
        },
        KnownTuiProgram {
            name: "Mutt",
            command: "mutt",
            description: "Open the Mutt terminal email client.",
        },
        KnownTuiProgram {
            name: "WeeChat",
            command: "weechat",
            description: "Open the WeeChat terminal chat client.",
        },
        KnownTuiProgram {
            name: "Irssi",
            command: "irssi",
            description: "Open the Irssi terminal chat client.",
        },
        KnownTuiProgram {
            name: "Tuigreet",
            command: "tuigreet",
            description: "Open the tuigreet terminal login greeter.",
        },
        KnownTuiProgram {
            name: "Music Player",
            command: "ncmpcpp",
            description: "Open the ncmpcpp music client.",
        },
        KnownTuiProgram {
            name: "Cmus",
            command: "cmus",
            description: "Open the cmus terminal music player.",
        },
        KnownTuiProgram {
            name: "Mocp",
            command: "mocp",
            description: "Open the MOC terminal music player.",
        },
        KnownTuiProgram {
            name: "Tuir",
            command: "tuir",
            description: "Open a terminal Reddit client.",
        },
        KnownTuiProgram {
            name: "Rtv",
            command: "rtv",
            description: "Open a terminal Reddit viewer.",
        },
        KnownTuiProgram {
            name: "Twtxt",
            command: "twtxt",
            description: "Open the twtxt command line social client.",
        },
        KnownTuiProgram {
            name: "W3m",
            command: "w3m",
            description: "Open the w3m terminal web browser.",
        },
        KnownTuiProgram {
            name: "Lynx",
            command: "lynx",
            description: "Open the Lynx terminal web browser.",
        },
        KnownTuiProgram {
            name: "El Links",
            command: "elinks",
            description: "Open the ELinks terminal web browser.",
        },
        KnownTuiProgram {
            name: "Bashmount",
            command: "bashmount",
            description: "Mount and unmount storage devices from the terminal.",
        },
        KnownTuiProgram {
            name: "Pulsemixer",
            command: "pulsemixer",
            description: "Control PulseAudio volume from the terminal.",
        },
        KnownTuiProgram {
            name: "Alsamixer",
            command: "alsamixer",
            description: "Control ALSA audio levels from the terminal.",
        },
        KnownTuiProgram {
            name: "Termscp",
            command: "termscp",
            description: "Open a terminal file transfer client.",
        },
        KnownTuiProgram {
            name: "Lftp",
            command: "lftp",
            description: "Open the lftp terminal file transfer client.",
        },
        KnownTuiProgram {
            name: "Gdb TUI",
            command: "gdb -tui",
            description: "Open GDB in terminal UI mode.",
        },
        KnownTuiProgram {
            name: "Lldb",
            command: "lldb",
            description: "Open the LLDB debugger.",
        },
        KnownTuiProgram {
            name: "Viddy",
            command: "viddy",
            description: "Run commands repeatedly in an interactive watch view.",
        },
    ]
}

fn should_show_program(command: &str) -> bool {
    match command_name(command) {
        "tmux" => env::var_os("TMUX").is_none(),
        "zellij" => env::var_os("ZELLIJ").is_none(),
        _ => true,
    }
}

fn command_exists(command: &str) -> bool {
    let command = command_name(command);
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&paths).any(|directory| is_executable_file(&directory.join(command)))
}

fn command_name(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or(command)
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };

    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}
