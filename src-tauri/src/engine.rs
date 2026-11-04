use anyhow::{anyhow, bail, Context};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

// Each request owns its process. Cleanup also runs on timeout, protocol errors,
// or a discarded session, so an engine cannot outlive its search indefinitely.
struct Engine {
    child: Child,
    lines: Receiver<String>,
    reader: Option<thread::JoinHandle<()>>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Engine {
    fn send(&mut self, command: &str) -> anyhow::Result<()> {
        let input = self
            .child
            .stdin
            .as_mut()
            .context("Stockfish stdin is unavailable")?;
        writeln!(input, "{command}")?;
        input.flush()?;
        Ok(())
    }

    fn wait_for(&self, prefix: &str) -> anyhow::Result<String> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let line = self.lines.recv_timeout(remaining).with_context(|| {
                format!("Stockfish stopped responding while waiting for {prefix}")
            })?;
            if line.split_whitespace().next() == Some(prefix) {
                return Ok(line);
            }
        }
    }
}

pub fn best_move(executable: &str, moves: &[String], skill: Option<u8>) -> anyhow::Result<String> {
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let child = command.spawn().with_context(|| {
        format!("Cannot launch Stockfish at '{executable}'. Set its executable path and retry")
    })?;
    let (sender, lines) = mpsc::channel();
    let mut engine = Engine {
        child,
        lines,
        reader: None,
    };
    let output = engine
        .child
        .stdout
        .take()
        .context("Stockfish stdout is unavailable")?;
    engine.reader = Some(thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    }));
    engine.send("uci")?;
    engine.wait_for("uciok")?;
    engine.send("setoption name Threads value 1")?;
    engine.send("setoption name Hash value 32")?;
    if let Some(skill) = skill {
        engine.send(&format!("setoption name Skill Level value {skill}"))?;
    }
    engine.send("ucinewgame")?;
    engine.send("isready")?;
    engine.wait_for("readyok")?;
    // Send history, not only FEN: the engine needs it to recognise repetitions.
    engine.send(&format!("position startpos moves {}", moves.join(" ")))?;
    engine.send("go movetime 700")?;
    let response = engine.wait_for("bestmove")?;
    let mv = response
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("Stockfish returned no move"))?;
    if matches!(mv, "(none)" | "0000") {
        bail!("Stockfish reported no legal move");
    }
    Ok(mv.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{uci::UciMove, Chess, Position};

    /// Exercise a real local Stockfish handshake and search from a non-starting
    /// position, including reduced skill for the fallback. Any legal engine
    /// choice is acceptable; its exact move is not fixed.
    #[test]
    #[ignore = "requires STOCKFISH_EXECUTABLE pointing to a local Stockfish binary"]
    fn stockfish_returns_legal_reply() {
        let executable = std::env::var("STOCKFISH_EXECUTABLE").expect("Set STOCKFISH_EXECUTABLE");
        let history: Vec<String> = vec!["e2e4".into(), "c7c5".into(), "g1f3".into()];
        let mut position = Chess::default();
        for mv in &history {
            let mv = mv
                .parse::<UciMove>()
                .expect("UCI history")
                .to_move(&position)
                .expect("Legal history");
            position.play_unchecked(mv);
        }
        for skill in [None, Some(0), Some(10)] {
            let reply = best_move(&executable, &history, skill).expect("Stockfish search");
            reply
                .parse::<UciMove>()
                .expect("UCI reply")
                .to_move(&position)
                .expect("Legal engine reply");
        }
    }
}
