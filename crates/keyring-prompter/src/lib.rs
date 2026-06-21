mod prompt;
mod prompter;
mod secret_exchange;

use std::sync::Arc;

use zbus::connection::Builder;
use zbus::fdo::RequestNameFlags;

pub use prompt::{Cancel, PromptKind, PromptRequest, PromptResponse, Prompter};

const BUS_NAME: &str = "org.gnome.keyring.SystemPrompter";
const OBJECT_PATH: &str = "/org/gnome/keyring/Prompter";

pub fn run<P: Prompter>(ui: P) -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Runtime::new()?.block_on(serve(Arc::new(ui)))
}

async fn serve(ui: Arc<dyn Prompter>) -> Result<(), Box<dyn std::error::Error>> {
    let service = prompter::Service::new(ui, tokio::runtime::Handle::current());
    let shared = service.shared();

    let connection = Builder::session()?
        .serve_at(OBJECT_PATH, service)?
        .build()
        .await?;

    tokio::spawn(prompter::watch_callers(connection.clone(), shared));

    connection
        .request_name_with_flags(
            BUS_NAME,
            RequestNameFlags::AllowReplacement | RequestNameFlags::ReplaceExisting,
        )
        .await?;

    std::future::pending::<()>().await;
    Ok(())
}
