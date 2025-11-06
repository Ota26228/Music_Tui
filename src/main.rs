// src/main.rs

use std::{
    io::{self, stdout,BufReader},
    path::PathBuf,
    fs
};
use std::fs::File;

use anyhow::Result;
use crossterm::{
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
    event::{self, Event, KeyCode}, // KeyCode を use
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState}, // 必要なものを整理
    style::{Style, Modifier}, // Modifier を use
};
use rodio::{Decoder, OutputStream, Sink};
use rand::seq::SliceRandom;
use rand::thread_rng;

#[derive(PartialEq)]
enum AppState {
    Normal,
    Playing,
    Paused,
}
// アプリケーションの状態を管理する構造体
struct App {
    current_path: String,
    files: Vec<PathBuf>,
    list_state: ListState,
    _stream: OutputStream,
    sink: Sink,
    currently_playing: Option<PathBuf>,
    state: AppState,
    is_shuffling: bool,
}

impl App {
    fn update_files(&mut self) -> Result<()> {
        self.files = fs::read_dir(&self.current_path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        
        if self.is_shuffling {
            self.shuffle_files();
        } else {
            self.sort_files();
        }
        if !self.files.is_empty() {
            self.list_state.select(Some(0));
        }
        Ok(())
    }

    fn enter_directory(&mut self) {
        if let Some(selected_index) = self.list_state.selected() {
            let selected_path = &self.files[selected_index].clone();
            if selected_path.is_dir() {
                self.current_path = selected_path.to_string_lossy().into_owned();
                self.update_files().expect("error");
            }else {
                if let Err(e) = self.play_music(selected_path){
                    eprintln!("Error playing music: {:?},path: {}", e, selected_path.
                        display());
                }
            }
        }
    }

    fn leave_directory(&mut self){
        if let Some(parent) = PathBuf::from(&self.current_path).parent() {
            self.current_path = parent.to_string_lossy().into_owned();
            self.update_files().unwrap_or_default();
        }
    }

    fn play_music(&mut self, path: &PathBuf) -> Result<()> {
        self.sink.stop();

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader)?;
        self.sink.append(source);
        
        self.currently_playing = Some(path.clone());
        self.state = AppState::Playing;

        Ok(())
    }

    fn pause_playback(&mut self) {
        if self.state == AppState::Playing {
            self.sink.pause();
            self.state = AppState::Paused;
        }
    }

    fn resume_playback(&mut self) {
        if self.state == AppState::Paused {
            self.sink.play();
            self.state = AppState::Playing;
        }
    }

    fn stop_playback(&mut self) {
        self.sink.stop();
        self.currently_playing = None;
        self.state = AppState::Normal;
    }

