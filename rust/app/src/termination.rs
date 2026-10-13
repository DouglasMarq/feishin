use tokio::runtime::Handle;

const FORCED_EXIT_CODE: i32 = 130;

pub fn quit_on_termination_signals(runtime: &Handle) {
    runtime.spawn(async {
        wait_for_termination_signal().await;
        tracing::info!("termination requested, saving state before quitting");
        request_quit();
        wait_for_termination_signal().await;
        tracing::warn!("second termination request, quitting without saving");
        std::process::exit(FORCED_EXIT_CODE);
    });
}

fn request_quit() {
    let scheduled = slint::invoke_from_event_loop(|| {
        if let Err(error) = slint::quit_event_loop() {
            tracing::warn!("quitting the event loop failed: {error}");
        }
    });
    if let Err(error) = scheduled {
        tracing::warn!("the event loop is gone, cannot quit it: {error}");
    }
}

async fn wait_for_termination_signal() {
    tokio::select! {
        () = interrupted() => {}
        () = terminated() => {}
    }
}

async fn interrupted() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!("listening for Ctrl+C failed: {error}");
        std::future::pending::<()>().await;
    }
}

#[cfg(unix)]
async fn terminated() {
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            terminate.recv().await;
        }
        Err(error) => {
            tracing::warn!("listening for SIGTERM failed: {error}");
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(not(unix))]
async fn terminated() {
    std::future::pending::<()>().await;
}
