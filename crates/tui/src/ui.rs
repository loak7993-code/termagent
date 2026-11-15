use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use termagent_core::types::{ApprovalRequest, Effort, Risk};

use crate::app::{
    App, ChatState, Item, Live, ModelPicker, Overlay, Pane, Screen, SessionPicker, Status,
};
use crate::onboarding::{Onboarding, Step};
use crate::theme::Palette;
use crate::util::{self, ellipsize, sym, word_width, wrap};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    match app.screen {
        Screen::Onboarding => draw_onboarding(f, app, area),
        Screen::Chat => draw_chat(f, app, area),
    }
    match &app.overlay {
        Overlay::None => {}
        Overlay::Models(picker) => draw_models(f, app, picker, area),
        Overlay::Sessions(picker) => draw_sessions(f, app, picker, area),
        Overlay::Help => draw_help(f, app, area),
    }
    if let Some(req) = &app.chat.pending {
        draw_approval(f, app, req, area);
    }
}

fn base(p: &Palette) -> Style {
    Style::default().fg(p.fg)
}

fn dim(p: &Palette) -> Style {
    Style::default().fg(p.dim)
}

fn accent(p: &Palette) -> Style {
    Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
}

fn risk_style(p: &Palette, risk: Risk) -> (&'static str, Style) {
    match risk {
        Risk::Low => ("low", dim(p)),
        Risk::Medium => ("medium", Style::default().fg(p.warn)),
        Risk::High => (
            "high",
            Style::default().fg(p.err).add_modifier(Modifier::BOLD),
        ),
    }
}

fn hline(left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let lw: usize = left.iter().map(|s| word_width(&s.content)).sum();
    let rw: usize = right.iter().map(|s| word_width(&s.content)).sum();
    let gap = width.saturating_sub(lw + rw);
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);
    Line::from(spans)
}

fn status_label(app: &App) -> (String, Style) {
    let p = &app.palette;
    match app.chat.status {
        Status::Idle => ("ready".to_string(), dim(p)),
        Status::Waiting => ("waiting for you".to_string(), Style::default().fg(p.warn)),
        Status::Streaming => (
            format!("{} working", util::spinner(app.tick)),
            Style::default().fg(p.accent),
        ),
    }
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    let p = &app.palette;
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let (header, feed, notice, composer, footer) = (rows[0], rows[1], rows[2], rows[3], rows[4]);

    let title = app
        .chat
        .session
        .as_ref()
        .map(|m| m.title.clone())
        .unwrap_or_else(|| "new session".into());
    let model = app
        .registry
        .model(&app.cfg.provider, &app.cfg.model)
        .map(|m| m.name.clone())
        .unwrap_or_else(|| app.cfg.model.clone());
    let effort = match (app.cfg.effort, app.chat.resolved) {
        (Effort::Auto, Some(r)) => format!("auto→{}", r.label().to_lowercase()),
        (e, _) => e.label().to_lowercase(),
    };
    let (status, status_style) = status_label(app);
    let left = vec![
        Span::styled(" termagent", accent(p)),
        Span::styled(format!(" {} ", termagent_core::VERSION), dim(p)),
        Span::styled("· ", dim(p)),
        Span::styled(ellipsize(&title, 40), base(p)),
        Span::styled("  ", dim(p)),
    ];
    let right = vec![
        Span::styled(format!("{model} "), Style::default().fg(p.fg)),
        Span::styled(format!("· {effort} · "), dim(p)),
        Span::styled(status, status_style),
        Span::raw(" "),
    ];
    f.render_widget(
        Paragraph::new(hline(left, right, header.width as usize)),
        header,
    );

    let lines = feed_lines(&app.chat, p, feed.width as usize, app.tick);
    let total = lines.len();
    let view = feed.height as usize;
    let max_scroll = total.saturating_sub(view);
    let back = (app.chat.scroll_back as usize).min(max_scroll);
    let offset = max_scroll.saturating_sub(back);
    let mut paragraph = Paragraph::new(Text::from(lines));
    if offset > 0 {
        paragraph = paragraph.scroll((offset as u16, 0));
    }
    f.render_widget(paragraph, feed);

    let notice_line = match &app.chat.notice {
        Some((text, at)) if at.elapsed().as_secs() < 4 => Line::from(vec![
            Span::raw("  "),
            Span::styled(text.clone(), Style::default().fg(p.warn)),
        ]),
        _ => Line::from(""),
    };
    f.render_widget(Paragraph::new(notice_line), notice);

    let avail = composer.width.saturating_sub(4) as usize;
    let before = app.input.display_before_cursor();
    let before_w = word_width(&before);
    let shift = before_w.saturating_sub(avail.saturating_sub(1));
    let visible = util::slice_width(&app.input.value(), shift, avail);
    let cursor_col = 2 + before_w.saturating_sub(shift);
    let composer_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(sym("›", ">"), accent(p)),
        Span::raw(" "),
        Span::styled(visible, base(p)),
    ]);
    f.render_widget(Paragraph::new(composer_line), composer);
    if app.chat.pending.is_none()
        && matches!(app.overlay, Overlay::None)
        && app.chat.status == Status::Idle
    {
        let x = composer.x + cursor_col.min(composer.width.saturating_sub(1) as usize) as u16;
        f.set_cursor_position((x, composer.y));
    }

    let hint = if app.chat.status == Status::Streaming {
        "esc stop · ↑ scroll · ^k models · ^s sessions · ^e effort · ^c quit"
    } else {
        "enter send · ^k models · ^s sessions · ^e effort · ^t theme · /help"
    };
    let model_ref = app.registry.model(&app.cfg.provider, &app.cfg.model);
    let cost = model_ref.map(|m| app.chat.usage.cost(m));
    let usage = if app.chat.usage.input + app.chat.usage.output == 0 {
        String::new()
    } else {
        format!(
            "{} in / {} out{} ",
            compact_tokens(app.chat.usage.input),
            compact_tokens(app.chat.usage.output),
            cost.map(|c| format!(" · ${c:.4}")).unwrap_or_default()
        )
    };
    let left = vec![Span::raw(" "), Span::styled(hint, dim(p))];
    let right = vec![Span::styled(usage, dim(p))];
    f.render_widget(
        Paragraph::new(hline(left, right, footer.width as usize)),
        footer,
    );
}

