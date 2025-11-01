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
// アプリケーションの状態を管理する構造体
struct App {
    current_path: String,
    files: Vec<PathBuf>,
    list_state: ListState,
    
}

impl App {
    fn update_files(&mut self) -> Result<()> {
        self.files = fs::read_dir(&self.current_path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();

        self.files.sort_by(|a,b| {
            b.is_dir().cmp(&a.is_dir()).then_with(|| a.cmp(b))
        });
        if !self.files.is_empty() {
            self.list_state.select(None);
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

    fn play_music(&self,path: &PathBuf) -> Result<()> {
        let (_stream,stream_handle) = OutputStream::try_default()?;

        let sink = Sink::try_new(&stream_handle)?;
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader)?;
        sink.append(source);
        sink.detach();
        Ok(())
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

        
    let mut app = App {
        current_path: music_dir.to_string_lossy().into_owned(),
        files: Vec::new(),
        list_state: ListState::default(),
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
        terminal.draw(|frame| {
            let size = frame.size();
            let block = Block::default()
                .title(app.current_path.as_str())
                .borders(Borders::ALL); // メソッドチェーンの途中にセミコロンは不要
            
            let inner_area = block.inner(size);
            frame.render_widget(block, size);

            let items: Vec<ListItem> = app.files
                .iter()
                .map(|path| {
                    let file_name = path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    if path.is_dir() {
                        let item = ListItem::new(format!("📁 {}",file_name))
                            .style(Style::default().fg(Color::Cyan));
                        item
                    } else if path.extension().map_or(false, |ext| ext == "mp3" || ext == "flac"){
                        let item = ListItem::new(format!("🎵 {}", file_name));
                        item
                    } else {
                        let item = ListItem::new(format!("📄 {}",file_name));
                        item
                    }
                })
                .collect();

            let list = List::new(items)
                .block(Block::default())
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol("> ");

            frame.render_stateful_widget(list, inner_area, &mut app.list_state);
        })?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => {
                        if app.files.is_empty() { continue; }
                        let i = match app.list_state.selected() { 
                            Some(i) => if i >= app.files.len() - 1 { 0 } else { i + 1 },
                            None => 0,
                        };
                        app.list_state.select(Some(i));
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if app.files.is_empty() { continue; }
                        let i = match app.list_state.selected() { // .selected() は1回だけ呼び出す
                            Some(i) => if i == 0 { app.files.len() - 1 } else { i - 1 },
                            None => 0,
                        };
                        app.list_state.select(Some(i));
                    }

                    KeyCode::Char('l') | KeyCode::Enter => {
                        app.enter_directory();
                    }
                    KeyCode::Char('h') => {
                        app.leave_directory();
                    }
                    _ => {}
                }
            }
        }
    } 
}
