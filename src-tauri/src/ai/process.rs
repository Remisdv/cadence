//! Outils communs aux processus des CLI d'IA : lancement en arbre (une CLI installée par npm
//! est un script `.cmd` qui lance `node`, qui lance le vrai binaire : l'arrêter doit tout
//! arrêter), dernières lignes d'erreur, identifiant aléatoire.

use std::{
    collections::VecDeque,
    path::Path,
    process::ExitStatus,
    sync::{Arc, Mutex},
};

use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::{ChildStderr, ChildStdin, ChildStdout, Command},
};

/// Commande d'une CLI d'IA (sans console sous Windows, ajoutée par `TreeChild::spawn`).
pub fn command(program: &Path) -> Command {
    Command::new(program)
}

/// Processus et descendants : job object (Windows) ou groupe de processus (Unix), tués
/// ensemble, et à la destruction.
pub struct TreeChild(Box<dyn ChildWrapper>);

impl TreeChild {
    pub fn spawn(command: Command) -> std::io::Result<Self> {
        let mut wrapped = CommandWrap::from(command);
        wrapped.wrap(KillOnDrop);
        #[cfg(windows)]
        {
            wrapped.wrap(process_wrap::tokio::CreationFlags(windows::Win32::System::Threading::CREATE_NO_WINDOW));
            wrapped.wrap(process_wrap::tokio::JobObject);
        }
        #[cfg(unix)]
        wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
        wrapped.spawn().map(TreeChild)
    }

    pub fn stdin(&mut self) -> Option<ChildStdin> {
        self.0.stdin().take()
    }

    pub fn stdout(&mut self) -> Option<ChildStdout> {
        self.0.stdout().take()
    }

    pub fn stderr(&mut self) -> Option<ChildStderr> {
        self.0.stderr().take()
    }

    /// Tue tout l'arbre.
    pub async fn kill(&mut self) {
        if let Err(err) = Box::into_pin(self.0.kill()).await {
            tracing::debug!(%err, "arrêt forcé d'une CLI d'IA");
        }
    }

    pub async fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.0.wait().await
    }
}

/// Nombre de lignes d'erreur gardées.
const TAIL_LINES: usize = 8;

/// Dernières lignes d'un flux d'erreur, lues en tâche de fond.
#[derive(Clone, Default)]
pub struct Tail(Arc<Mutex<VecDeque<String>>>);

impl Tail {
    /// Lit `stream` jusqu'à sa fin ; chaque ligne part aussi dans le journal de débogage.
    pub fn follow(stream: impl AsyncRead + Unpin + Send + 'static, cli: &'static str) -> Self {
        let tail = Tail::default();
        let lines = tail.0.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stream).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                tracing::debug!(cli, line, "sortie d'erreur");
                tail_push(&lines, line);
            }
        });
        tail
    }

    pub fn push(&self, line: String) {
        tail_push(&self.0, line);
    }

    /// « <cli> s'est arrêté pendant le tour : <dernières lignes> ».
    pub fn crash_message(&self, cli: &str) -> String {
        let tail = self.0.lock().unwrap();
        if tail.is_empty() {
            format!("{cli} s'est arrêté pendant le tour.")
        } else {
            format!("{cli} s'est arrêté pendant le tour : {}", tail.iter().cloned().collect::<Vec<_>>().join(" ⏎ "))
        }
    }
}

fn tail_push(lines: &Mutex<VecDeque<String>>, line: String) {
    let mut lines = lines.lock().unwrap();
    if lines.len() == TAIL_LINES {
        lines.pop_front();
    }
    lines.push_back(line);
}

/// 16 octets aléatoires en hexadécimal (mot de passe d'un serveur local).
pub fn random_secret() -> String {
    let mut bytes = [0u8; 16];
    let _ = getrandom::fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