fn compact_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

fn feed_lines(chat: &ChatState, p: &Palette, width: usize, tick: usize) -> Vec<Line<'static>> {
    let width = width.max(20);
    let inner = width.saturating_sub(2);
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (idx, item) in chat.items.iter().enumerate() {
        let live = idx + 1 == chat.items.len();
        match item {
            Item::User(text) => {
                let wrapped = wrap(text, inner.saturating_sub(2));
                for (i, l) in wrapped.iter().enumerate() {
                    let prefix = if i == 0 {
                        format!(" {} ", sym("›", ">"))
                    } else {
                        "   ".into()
                    };
                    lines.push(Line::from(vec![
                        Span::styled(prefix, accent(p)),
                        Span::styled(
                            l.clone(),
                            Style::default().fg(p.fg).add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
                lines.push(Line::from(""));
            }
            Item::Assistant(text) => {
                let mut body: Vec<&str> = text.lines().collect();
                if live && chat.live == Live::Assistant {
                    body.push("");
                }
                let mut in_code = false;
                for l in body.iter() {
                    let trimmed = l.trim_start();
                    if trimmed.starts_with("```") {
                        in_code = !in_code;
                        continue;
                    }
                    if in_code {
                        for wrapped in wrap(l, inner.saturating_sub(2)) {
                            lines.push(Line::from(vec![
                                Span::raw("   "),
                                Span::styled(wrapped, dim(p)),
                            ]));
                        }
                        continue;
                    }
                    let (cleaned, bold) = clean_markdown(l);
                    if cleaned.trim().is_empty() {
                        lines.push(Line::from(""));
                        continue;
                    }
                    let style = if bold {
                        base(p).add_modifier(Modifier::BOLD)
                    } else {
                        base(p)
                    };
                    for wrapped in wrap(&cleaned, inner) {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(wrapped, style),
                        ]));
                    }
                }
                if live && chat.live == Live::Assistant {
                    if let Some(last) = lines.last_mut() {
                        last.spans
                            .push(Span::styled(sym("▍", "_"), Style::default().fg(p.accent)));
                    }
                }
                lines.push(Line::from(""));
            }
            Item::Reasoning(text) => {
                let wrapped = wrap(text, inner.saturating_sub(2));
                let mut shown: Vec<String> = wrapped.iter().take(6).cloned().collect();
                if wrapped.len() > 6 {
                    shown.push(format!("… {} more lines of reasoning", wrapped.len() - 6));
                }
                for (i, l) in shown.iter().enumerate() {
                    let mut spans = Vec::new();
                    if i == 0 {
                        spans.push(Span::styled(
                            format!(" {} ", sym("✻", "*")),
                            Style::default().fg(p.accent),
                        ));
                        spans.push(Span::styled("thinking ", dim(p)));
                    } else {
                        spans.push(Span::raw("   "));
                    }
                    spans.push(Span::styled(
                        l.clone(),
                        Style::default().fg(p.dim).add_modifier(Modifier::ITALIC),
                    ));
                    lines.push(Line::from(spans));
                }
                lines.push(Line::from(""));
            }
            Item::Tool(card) => {
                let (risk_label, risk_style) = risk_style(p, card.risk);
                let status = if card.running {
                    Span::styled(
                        util::spinner(tick).to_string(),
                        Style::default().fg(p.accent),
                    )
                } else if card.ok {
                    Span::styled(
                        if card.elapsed_ms > 0 {
                            format!("{} {}ms", sym("✓", "ok"), card.elapsed_ms)
                        } else {
                            sym("✓", "ok").to_string()
                        },
                        Style::default().fg(p.ok),
                    )
                } else {
                    Span::styled(sym("✖", "x"), Style::default().fg(p.err))
                };
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        sym("⏺", "*"),
                        Style::default().fg(if card.ok { p.ok } else { p.err }),
                    ),
                    Span::raw(" "),
                    Span::styled(card.name.clone(), base(p).add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(ellipsize(&card.summary, (inner / 2).max(20)), dim(p)),
                    Span::raw(" "),
                    Span::styled(format!("[{risk_label}]"), risk_style),
                    Span::raw(" "),
                    status,
                ]));
                if !card.output.is_empty() {
                    let mut out_lines: Vec<String> = Vec::new();
                    for l in card.output.lines().take(8) {
                        out_lines.extend(wrap(l, inner.saturating_sub(4)));
                    }
                    if card.output.lines().count() > 8 {
                        out_lines.push("…".into());
                    }
                    let style = if card.ok {
                        dim(p)
                    } else {
                        Style::default().fg(p.err)
                    };
                    for l in out_lines {
                        lines.push(Line::from(vec![Span::raw("    "), Span::styled(l, style)]));
                    }
                }
                lines.push(Line::from(""));
            }
            Item::Error(text) => {
                for (i, l) in wrap(text, inner.saturating_sub(2)).iter().enumerate() {
                    let prefix = if i == 0 {
                        format!(" {} ", sym("✖", "x"))
                    } else {
                        "   ".into()
                    };
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(p.err)),
                        Span::styled(l.clone(), Style::default().fg(p.err)),
                    ]));
                }
                lines.push(Line::from(""));
            }
            Item::Info(text) => {
                for l in wrap(text, inner.saturating_sub(2)) {
                    lines.push(Line::from(vec![Span::raw("  "), Span::styled(l, dim(p))]));
                }
                lines.push(Line::from(""));
            }
        }
    }

    if chat.items.is_empty() {
        lines.extend(empty_state(p, width));
    }
    lines
}

