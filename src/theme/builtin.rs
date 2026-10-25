//! Compiled-in themes. Order: bg surface overlay fg muted accent unread flagged danger ok selection border.

use super::Theme;

pub const THEMES: [(&str, [&str; 12]); 8] = [
    ("tokyo-night", ["#1a1b26", "#16161e", "#292e42", "#c0caf5", "#565f89", "#7aa2f7", "#7dcfff", "#e0af68", "#f7768e", "#9ece6a", "#283457", "#3b4261"]),
    ("catppuccin", ["#1e1e2e", "#181825", "#313244", "#cdd6f4", "#6c7086", "#cba6f7", "#89dceb", "#f9e2af", "#f38ba8", "#a6e3a1", "#45475a", "#313244"]),
    ("gruvbox", ["#282828", "#1d2021", "#3c3836", "#ebdbb2", "#928374", "#fe8019", "#83a598", "#fabd2f", "#fb4934", "#b8bb26", "#504945", "#3c3836"]),
    ("rose-pine", ["#191724", "#1f1d2e", "#26233a", "#e0def4", "#6e6a86", "#ebbcba", "#9ccfd8", "#f6c177", "#eb6f92", "#31748f", "#403d52", "#26233a"]),
    ("nord", ["#2e3440", "#272c36", "#3b4252", "#eceff4", "#7b88a1", "#88c0d0", "#8fbcbb", "#ebcb8b", "#bf616a", "#a3be8c", "#434c5e", "#3b4252"]),
    ("everforest", ["#2d353b", "#232a2e", "#343f44", "#d3c6aa", "#859289", "#a7c080", "#7fbbb3", "#dbbc7f", "#e67e80", "#83c092", "#475258", "#3d484d"]),
    ("kanagawa", ["#1f1f28", "#16161d", "#2a2a37", "#dcd7ba", "#727169", "#7e9cd8", "#7fb4ca", "#e6c384", "#e82424", "#98bb6c", "#2d4f67", "#363646"]),
    ("latte", ["#eff1f5", "#e6e9ef", "#ccd0da", "#4c4f69", "#8c8fa1", "#8839ef", "#04a5e5", "#df8e1d", "#d20f39", "#40a02b", "#bcc0cc", "#ccd0da"]),
];

pub fn get(name: &str) -> Option<Theme> {
    THEMES.iter().find(|(n, _)| *n == name).map(|(_, h)| Theme::from_hex(*h).expect("built-in themes are valid"))
}

pub fn default() -> Theme {
    get("tokyo-night").expect("tokyo-night is built in")
}
