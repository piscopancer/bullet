#![allow(unused_must_use, unused_variables, dead_code, warnings)]

use std::{ cell::RefCell, fs::File, io::Read, time::Duration };
use crossterm::event::{ self, poll, KeyCode, KeyEvent };
use ratatui::{
  layout::{ Constraint, Layout, Rows },
  style::{ Style, Styled, Stylize },
  text::Line,
  widgets::{ Block, BorderType, Cell, Padding, Paragraph, Row, Table },
};
use serde::{ Deserialize };
use tui_textarea::TextArea;

#[derive(Deserialize, Debug, Clone)]
struct Shortcut {
  path: String,
  seq: Vec<String>,
  description: String,
}

#[derive(Deserialize, Debug)]
struct Shortcuts {
  urls: Option<Vec<Shortcut>>,
  dirs: Option<Vec<Shortcut>>,
  apps: Option<Vec<Shortcut>>,
}

struct MatchedShortcuts {
  urls: Option<Vec<Shortcut>>,
  dirs: Option<Vec<Shortcut>>,
  apps: Option<Vec<Shortcut>>,
}

trait ShortcutsTrait {
  fn find(&self, search: String) -> Vec<Shortcut>;
}

impl ShortcutsTrait for Vec<Shortcut> {
  fn find(&self, search: String) -> Vec<Shortcut> {
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
}

struct App {
  config: Result<Shortcuts, LoadConfigError>,
  matches: MatchedShortcuts,
  running: bool,
}

impl App {
  fn new() -> Self {
    App {
      running: true,
      config: App::load_config(),
      matches: MatchedShortcuts {
        urls: None,
        dirs: None,
        apps: None,
      },
    }
  }
  fn load_config() -> Result<Shortcuts, LoadConfigError> {
    let f = File::open("./config.json");
    if f.is_err() {
      return Err(LoadConfigError::IoError(f.err().unwrap()));
    }
    let mut content = String::new();
    f.map(|mut f| f.read_to_string(&mut content));
    let cfg = serde_json::from_str::<Shortcuts>(&content);
    cfg.map_err(|e| LoadConfigError::ParseError(e))
  }
  fn search_matches(&mut self, search: String) {
    if let Ok(config) = &self.config {
      self.matches.apps = config.apps.as_ref().map(|a| a.find(search.clone()));
      self.matches.dirs = config.dirs.as_ref().map(|d| d.find(search.clone()));
      self.matches.urls = config.urls.as_ref().map(|u| u.find(search.clone()));
    }
    if search == "mkrs" {
      // open::that_detached("https://mkrs-beta.vercel.app");
      open::that_detached("C:/Users/Igor/Pictures/Warframe/Warframe0000.jpg");
      self.running = false;
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

      let table_pairs: [(&str, &Option<Vec<Shortcut>>); 3] = [
        ("Apps", &app.matches.apps),
        ("Dirs", &app.matches.dirs),
        ("Urls", &app.matches.urls),
      ];

      let mut table_rows: Vec<Row> = Vec::new();
      for pair in table_pairs {
        let (title, shortcuts) = pair;
        if let Some(shortcuts) = shortcuts.as_ref() {
          table_rows.push(Row::new(vec![Cell::new(Line::from(title).centered().dark_gray())]));
          for s in shortcuts {
            let keys = &s.seq[0];
            let desc = &s.description;
            let cells = vec![Cell::new(keys.clone()), Cell::new(desc.clone())];
            table_rows.push(Row::new(cells));
          }
        }
      }
      let mut shortcuts_table = Table::new(
        table_rows,
        vec![Constraint::Length(5), Constraint::Fill(1)]
      );

      frame.render_widget(&search_input, search_area);
      match &app.config {
        Ok(_) => {
          frame.render_widget(&shortcuts_table, main_area);
        }
        Err(e) => {
          let error_p = Paragraph::new(match e {
            LoadConfigError::IoError(e) => e.to_string(),
            LoadConfigError::ParseError(e) => e.to_string(),
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
            app.search_matches(search);
          }
        }
      }
    }
  }

  ratatui::restore();
}
