//! Exécution d'une installation ou d'une connexion : commande exacte, sortie lue au fil de l'eau
//! (lignes et retours chariot des barres de progression), annulable.
//!
//! v1 sans pseudo-terminal : les commandes retenues n'attendent pas de saisie (drapeaux
//! `--accept-*`, `pkexec` pour le mot de passe), sauf la touche Entrée de `gh auth login`,
//! envoyée d'office. Les connexions qui demandent un choix au clavier s'ouvrent dans un terminal
//! du système.

use std::{
    path::Path,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::Notify,
};

use super::ToolRunResult;
use crate::error::{AppError, AppResult};

/// Installations longues (winget, téléchargements), connexions dans le navigateur.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(20 * 60);
/// Taille maximale d'une ligne gardée (au-delà, coupée).
const MAX_LINE: usize = 2000;

pub fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Découpe un flux en lignes ; un `\r` seul remplace la ligne en cours (barres de progression),
/// signalé par `replace = true`.
pub struct LineSplitter {
    buf: Vec<u8>,
    /// `\r` lu : fin de ligne Windows si `\n` suit, sinon retour en début de ligne.
    pending_cr: bool,
    /// La ligne en cours remplace la précédente.
    replacing: bool,
}

impl LineSplitter {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            pending_cr: false,
            replacing: false,
        }
    }

    /// Lignes complètes `(texte, remplace la précédente)`.
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<(String, bool)>) {
        for &b in bytes {
            match b {
                b'\r' => self.pending_cr = true,
                b'\n' => {
                    self.pending_cr = false;
                    self.flush(out);
                    self.replacing = false;
                }
                _ => {
                    if std::mem::take(&mut self.pending_cr) {
                        self.flush(out);
                        self.replacing = true;
                    }
                    if self.buf.len() < MAX_LINE {
                        self.buf.push(b);
                    }
                }
            }
        }
    }

    fn flush(&mut self, out: &mut Vec<(String, bool)>) {
        let text = String::from_utf8_lossy(&self.buf).trim_end().to_string();
        self.buf.clear();
        if !text.trim().is_empty() {
            out.push((strip_ansi(&text), self.replacing));
        }
    }

    pub fn finish(&mut self, out: &mut Vec<(String, bool)>) {
        self.flush(out);
    }
}

impl Default for LineSplitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Retire les séquences d'échappement ANSI (couleurs, déplacements du curseur).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

async fn pump(
    mut reader: impl AsyncRead + Unpin,
    on_line: Arc<dyn Fn(String, bool) + Send + Sync>,
) {
    let mut splitter = LineSplitter::new();
    let mut chunk = [0u8; 4096];
    let mut lines = Vec::new();
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                splitter.push(&chunk[..n], &mut lines);
                for (line, replace) in lines.drain(..) {
                    on_line(line, replace);
                }
            }
        }
    }
    splitter.finish(&mut lines);
    for (line, replace) in lines {
        on_line(line, replace);
    }
}

/// Lance `program args`, relaie chaque ligne, attend la fin (ou `cancel`, ou le délai).
pub async fn run_streaming(
    program: &Path,
    args: &[String],
    press_enter: bool,
    cancel: Arc<Notify>,
    on_line: Arc<dyn Fn(String, bool) + Send + Sync>,
) -> ToolRunResult {
    let started = Instant::now();
    let elapsed = |started: Instant| started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    let mut cmd = command(program);
    cmd.args(args);
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            return ToolRunResult {
                ok: false,
                exit_code: None,
                message: Some(format!("lancement impossible : {err}")),
                duration_ms: elapsed(started),
            };
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        if press_enter {
            // `gh auth login` : « Press Enter to open github.com in your browser… »
            let _ = stdin.write_all(b"\n").await;
        }
        drop(stdin);
    }
    let out = child
        .stdout
        .take()
        .map(|r| tokio::spawn(pump(r, on_line.clone())));
    let err = child
        .stderr
        .take()
        .map(|r| tokio::spawn(pump(r, on_line.clone())));

    let status = tokio::select! {
        status = child.wait() => Some(status),
        _ = cancel.notified() => None,
        _ = tokio::time::sleep(RUN_TIMEOUT) => None,
    };
    let cancelled = status.is_none();
    if cancelled {
        let _ = child.kill().await;
    }
    for task in [out, err].into_iter().flatten() {
        let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
    }
    match status {
        Some(Ok(status)) => ToolRunResult {
            ok: status.success(),
            exit_code: status.code(),
            message: (!status.success()).then(|| match status.code() {
                Some(code) => format!("la commande s'est terminée avec le code {code}"),
                None => "la commande a été interrompue".into(),
            }),
            duration_ms: elapsed(started),
        },
        Some(Err(err)) => ToolRunResult {
            ok: false,
            exit_code: None,
            message: Some(err.to_string()),
            duration_ms: elapsed(started),
        },
        None => ToolRunResult {
            ok: false,
            exit_code: None,
            message: Some(if started.elapsed() >= RUN_TIMEOUT {
                "délai dépassé".into()
            } else {
                "annulé".into()
            }),
            duration_ms: elapsed(started),
        },
    }
}

/// Ouvre la commande dans un terminal du système (saisie au clavier possible).
pub fn open_in_terminal(program: &Path, args: &[&str]) -> AppResult<()> {
    let line = std::iter::once(program.display().to_string())
        .chain(args.iter().map(|a| a.to_string()))
        .map(|a| {
            if a.contains(' ') {
                format!("\"{a}\"")
            } else {
                a
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let spawned = if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", "start", "Cadence", "cmd", "/K", &line])
            .spawn()
    } else if cfg!(target_os = "macos") {
        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            line.replace('"', "\\\"")
        );
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn()
    } else {
        std::process::Command::new("x-terminal-emulator")
            .args(["-e", "sh", "-c", &line])
            .spawn()
    };
    spawned
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("terminal introuvable : {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(chunks: &[&[u8]]) -> Vec<(String, bool)> {
        let mut s = LineSplitter::new();
        let mut out = Vec::new();
        for c in chunks {
            s.push(c, &mut out);
        }
        s.finish(&mut out);
        out
    }

    #[test]
    fn splits_lines_and_progress() {
        let out = split(&[
            b"Trouv\xc3\xa9 GitHub CLI\r\n  ##   10%\r  ####",
            b"  40%\r\nInstall\xc3\xa9\n",
        ]);
        assert_eq!(
            out,
            vec![
                ("Trouvé GitHub CLI".to_string(), false),
                ("  ##   10%".to_string(), false),
                ("  ####  40%".to_string(), true),
                ("Installé".to_string(), false),
            ]
        );
    }

    #[test]
    fn strips_colors() {
        assert_eq!(strip_ansi("\u{1b}[32m✓\u{1b}[0m Logged in"), "✓ Logged in");
    }
}
