#![allow(unused_must_use, unused_variables, dead_code, warnings)]

use core::fmt;
use std::{
  cell::RefCell,
  fmt::{ Display, Formatter },
  fs::File,
  io::Read,
  path::Path,
  process::Command,
  thread::sleep,
  time::Duration,
};
use crossterm::event::{ self, poll, KeyCode, KeyEvent };
use directories::{ BaseDirs, ProjectDirs, UserDirs };
use ratatui::{
  layout::{ Constraint, Layout, Rows },
  style::{ Style, Styled, Stylize },
  text::{ Line, Span },
  widgets::{ Block, BorderType, Cell, Padding, Paragraph, Row, Table },
};
use serde::{ Deserialize };
use tui_textarea::TextArea;

#[derive(Deserialize, Debug, Clone)]
enum ShortcutKind {
  #[serde(rename = "app")]
  App,
  #[serde(rename = "dir")]
  Dir,
  #[serde(rename = "file")]
  File,
  #[serde(rename = "url")]
  Url,
}

#[derive(Deserialize, Debug, Clone)]
enum ShortcutPathPrefix {
  #[serde(rename = "documents")]
  Documents,
  #[serde(rename = "appdata")]
  Appdata,
}

#[derive(Deserialize, Debug, Clone)]
struct Shortcut {
  path: String,
  seq: Vec<String>,
  description: Option<String>,
  kind: ShortcutKind,
  path_prefix: Option<ShortcutPathPrefix>,
}

impl Shortcut {
  /// Returns with prefixed path if `path_prefix` is defined, just `path` otherwise
  fn get_prefixed_path(&self) -> String {
    let mut path = self.path.clone();
    if let Some(prefix) = &self.path_prefix {
      path = Path::new(&prefix.to_string()).join(path).to_str().unwrap().to_string();
    }
    path
  }
}

impl Display for ShortcutPathPrefix {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", match self {
      ShortcutPathPrefix::Documents => {
        let user_dirs = UserDirs::new().unwrap();
        user_dirs.document_dir().unwrap().to_str().unwrap().to_string().replace("\\", "/")
      }
      ShortcutPathPrefix::Appdata => {
        let base_dirs = BaseDirs::new().unwrap();
        base_dirs.config_dir().to_str().unwrap().to_string().replace("\\", "/")
      }
    })
  }
}

#[derive(Deserialize, Debug)]
struct Config {
  shortcuts: Vec<Shortcut>,
}

trait ShortcutsTrait {
  fn find(&self, search: String) -> Vec<Shortcut>;
}

impl ShortcutsTrait for Vec<Shortcut> {
  fn find(&self, search: String) -> Self {
    if search.trim().is_empty() {
      return self.to_vec();
    }
    self
      .iter()
      .filter(|s| s.seq.iter().any(|seq| seq.contains(&search)))
      .map(|seq| seq.clone())
      .collect()
  }
}

#[derive(Debug)]
enum LoadConfigError {
  IoError(std::io::Error),
  ParseError(serde_json::Error),
  NoConfig,
}

struct App {
  config: Result<Config, LoadConfigError>,
  matched_shortcuts: Vec<Shortcut>,
  running: bool,
}

impl App {
  fn new() -> Self {
    let mut app = App {
      running: true,
      config: App::load_config(),
      matched_shortcuts: vec![],
    };
    if let Ok(config) = &app.config {
      app.matched_shortcuts = config.shortcuts.clone();
    }
    app
  }
  fn load_config() -> Result<Config, LoadConfigError> {
    let config_path = UserDirs::new().map(|user_dirs|
      user_dirs.document_dir().unwrap().join("bullet/config.json").to_str().unwrap().to_string()
    );
    if config_path.is_none() {
      return Err(LoadConfigError::NoConfig);
    }
    let config_file = File::open(config_path.unwrap());
    if config_file.is_err() {
      return Err(LoadConfigError::IoError(config_file.err().unwrap()));
    }
    let mut content = String::new();
    config_file.map(|mut f| f.read_to_string(&mut content));
    let config = serde_json::from_str::<Config>(&content);
    config.map_err(|e| LoadConfigError::ParseError(e))
  }
  fn find_and_handle_matches(&mut self, search: String) {
    if let Ok(cfg) = &self.config {
      self.matched_shortcuts = cfg.shortcuts.find(search.clone());
    }
    let path: Option<String> = {
      if self.matched_shortcuts.len() == 1 {
        Some(self.matched_shortcuts[0].get_prefixed_path())
      } else {
        self.matched_shortcuts
          .iter()
          .find(|s| s.seq.iter().any(|seq| *seq == search))
          .map(|s| s.get_prefixed_path())
      }
    };
    if let Some(p) = path {
      let shortcut_res = open::that_detached(p);
      match shortcut_res {
        Ok(_) => {
          self.running = false;
        }
        Err(_) => {}
      }
    }
  }
}

