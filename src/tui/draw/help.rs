use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::draw::form::centered_rect;
use crate::tui::state::{FocusPanel, HelpModalState, TuiState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHelpItem {
    pub key: &'static str,
    pub description: &'static str,
    pub is_local: bool,
}

pub fn get_keybindings(focus: FocusPanel) -> Vec<KeyHelpItem> {
    let mut items = Vec::new();

    match focus {
        FocusPanel::Environments => {
            items.push(KeyHelpItem {
                key: "<space> / <enter>",
                description: "Toggle expand / collapse node",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "s",
                description: "SSH into selected host",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "l",
                description: "Stream journalctl logs",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "e",
                description: "Edit configuration in $EDITOR",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "J",
                description: "Join Tailscale mesh",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "u",
                description: "Deploy environment",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "c",
                description: "Run flake check",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "n",
                description: "Create new entity (host, svc, env, sec)",
                is_local: true,
            });
        }
        FocusPanel::Services => {
            items.push(KeyHelpItem {
                key: "l",
                description: "Stream service journalctl logs",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "e",
                description: "Edit service definition in $EDITOR",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "n",
                description: "Create new service",
                is_local: true,
            });
        }
        FocusPanel::Secrets => {
            items.push(KeyHelpItem {
                key: "<enter> / v",
                description: "View decrypted secret",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "e",
                description: "Edit secret (decrypt -> edit -> re-encrypt)",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "i",
                description: "Set or change identity key (-i)",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "n",
                description: "Create new encrypted secret",
                is_local: true,
            });
        }
        FocusPanel::Log => {
            items.push(KeyHelpItem {
                key: "j / <down>",
                description: "Scroll down towards recent logs",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "k / <up>",
                description: "Scroll up towards older logs",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "<pageup>",
                description: "Scroll up 10 lines",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "<pagedown>",
                description: "Scroll down 10 lines",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "g",
                description: "Scroll to top (oldest log)",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "G",
                description: "Scroll to bottom (most recent log)",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "m / <enter>",
                description: "Maximize / restore log panel",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "c",
                description: "Clear command log buffer",
                is_local: true,
            });
        }
        FocusPanel::Details => {
            items.push(KeyHelpItem {
                key: "e",
                description: "Edit inspected entity in $EDITOR",
                is_local: true,
            });
        }
        FocusPanel::Status => {
            items.push(KeyHelpItem {
                key: "c",
                description: "Run flake check",
                is_local: true,
            });
            items.push(KeyHelpItem {
                key: "u",
                description: "Deploy fleet",
                is_local: true,
            });
        }
    }

    items.push(KeyHelpItem {
        key: "<tab>",
        description: "Cycle focus to next panel",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "<backtab>",
        description: "Cycle focus to previous panel",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "0",
        description: "Focus Details / Inspector panel",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "1",
        description: "Focus Status panel",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "2",
        description: "Focus Environments & Hosts panel",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "3",
        description: "Focus Services panel",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "4",
        description: "Focus Secrets panel",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "5",
        description: "Focus Command Log panel",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "m",
        description: "Maximize / restore command log",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "? / h",
        description: "Toggle this keybindings help",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "r",
        description: "Reload fleet from disk",
        is_local: false,
    });
    items.push(KeyHelpItem {
        key: "q",
        description: "Quit Rocky TUI",
        is_local: false,
    });

    items
}

pub fn filter_keybindings(items: &[KeyHelpItem], filter: &str) -> Vec<KeyHelpItem> {
    if filter.trim().is_empty() {
        return items.to_vec();
    }
    let query = filter.trim().to_lowercase();
    items
        .iter()
        .filter(|item| {
            item.key.to_lowercase().contains(&query)
                || item.description.to_lowercase().contains(&query)
        })
        .cloned()
        .collect()
}

pub fn draw_help_modal(frame: &mut Frame, area: Rect, state: &TuiState, modal: &HelpModalState) {
    let popup_area = centered_rect(65, 75, area);
    frame.render_widget(Clear, popup_area);

    let all_items = get_keybindings(state.focus);
    let filtered = filter_keybindings(&all_items, &modal.filter);

    let total_count = filtered.len();
    let selected_idx = if total_count == 0 {
        0
    } else {
        modal.selected_index.min(total_count - 1)
    };

    let title_left = if modal.filter.is_empty() {
        "Keybindings".to_string()
    } else {
        format!("Keybindings [Filter: \"{}\"]", modal.filter)
    };

    let title_right = if modal.filter.is_empty() {
        "(Type to filter)".to_string()
    } else {
        "(Esc to clear)".to_string()
    };

    let bottom_right = format!(
        "{} of {}",
        if total_count == 0 {
            0
        } else {
            selected_idx + 1
        },
        total_count
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title_top(Line::from(format!(" {title_left} ")).left_aligned())
        .title_top(
            Line::from(vec![Span::styled(
                format!(" {title_right} "),
                Style::default().fg(Color::DarkGray),
            )])
            .right_aligned(),
        )
        .title_bottom(
            Line::from(vec![Span::styled(
                format!(" {bottom_right} "),
                Style::default().fg(Color::DarkGray),
            )])
            .right_aligned(),
        );

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let max_rows = inner.height as usize;
    let inner_width = inner.width as usize;
    let mut rendered_lines: Vec<(Option<usize>, Line<'static>)> = Vec::new();

    let local_items: Vec<(usize, &KeyHelpItem)> = filtered
        .iter()
        .enumerate()
        .filter(|(_, it)| it.is_local)
        .collect();
    let global_items: Vec<(usize, &KeyHelpItem)> = filtered
        .iter()
        .enumerate()
        .filter(|(_, it)| !it.is_local)
        .collect();

    if !local_items.is_empty() {
        rendered_lines.push((
            None,
            Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    "── Local",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
        ));
        for (orig_idx, item) in local_items {
            let is_selected = orig_idx == selected_idx;
            rendered_lines.push((
                Some(orig_idx),
                format_key_line(item, is_selected, inner_width),
            ));
        }
    }

    if !global_items.is_empty() {
        if !rendered_lines.is_empty() {
            rendered_lines.push((None, Line::raw("")));
        }
        rendered_lines.push((
            None,
            Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    "── Global",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
        ));
        for (orig_idx, item) in global_items {
            let is_selected = orig_idx == selected_idx;
            rendered_lines.push((
                Some(orig_idx),
                format_key_line(item, is_selected, inner_width),
            ));
        }
    }

    if rendered_lines.is_empty() {
        rendered_lines.push((
            None,
            Line::styled(
                "   No matching keybindings",
                Style::default().fg(Color::DarkGray),
            ),
        ));
    }

    let selected_line_pos = rendered_lines
        .iter()
        .position(|(idx_opt, _)| *idx_opt == Some(selected_idx))
        .unwrap_or(0);

    let skip = if selected_line_pos >= max_rows {
        selected_line_pos - max_rows + 1
    } else {
        0
    };

    let display: Vec<Line> = rendered_lines
        .into_iter()
        .skip(skip)
        .take(max_rows)
        .map(|(_, line)| line)
        .collect();

    frame.render_widget(Paragraph::new(display), inner);
}

fn format_key_line(item: &KeyHelpItem, is_selected: bool, width: usize) -> Line<'static> {
    if is_selected {
        let content = format!("  {:>14}  {}", item.key, item.description);
        let padded = if content.len() < width {
            format!("{content}{}", " ".repeat(width - content.len()))
        } else {
            content
        };
        Line::from(vec![Span::styled(
            padded,
            Style::default()
                .bg(Color::Rgb(45, 52, 75))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )])
    } else {
        Line::from(vec![
            Span::styled(
                format!("  {:>14}  ", item.key),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                item.description.to_string(),
                Style::default().fg(Color::White),
            ),
        ])
    }
}