fn empty_state(p: &Palette, width: usize) -> Vec<Line<'static>> {
    let mut out = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("termagent", accent(p)),
            Span::styled(" runs in this workspace.", base(p)),
        ]),
    ];
    out.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("Ask it to read, edit, search or run something.", dim(p)),
    ]));
    out.push(Line::from(""));
    for tip in [
        "read src and explain how the config loads",
        "fix the failing test in tests/",
        "add a --quiet flag to the cli",
    ] {
        out.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(sym("›", ">"), accent(p)),
            Span::raw(" "),
            Span::styled(tip, dim(p)),
        ]));
    }
    out.push(Line::from(""));
    out.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            ellipsize("ctrl+k picks any model from any configured provider", width),
            dim(p),
        ),
    ]));
    out
}

fn draw_onboarding(f: &mut Frame, app: &App, area: Rect) {
    let p = &app.palette;
    let Some(ob) = &app.onboarding else {
        return;
    };
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .split(area);
    let (header, steps, body, footer) = (rows[0], rows[1], rows[2], rows[3]);

    let left = vec![
        Span::styled(" termagent", accent(p)),
        Span::styled(format!(" {} ", termagent_core::VERSION), dim(p)),
        Span::styled(
            if ob.reconfigure {
                "· configure"
            } else {
                "· first run"
            },
            dim(p),
        ),
    ];
    let flow: Vec<String> = step_entries(ob)
        .into_iter()
        .map(|(_, label)| label)
        .collect();
    let right = vec![Span::styled(format!("{} ", flow.join(" → ")), dim(p))];
    f.render_widget(
        Paragraph::new(hline(left, right, header.width as usize)),
        header,
    );
    f.render_widget(Paragraph::new(step_line(ob, p)), steps);

    match ob.step {
        Step::Provider => draw_provider_step(f, app, ob, body),
        Step::Key => draw_key_step(f, app, ob, body),
        Step::Model => draw_model_step(f, app, ob, body),
        Step::Effort => draw_effort_step(f, app, ob, body),
    }

    let hint = match ob.step {
        Step::Provider => "type to search a provider · ↑↓ choose · enter select · esc quit",
        Step::Key => "paste or type the key · enter continue · esc back",
        Step::Model => "type to filter models · ↑↓ choose · enter select · esc back",
        Step::Effort => "↑↓ choose reasoning effort · enter start · esc back",
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(hint, dim(p))])),
        footer,
    );
}

