#![feature(ascii_char)]
#![feature(ascii_char_variants)]

mod session;

use std::time::Duration;
use std::{ascii, io};

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::style::Print;
use crossterm::ExecutableCommand;
use probe_rs::rtt::Rtt;

fn main() -> Result<()> {
    let mut session = session::get_session()?;

    let memory_map = session.target().memory_map.clone();
    // Select a core.
    let mut core = session.core(0)?;

    // Attach to RTT
    let mut rtt = Rtt::attach(&mut core, &memory_map)?;
    let down_channel = rtt.down_channels().take(0).unwrap();
    let up_channel = rtt.up_channels().take(0).unwrap();

    let mut stdout = io::stdout();

    loop {
        // terminal -> RTT
        if let Some(char) = read_char()? {
            down_channel.write(&mut core, &[char.as_u8()])?;
        }
        // RTT -> terminal
        let mut buf = [0];
        let count = up_channel.read(&mut core, &mut buf)?;
        if count > 0 {
            if let Some(char) = buf[0].as_ascii() {
                stdout
                    .execute(Print(char))
                    .context("ExecutableCommand::execute")?;
            }
        }
    }
}

fn read_char() -> Result<Option<ascii::Char>> {
    if !crossterm::event::poll(Duration::from_millis(1)).context("crossterm::event::poll")? {
        return Ok(None);
    }
    Ok(
        match crossterm::event::read().context("crossterm::event::read()")? {
            Event::Key(KeyEvent {
                           code,
                           kind: KeyEventKind::Press,
                           ..
                       }) => match code {
                KeyCode::Char(c) => c.as_ascii(),
                KeyCode::Enter => Some(ascii::Char::LineFeed),
                KeyCode::Tab => Some(ascii::Char::CharacterTabulation),
                _ => None,
            },
            _ => None,
        },
    )
}
