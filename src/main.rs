use std::{
    env, fs, io,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use libc::{ECHO, ICANON, STDIN_FILENO, TCSANOW, VMIN, VTIME, tcgetattr, tcsetattr, termios};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use serde::Deserialize;

const DEFAULT_CONFIG: &str = "net-stats.toml";
const DEFAULT_REFRESH_MS: u64 = 1000;

#[derive(Debug, Deserialize)]
struct Config {
    interfaces: Vec<InterfaceConfig>,
    refresh_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct InterfaceConfig {
    alias: String,
    device: String,
}

#[derive(Clone, Copy, Debug)]
struct Counters {
    rx_bytes: u64,
    tx_bytes: u64,
}

#[derive(Debug)]
struct InterfaceState {
    alias: String,
    device: String,
    last_counters: Option<Counters>,
    last_sample: Option<Instant>,
    rx_rate_bps: f64,
    tx_rate_bps: f64,
    last_error: Option<String>,
}

impl InterfaceState {
    fn new(config: InterfaceConfig) -> Self {
        Self {
            alias: config.alias,
            device: config.device,
            last_counters: None,
            last_sample: None,
            rx_rate_bps: 0.0,
            tx_rate_bps: 0.0,
            last_error: None,
        }
    }

    fn sample(&mut self) {
        match read_counters(&self.device) {
            Ok(counters) => {
                let now = Instant::now();

                if let (Some(previous), Some(previous_at)) = (self.last_counters, self.last_sample)
                {
                    let elapsed = now.duration_since(previous_at).as_secs_f64().max(0.001);
                    let rx_delta = counters.rx_bytes.saturating_sub(previous.rx_bytes);
                    let tx_delta = counters.tx_bytes.saturating_sub(previous.tx_bytes);
                    self.rx_rate_bps = (rx_delta as f64) / elapsed;
                    self.tx_rate_bps = (tx_delta as f64) / elapsed;
                }

                self.last_counters = Some(counters);
                self.last_sample = Some(now);
                self.last_error = None;
            }
            Err(error) => {
                self.rx_rate_bps = 0.0;
                self.tx_rate_bps = 0.0;
                self.last_error = Some(error.to_string());
            }
        }
    }
}

struct App {
    config_path: PathBuf,
    refresh_interval: Duration,
    interfaces: Vec<InterfaceState>,
}

struct InputMode {
    original: termios,
}

impl App {
    fn from_config(config_path: PathBuf, config: Config) -> Result<Self> {
        if config.interfaces.is_empty() {
            bail!("config must contain at least one interface");
        }

        let refresh_ms = config.refresh_ms.unwrap_or(DEFAULT_REFRESH_MS).max(100);
        let interfaces = config
            .interfaces
            .into_iter()
            .map(InterfaceState::new)
            .collect();

        Ok(Self {
            config_path,
            refresh_interval: Duration::from_millis(refresh_ms),
            interfaces,
        })
    }

    fn sample_all(&mut self) {
        for interface in &mut self.interfaces {
            interface.sample();
        }
    }
}

fn main() -> Result<()> {
    let config_path = resolve_config_path();
    let config = load_config(&config_path)?;
    let mut app = App::from_config(config_path, config)?;
    app.sample_all();

    let (mut terminal, input_mode) = setup_terminal()?;
    let result = run_app(&mut terminal, &mut app);
    restore_terminal(&mut terminal, input_mode)?;
    result
}

fn resolve_config_path() -> PathBuf {
    env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG))
}

fn load_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read config at {}. Create it from {}",
            path.display(),
            DEFAULT_CONFIG.to_owned() + ".example"
        )
    })?;
    toml::from_str(&content).with_context(|| format!("failed to parse TOML in {}", path.display()))
}

