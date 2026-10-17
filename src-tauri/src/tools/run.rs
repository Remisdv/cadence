//! Exécution d'une installation ou d'une connexion : commande exacte, sortie lue au fil de l'eau
//! (lignes et retours chariot des barres de progression), annulable.
//!
//! La commande tourne dans un pseudo-terminal (ConPTY sous Windows) : une invite (« Continuer ?
//! [Y/n] », code de vérification…) reçoit la réponse tapée dans la fenêtre d'installation
//! (`ToolService::send_input`). Si le pseudo-terminal ne s'ouvre pas, repli sur des tubes
//! (sans saisie possible). La touche Entrée de `gh auth login` est envoyée d'office ; les
//! connexions à menus plein écran s'ouvrent toujours dans un terminal du système.

use std::{
    io::{Read, Write},
    path::Path,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

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

    /// Ligne en cours, pas encore terminée (invite « Continuer ? [Y/n] », « Nom : »…).
    pub fn partial(&self) -> Option<String> {
        let text = strip_ansi(String::from_utf8_lossy(&self.buf).trim_end());
        (!text.trim().is_empty()).then_some(text)
    }
}

impl Default for LineSplitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Retire les séquences d'échappement ANSI : CSI (couleurs, curseur, `ESC [ ? 25 l`…), OSC
/// (titre de fenêtre, jusqu'à BEL ou `ESC \\`), et les échappements à deux caractères.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.peek() == Some(&'\\')) {
                            if c == '\u{1b}' {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
            continue;
        }
        if c == '\u{7}' {
            continue;
        }
        out.push(c);
    }
    out
}

/// Entrée de la commande en cours, quand elle tourne dans un pseudo-terminal.
pub type RunInput = Arc<Mutex<Option<Box<dyn Write + Send>>>>;

/// Écrit une réponse (suivie d'Entrée) sur l'entrée de la commande ; `false` sans terminal.
pub fn write_input(input: &RunInput, text: &str) -> bool {
    let mut slot = input.lock().unwrap();
    let Some(writer) = slot.as_mut() else { return false };
    writer.write_all(format!("{text}\r").as_bytes()).and_then(|_| writer.flush()).is_ok()
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

/// Lance `program args` dans un pseudo-terminal, relaie chaque ligne, attend la fin (ou
/// `cancel`, ou le délai). Sans pseudo-terminal, repli sur des tubes.
pub async fn run_streaming(
    program: &Path,
    args: &[String],
    press_enter: bool,
    cancel: Arc<Notify>,
    input: RunInput,
    on_line: Arc<dyn Fn(String, bool) + Send + Sync>,
) -> ToolRunResult {
    match run_in_pty(program, args, press_enter, cancel.clone(), input, on_line.clone()).await {
        Ok(result) => result,
        Err(err) => {
            tracing::info!(%err, "pseudo-terminal indisponible, sortie lue par tubes");
            run_with_pipes(program, args, press_enter, cancel, on_line).await
        }
    }
}

/// Résultat d'une fin de commande (code, interruption, délai).
fn result_of(code: Option<Option<i32>>, started: Instant) -> ToolRunResult {
    let duration_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    match code {
        Some(Some(0)) => ToolRunResult { ok: true, exit_code: Some(0), message: None, duration_ms },
        Some(Some(code)) => ToolRunResult {
            ok: false,
            exit_code: Some(code),
            message: Some(format!("la commande s'est terminée avec le code {code}")),
            duration_ms,
        },
        Some(None) => ToolRunResult {
            ok: false,
            exit_code: None,
            message: Some("la commande a été interrompue".into()),
            duration_ms,
        },
        None => ToolRunResult {
            ok: false,
            exit_code: None,
            message: Some(if started.elapsed() >= RUN_TIMEOUT { "délai dépassé".into() } else { "annulé".into() }),
            duration_ms,
        },
    }
}

async fn run_in_pty(
    program: &Path,
    args: &[String],
    press_enter: bool,
    cancel: Arc<Notify>,
    input: RunInput,
    on_line: Arc<dyn Fn(String, bool) + Send + Sync>,
) -> Result<ToolRunResult, String> {
    let started = Instant::now();
    let pty = native_pty_system()
        .openpty(PtySize { rows: 40, cols: 200, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())?;
    let mut cmd = CommandBuilder::new(program);
    cmd.args(args);
    if let Ok(dir) = std::env::current_dir() {
        cmd.cwd(dir);
    }
    let mut child = pty.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pty.slave);
    let mut reader = pty.master.try_clone_reader().map_err(|e| e.to_string())?;
    let mut writer = pty.master.take_writer().map_err(|e| e.to_string())?;
    if press_enter {
        // `gh auth login` : « Press Enter to open github.com in your browser… »
        let _ = writer.write_all(b"\r");
        let _ = writer.flush();
    }
    *input.lock().unwrap() = Some(writer);

    // Lecture bloquante du terminal, sur un fil à part.
    let answer = input.clone();
    let pump = tokio::task::spawn_blocking(move || {
        let mut splitter = LineSplitter::new();
        let mut chunk = [0u8; 4096];
        let mut lines = Vec::new();
        // Ligne inachevée déjà montrée (une invite attend sa réponse sans retour à la ligne) :
        // la ligne suivante la remplace.
        let mut partial_shown = false;
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    // ConPTY demande la position du curseur (`ESC [ 6 n`) et attend la réponse
                    // avant de laisser la commande écrire.
                    if chunk[..n].windows(4).any(|w| w == b"[6n")
                        && let Some(writer) = answer.lock().unwrap().as_mut()
                    {
                        let _ = writer.write_all(b"[1;1R");
                        let _ = writer.flush();
                    }
                    splitter.push(&chunk[..n], &mut lines);
                    for (line, replace) in lines.drain(..) {
                        on_line(line, replace || std::mem::take(&mut partial_shown));
                    }
                    if let Some(partial) = splitter.partial() {
                        on_line(partial, partial_shown || splitter.replacing);
                        partial_shown = true;
                    }
                }
            }
        }
        splitter.finish(&mut lines);
        for (line, replace) in lines {
            on_line(strip_ansi(&line), replace);
        }
    });

    let mut killer = child.clone_killer();
    let waiter = tokio::task::spawn_blocking(move || child.wait());
    let status = tokio::select! {
        status = waiter => Some(status),
        _ = cancel.notified() => None,
        _ = tokio::time::sleep(RUN_TIMEOUT) => None,
    };
    if status.is_none() {
        let _ = killer.kill();
    }
    // Fin de la commande : plus de saisie, le terminal se ferme et la lecture s'arrête.
    input.lock().unwrap().take();
    drop(pty.master);
    let _ = tokio::time::timeout(Duration::from_secs(2), pump).await;
    let code = status.map(|joined| match joined {
        Ok(Ok(status)) => Some(status.exit_code() as i32),
        _ => None,
    });
    Ok(result_of(code, started))
}