    fn select_next(&mut self) {
        if self.files.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.files.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn select_previous(&mut self) {
        if self.files.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { self.files.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn play_next_song(&mut self) {
        if self.files.is_empty() {
            self.stop_playback();
            return;
        }

        let current_index = self.currently_playing.as_ref()
            .and_then(|p| self.files.iter().position(|f| f == p));
        let start_index = current_index.map_or(0, |i| i + 1);
        let next_song = self.files.iter().cycle().skip
            (start_index).take(self.files.len())
            .find(|path| path.is_file() &&
                path.extension().map_or(false, |ext| ext == "mp3" || ext == "flac")
            );

        if let Some(song_path) = next_song {
            let _ = self.play_music(&song_path.clone());
        } else {
            self.stop_playback();
        }
    }

    fn shuffle_files(&mut self) {
        let mut rng = thread_rng();
        let (mut dirs, mut files): (Vec<_>,Vec<_>) = self
                .files.iter().cloned().partition(|p| p.is_dir());
        files.shuffle(&mut rng);
        dirs.sort();
        dirs.append(&mut files);
        self.files = dirs;
    }

    fn toggle_shuffle(&mut self) {
        self.is_shuffling = !self.is_shuffling;
        if self.is_shuffling {
            self.shuffle_files();
        } else {
            self.sort_files();
        }
        self.list_state.select(Some(0));
    }

    fn sort_files(&mut self) {
        self.files.sort_by(|a,b| {
            b.is_dir().cmp(&a.is_dir()).then_with(|| a.cmp(b))
        });
    }

        
}

fn main() -> Result<()> {
    // --- 1. ターミナルのセットアップ ---
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    let music_dir = dirs::audio_dir()
        .unwrap_or_else(|| {
            dirs::home_dir().expect("Coule not find home directory").join("Music")
        });
    fs::create_dir_all(&music_dir)?;

    let (_stream,stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let mut app = App {
        current_path: music_dir.to_string_lossy().into_owned(),
        files: Vec::new(),
        list_state: ListState::default(),
        _stream,
        sink,
        currently_playing: None,
        state:AppState::Normal,
        is_shuffling: false,
    };
    app.update_files()?;

       

    // --- 3. アプリケーションの実行 ---
    let res = run_app(&mut terminal, app); // appの所有権を渡す

    // --- 4. ターミナルのクリーンアップ ---
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    res
}
// アプリケーションのメインループ
fn run_app(terminal: &mut Terminal<impl Backend>, mut app: App) -> Result<()> {
    loop {
        if app.state == AppState::Playing && app.sink.empty() {
            app.play_next_song();
        }
        terminal.draw(|frame| {
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(0),
                    ratatui::layout::Constraint::Length(1),
                ])
                .split(frame.size());
            let main_area = chunks[0];
            let footer_area = chunks[1];

            let block = Block::default()
                .title(app.current_path.as_str())
                .borders(Borders::ALL); // メソッドチェーンの途中にセミコロンは不要
            
            let inner_area = block.inner(main_area);
            frame.render_widget(block, main_area);

            let items: Vec<ListItem> = app.files
                .iter()
                .map(|path| {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();

                    // 1. ファイル種別に応じて、アイコンと基本スタイルを決める
                    let (icon, base_style) = if path.is_dir() {
                        ("📁", Style::default().fg(Color::Cyan))
                    } else if path.extension().map_or(false, |ext| ext == "mp3" || ext == "flac") {
                        ("🎵", Style::default())
                    } else {
                        ("📄", Style::default())
                    };
                    
                    let text = format!("{} {}", icon, file_name);
                    let mut item = ListItem::new(text).style(base_style);

                    // 2. もし再生中の曲なら、スタイルを上書きする
                    if app.currently_playing.as_ref() == Some(path) {
                        item = item.style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
                    }
                    
                    item
                })
                .collect();

            let list = List::new(items)
                .block(Block::default())
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol("> ");

            frame.render_stateful_widget(list, inner_area, &mut app.list_state);
            let mode_str = match app.state {
                AppState::Normal => "NORMAL",
                AppState::Playing => "PLAYING",
                AppState::Paused => "PAUSED",
            };
            let shuffle_str = if app.is_shuffling { "SHUFFLE" } else { "" };

            let footer_line = ratatui::text::Line::from(vec![
                ratatui::text::Span::raw("-- "),
                ratatui::text::Span::styled(mode_str, Style::default().add_modifier(Modifier::BOLD)),
                ratatui::text::Span::raw(" --"),
                ratatui::text::Span::raw(" | "),
                ratatui::text::Span::styled(shuffle_str, Style::default().fg(Color::Yellow)),
                ratatui::text::Span::raw(" "),
            ]);

            let footer_widget = ratatui::widgets::Paragraph::new(footer_line)
                .alignment(ratatui::layout::Alignment::Right);
            
            frame.render_widget(footer_widget, footer_area);
        })?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // 最初に、状態に依存しないグローバルなキーを処理
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('d') => app.toggle_shuffle(),
                    KeyCode::Esc => {
                        app.stop_playback();
                        continue; // 他のキー処理はスキップ
                    }
                    KeyCode::Char('j') | KeyCode::Down => app.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => app.select_previous(),
                    KeyCode::Char('h') => app.leave_directory(),
                    // 上記以外の場合は、状態依存のキー処理に移る
                    _ => {
                        match app.state {
                            AppState::Normal => match key.code {
                                KeyCode::Char('l') | KeyCode::Enter => app.enter_directory(),
                                _ => {}
                            },
                            AppState::Playing => match key.code {
                                KeyCode::Char('s') => app.pause_playback(),
                                KeyCode::Char('l') | KeyCode::Enter => app.enter_directory(),
                                _ => {}
                            },
                            AppState::Paused => match key.code {
                                KeyCode::Char('s') => app.resume_playback(),
                                KeyCode::Char('l') | KeyCode::Enter => app.enter_directory(),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    } 
}