fn step_entries(_ob: &Onboarding) -> Vec<(Step, String)> {
    let mut entries = vec![
        (Step::Provider, "provider".to_string()),
        (Step::Key, "key".to_string()),
    ];
    entries.push((Step::Model, "model".to_string()));
    entries.push((Step::Effort, "effort".to_string()));
    entries
}

fn step_line(ob: &Onboarding, p: &Palette) -> Line<'static> {
    let entries = step_entries(ob);
    let current = entries
        .iter()
        .position(|(step, _)| *step == ob.step)
        .unwrap_or(0);
    let mut spans = vec![Span::raw(" ")];
    for (idx, (step, label)) in entries.iter().enumerate() {
        let style = if *step == ob.step {
            accent(p)
        } else if idx < current {
            Style::default().fg(p.ok)
        } else {
            dim(p)
        };
        if idx > 0 {
            spans.push(Span::styled("  ·  ", dim(p)));
        }
        spans.push(Span::styled(format!("{} {label}", idx + 1), style));
    }
    Line::from(spans)
}

fn body_area(area: Rect) -> Rect {
    Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(4),
        height: area.height,
    }
}

fn draw_provider_step(f: &mut Frame, app: &App, ob: &Onboarding, area: Rect) {
    let p = &app.palette;
    let area = body_area(area);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(2),
    ])
    .split(area);
    let (title, search, list) = (rows[0], rows[1], rows[2]);
    let providers = ob.providers(&app.registry);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Pick a provider", base(p).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("  {} with tool calling · models.dev", providers.len()),
                dim(p),
            ),
        ])),
        title,
    );
    f.render_widget(search_line(&ob.query, p, "filter providers"), search);
    let height = list.height as usize;
    let offset = util::window(providers.len(), ob.sel, height);
    for (row, provider) in providers.iter().skip(offset).take(height).enumerate() {
        let idx = offset + row;
        let y = list.y + row as u16;
        let selected = idx == ob.sel;
        let name_style = if selected {
            accent(p)
        } else {
            base(p).add_modifier(Modifier::BOLD)
        };
        let key_badge = if app.cfg.direct_key(provider).is_some() {
            Span::styled(
                format!(" {} key ", sym("●", "*")),
                Style::default().fg(p.ok),
            )
        } else if provider.api.contains("127.0.0.1") || provider.api.contains("localhost") {
            Span::styled(" local ", dim(p))
        } else {
            Span::styled("", dim(p))
        };
        let right = vec![
            Span::styled(format!("{} models ", provider.models.len()), dim(p)),
            Span::styled(format!("{} ", provider.protocol.label()), dim(p)),
            key_badge,
        ];
        let left = vec![
            Span::styled(if selected { "▸ " } else { "  " }, accent(p)),
            Span::styled(ellipsize(&provider.name, 32), name_style),
            Span::styled(format!("  {}", ellipsize(&provider.id, 26)), dim(p)),
        ];
        f.render_widget(
            Paragraph::new(hline(left, right, list.width as usize)),
            Rect { y, ..list },
        );
    }
    if providers.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  no provider matches that search",
                dim(p),
            ))),
            Rect { y: list.y, ..list },
        );
    }
}

