//! The UI-facing contract for the prompter.
//!
//! The gcr/D-Bus machinery in this crate knows nothing about how a prompt is
//! actually shown — it hands a [`PromptRequest`] to a [`Prompter`] and gets a
//! [`PromptResponse`] back. The binary supplies the implementation (in our
//! case, by spawning the `hush` dialog), so this crate stays free of any UI
//! or pinentry dependency.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use zeroize::Zeroizing;

/// A cooperative cancellation flag handed to a [`Prompter`]. The daemon trips
/// it when the keyring client calls `StopPrompting` (or disconnects), so a
/// long-running prompt can tear itself down instead of grabbing the keyboard
/// forever.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub(crate) fn trigger(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether the prompt should abort. Implementations should poll this.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What kind of answer the keyring is asking the user for.
pub enum PromptKind {
    /// A password. `confirm` requests a second "confirm password" field
    /// (gcr's `password-new`).
    Password { confirm: bool },
    /// A yes/no confirmation.
    Confirm,
}

/// A request to show something to the user, derived from the gcr prompt
/// properties. Plain, UI-agnostic data.
pub struct PromptRequest {
    pub kind: PromptKind,
    pub title: Option<String>,
    pub description: Option<String>,
    pub warning: Option<String>,
    pub continue_label: Option<String>,
    pub cancel_label: Option<String>,
    /// When set, the prompt shows a checkbox with this label (e.g.
    /// "Automatically unlock this keyring whenever I'm logged in").
    pub choice_label: Option<String>,
    /// The checkbox's initial state.
    pub choice: bool,
}

/// The user's answer. `choice` carries the checkbox state back; it is only
/// meaningful (and only reported to the keyring) when the request set a
/// `choice_label`.
pub enum PromptResponse {
    /// A password was entered (buffer wiped on drop).
    Password {
        secret: Zeroizing<String>,
        choice: bool,
    },
    /// The user confirmed (the "yes" path).
    Confirmed { choice: bool },
    /// The user declined, cancelled, or the prompt was torn down.
    Dismissed,
}

/// Shows prompts to the user. Implementations must be safe to call from a
/// blocking worker thread.
pub trait Prompter: Send + Sync + 'static {
    fn prompt(&self, request: PromptRequest, cancel: &Cancel) -> PromptResponse;
}