fn setup_terminal() -> Result<(Terminal<CrosstermBackend<io::Stdout>>, InputMode)> {
    let input_mode = enable_cbreak_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let mut terminal =
        Terminal::new(CrosstermBackend::new(stdout)).context("failed to create terminal")?;
    terminal.clear().context("failed to clear terminal")?;
    Ok((terminal, input_mode))
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    input_mode: InputMode,
) -> Result<()> {
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    disable_cbreak_mode(input_mode)?;
    terminal.show_cursor().context("failed to restore cursor")
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let mut last_refresh = Instant::now();
    let (quit_tx, quit_rx) = mpsc::channel();

    thread::spawn(move || {
        let stdin = io::stdin();
        for byte in stdin.lock().bytes().flatten() {
            if byte == b'q' {
                let _ = quit_tx.send(());
                break;
            }
        }
    });

    loop {
        terminal.draw(|frame| draw(frame, app))?;

        let until_next = app
            .refresh_interval
            .saturating_sub(last_refresh.elapsed())
            .min(Duration::from_millis(250));

        if quit_rx.recv_timeout(until_next).is_ok() {
            break;
        }

        if last_refresh.elapsed() >= app.refresh_interval {
            app.sample_all();
            last_refresh = Instant::now();
        }
    }

    Ok(())
}

fn draw(frame: &mut Frame, app: &App) {
    let block = Block::default().title("Net Rates").borders(Borders::ALL);
    let area = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    let chunks = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).split(area);
    let header = Paragraph::new(vec![Line::from(format!(
        "config: {}   refresh: {} ms   q/Esc quit",
        app.config_path.display(),
        app.refresh_interval.as_millis()
    ))]);
    frame.render_widget(header, chunks[0]);

    let rows = app
        .interfaces
        .iter()
        .map(|interface| {
            if let Some(error) = &interface.last_error {
                Row::new(vec![
                    Cell::from(interface.alias.clone()),
                    Cell::from(interface.device.clone()),
                    Cell::from("error"),
                    Cell::from(error.clone()),
                ])
            } else {
                Row::new(vec![
                    Cell::from(interface.alias.clone()),
                    Cell::from(interface.device.clone()),
                    Cell::from(format!("{}/s", format_bytes(interface.rx_rate_bps))),
                    Cell::from(format!("{}/s", format_bytes(interface.tx_rate_bps))),
                ])
            }
        })
        .collect::<Vec<_>>();

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Min(16),
        ],
    )
    .header(Row::new(vec!["ALIAS", "DEVICE", "RX", "TX"]))
    .column_spacing(1);

    frame.render_widget(table, chunks[1]);
}

fn read_counters(interface: &str) -> Result<Counters> {
    let base = Path::new("/sys/class/net")
        .join(interface)
        .join("statistics");
    let rx_bytes = fs::read_to_string(base.join("rx_bytes"))
        .with_context(|| format!("failed to read {} rx_bytes", interface))?
        .trim()
        .parse()
        .with_context(|| format!("failed to parse {} rx_bytes", interface))?;
    let tx_bytes = fs::read_to_string(base.join("tx_bytes"))
        .with_context(|| format!("failed to read {} tx_bytes", interface))?
        .trim()
        .parse()
        .with_context(|| format!("failed to parse {} tx_bytes", interface))?;

    Ok(Counters { rx_bytes, tx_bytes })
}

fn format_bytes(value: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut scaled = value.max(0.0);
    let mut unit_index = 0;

    while scaled >= 1024.0 && unit_index < UNITS.len() - 1 {
        scaled /= 1024.0;
        unit_index += 1;
    }

    if scaled >= 100.0 {
        format!("{scaled:.0} {}", UNITS[unit_index])
    } else {
        format!("{scaled:.1} {}", UNITS[unit_index])
    }
}

fn enable_cbreak_mode() -> Result<InputMode> {
    let mut original = std::mem::MaybeUninit::<termios>::uninit();
    let get_result = unsafe { tcgetattr(STDIN_FILENO, original.as_mut_ptr()) };
    if get_result != 0 {
        bail!("failed to read terminal attributes");
    }

    let original = unsafe { original.assume_init() };
    let mut updated = original;
    updated.c_lflag &= !(ICANON | ECHO);
    updated.c_cc[VMIN] = 1;
    updated.c_cc[VTIME] = 0;

    let set_result = unsafe { tcsetattr(STDIN_FILENO, TCSANOW, &updated) };
    if set_result != 0 {
        bail!("failed to switch terminal input mode");
    }

    Ok(InputMode { original })
}

fn disable_cbreak_mode(input_mode: InputMode) -> Result<()> {
    let set_result = unsafe { tcsetattr(STDIN_FILENO, TCSANOW, &input_mode.original) };
    if set_result != 0 {
        bail!("failed to restore terminal input mode");
    }

    Ok(())
}