fn draw_key_step(f: &mut Frame, app: &App, ob: &Onboarding, area: Rect) {
    let p = &app.palette;
    let area = body_area(area);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let provider = ob.provider.as_ref();
    let name = provider.map(|pr| pr.name.clone()).unwrap_or_default();
    lines.push(Line::from(vec![
        Span::styled("Connect ", base(p)),
        Span::styled(name, accent(p)),
    ]));
    lines.push(Line::from(""));
    if let Some(pr) = provider {
        if let Some(var) = pr.env.first() {
            let found = ob.env_found.as_deref() == Some(var.as_str());
            lines.push(Line::from(vec![
                Span::styled("  env ", dim(p)),
                Span::styled(var.clone(), Style::default().fg(p.fg)),
                Span::styled(
                    if found {
                        "  found in your environment"
                    } else {
                        "  not set"
                    },
                    Style::default().fg(if found { p.ok } else { p.dim }),
                ),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled("  endpoint ", dim(p)),
            Span::styled(pr.api.clone(), Style::default().fg(p.fg)),
        ]));
    }
    lines.push(Line::from(""));
    let saved = ob
        .provider
        .as_ref()
        .map(|provider| app.cfg.keys.contains_key(&provider.id))
        .unwrap_or(false);
    if saved {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "A key is saved. Paste a new key to replace it, or press Enter to keep it.",
                dim(p),
            ),
        ]));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(vec![
        Span::styled("  Paste a key", base(p)),
        Span::styled(" (stored 0600 in ", dim(p)),
        Span::styled(
            termagent_core::Config::path().display().to_string(),
            Style::default().fg(p.fg),
        ),
        Span::styled(")", dim(p)),
    ]));
    lines.push(Line::from(""));
    let key_row = lines.len() as u16;
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(sym("›", ">"), accent(p)),
        Span::raw(" "),
        Span::styled(ob.key.masked(), base(p)),
    ]));
    if let Some(notice) = &ob.notice {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(notice.clone(), Style::default().fg(p.warn)),
        ]));
    }
    f.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        area,
    );
    let x = area.x + 4 + word_width(&ob.key.masked()).min(area.width as usize) as u16;
    f.set_cursor_position((x.min(area.x + area.width - 1), area.y + key_row));
}

fn draw_model_step(f: &mut Frame, app: &App, ob: &Onboarding, area: Rect) {
    let p = &app.palette;
    let area = body_area(area);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(2),
    ])
    .split(area);
    let (title, search, list) = (rows[0], rows[1], rows[2]);
    let provider = ob
        .provider
        .as_ref()
        .map(|pr| pr.name.clone())
        .unwrap_or_default();
    let models = ob.models();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Pick a model", base(p).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("  {provider} · {} tool models", models.len()),
                dim(p),
            ),
        ])),
        title,
    );
    f.render_widget(search_line(&ob.query, p, "filter models"), search);
    let height = list.height as usize;
    let offset = util::window(models.len(), ob.sel, height);
    for (row, model) in models.iter().skip(offset).take(height).enumerate() {
        let idx = offset + row;
        let selected = idx == ob.sel;
        let left = vec![
            Span::styled(if selected { "▸ " } else { "  " }, accent(p)),
            Span::styled(
                ellipsize(&model.name, 40),
                if selected { accent(p) } else { base(p) },
            ),
            Span::styled(format!("  {}", ellipsize(&model.id, 34)), dim(p)),
        ];
        let mut right = Vec::new();
        if model.reasoning {
            right.push(Span::styled("reasoning ", Style::default().fg(p.accent)));
        }
        if model.context > 0 {
            right.push(Span::styled(
                format!("{} ctx ", compact_tokens(model.context)),
                dim(p),
            ));
        }
        if model.cost_in > 0.0 || model.cost_out > 0.0 {
            right.push(Span::styled(
                format!("${:.2}/${:.2} ", model.cost_in, model.cost_out),
                dim(p),
            ));
        }
        f.render_widget(
            Paragraph::new(hline(left, right, list.width as usize)),
            Rect {
                y: list.y + row as u16,
                ..list
            },
        );
    }
    if models.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  no model matches that search",
                dim(p),
            ))),
            Rect { y: list.y, ..list },
        );
    }
}