async fn run_with_pipes(
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
    fn strips_terminal_sequences() {
        assert_eq!(strip_ansi("\u{1b}[?25l\u{1b}[2J\u{1b}[HFound Git\u{1b}[K"), "Found Git");
        assert_eq!(strip_ansi("\u{1b}]0;C:\\Windows\\cmd.exe\u{7}Prêt"), "Prêt");
        assert_eq!(strip_ansi("\u{1b}]0;titre\u{1b}\\ok\u{7}"), "ok");
    }

    #[tokio::test]
    async fn answers_a_prompt_through_the_terminal() {
        let input: RunInput = Arc::default();
        let lines = Arc::new(Mutex::new(Vec::new()));
        let sink = lines.clone();
        let on_line: Arc<dyn Fn(String, bool) + Send + Sync> = Arc::new(move |line, _| sink.lock().unwrap().push(line));
        let (program, args): (&str, Vec<String>) = if cfg!(windows) {
            ("powershell", vec!["-NoProfile".into(), "-Command".into(), "$r = Read-Host 'Nom'; Write-Output \"bonjour $r\"".into()])
        } else {
            ("sh", vec!["-c".into(), "printf 'Nom: '; read r; echo \"bonjour $r\"".into()])
        };
        let program = which::which(program).unwrap();
        let run = {
            let input = input.clone();
            tokio::spawn(async move { run_streaming(&program, &args, false, Arc::new(Notify::new()), input, on_line).await })
        };
        // Attend l'invite, puis répond.
        let start = Instant::now();
        while !lines.lock().unwrap().iter().any(|l| l.contains("Nom")) {
            assert!(start.elapsed() < Duration::from_secs(20), "invite jamais affichée : {:?}", lines.lock().unwrap());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(write_input(&input, "Ada"));
        let result = tokio::time::timeout(Duration::from_secs(20), run).await.unwrap().unwrap();
        assert!(result.ok, "{result:?} {:?}", lines.lock().unwrap());
        assert!(lines.lock().unwrap().iter().any(|l| l.contains("bonjour Ada")), "{:?}", lines.lock().unwrap());
        assert!(!write_input(&input, "trop tard"), "plus de saisie après la fin");
    }

    #[test]
    fn strips_colors() {
        assert_eq!(strip_ansi("\u{1b}[32m✓\u{1b}[0m Logged in"), "✓ Logged in");
    }
}
