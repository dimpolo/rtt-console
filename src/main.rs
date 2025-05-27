mod session;

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use ascii::ToAsciiChar;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::style::Print;
use crossterm::ExecutableCommand;
use probe_rs::rtt::Rtt;

fn main() -> Result<()> {
    let mut session = session::get_session()?;

    // Select a core.
    let mut core = session.core(0)?;

    // Attach to RTT
    let mut rtt = Rtt::attach(&mut core)?;
    let mut stdout = io::stdout();

    loop {
        // terminal -> RTT
        if let Some(char) = read_char()? {
            rtt.down_channels()
                .first_mut()
                .unwrap()
                .write(&mut core, &[char.as_byte()])?;
        }
        // RTT -> terminal
        let mut buf = [0];
        let count = rtt
            .up_channels()
            .first_mut()
            .unwrap()
            .read(&mut core, &mut buf)?;
        if count > 0 {
            if let Some(char) = buf[0].to_ascii_char().ok() {
                stdout
                    .execute(Print(char))
                    .context("ExecutableCommand::execute")?;
            }
        }
    }
}

fn read_char() -> Result<Option<ascii::AsciiChar>> {
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
                KeyCode::Char(c) => c.to_ascii_char().ok(),
                KeyCode::Enter => Some(ascii::AsciiChar::LineFeed),
                KeyCode::Tab => Some(ascii::AsciiChar::Tab),
                _ => None,
            },
            _ => None,
        },
    )
}
