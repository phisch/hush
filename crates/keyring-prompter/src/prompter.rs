use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;
use zbus::message::Header;
use zbus::names::BusName;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::Connection;

use crate::prompt::{Cancel, PromptKind, PromptRequest, PromptResponse, Prompter as Ui};
use crate::secret_exchange::SecretExchange;

const CALLBACK_INTERFACE: &str = "org.gnome.keyring.internal.Prompter.Callback";

type SessionKey = (String, String);

pub(crate) struct Shared {
    sessions: Mutex<HashMap<SessionKey, SecretExchange>>,
    active: Mutex<HashMap<SessionKey, Cancel>>,
}

impl Shared {
    fn new() -> Self {
        Shared {
            sessions: Mutex::new(HashMap::new()),
            active: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn cleanup_caller(&self, name: &str) {
        let keys: Vec<SessionKey> = {
            let mut sessions = self.sessions.lock().unwrap();
            let keys: Vec<SessionKey> = sessions
                .keys()
                .filter(|(sender, _)| sender == name)
                .cloned()
                .collect();
            for key in &keys {
                sessions.remove(key);
            }
            keys
        };
        if keys.is_empty() {
            return;
        }
        let mut active = self.active.lock().unwrap();
        for key in &keys {
            if let Some(cancel) = active.remove(key) {
                cancel.trigger();
            }
        }
    }
}

pub struct Service {
    ui: Arc<dyn Ui>,
    shared: Arc<Shared>,
    dialog_slot: Arc<Semaphore>,
}

impl Service {
    pub fn new(ui: Arc<dyn Ui>) -> Self {
        Service {
            ui,
            shared: Arc::new(Shared::new()),
            dialog_slot: Arc::new(Semaphore::new(1)),
        }
    }

    pub(crate) fn shared(&self) -> Arc<Shared> {
        self.shared.clone()
    }
}

#[zbus::interface(name = "org.gnome.keyring.internal.Prompter")]
impl Service {
    #[zbus(name = "BeginPrompting")]
    async fn begin_prompting(
        &self,
        callback: OwnedObjectPath,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<()> {
        let Some(key) = session_key(&header, &callback) else {
            return Ok(());
        };
        if !authorized(connection, &key.0).await {
            eprintln!(
                "hush-keyring: refusing prompt from unauthorized caller {}",
                key.0
            );
            return Ok(());
        }

        let message = {
            let mut sessions = self.shared.sessions.lock().unwrap();
            if sessions.contains_key(&key) {
                return Err(zbus::fdo::Error::Failed(
                    "Already begun prompting for this prompt callback".into(),
                ));
            }
            let exchange = SecretExchange::generate();
            let message = exchange.public_message();
            sessions.insert(key.clone(), exchange);
            message
        };

        let connection = connection.clone();
        tokio::spawn(async move {
            prompt_ready(&connection, &key, "", &message, HashMap::new()).await;
        });
        Ok(())
    }

    #[zbus(name = "PerformPrompt")]
    async fn perform_prompt(
        &self,
        callback: OwnedObjectPath,
        r#type: String,
        properties: HashMap<String, OwnedValue>,
        exchange: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<()> {
        let Some(key) = session_key(&header, &callback) else {
            return Ok(());
        };
        if !authorized(connection, &key.0).await {
            return Ok(());
        }

        let kind = match r#type.as_str() {
            "password" => PromptKind::Password {
                confirm: read_bool(&properties, "password-new"),
            },
            "confirm" => PromptKind::Confirm,
            _ => {
                return Err(zbus::fdo::Error::InvalidArgs(
                    "Invalid type argument".into(),
                ));
            }
        };

        let Some(session) = self.shared.sessions.lock().unwrap().get(&key).cloned() else {
            return Err(zbus::fdo::Error::Failed(
                "Not begun prompting for this prompt callback".into(),
            ));
        };
        let Some(transport_key) = session.transport_key(&exchange) else {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Invalid secret exchange received".into(),
            ));
        };

        let request = build_request(&properties, kind);
        let has_choice = request.choice_label.is_some();

        let cancel = Cancel::default();
        {
            let mut active = self.shared.active.lock().unwrap();
            if active.contains_key(&key) {
                return Err(zbus::fdo::Error::Failed(
                    "Already performing a prompt for this prompt callback".into(),
                ));
            }
            active.insert(key.clone(), cancel.clone());
        }

        let ui = self.ui.clone();
        let shared = self.shared.clone();
        let slot = self.dialog_slot.clone();
        let connection = connection.clone();
        let prompt_cancel = cancel.clone();
        tokio::spawn(async move {
            let Ok(_permit) = slot.acquire().await else {
                shared.active.lock().unwrap().remove(&key);
                return;
            };

            if cancel.is_cancelled() {
                shared.active.lock().unwrap().remove(&key);
                return;
            }

            let response = tokio::task::spawn_blocking(move || ui.prompt(request, &prompt_cancel))
                .await
                .unwrap_or(PromptResponse::Dismissed);
            shared.active.lock().unwrap().remove(&key);

            if cancel.is_cancelled() {
                return;
            }

            let (reply, message, properties) = match response {
                PromptResponse::Password { secret, choice } => {
                    // gnome-keyring shows a "store unencrypted?" confirmation
                    // unless the prompter reports a non-zero password strength.
                    let strength = if secret.is_empty() { 0 } else { 1 };
                    (
                        "yes",
                        session.encrypted_message(&transport_key, secret.as_bytes()),
                        reply_properties(Some(strength), has_choice.then_some(choice)),
                    )
                }
                PromptResponse::Confirmed { choice } => (
                    "yes",
                    session.public_message(),
                    reply_properties(None, has_choice.then_some(choice)),
                ),
                PromptResponse::Dismissed => ("no", session.public_message(), HashMap::new()),
            };

            prompt_ready(&connection, &key, reply, &message, properties).await;
        });

        Ok(())
    }

    #[zbus(name = "StopPrompting")]
    async fn stop_prompting(
        &self,
        callback: OwnedObjectPath,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) {
        let Some(key) = session_key(&header, &callback) else {
            return;
        };
        self.shared.sessions.lock().unwrap().remove(&key);

        let Some(cancel) = self.shared.active.lock().unwrap().remove(&key) else {
            return;
        };
        cancel.trigger();

        let connection = connection.clone();
        tokio::spawn(async move {
            prompt_done(&connection, &key).await;
        });
    }
}

pub(crate) async fn watch_callers(connection: Connection, shared: Arc<Shared>) {
    use futures_util::StreamExt;

    let Ok(dbus) = zbus::fdo::DBusProxy::new(&connection).await else {
        return;
    };
    let Ok(mut changes) = dbus.receive_name_owner_changed().await else {
        return;
    };
    while let Some(signal) = changes.next().await {
        let Ok(args) = signal.args() else {
            continue;
        };
        if args.new_owner().is_none() {
            shared.cleanup_caller(&args.name().to_string());
        }
    }
}

async fn authorized(connection: &Connection, sender: &str) -> bool {
    if std::env::var_os("HUSH_ALLOW_ANY_CALLER").is_some() {
        return true;
    }
    let Ok(bus_name) = BusName::try_from(sender) else {
        return false;
    };
    let Ok(proxy) = zbus::fdo::DBusProxy::new(connection).await else {
        return false;
    };
    let Ok(pid) = proxy.get_connection_unix_process_id(bus_name).await else {
        return false;
    };
    match std::fs::read_link(format!("/proc/{pid}/exe")) {
        Ok(path) => path
            .file_name()
            .is_some_and(|name| name == "gnome-keyring-daemon"),
        Err(_) => false,
    }
}

fn session_key(header: &Header<'_>, callback: &OwnedObjectPath) -> Option<SessionKey> {
    let sender = header.sender()?.to_string();
    Some((sender, callback.as_str().to_string()))
}

fn read_string(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    properties
        .get(key)
        .and_then(|value| String::try_from(value.clone()).ok())
        .filter(|value| !value.is_empty())
}

fn read_bool(properties: &HashMap<String, OwnedValue>, key: &str) -> bool {
    properties
        .get(key)
        .and_then(|value| bool::try_from(value.clone()).ok())
        .unwrap_or(false)
}

fn build_request(properties: &HashMap<String, OwnedValue>, kind: PromptKind) -> PromptRequest {
    PromptRequest {
        kind,
        title: read_string(properties, "message").or_else(|| read_string(properties, "title")),
        description: read_string(properties, "description"),
        warning: read_string(properties, "warning"),
        continue_label: read_string(properties, "continue-label"),
        cancel_label: read_string(properties, "cancel-label"),
        choice_label: read_string(properties, "choice-label"),
        choice: read_bool(properties, "choice-chosen"),
    }
}

fn nested(value: Value<'static>) -> OwnedValue {
    OwnedValue::try_from(Value::Value(Box::new(value))).expect("nested variant is valid")
}

fn reply_properties(strength: Option<i32>, choice: Option<bool>) -> HashMap<String, OwnedValue> {
    let mut properties = HashMap::new();
    if let Some(strength) = strength {
        properties.insert(
            "password-strength".to_string(),
            nested(Value::I32(strength)),
        );
    }
    if let Some(choice) = choice {
        properties.insert("choice-chosen".to_string(), nested(Value::Bool(choice)));
    }
    properties
}

async fn prompt_ready(
    connection: &Connection,
    key: &SessionKey,
    reply: &str,
    exchange: &str,
    properties: HashMap<String, OwnedValue>,
) {
    let (sender, path) = key;
    if let Err(error) = connection
        .call_method(
            Some(sender.as_str()),
            path.as_str(),
            Some(CALLBACK_INTERFACE),
            "PromptReady",
            &(reply, properties, exchange),
        )
        .await
    {
        eprintln!("hush-keyring: PromptReady call failed: {error}");
    }
}

async fn prompt_done(connection: &Connection, key: &SessionKey) {
    let (sender, path) = key;
    if let Err(error) = connection
        .call_method(
            Some(sender.as_str()),
            path.as_str(),
            Some(CALLBACK_INTERFACE),
            "PromptDone",
            &(),
        )
        .await
    {
        eprintln!("hush-keyring: PromptDone call failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_properties_are_nested_variants() {
        let properties = reply_properties(Some(1), Some(true));

        let strength: Value = properties.get("password-strength").unwrap().clone().into();
        assert_eq!(strength, Value::Value(Box::new(Value::I32(1))));

        let choice: Value = properties.get("choice-chosen").unwrap().clone().into();
        assert_eq!(choice, Value::Value(Box::new(Value::Bool(true))));
    }

    #[test]
    fn reply_properties_omits_absent_fields() {
        let properties = reply_properties(None, None);
        assert!(properties.is_empty());
    }
}