fn main() {
  let mut app = App::new();
  let mut term = ratatui::init();

  let mut search_input = TextArea::default();
  search_input.set_block(
    Block::bordered()
      .border_type(BorderType::Rounded)
      .border_style(Style::new().dark_gray())
      .padding(Padding::horizontal(1))
  );

  loop {
    if !app.running {
      break;
    }
    term.draw(|frame| {
      let layout = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]);
      let [search_area, main_area] = layout.areas(frame.area());

      let (matched_apps, matched_dirs, matched_files, matched_urls) = {
        let mut apps: Vec<Shortcut> = vec![];
        let mut urls: Vec<Shortcut> = vec![];
        let mut dirs: Vec<Shortcut> = vec![];
        let mut files: Vec<Shortcut> = vec![];
        for s in &app.matched_shortcuts {
          match s.kind {
            ShortcutKind::App => apps.push(s.clone()),
            ShortcutKind::File => files.push(s.clone()),
            ShortcutKind::Dir => dirs.push(s.clone()),
            ShortcutKind::Url => urls.push(s.clone()),
          }
        }
        (apps, dirs, files, urls)
      };

      let mut table_rows: Vec<Row> = Vec::new();
      for shortcuts in [matched_apps, matched_dirs, matched_files, matched_urls] {
        if shortcuts.len() > 0 {
          for s in shortcuts {
            let seq = s.seq[0].clone();
            match s.kind {
              ShortcutKind::App => {
                let desc = &s.description.unwrap_or("".to_string());
                let cells = vec![
                  Cell::new(
                    Line::from(vec![Span::from(">__ ").red(), Span::from(seq).bold().light_red()])
                  ),
                  Cell::new(desc.clone())
                ];
                table_rows.push(Row::new(cells));
              }
              ShortcutKind::Dir => {
                let path = s.path.clone();
                let prefix = s.path_prefix.map(|p| p.to_string());
                let cells = vec![
                  Cell::new(
                    Line::from(
                      vec![Span::from("[_] ").green(), Span::from(seq).bold().light_green()]
                    )
                  ),
                  Cell::new(
                    Line::from({
                      let mut spans = vec![];
                      if let Some(p) = prefix {
                        spans.push(Span::from(p).underlined());
                        spans.push(Span::from("/").underlined());
                      }
                      spans.push(Span::from(path));
                      spans
                    })
                  )
                ];
                table_rows.push(Row::new(cells));
              }
              ShortcutKind::File => {
                let path = s.path.clone();
                let prefix = s.path_prefix.map(|p| p.to_string());
                let cells = vec![
                  Cell::new(
                    Line::from(
                      vec![Span::from("[_] ").yellow(), Span::from(seq).bold().light_yellow()]
                    )
                  ),
                  Cell::new(
                    Line::from({
                      let mut spans = vec![];
                      if let Some(p) = prefix {
                        spans.push(Span::from(p).underlined());
                        spans.push(Span::from("/").underlined());
                      }
                      spans.push(Span::from(path));
                      spans
                    })
                  )
                ];
                table_rows.push(Row::new(cells));
              }
              ShortcutKind::Url => {
                let desc = s.description.unwrap_or_default();
                let cells = vec![
                  Cell::new(
                    Line::from(vec![Span::from("(#) ").blue(), Span::from(seq).bold().light_blue()])
                  ),
                  Cell::new(desc)
                ];
                table_rows.push(Row::new(cells));
              }
            }
          }
        }
      }
      let mut shortcuts_table = Table::new(
        table_rows,
        vec![Constraint::Length(8), Constraint::Fill(1)]
      ).column_spacing(1);

      frame.render_widget(&search_input, search_area);
      match &app.config {
        Ok(_) => {
          frame.render_widget(&shortcuts_table, main_area);
        }
        Err(e) => {
          let error_p = Paragraph::new(match e {
            LoadConfigError::IoError(e) => e.to_string(),
            LoadConfigError::ParseError(e) => e.to_string(),
            LoadConfigError::NoConfig =>
              "Config does not exist in \"documents/bullet/config.json\"".to_string(),
          });
          frame.render_widget(&error_p, main_area);
        }
      }
    });
    if poll(Duration::from_millis(100)).unwrap() {
      if let event::Event::Key(key_event) = event::read().unwrap() {
        match key_event.code {
          KeyCode::Esc => {
            app.running = false;
          }
          _ => {
            search_input.input(key_event);
            let search = search_input.lines()[0].clone();
            app.find_and_handle_matches(search);
          }
        }
      }
    }
  }

  ratatui::restore();
}
