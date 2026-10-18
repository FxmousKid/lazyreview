use ratatui::style::{Color, Modifier, Style};
use serde_json::Value;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub green: Color,
    pub red: Color,
    pub purple: Color,
    pub border: Color,
    pub active: Style,
    pub inactive: Style,
    pub searching: Style,
    pub selected: Style,
    pub add_bg: Color,
    pub delete_bg: Color,
    pub hunk_bg: Color,
}
impl Theme {
    pub fn blue() -> Self {
        let accent = Color::Rgb(88, 166, 255);
        let border = Color::Rgb(48, 54, 61);
        Self {
            name: "blue",
            bg: Color::Rgb(13, 17, 23),
            fg: Color::Rgb(230, 237, 243),
            muted: Color::Rgb(145, 152, 161),
            accent,
            green: Color::Rgb(63, 185, 80),
            red: Color::Rgb(255, 123, 114),
            purple: Color::Rgb(188, 140, 255),
            border,
            active: Style::default().fg(accent),
            inactive: Style::default().fg(border),
            searching: Style::default().fg(accent).add_modifier(Modifier::BOLD),
            selected: Style::default()
                .bg(Color::Rgb(22, 39, 57))
                .add_modifier(Modifier::BOLD),
            add_bg: Color::Rgb(18, 38, 30),
            delete_bg: Color::Rgb(48, 26, 30),
            hunk_bg: Color::Rgb(17, 35, 56),
        }
    }
    pub fn apply(&mut self, doc: &Value) -> Result<(), String> {
        let fields = &doc["gui"]["theme"];
        if fields.is_null() {
            return Ok(());
        }
        if !fields.is_object() {
            return Err("gui.theme must be a mapping".into());
        }
        for (key, target, background) in [
            ("activeBorderColor", &mut self.active, false),
            ("inactiveBorderColor", &mut self.inactive, false),
            ("searchingActiveBorderColor", &mut self.searching, false),
            ("selectedLineBgColor", &mut self.selected, true),
        ] {
            if !fields[key].is_null() {
                *target = attributes(&fields[key], background)?;
            }
        }
        if !fields["selectedLineFgColor"].is_null() {
            self.selected = self
                .selected
                .patch(attributes(&fields["selectedLineFgColor"], false)?);
        }
        for (key, target) in [
            ("defaultFgColor", &mut self.fg),
            ("optionsTextColor", &mut self.accent),
            ("unstagedChangesColor", &mut self.red),
            ("inactiveBorderColor", &mut self.muted),
            ("activeBorderColor", &mut self.purple),
        ] {
            if !fields[key].is_null() {
                if let Some(color) = attributes(&fields[key], false)?.fg {
                    *target = color;
                }
            }
        }
        self.border = self.inactive.fg.unwrap_or(Color::Reset);
        Ok(())
    }
}
fn attributes(value: &Value, background: bool) -> Result<Style, String> {
    let mut style = Style::default();
    let list = value
        .as_array()
        .ok_or("Theme attributes must be YAML lists, as in Lazygit")?;
    for entry in list {
        let s = entry.as_str().ok_or("Theme attributes must be strings")?;
        let modifier = match s {
            "bold" => Some(Modifier::BOLD),
            "reverse" => Some(Modifier::REVERSED),
            "underline" => Some(Modifier::UNDERLINED),
            "strikethrough" => Some(Modifier::CROSSED_OUT),
            "dim" => Some(Modifier::DIM),
            "italic" => Some(Modifier::ITALIC),
            _ => None,
        };
        if let Some(modifier) = modifier {
            style = style.add_modifier(modifier);
            continue;
        }
        let color = match s {
            "default" => Color::Reset,
            "black" => Color::Black,
            "red" => Color::Red,
            "green" => Color::Green,
            "yellow" => Color::Yellow,
            "blue" => Color::Blue,
            "magenta" => Color::Magenta,
            "cyan" => Color::Cyan,
            "white" => Color::White,
            _ if s.starts_with('#') && s.len() == 7 => {
                let rgb =
                    u32::from_str_radix(&s[1..], 16).map_err(|_| "Invalid theme hex color")?;
                Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
            }
            _ => return Err("Unknown theme color or modifier".into()),
        };
        style = if background {
            style.bg(color)
        } else {
            style.fg(color)
        };
    }
    Ok(style)
}
fn read(path: &Path) -> Result<Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|_| format!("Cannot read {}", path.display()))?;
    serde_yaml_ng::from_str(&text).map_err(|_| format!("Invalid YAML in {}", path.display()))
}
fn config_root() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}
pub fn companion_style(override_name: Option<&str>) -> Result<crate::reader::Appearance, String> {
    let path = crate::preferences::read_path();
    let doc = if path.exists() {
        read(&path)?
    } else {
        Value::Null
    };
    if !doc["gui"]["companionStyle"].is_null() && !doc["gui"]["companionStyle"].is_string() {
        return Err("gui.companionStyle must be ledger, paper, or studio".into());
    }
    crate::reader::Appearance::parse(
        override_name
            .or_else(|| doc["gui"]["companionStyle"].as_str())
            .unwrap_or("paper"),
    )
}
pub fn syntax_palette(override_name: Option<&str>) -> Result<crate::syntax::Palette, String> {
    let path = crate::preferences::read_path();
    let config = if path.exists() {
        read(&path)?
    } else {
        Value::Null
    };
    crate::syntax::Palette::parse(
        override_name
            .or_else(|| config["gui"]["syntaxTheme"].as_str())
            .unwrap_or("github-dark"),
    )
}
pub fn load(override_name: Option<&str>) -> Result<Theme, String> {
    let path = crate::preferences::read_path();
    let config = if path.exists() {
        read(&path)?
    } else {
        Value::Null
    };
    let name = override_name
        .or_else(|| config["gui"]["themePreset"].as_str())
        .unwrap_or("lazygit");
    let mut theme = Theme::blue();
    match name {
        "blue" => {}
        "lazygit" | "custom" => {
            theme.name = if name == "custom" {
                "custom"
            } else {
                "lazygit"
            };
            theme.bg = Color::Reset;
            theme.add_bg = Color::Reset;
            theme.delete_bg = Color::Reset;
            theme.hunk_bg = Color::Reset;
            if name == "lazygit" {
                let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
                let paths = [
                    config_root().join("lazygit/config.yml"),
                    home.join("Library/Application Support/lazygit/config.yml"),
                    home.join("Library/Application Support/jesseduffield/lazygit/config.yml"),
                ];
                if let Some(path) = paths.iter().find(|p| p.exists()) {
                    theme.apply(&read(path)?)?;
                }
            }
            theme.apply(&config)?;
        }
        _ => return Err("Theme must be lazygit, blue, or custom".into()),
    }
    Ok(theme)
}
thread_local! { static CURRENT: RefCell<Theme> = RefCell::new(Theme::blue()); }
pub fn current() -> Theme {
    CURRENT.with(|t| *t.borrow())
}
pub fn set(theme: Theme) {
    CURRENT.with(|t| *t.borrow_mut() = theme);
}
pub fn cycle() -> Result<&'static str, String> {
    let next = match current().name {
        "lazygit" => "blue",
        "blue" => "custom",
        _ => "lazygit",
    };
    set(load(Some(next))?);
    Ok(current().name)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lazygit_yaml_colors_and_modifiers_are_preserved() {
        let yaml = "gui:\n  theme:\n    activeBorderColor: ['#a184df', bold]\n    selectedLineBgColor: ['#2d3f76']\n    selectedLineFgColor: [underline]\n    defaultFgColor: ['#c8d3f5']\nos:\n  editPreset: lvim\n";
        let mut t = Theme::blue();
        t.apply(&serde_yaml_ng::from_str(yaml).unwrap()).unwrap();
        assert_eq!(t.active.fg, Some(Color::Rgb(161, 132, 223)));
        assert!(t.active.add_modifier.contains(Modifier::BOLD));
        assert_eq!(t.selected.bg, Some(Color::Rgb(45, 63, 118)));
        assert!(t.selected.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(t.fg, Color::Rgb(200, 211, 245));
        assert_ne!(Theme::blue().active.fg, t.active.fg);
    }
    #[test]
    fn modifier_only_and_default_attributes_work_and_invalid_colors_fail() {
        assert_eq!(
            attributes(&serde_json::json!(["reverse"]), true)
                .unwrap()
                .bg,
            None
        );
        assert_eq!(
            attributes(&serde_json::json!(["default"]), true)
                .unwrap()
                .bg,
            Some(Color::Reset)
        );
        assert!(attributes(&serde_json::json!(["#oops"]), false).is_err());
    }
}
