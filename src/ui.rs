use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{
    app::{AddForm, AddStep, App, AppMode},
    launcher::config_file_path,
};

pub fn draw_app(frame: &mut Frame, app: &App) {
    let page = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(6),
        ])
        .split(frame.area());

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(page[1]);

    draw_header(frame, page[0]);
    draw_launcher_list(frame, body[0], app);
    draw_detail_panel(frame, body[1], app);
    draw_footer(frame, page[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "TUI Launcher",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  detected apps plus your saved launchers"),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(title, area);
}

fn draw_launcher_list(frame: &mut Frame, area: Rect, app: &App) {
    let list_items: Vec<ListItem> = app
        .items
        .iter()
        .map(|item| {
            let favorite = if item.is_favorite { "[fav] " } else { "" };
            ListItem::new(Line::from(vec![
                Span::styled(
                    favorite,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(item.name.as_str()),
                Span::styled(
                    format!(
                        "  [{} | {}]",
                        item.source.short_label(),
                        usage_label(item.launch_count)
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(list_items)
        .block(Block::default().title("Launchers").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" > ");

    let mut state = ListState::default();
    if !app.items.is_empty() {
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_detail_panel(frame: &mut Frame, area: Rect, app: &App) {
    match &app.mode {
        AppMode::Normal => draw_selected_item(frame, area, app),
        AppMode::Adding(form) => draw_add_form(frame, area, form),
    }
}

fn draw_selected_item(frame: &mut Frame, area: Rect, app: &App) {
    let details = if let Some(selected) = app.items.get(app.selected) {
        vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(selected.name.as_str()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Command: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(selected.command.as_str()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Source: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(selected.source.label()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Priority: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(priority_text(selected.is_favorite, selected.launch_count)),
            ]),
            Line::from(""),
            Line::from(selected.description.as_str()),
        ]
    } else {
        vec![
            Line::from("No launchers found yet."),
            Line::from(""),
            Line::from("Press a to add one. The launcher will save it to:"),
            Line::from(config_file_path().display().to_string()),
        ]
    };

    let paragraph = Paragraph::new(details)
        .block(Block::default().title("Selected TUI").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_add_form(frame: &mut Frame, area: Rect, form: &AddForm) {
    let name_style = field_style(form.step == AddStep::Name);
    let command_style = field_style(form.step == AddStep::Command);
    let description_style = field_style(form.step == AddStep::Description);

    let details = vec![
        Line::from("Add a launcher. Press Enter to move to the next field."),
        Line::from(""),
        Line::from(vec![
            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                editing_text(&form.name, form.step == AddStep::Name),
                name_style,
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Command: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                editing_text(&form.command, form.step == AddStep::Command),
                command_style,
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Description: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                editing_text(&form.description, form.step == AddStep::Description),
                description_style,
            ),
        ]),
        Line::from(""),
        Line::from("The command can include arguments, for example: nvim ~/.config"),
    ];

    let paragraph = Paragraph::new(details)
        .block(Block::default().title("Add TUI").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let help = match app.mode {
        AppMode::Normal => Line::from(vec![
            Span::styled("j/down", Style::default().fg(Color::Yellow)),
            Span::raw(" move  "),
            Span::styled("enter", Style::default().fg(Color::Yellow)),
            Span::raw(" launch  "),
            Span::styled("a", Style::default().fg(Color::Yellow)),
            Span::raw(" add  "),
            Span::styled("f", Style::default().fg(Color::Yellow)),
            Span::raw(" favorite  "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(" rescan  "),
            Span::styled("q/esc", Style::default().fg(Color::Yellow)),
            Span::raw(" quit"),
        ]),
        AppMode::Adding(_) => Line::from(vec![
            Span::styled("enter", Style::default().fg(Color::Yellow)),
            Span::raw(" next/save  "),
            Span::styled("backspace", Style::default().fg(Color::Yellow)),
            Span::raw(" delete  "),
            Span::styled("esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ]),
    };

    let footer = Paragraph::new(vec![Line::from(app.status.as_str()), Line::from(""), help])
        .block(Block::default().title("Status").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    frame.render_widget(footer, area);
}

fn field_style(is_active: bool) -> Style {
    if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

fn editing_text(value: &str, is_active: bool) -> String {
    if is_active {
        format!("{value}_")
    } else if value.is_empty() {
        "-".to_string()
    } else {
        value.to_string()
    }
}

fn priority_text(is_favorite: bool, launch_count: u32) -> String {
    let favorite = if is_favorite {
        "favorite"
    } else {
        "not favorited"
    };

    format!("{favorite}, {}", usage_label(launch_count))
}

fn usage_label(launch_count: u32) -> String {
    match launch_count {
        0 => "never used".to_string(),
        1 => "used once".to_string(),
        count => format!("used {count}x"),
    }
}