fn draw_effort_step(f: &mut Frame, app: &App, ob: &Onboarding, area: Rect) {
    let p = &app.palette;
    let area = body_area(area);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Reasoning effort", base(p).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(
                    "  {} · {}",
                    ob.provider.as_ref().map(|x| x.name.as_str()).unwrap_or(""),
                    ob.model.as_ref().map(|m| m.name.as_str()).unwrap_or("")
                ),
                dim(p),
            ),
        ]),
        Line::from(""),
    ];
    let options = ob.effort_options();
    for (idx, effort) in options.iter().enumerate() {
        let selected = idx == ob.effort_sel;
        let desc = match effort {
            Effort::Auto => "picks low, medium or high per message",
            Effort::Low => "fast answers, short edits",
            Effort::Medium => "default balance",
            Effort::High => "multi-step tasks, debugging, design",
            Effort::Max => "deepest pass, slowest, highest cost",
        };
        lines.push(Line::from(vec![
            Span::styled(if selected { "▸ " } else { "  " }, accent(p)),
            Span::styled(
                format!("{:<7}", effort.label()),
                if selected { accent(p) } else { base(p) },
            ),
            Span::styled(desc, dim(p)),
        ]));
    }
    if options.len() == 1 {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("this model does not expose reasoning levels", dim(p)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("enter starts the session", dim(p)),
    ]));
    f.render_widget(Paragraph::new(Text::from(lines)), area);
}

fn search_line(
    input: &crate::input::TextInput,
    p: &Palette,
    placeholder: &str,
) -> Paragraph<'static> {
    let text = if input.is_empty() {
        Span::styled(format!("  {placeholder}"), dim(p))
    } else {
        Span::styled(format!("  {}", input.value()), base(p))
    };
    Paragraph::new(Line::from(vec![text]))
}

fn centered(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - pct_y) / 2),
        Constraint::Percentage(pct_y),
        Constraint::Percentage((100 - pct_y) / 2),
    ])
    .split(area);
    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - pct_x) / 2),
        Constraint::Percentage(pct_x),
        Constraint::Percentage((100 - pct_x) / 2),
    ])
    .split(vertical[1]);
    horizontal[1]
}

fn center_rect(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn panel(p: &Palette, title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.accent))
        .title(Span::styled(format!(" {title} "), accent(p)))
        .title_alignment(Alignment::Left)
}

fn draw_models(f: &mut Frame, app: &App, picker: &ModelPicker, area: Rect) {
    let p = &app.palette;
    let providers_len = app.picker_provider_list(picker).len();
    let models_len = app.picker_model_list(picker).len();
    let rows = providers_len.max(models_len) + 6;
    let height = (rows as u16).clamp(12, (area.height as u32 * 86 / 100) as u16);
    let width = ((area.width as u32 * 92 / 100) as u16).max(56);
    let rect = center_rect(area, width, height);
    f.render_widget(Clear, rect);
    f.render_widget(panel(p, "models"), rect);
    let inner = rect.inner(ratatui::layout::Margin::new(1, 1));
    let rows = Layout::vertical([Constraint::Min(4), Constraint::Length(1)]).split(inner);
    let cols = Layout::horizontal([Constraint::Length(34), Constraint::Min(20)]).split(rows[0]);
    let (left, right) = (cols[0], cols[1]);

    let providers = app.picker_provider_list(picker);
    let models = app.picker_model_list(picker);
    let provider_rows = Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).split(left);
    f.render_widget(
        search_line(&picker.provider_query, p, "filter providers"),
        provider_rows[0],
    );
    let list = provider_rows[1];
    let height = list.height as usize;
    let offset = util::window(providers.len(), picker.provider_sel, height);
    for (row, provider) in providers.iter().skip(offset).take(height).enumerate() {
        let idx = offset + row;
        let selected = idx == picker.provider_sel;
        let active = provider.id == app.cfg.provider;
        let cursor = selected && matches!(picker.pane, Pane::Providers);
        let left_spans = vec![
            Span::styled(if cursor { "▸ " } else { "  " }, accent(p)),
            Span::styled(
                ellipsize(&provider.name, 18),
                if selected { accent(p) } else { base(p) },
            ),
            Span::styled(
                format!(" {}", if active { sym("●", "*") } else { " " }),
                Style::default().fg(p.ok),
            ),
        ];
        let right_spans = vec![Span::styled(format!("{} ", provider.models.len()), dim(p))];
        f.render_widget(
            Paragraph::new(hline(left_spans, right_spans, list.width as usize)),
            Rect {
                y: list.y + row as u16,
                ..list
            },
        );
    }

    let model_rows = Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).split(right);
    let provider_name = providers
        .get(picker.provider_sel)
        .map(|pr| format!("{} · {} models", pr.name, pr.models.len()))
        .unwrap_or_else(|| "no provider".into());
    let search = if picker.model_query.is_empty() {
        Span::styled("  filter models", dim(p))
    } else {
        Span::styled(format!("  {}", picker.model_query.value()), base(p))
    };
    let hint = if matches!(picker.pane, Pane::Models) {
        Span::styled(" typing filters ", dim(p))
    } else {
        Span::styled("", dim(p))
    };
    let mut left = vec![search];
    left.push(Span::raw("  "));
    left.push(Span::styled(ellipsize(&provider_name, 40), dim(p)));
    f.render_widget(
        Paragraph::new(hline(left, vec![hint], model_rows[0].width as usize)),
        model_rows[0],
    );
    let list = model_rows[1];
    let height = list.height as usize;
    let offset = util::window(models.len(), picker.model_sel, height);
    for (row, model) in models.iter().skip(offset).take(height).enumerate() {
        let idx = offset + row;
        let selected = idx == picker.model_sel;
        let current = model.id == app.cfg.model
            && providers.get(picker.provider_sel).map(|pr| pr.id.as_str())
                == Some(app.cfg.provider.as_str());
        let cursor = selected && matches!(picker.pane, Pane::Models);
        let left_spans = vec![
            Span::styled(if cursor { "▸ " } else { "  " }, accent(p)),
            Span::styled(
                ellipsize(&model.name, 34),
                if selected { accent(p) } else { base(p) },
            ),
            Span::styled(
                format!(" {}", if current { sym("●", "*") } else { " " }),
                Style::default().fg(p.ok),
            ),
        ];
        let mut right_spans = Vec::new();
        if model.reasoning {
            right_spans.push(Span::styled("reasoning ", Style::default().fg(p.accent)));
        }
        if model.context > 0 {
            right_spans.push(Span::styled(
                format!("{} ", compact_tokens(model.context)),
                dim(p),
            ));
        }
        if model.cost_in > 0.0 || model.cost_out > 0.0 {
            right_spans.push(Span::styled(
                format!("${:.2}/${:.2} ", model.cost_in, model.cost_out),
                dim(p),
            ));
        }
        f.render_widget(
            Paragraph::new(hline(left_spans, right_spans, list.width as usize)),
            Rect {
                y: list.y + row as u16,
                ..list
            },
        );
    }

    let effort = match (app.cfg.effort, app.chat.resolved) {
        (Effort::Auto, Some(r)) => format!("auto → {}", r.label()),
        (e, _) => e.label().to_string(),
    };
    let footer = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "tab switch pane · ↑↓ move · type to filter · enter pick",
            dim(p),
        ),
        Span::styled("  effort: ", dim(p)),
        Span::styled(effort, accent(p)),
        Span::styled(" · ctrl+e · esc close", dim(p)),
    ]);
    f.render_widget(Paragraph::new(footer), rows[1]);
}

