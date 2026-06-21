use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::Notify;
use ui::{run_conversation, Reply, Update};
use zbus_polkit_agent::{
    agent_session::{Message, PolkitAgentSession, Response},
    polkit_agent_instance,
    server::Error,
    Identity, UnixUser,
};
use zeroize::Zeroizing;

const DIALOG_FLAG: &str = "--dialog";
const OBJECT_PATH: &str = "/org/psst/PolicyKit1/AuthenticationAgent";

#[derive(Clone)]
struct Agent {
    cancels: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
    handle: tokio::runtime::Handle,
    warm: Arc<Mutex<Option<Child>>>,
}

fn main() {
    hardening::forbid_dumps();
    if std::env::args().nth(1).as_deref() == Some(DIALOG_FLAG) {
        run_conversation();
        return;
    }
    if let Err(error) = serve() {
        eprintln!("psst-polkit-agent: {error:?}");
        std::process::exit(1);
    }
}

#[tokio::main]
async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let agent = Agent {
        cancels: Arc::new(Mutex::new(HashMap::new())),
        handle: tokio::runtime::Handle::current(),
        warm: Arc::new(Mutex::new(spawn_dialog().ok())),
    };
    let _connection =
        polkit_agent_instance(move || agent.clone(), authenticate, cancel_authentication)
            .connect(OBJECT_PATH)
            .await?;
    std::future::pending::<()>().await;
    Ok(())
}

async fn authenticate(
    agent: &mut Agent,
    _action_id: &str,
    message: &str,
    _icon_name: &str,
    _details: HashMap<&str, &str>,
    cookie: &str,
    mut identities: Vec<Identity<'_>>,
) -> Result<(), Error> {
    let user: UnixUser = identities
        .drain(..)
        .next()
        .ok_or(Error::Failed)?
        .try_into()?;
    let session = PolkitAgentSession::new(user, cookie)?;

    let cancel = Arc::new(Notify::new());
    agent
        .cancels
        .lock()
        .unwrap()
        .insert(cookie.to_owned(), cancel.clone());

    let authenticated = agent
        .handle
        .spawn(converse(
            agent.warm.clone(),
            session,
            message.to_owned(),
            cancel,
        ))
        .await
        .unwrap_or(None);

    agent.cancels.lock().unwrap().remove(cookie);

    match authenticated {
        Some(true) => Ok(()),
        Some(false) => Err(Error::Failed),
        None => Err(Error::Cancelled),
    }
}

async fn cancel_authentication(agent: &mut Agent, cookie: &str) -> Result<(), Error> {
    if let Some(cancel) = agent.cancels.lock().unwrap().remove(cookie) {
        cancel.notify_one();
    }
    Ok(())
}

async fn converse(
    warm: Arc<Mutex<Option<Child>>>,
    session: PolkitAgentSession,
    message: String,
    cancel: Arc<Notify>,
) -> Option<bool> {
    let mut child = take_warm(&warm)?;
    let mut stdin = child.stdin.take()?;
    let mut lines = BufReader::new(child.stdout.take()?).lines();

    let start = Update::Start {
        title: "Authentication required".to_owned(),
        message,
    };
    let outcome = if send(&mut stdin, &start).await.is_ok() {
        drive(session, &mut stdin, &mut lines, &cancel).await
    } else {
        None
    };

    kill(&mut child).await;
    *warm.lock().unwrap() = spawn_dialog().ok();
    outcome
}

fn take_warm(warm: &Mutex<Option<Child>>) -> Option<Child> {
    let taken = warm.lock().unwrap().take();
    if let Some(mut child) = taken {
        if matches!(child.try_wait(), Ok(None)) {
            return Some(child);
        }
        let _ = child.start_kill();
    }
    spawn_dialog().ok()
}

async fn drive(
    mut session: PolkitAgentSession,
    stdin: &mut ChildStdin,
    lines: &mut Lines,
    cancel: &Notify,
) -> Option<bool> {
    loop {
        let message = tokio::select! {
            biased;
            _ = cancel.notified() => return None,
            _ = lines.next_line() => return None,
            next = dispatch(session) => {
                let (returned, message) = next?;
                session = returned;
                message
            }
        };

        match message {
            Message::Info(text) => send(stdin, &Update::Info { text }).await.ok()?,
            Message::Error(text) => send(stdin, &Update::Error { text }).await.ok()?,
            Message::Complete(success) => {
                let _ = send(stdin, &Update::Done { success }).await;
                return Some(success);
            }
            Message::Request { echo_on, prompt } => {
                send(
                    stdin,
                    &Update::Prompt {
                        echo_on,
                        label: prompt,
                    },
                )
                .await
                .ok()?;
                let secret = tokio::select! {
                    biased;
                    _ = cancel.notified() => return None,
                    line = lines.next_line() => response(line)?,
                };
                session = respond(session, secret).await?;
            }
        }
    }
}

type Lines = tokio::io::Lines<BufReader<tokio::process::ChildStdout>>;

async fn dispatch(mut session: PolkitAgentSession) -> Option<(PolkitAgentSession, Message)> {
    let (session, message) = tokio::task::spawn_blocking(move || {
        let message = session.dispatch();
        (session, message)
    })
    .await
    .ok()?;
    Some((session, message.ok()?))
}

async fn respond(
    mut session: PolkitAgentSession,
    secret: Zeroizing<String>,
) -> Option<PolkitAgentSession> {
    let (session, result) = tokio::task::spawn_blocking(move || {
        let result = session.response(Response { password: &secret });
        (session, result)
    })
    .await
    .ok()?;
    result.ok().map(|()| session)
}

fn response(line: std::io::Result<Option<String>>) -> Option<Zeroizing<String>> {
    let line = line.ok().flatten()?;
    match serde_json::from_str::<Reply>(line.trim()).ok()? {
        Reply::Response { secret } => Some(Zeroizing::new(secret)),
        Reply::Cancel => None,
    }
}

fn spawn_dialog() -> std::io::Result<Child> {
    tokio::process::Command::new(std::env::current_exe()?)
        .arg(DIALOG_FLAG)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
}

async fn send(stdin: &mut ChildStdin, message: &Update) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(message).unwrap_or_default();
    line.push(b'\n');
    stdin.write_all(&line).await?;
    stdin.flush().await
}

async fn kill(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}
