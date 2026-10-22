use feishin_remote::{Host, PREFERRED_PORT};
use tokio::net::TcpListener;

use super::RemoteService;

impl RemoteService {
    pub(super) async fn set_hosting(&mut self, is_hosting: bool) {
        match (is_hosting, self.hosting.is_some()) {
            (true, false) => self.start_hosting().await,
            (false, true) => self.stop_hosting(),
            _ => {}
        }
    }

    async fn start_hosting(&mut self) {
        let Some(discovery) = &self.discovery else {
            self.status(tr!(
                "Remote control is unavailable: other players cannot find this one"
            ));
            return;
        };
        let Some((listener, port)) = self.open_listener().await else {
            return;
        };
        if let Err(error) = discovery.advertise(&self.deps.me, port) {
            tracing::warn!("announcing this player failed: {error}");
        }
        let host = Host::new(
            self.deps.me.clone(),
            self.deps.delegate.clone(),
            self.deps.trust.clone(),
            self.deps.host_state.clone(),
        );
        self.hosting = Some(tokio::spawn(host.serve(listener)));
        tracing::info!("other players can control this one on port {port}");
    }

    fn stop_hosting(&mut self) {
        if let Some(task) = self.hosting.take() {
            task.abort();
        }
        if let Some(discovery) = &self.discovery {
            discovery.withdraw();
        }
    }

    async fn open_listener(&self) -> Option<(TcpListener, u16)> {
        let opened = bind_preferring(PREFERRED_PORT).await.and_then(|listener| {
            let port = listener.local_addr()?.port();
            Ok((listener, port))
        });
        match opened {
            Ok(opened) => Some(opened),
            Err(error) => {
                tracing::warn!("listening for remote control failed: {error}");
                self.status(tr!(
                    "Remote control is unavailable: the network port could not be opened"
                ));
                None
            }
        }
    }
}

async fn bind_preferring(port: u16) -> std::io::Result<TcpListener> {
    match TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => Ok(listener),
        Err(error) => {
            tracing::warn!(
                "port {port} is unavailable for remote control ({error}), using another"
            );
            TcpListener::bind(("0.0.0.0", 0)).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn given_the_preferred_port_is_taken_another_one_is_used() {
        let taken = TcpListener::bind(("0.0.0.0", 0)).await.unwrap();
        let port = taken.local_addr().unwrap().port();

        let listener = bind_preferring(port).await.unwrap();

        assert_ne!(listener.local_addr().unwrap().port(), port);
    }

    #[tokio::test]
    async fn given_the_preferred_port_is_free_it_is_used() {
        let port = TcpListener::bind(("0.0.0.0", 0))
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let listener = bind_preferring(port).await.unwrap();

        assert_eq!(listener.local_addr().unwrap().port(), port);
    }
}
