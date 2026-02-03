use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use phaedra_core::stats::{LiveStats, SharedStats};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::Stdout;

pub fn init_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

pub async fn run_tui(
    shared: SharedStats,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<()> {
    let mut terminal = init_terminal()?;

    loop {
        if shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
            }
        }

        let stats = shared.lock().unwrap().clone();
        terminal.draw(|f| render(f, &stats))?;
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

fn render(f: &mut Frame, stats: &LiveStats) {
    let size = f.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    // Title bar
    let elapsed = format!(
        "{:02}:{:02}:{:02}",
        stats.elapsed_secs / 3600,
        (stats.elapsed_secs % 3600) / 60,
        stats.elapsed_secs % 60,
    );
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "PHAEDRA ",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "v0.1.0 -- local-first protocol fuzzer",
            Style::default().fg(Color::White),
        ),
        Span::styled(
            format!("  [{elapsed}]"),
            Style::default().fg(Color::Yellow),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, outer[0]);

    // Main: left + right
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[1]);

    // Left column
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Min(4),
            Constraint::Min(4),
        ])
        .split(main[0]);

    // Stats box
    let stats_lines = vec![
        Line::from(vec![
            Span::styled("  Executions : ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{}", stats.total_execs),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Exec / sec : ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{:.0}", stats.execs_per_sec),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Corpus     : ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{}", stats.corpus_size),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Edges      : ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{}", stats.unique_edges),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Crashes    : ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!(
                    "{} total / {} unique",
                    stats.total_crashes, stats.unique_crashes
                ),
                if stats.unique_crashes > 0 {
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("  Timeouts   : ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{}", stats.total_timeouts),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Interesting: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{}", stats.interesting_execs),
                Style::default().fg(Color::Green),
            ),
        ]),
    ];
    let stats_widget = Paragraph::new(stats_lines)
        .block(Block::default().borders(Borders::ALL).title(" Stats "));
    f.render_widget(stats_widget, left[0]);

    // Recent finds
    let find_items: Vec<ListItem> = if stats.recent_finds.is_empty() {
        vec![ListItem::new("  (none yet)")]
    } else {
        stats
            .recent_finds
            .iter()
            .map(|s| {
                ListItem::new(format!("  + {s}")).style(Style::default().fg(Color::Green))
            })
            .collect()
    };
    f.render_widget(
        List::new(find_items)
            .block(Block::default().borders(Borders::ALL).title(" Recent Finds ")),
        left[1],
    );

    // Recent crashes
    let crash_items: Vec<ListItem> = if stats.recent_crashes.is_empty() {
        vec![ListItem::new("  (none yet)")]
    } else {
        stats
            .recent_crashes
            .iter()
            .map(|s| ListItem::new(format!("  ! {s}")).style(Style::default().fg(Color::Red)))
            .collect()
    };
    f.render_widget(
        List::new(crash_items)
            .block(Block::default().borders(Borders::ALL).title(" Recent Crashes ")),
        left[2],
    );

    // Right column
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(main[1]);

    // Strategy bar chart — build owned strings first, then borrow as &str
    let bar_strings: Vec<(String, u64)> = stats
        .strategy_weights
        .iter()
        .map(|(name, w)| (name.clone(), (*w as u64).max(1)))
        .collect();
    let bar_data: Vec<(&str, u64)> = bar_strings
        .iter()
        .map(|(s, v)| (s.as_str(), *v))
        .collect();

    if !bar_data.is_empty() {
        let chart = BarChart::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Strategy Weights "),
            )
            .data(&bar_data)
            .bar_width(3)
            .bar_gap(1)
            .bar_style(Style::default().fg(Color::Blue))
            .value_style(Style::default().fg(Color::White));
        f.render_widget(chart, right[0]);
    } else {
        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title(" Strategy Weights "),
            right[0],
        );
    }

    // Coverage gauge
    let edge_ratio = (stats.unique_edges as f64 / 65536.0).min(1.0);
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Edge Coverage (/ 65536) "),
        )
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::Black))
        .ratio(edge_ratio)
        .label(format!("{} / 65536 edges", stats.unique_edges));
    f.render_widget(gauge, right[1]);

    // Help bar
    let help = Paragraph::new(Line::from(vec![
        Span::styled(
            " q",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" quit  "),
        Span::styled(
            " Ctrl-C",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" stop campaign"),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, outer[2]);
}