fn draw_sessions(f: &mut Frame, app: &App, picker: &SessionPicker, area: Rect) {
    let p = &app.palette;
    let height = (picker.items.len() as u16 + 5).clamp(8, (area.height as u32 * 74 / 100) as u16);
    let width = ((area.width as u32 * 76 / 100) as u16).max(50);
    let rect = center_rect(area, width, height);
    f.render_widget(Clear, rect);
    f.render_widget(panel(p, "sessions"), rect);
    let inner = rect.inner(ratatui::layout::Margin::new(1, 1));
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(inner);
    let list = rows[0];
    if picker.items.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "no saved sessions yet · enter a prompt to start one",
                    dim(p),
                ),
            ])),
            list,
        );
    }
    let height = list.height as usize;
    let offset = util::window(picker.items.len(), picker.sel, height);
    for (row, meta) in picker.items.iter().skip(offset).take(height).enumerate() {
        let idx = offset + row;
        let selected = idx == picker.sel;
        let active = app
            .chat
            .session
            .as_ref()
            .map(|s| s.id == meta.id)
            .unwrap_or(false);
        let when = chrono::DateTime::from_timestamp(meta.created, 0)
            .map(|d| {
                d.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_default();
        let left = vec![
            Span::styled(if selected { "▸ " } else { "  " }, accent(p)),
            Span::styled(
                ellipsize(&meta.title, 40),
                if selected { accent(p) } else { base(p) },
            ),
            Span::styled(
                format!(" {}", if active { sym("●", "*") } else { " " }),
                Style::default().fg(p.ok),
            ),
        ];
        let right = vec![
            Span::styled(format!("{} ", meta.model), dim(p)),
            Span::styled(format!("{when} "), dim(p)),
        ];
        f.render_widget(
            Paragraph::new(hline(left, right, list.width as usize)),
            Rect {
                y: list.y + row as u16,
                ..list
            },
        );
    }
    let confirm = picker
        .confirm
        .as_ref()
        .map(|_| "  press d again to delete")
        .unwrap_or("");
    let hint = format!("enter load · n new · d delete{confirm} · esc close");
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(hint, dim(p))])),
        rows[1],
    );
}

fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let p = &app.palette;
    let rect = centered(area, 70, 70);
    f.render_widget(Clear, rect);
    f.render_widget(panel(p, "keys and commands"), rect);
    let inner = rect.inner(ratatui::layout::Margin::new(2, 1));
    let keys = [
        ("enter", "send the prompt"),
        ("esc", "stop the current turn / clear input"),
        ("ctrl+k", "model picker (all providers)"),
        ("ctrl+s", "sessions"),
        ("ctrl+e", "cycle effort: auto low medium high max"),
        ("ctrl+t", "cycle theme"),
        ("↑ ↓ / page", "scroll the conversation"),
        ("ctrl+c", "quit"),
        ("/model", "model picker"),
        ("/effort <level>", "set effort"),
        ("/sessions", "sessions"),
        ("/new", "fresh session"),
        ("/config", "re-run onboarding"),
        ("/key", "replace the API key"),
        ("/tools", "list tools"),
        ("/theme <name>", "switch theme"),
    ];
    let mut lines = Vec::new();
    for (key, desc) in keys {
        lines.push(Line::from(vec![
            Span::styled(format!("{key:<16}"), accent(p)),
            Span::styled(desc, base(p)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("tools ", dim(p)),
        Span::styled("read write edit ls glob grep bash todo webfetch", base(p)),
    ]));
    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn draw_approval(f: &mut Frame, app: &App, req: &ApprovalRequest, area: Rect) {
    let p = &app.palette;
    let args_lines = serde_json::to_string_pretty(&req.args)
        .map(|raw| raw.lines().count())
        .unwrap_or(1)
        .min(8);
    let height = (args_lines as u16 + 10).clamp(12, area.height.saturating_sub(2));
    let width = ((area.width as u32 * 74 / 100) as u16).max(54);
    let rect = center_rect(area, width, height);
    f.render_widget(Clear, rect);
    let (risk_label, risk_style) = risk_style(p, req.risk);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.warn))
        .title(Line::from(vec![
            Span::styled(
                " approval ",
                Style::default().fg(p.warn).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{risk_label} risk "), risk_style),
        ]));
    f.render_widget(block, rect);
    let inner = rect.inner(ratatui::layout::Margin::new(2, 1));
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(inner);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("tool  ", dim(p)),
            Span::styled(req.tool.clone(), accent(p)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("command  ", dim(p)),
            Span::styled(
                ellipsize(
                    req.summary
                        .strip_prefix(&format!("{}: ", req.tool))
                        .unwrap_or(&req.summary),
                    inner.width.saturating_sub(12) as usize,
                ),
                base(p),
            ),
        ]),
        Line::from(""),
    ];
    let pretty = serde_json::to_string_pretty(&req.args).unwrap_or_default();
    for l in wrap(&pretty, inner.width as usize - 2).iter().take(8) {
        lines.push(Line::from(Span::styled(format!("  {l}"), dim(p))));
    }
    lines.push(Line::from(""));
    f.render_widget(Paragraph::new(Text::from(lines)), rows[0]);
    let footer = Line::from(vec![
        Span::styled(
            "[y]",
            Style::default().fg(p.ok).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" approve   ", dim(p)),
        Span::styled(
            "[a]",
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" always allow this session   ", dim(p)),
        Span::styled(
            "[n]",
            Style::default().fg(p.err).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" deny", dim(p)),
    ]);
    f.render_widget(Paragraph::new(footer), rows[1]);
}

/// Terminal markdown: drop emphasis markers, keep headings bold, bullets round.
fn clean_markdown(line: &str) -> (String, bool) {
    let trimmed = line.trim_end();
    let mut text = trimmed.to_string();
    let mut bold = false;
    if text.trim_start().starts_with('#') {
        text = text
            .trim_start()
            .trim_start_matches('#')
            .trim_start()
            .to_string();
        bold = true;
    }
    if text.starts_with("**") && text.ends_with("**") && text.matches("**").count() == 2 {
        text = text
            .trim_start_matches("**")
            .trim_end_matches("**")
            .to_string();
        bold = true;
    }
    text = text.replace("**", "").replace('`', "");
    for (from, to) in [("- ", "• "), ("* ", "• ")] {
        if text.starts_with(from) {
            text = format!("{to}{}", &text[from.len()..]);
            break;
        }
    }
    (text, bold)
}
