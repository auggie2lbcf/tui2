use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEvent};

use crate::{
    launcher::{
        LauncherItem, build_launcher_items, config_file_path, load_custom_items, record_launch,
        save_custom_items, toggle_favorite,
    },
    terminal::{Tui, restore_terminal, run_shell_command, setup_existing_terminal},
    ui,
};

pub struct App {
    pub(crate) items: Vec<LauncherItem>,
    pub(crate) visible_items: Vec<usize>,
    pub(crate) selected: usize,
    pub(crate) search_query: String,
    pub(crate) status: String,
    pub(crate) mode: AppMode,
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let custom_items = load_custom_items().unwrap_or_else(|_| Vec::new());
        let items = build_launcher_items(custom_items);
        let visible_items = (0..items.len()).collect();
        let status = format!(
            "Found {} launcher{}.",
            items.len(),
            if items.len() == 1 { "" } else { "s" }
        );

        Self {
            items,
            visible_items,
            selected: 0,
            search_query: String::new(),
            status,
            mode: AppMode::Normal,
            should_quit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut Tui) -> io::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| ui::draw_app(frame, self))?;

            if event::poll(Duration::from_millis(150))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key, terminal)?;
                }
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent, terminal: &mut Tui) -> io::Result<()> {
        match self.mode {
            AppMode::Normal => self.handle_normal_key(key, terminal),
            AppMode::Searching => self.handle_search_key(key, terminal),
            AppMode::Adding(_) => self.handle_add_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent, terminal: &mut Tui) -> io::Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_previous(),
            KeyCode::Char('/') => self.start_search(),
            KeyCode::Char('a') => self.start_add_form(),
            KeyCode::Char('f') => self.toggle_selected_favorite(),
            KeyCode::Char('r') => self.reload_items(),
            KeyCode::Enter => self.launch_selected(terminal)?,
            _ => {}
        }

        Ok(())
    }

    fn handle_search_key(&mut self, key: KeyEvent, terminal: &mut Tui) -> io::Result<()> {
        match key.code {
            KeyCode::Esc => self.clear_search(),
            KeyCode::Enter => self.launch_selected(terminal)?,
            KeyCode::Backspace => {
                self.search_query.pop();
                self.rebuild_visible_items(None);
                self.update_search_status();
            }
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_previous(),
            KeyCode::Char(character) => {
                self.search_query.push(character);
                self.rebuild_visible_items(None);
                self.update_search_status();
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_add_key(&mut self, key: KeyEvent) -> io::Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.status = "Add cancelled.".to_string();
            }
            KeyCode::Enter => self.advance_add_form(),
            KeyCode::Backspace => {
                if let AppMode::Adding(form) = &mut self.mode {
                    form.current_value_mut().pop();
                }
            }
            KeyCode::Char(character) => {
                if let AppMode::Adding(form) = &mut self.mode {
                    form.current_value_mut().push(character);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn select_next(&mut self) {
        if !self.visible_items.is_empty() {
            self.selected = (self.selected + 1) % self.visible_items.len();
        }
    }

    fn select_previous(&mut self) {
        if self.visible_items.is_empty() {
            return;
        }

        if self.selected == 0 {
            self.selected = self.visible_items.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    fn start_search(&mut self) {
        self.mode = AppMode::Searching;
        self.status = "Search launchers.".to_string();
    }

    fn clear_search(&mut self) {
        self.search_query.clear();
        self.mode = AppMode::Normal;
        self.rebuild_visible_items(None);
        self.status = format!("Showing all {} launcher(s).", self.items.len());
    }

    fn start_add_form(&mut self) {
        self.mode = AppMode::Adding(AddForm::default());
        self.status = "Adding a launcher.".to_string();
    }

    fn advance_add_form(&mut self) {
        let AppMode::Adding(form) = &mut self.mode else {
            return;
        };

        match form.step {
            AddStep::Name => form.step = AddStep::Command,
            AddStep::Command => form.step = AddStep::Description,
            AddStep::Description => {
                let new_item = form.to_launcher_item();
                match new_item {
                    Some(item) => self.save_new_item(item),
                    None => self.status = "Name and command are required.".to_string(),
                }
            }
        }
    }

    fn save_new_item(&mut self, item: LauncherItem) {
        let mut custom_items = load_custom_items().unwrap_or_else(|_| Vec::new());
        custom_items.push(item.clone());

        match save_custom_items(&custom_items) {
            Ok(()) => {
                let command = item.command.clone();
                self.items.push(item);
                self.rebuild_visible_items(Some(&command));
                self.mode = AppMode::Normal;
                self.status = format!("Saved launcher to {}.", config_file_path().display());
            }
            Err(error) => {
                self.status = format!("Could not save launcher: {error}");
            }
        }
    }

    fn reload_items(&mut self) {
        let selected_command = self.selected_item().map(|item| item.command.clone());
        self.reload_items_keeping(selected_command.as_deref());
        self.status = if self.search_query.is_empty() {
            format!("Rescanned and found {} launcher(s).", self.items.len())
        } else {
            format!(
                "Rescanned and found {} launcher(s), {} matching search.",
                self.items.len(),
                self.visible_items.len()
            )
        };
    }

    fn reload_items_keeping(&mut self, selected_command: Option<&str>) {
        let custom_items = load_custom_items().unwrap_or_else(|_| Vec::new());
        self.items = build_launcher_items(custom_items);
        self.rebuild_visible_items(selected_command);
    }

    fn rebuild_visible_items(&mut self, selected_command: Option<&str>) {
        let previous_command = selected_command
            .map(str::to_string)
            .or_else(|| self.selected_item().map(|item| item.command.clone()));
        self.visible_items = matching_item_indices(&self.items, &self.search_query);
        self.selected = previous_command
            .and_then(|command| {
                self.visible_items
                    .iter()
                    .position(|&index| self.items[index].command == command)
            })
            .unwrap_or_else(|| {
                self.selected
                    .min(self.visible_items.len().saturating_sub(1))
            });
    }

    fn update_search_status(&mut self) {
        if self.search_query.is_empty() {
            self.status = format!("Search cleared. Showing {} launcher(s).", self.items.len());
        } else {
            self.status = format!(
                "Search: {} ({} match{})",
                self.search_query,
                self.visible_items.len(),
                if self.visible_items.len() == 1 {
                    ""
                } else {
                    "es"
                }
            );
        }
    }

    fn toggle_selected_favorite(&mut self) {
        let Some(item) = self.selected_item().cloned() else {
            self.status = "Nothing to favorite. Press a to add a TUI.".to_string();
            return;
        };

        match toggle_favorite(&item.command) {
            Ok(is_favorite) => {
                self.reload_items_keeping(Some(&item.command));
                let action = if is_favorite {
                    "Favorited"
                } else {
                    "Unfavorited"
                };
                self.status = format!("{action} {}.", item.name);
            }
            Err(error) => {
                self.status = format!("Could not update favorite: {error}");
            }
        }
    }

    fn launch_selected(&mut self, terminal: &mut Tui) -> io::Result<()> {
        let Some(item) = self.selected_item().cloned() else {
            self.status = "Nothing to launch. Press a to add a TUI.".to_string();
            return Ok(());
        };

        if let Err(error) = record_launch(&item.command) {
            self.status = format!("Could not record launch: {error}");
        }

        restore_terminal(terminal)?;
        let result = run_shell_command(&item.command);
        setup_existing_terminal(terminal)?;
        self.reload_items_keeping(Some(&item.command));

        self.status = match result {
            Ok(code) => format!("{} exited with status {}.", item.name, code),
            Err(error) => format!("Could not launch {}: {}", item.name, error),
        };

        Ok(())
    }

    pub(crate) fn selected_item(&self) -> Option<&LauncherItem> {
        let item_index = *self.visible_items.get(self.selected)?;
        self.items.get(item_index)
    }

    pub(crate) fn visible_items(&self) -> impl Iterator<Item = &LauncherItem> {
        self.visible_items
            .iter()
            .filter_map(|&item_index| self.items.get(item_index))
    }
}

pub(crate) enum AppMode {
    Normal,
    Searching,
    Adding(AddForm),
}

fn matching_item_indices(items: &[LauncherItem], query: &str) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return (0..items.len()).collect();
    }

    let mut matches: Vec<(usize, i32)> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| fuzzy_score(item, query).map(|score| (index, score)))
        .collect();

    matches.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    matches.into_iter().map(|(index, _)| index).collect()
}

fn fuzzy_score(item: &LauncherItem, query: &str) -> Option<i32> {
    [
        (&item.name, 300),
        (&item.command, 200),
        (&item.description, 100),
    ]
    .into_iter()
    .filter_map(|(value, base_score)| fuzzy_text_score(value, query, base_score))
    .max()
}

fn fuzzy_text_score(text: &str, query: &str, base_score: i32) -> Option<i32> {
    let query = query.to_lowercase();
    let text = text.to_lowercase();
    let mut positions = Vec::new();
    let mut text_chars = text.chars().enumerate();

    for query_char in query.chars() {
        let (position, _) = text_chars.find(|(_, text_char)| *text_char == query_char)?;
        positions.push(position as i32);
    }

    let start = *positions.first()?;
    let gaps: i32 = positions.windows(2).map(|pair| pair[1] - pair[0] - 1).sum();
    let contiguous_bonus: i32 = positions
        .windows(2)
        .filter(|pair| pair[1] == pair[0] + 1)
        .count() as i32
        * 15;

    Some(base_score + contiguous_bonus - start - gaps)
}

#[derive(Default)]
pub(crate) struct AddForm {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) description: String,
    pub(crate) step: AddStep,
}

impl AddForm {
    fn current_value_mut(&mut self) -> &mut String {
        match self.step {
            AddStep::Name => &mut self.name,
            AddStep::Command => &mut self.command,
            AddStep::Description => &mut self.description,
        }
    }

    fn to_launcher_item(&self) -> Option<LauncherItem> {
        let name = self.name.trim();
        let command = self.command.trim();
        let description = self.description.trim();

        if name.is_empty() || command.is_empty() {
            return None;
        }

        let description = if description.is_empty() {
            "Added from inside the launcher."
        } else {
            description
        };

        Some(LauncherItem::custom(name, command, description))
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum AddStep {
    #[default]
    Name,
    Command,
    Description,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, command: &str, description: &str) -> LauncherItem {
        LauncherItem::custom(name, command, description)
    }

    #[test]
    fn fuzzy_search_matches_non_contiguous_characters() {
        let items = vec![
            item("Neovim", "nvim", "Edit files"),
            item("Lazygit", "lazygit", "Git dashboard"),
            item("Bottom", "btm", "System monitor"),
        ];

        let matches = matching_item_indices(&items, "lg");

        assert_eq!(matches, vec![1]);
    }

    #[test]
    fn fuzzy_search_prefers_name_matches_over_description_matches() {
        let items = vec![
            item("GitUI", "gitui", "Open a keyboard-driven Git interface"),
            item(
                "Lazy Docker",
                "lazydocker",
                "Open a dashboard for Git-adjacent workflows",
            ),
        ];

        let matches = matching_item_indices(&items, "git");

        assert_eq!(matches[0], 0);
    }

    #[test]
    fn empty_search_keeps_existing_order() {
        let items = vec![
            item("Neovim", "nvim", "Edit files"),
            item("Lazygit", "lazygit", "Git dashboard"),
        ];

        let matches = matching_item_indices(&items, "");

        assert_eq!(matches, vec![0, 1]);
    }
}
