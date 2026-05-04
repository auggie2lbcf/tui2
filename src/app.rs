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
    pub(crate) selected: usize,
    pub(crate) status: String,
    pub(crate) mode: AppMode,
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let custom_items = load_custom_items().unwrap_or_else(|_| Vec::new());
        let items = build_launcher_items(custom_items);
        let status = format!(
            "Found {} launcher{}.",
            items.len(),
            if items.len() == 1 { "" } else { "s" }
        );

        Self {
            items,
            selected: 0,
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
            AppMode::Adding(_) => self.handle_add_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent, terminal: &mut Tui) -> io::Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('a') => self.start_add_form(),
            KeyCode::Char('f') => self.toggle_selected_favorite(),
            KeyCode::Char('r') => self.reload_items(),
            KeyCode::Enter => self.launch_selected(terminal)?,
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
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    fn select_previous(&mut self) {
        if self.items.is_empty() {
            return;
        }

        if self.selected == 0 {
            self.selected = self.items.len() - 1;
        } else {
            self.selected -= 1;
        }
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
                self.items.push(item);
                self.selected = self.items.len() - 1;
                self.mode = AppMode::Normal;
                self.status = format!("Saved launcher to {}.", config_file_path().display());
            }
            Err(error) => {
                self.status = format!("Could not save launcher: {error}");
            }
        }
    }

    fn reload_items(&mut self) {
        let selected_command = self
            .items
            .get(self.selected)
            .map(|item| item.command.clone());
        self.reload_items_keeping(selected_command.as_deref());
        self.status = format!("Rescanned and found {} launcher(s).", self.items.len());
    }

    fn reload_items_keeping(&mut self, selected_command: Option<&str>) {
        let custom_items = load_custom_items().unwrap_or_else(|_| Vec::new());
        self.items = build_launcher_items(custom_items);

        self.selected = selected_command
            .and_then(|command| self.items.iter().position(|item| item.command == command))
            .unwrap_or_else(|| self.selected.min(self.items.len().saturating_sub(1)));
    }

    fn toggle_selected_favorite(&mut self) {
        let Some(item) = self.items.get(self.selected).cloned() else {
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
        let Some(item) = self.items.get(self.selected).cloned() else {
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
}

pub(crate) enum AppMode {
    Normal,
    Adding(AddForm),
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
