use console_api::instrument::StateRequest;
use console_api::instrument::{
    instrument_client::InstrumentClient, InstrumentRequest, PauseRequest, ResumeRequest,
    State as InstrumentState, TaskDetailsRequest, Update,
};
use console_api::tasks::TaskDetails;
use futures::stream::StreamExt;
use futures::TryFutureExt;
use hyper_util::rt::TokioIo;
use std::{error::Error, time::Duration};
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(feature = "vsock")]
use tokio_vsock::VsockStream;
use tonic::{
    transport::{Channel, Endpoint, Uri},
    Streaming,
};

#[derive(Debug)]
pub struct Connection {
    target: Uri,
    state: State,
    /// Why the last connection attempt failed, or why the connection was lost.
    ///
    /// This is cleared as soon as a connection is established.
    last_error: Option<String>,
}

// clippy doesn't like that the "connected" case is much larger than the
// disconnected case, and suggests boxing the connected side's stream.
// however, this is rarely disconnected; it's normally connected. boxing the
// stream just adds a heap pointer dereference, slightly penalizing polling
// the stream in most cases. so, don't listen to clippy on this.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum State {
    Connected {
        client: InstrumentClient<Channel>,
        update_stream: Box<Streaming<Update>>,
        state_stream: Box<Streaming<InstrumentState>>,
    },
    Disconnected(Duration),
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum Message {
    Update(Update),
    State(InstrumentState),
    /// The connection was lost or a connection attempt failed.
    ///
    /// This carries no data. It lets the caller redraw, so that the
    /// connection status and error shown in the header stay current.
    Disconnected,
    /// A connection to the server was just established.
    Connected,
}

macro_rules! with_client {
    ($me:ident, $client:ident, $block:expr) => ({
        loop {
            match $me.state {
                State::Connected { client: ref mut $client, .. } => {
                    match $block {
                        Ok(resp) => break Ok(resp),
                        // If the error is a `h2::Error`, that indicates
                        // something went wrong at the connection level, rather
                        // than the server returning an error code. In that
                        // case, let's try reconnecting...
                        Err(error) if error.source().iter().any(|src| src.is::<h2::Error>()) => {
                            tracing::warn!(
                                error = %error,
                                "connection error sending command"
                            );
                            $me.state = State::Disconnected(Self::BACKOFF);
                        }
                        // Otherwise, return the error.
                        Err(e) => {
                            break Err(e);
                        }
                    }
                }
                State::Disconnected(_) => $me.connect().await,
            }
        }
    })
}

impl Connection {
    const BACKOFF: Duration = Duration::from_millis(500);
    pub fn new(target: Uri) -> Self {
        Self {
            target,
            state: State::Disconnected(Duration::from_secs(0)),
            last_error: None,
        }
    }

    /// Returns a description of the reason the console is not connected, if
    /// there is one.
    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Keeps trying to connect until a connection is established.
    async fn connect(&mut self) {
        while let State::Disconnected(_) = self.state {
            self.connect_once().await;
        }
    }

    /// Makes a single connection attempt, after waiting out the backoff.
    ///
    /// Does nothing if the connection is already established. On failure the
    /// reason is kept in `last_error` and the backoff is increased.
    async fn connect_once(&mut self) {
        const MAX_BACKOFF: Duration = Duration::from_secs(5);

        if let State::Disconnected(backoff) = self.state {
            if backoff == Duration::from_secs(0) {
                tracing::debug!(to = %self.target, "connecting");
            } else {
                tracing::debug!(reconnect_in = ?backoff, "reconnecting");
                tokio::time::sleep(backoff).await;
            }
            let try_connect = async {
                let channel = match self.target.scheme_str() {
                    #[cfg(unix)]
                    Some("file") => {
                        if !matches!(self.target.host(), None | Some("localhost")) {
                            return Err("cannot connect to non-localhost unix domain socket".into());
                        }
                        let path = self.target.path().to_owned();
                        // Dummy endpoint is ignored by the connector.
                        let endpoint = Endpoint::from_static("http://localhost");
                        endpoint
                            .connect_with_connector(tower::service_fn(move |_| {
                                UnixStream::connect(path.clone()).map_ok(TokioIo::new)
                            }))
                            .await?
                    }
                    #[cfg(not(unix))]
                    Some("file") => {
                        return Err("unix domain sockets are not supported on this platform".into());
                    }
                    #[cfg(feature = "vsock")]
                    Some("vsock") => {
                        if !matches!(self.target.host(), None | Some("localhost") | Some("any")) {
                            return Err("cannot connect to non-localhost vsock".into());
                        }

                        // Parse URI path in the format vsock://<cid>:<port>
                        let uri_path = self.target.path();
                        let parts: Vec<&str> =
                            uri_path.trim_start_matches('/').split(':').collect();
                        if parts.len() != 2 {
                            return Err(format!(
                                "invalid vsock URI format, expected vsock://<cid>:<port>, got {}",
                                self.target
                            )
                            .into());
                        }

                        let cid = parts[0]
                            .parse::<u32>()
                            .map_err(|_| format!("invalid CID: {}", parts[0]))?;
                        let port = parts[1]
                            .parse::<u32>()
                            .map_err(|_| format!("invalid port: {}", parts[1]))?;

                        let vsock_addr = tokio_vsock::VsockAddr::new(cid, port);

                        // Dummy endpoint is ignored by the connector
                        let endpoint = Endpoint::from_static("http://localhost");
                        endpoint
                            .connect_with_connector(tower::service_fn(move |_| {
                                VsockStream::connect(vsock_addr).map_ok(TokioIo::new)
                            }))
                            .await?
                    }
                    #[cfg(not(feature = "vsock"))]
                    Some("vsock") => {
                        return Err("vsock feature is not enabled".into());
                    }
                    _ => {
                        let endpoint = Endpoint::from(self.target.clone());
                        endpoint.connect().await?
                    }
                };
                let mut client = InstrumentClient::new(channel);
                let update_request = tonic::Request::new(InstrumentRequest {});
                let update_stream =
                    Box::new(client.watch_updates(update_request).await?.into_inner());
                let state_request = tonic::Request::new(StateRequest {});
                let state_stream = match client.watch_state(state_request).await {
                    Ok(stream) => Box::new(stream.into_inner()),
                    Err(e) => {
                        if e.code() == tonic::Code::Unimplemented {
                            let message = format!(
                                "The server at {} does not support state streaming. Please update the console-subscriber to v0.5.0 or later version.",
                                self.target
                            );
                            tracing::error!("{message}");
                            return Err(message.into());
                        }
                        return Err(e.into());
                    }
                };
                Ok::<State, Box<dyn Error + Send + Sync>>(State::Connected {
                    client,
                    update_stream,
                    state_stream,
                })
            };
            self.state = match try_connect.await {
                Ok(connected) => {
                    tracing::debug!("connected successfully!");
                    self.last_error = None;
                    connected
                }
                Err(error) => {
                    tracing::warn!(%error, "error connecting");
                    self.last_error = Some(describe_error(&*error));
                    let backoff = std::cmp::min(backoff + Self::BACKOFF, MAX_BACKOFF);
                    State::Disconnected(backoff)
                }
            };
        }
    }

    /// Marks the connection as lost and remembers why.
    fn disconnect(&mut self, reason: String) {
        self.last_error = Some(reason);
        self.state = State::Disconnected(Self::BACKOFF);
    }

    pub async fn next_message(&mut self) -> Message {
        match &mut self.state {
            State::Connected {
                update_stream,
                state_stream,
                ..
            } => {
                tokio::select! { biased; // Always biased to update stream.
                    update = update_stream.next() => match update {
                        Some(Ok(update)) => Message::Update(update),
                        Some(Err(status)) => {
                            tracing::warn!(%status, "error from update stream");
                            self.disconnect(format!("error from update stream: {}", describe_status(&status)));
                            Message::Disconnected
                        }
                        None => {
                            tracing::error!("update stream closed by server");
                            self.disconnect("update stream closed by server".to_string());
                            Message::Disconnected
                        }
                    },
                    state = state_stream.next() => match state {
                        Some(Ok(state)) => Message::State(state),
                        Some(Err(status)) => {
                            tracing::warn!(%status, "error from state stream");
                            self.disconnect(format!("error from state stream: {}", describe_status(&status)));
                            Message::Disconnected
                        }
                        None => {
                            tracing::error!("state stream closed by server");
                            self.disconnect("state stream closed by server".to_string());
                            Message::Disconnected
                        }
                    },
                }
            }
            State::Disconnected(_) => {
                // Make one attempt at a time, so that the caller gets to
                // redraw the connection status and error between attempts.
                self.connect_once().await;
                match self.state {
                    State::Connected { .. } => Message::Connected,
                    State::Disconnected(_) => Message::Disconnected,
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn watch_details(
        &mut self,
        task_id: u64,
    ) -> Result<Streaming<TaskDetails>, tonic::Status> {
        with_client!(self, client, {
            let request = tonic::Request::new(TaskDetailsRequest {
                id: Some(task_id.into()),
            });
            client.watch_task_details(request).await
        })
        .map(|watch| watch.into_inner())
    }

    #[tracing::instrument(skip(self))]
    pub async fn pause(&mut self) {
        let res = with_client!(self, client, {
            let request = tonic::Request::new(PauseRequest {});
            client.pause(request).await
        });

        if let Err(e) = res {
            tracing::error!(error = %e, "rpc error sending pause command");
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn resume(&mut self) {
        let res = with_client!(self, client, {
            let request = tonic::Request::new(ResumeRequest {});
            client.resume(request).await
        });

        if let Err(e) = res {
            tracing::error!(error = %e, "rpc error sending resume command");
        }
    }

    pub fn render(&self, styles: &crate::view::Styles) -> ratatui::text::Line<'_> {
        use ratatui::{
            style::{Color, Modifier},
            text::{Line, Span},
        };
        let state = match self.state {
            State::Connected { .. } => Span::styled(
                "(CONNECTED)",
                styles.fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            State::Disconnected(d) if d == Duration::from_secs(0) => Span::styled(
                "(CONNECTING)",
                styles.fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            State::Disconnected(d) => Span::styled(
                format!("(RECONNECTING IN {:?})", d),
                styles.fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        };
        Line::from(vec![
            Span::raw("connection: "),
            Span::raw(self.target.to_string()),
            Span::raw(" "),
            state,
        ])
    }

    /// Renders the reason the console is not connected, if there is one.
    pub fn render_error(&self, styles: &crate::view::Styles) -> Option<ratatui::text::Line<'_>> {
        use ratatui::{
            style::{Color, Modifier},
            text::{Line, Span},
        };
        let error = self.last_error()?;
        Some(Line::from(vec![
            Span::styled(
                "error: ",
                styles.fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(error, styles.fg(Color::Red)),
        ]))
    }
}

/// Describes an error together with the chain of errors that caused it.
///
/// Transport errors from tonic are usually a bare "transport error", with the
/// useful part (such as "Connection refused") only in the source chain.
fn describe_error(error: &(dyn Error + 'static)) -> String {
    let mut description = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let cause_text = cause.to_string();
        // Some errors repeat their source in their own message, so skip
        // causes that would only say the same thing again.
        if !description.contains(&cause_text) {
            description.push_str(": ");
            description.push_str(&cause_text);
        }
        source = cause.source();
    }
    description
}

fn describe_status(status: &tonic::Status) -> String {
    let message = status.message();
    if message.is_empty() {
        format!("{:?}", status.code())
    } else {
        format!("{:?}: {}", status.code(), message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_api::instrument::{
        instrument_server::{Instrument, InstrumentServer},
        PauseResponse, ResumeResponse,
    };
    use futures::stream::{self, Pending};
    use std::fmt;
    use tonic::{transport::server::TcpIncoming, Request, Response, Status};

    #[derive(Debug)]
    struct Wrapped(&'static str, Option<Box<Wrapped>>);

    impl fmt::Display for Wrapped {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for Wrapped {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.1.as_deref().map(|e| e as &(dyn Error + 'static))
        }
    }

    #[test]
    fn describe_error_includes_the_source_chain() {
        let error = Wrapped(
            "transport error",
            Some(Box::new(Wrapped(
                "tcp connect error",
                Some(Box::new(Wrapped("Connection refused (os error 111)", None))),
            ))),
        );
        assert_eq!(
            describe_error(&error),
            "transport error: tcp connect error: Connection refused (os error 111)"
        );
    }

    #[test]
    fn describe_error_skips_repeated_sources() {
        let error = Wrapped(
            "connection refused",
            Some(Box::new(Wrapped("connection refused", None))),
        );
        assert_eq!(describe_error(&error), "connection refused");
    }

    #[test]
    fn describe_status_without_a_message() {
        let status = Status::unavailable("");
        assert_eq!(describe_status(&status), "Unavailable");
        let status = Status::unavailable("server is going away");
        assert_eq!(
            describe_status(&status),
            "Unavailable: server is going away"
        );
    }

    #[tokio::test]
    async fn refused_connection_reports_the_reason() {
        // Bind a port and release it again, so that nothing listens on it.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut conn = Connection::new(format!("http://127.0.0.1:{port}").parse().unwrap());
        assert_eq!(conn.last_error(), None);

        let message = conn.next_message().await;

        assert!(matches!(message, Message::Disconnected));
        let error = conn.last_error().expect("a failed attempt records why");
        assert!(error.starts_with("transport error"), "{error}");
        // The first retry must come soon, not after the maximum backoff.
        assert!(matches!(conn.state, State::Disconnected(d) if d == Connection::BACKOFF));
    }

    /// A server from before state streaming existed.
    struct OldServer;

    #[tonic::async_trait]
    impl Instrument for OldServer {
        type WatchUpdatesStream = Pending<Result<Update, Status>>;
        type WatchTaskDetailsStream = Pending<Result<TaskDetails, Status>>;
        type WatchStateStream = Pending<Result<InstrumentState, Status>>;

        async fn watch_updates(
            &self,
            _: Request<InstrumentRequest>,
        ) -> Result<Response<Self::WatchUpdatesStream>, Status> {
            Ok(Response::new(stream::pending()))
        }

        async fn watch_task_details(
            &self,
            _: Request<TaskDetailsRequest>,
        ) -> Result<Response<Self::WatchTaskDetailsStream>, Status> {
            Err(Status::unimplemented("not under test"))
        }

        async fn watch_state(
            &self,
            _: Request<StateRequest>,
        ) -> Result<Response<Self::WatchStateStream>, Status> {
            Err(Status::unimplemented("watch_state"))
        }

        async fn pause(&self, _: Request<PauseRequest>) -> Result<Response<PauseResponse>, Status> {
            Err(Status::unimplemented("not under test"))
        }

        async fn resume(
            &self,
            _: Request<ResumeRequest>,
        ) -> Result<Response<ResumeResponse>, Status> {
            Err(Status::unimplemented("not under test"))
        }
    }

    #[tokio::test]
    async fn server_without_state_streaming_is_reported() {
        let incoming = TcpIncoming::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = incoming.local_addr().unwrap();
        let server = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(InstrumentServer::new(OldServer))
                .serve_with_incoming(incoming),
        );

        let mut conn = Connection::new(format!("http://{addr}").parse().unwrap());
        let message = conn.next_message().await;
        server.abort();

        assert!(matches!(message, Message::Disconnected));
        let error = conn.last_error().expect("the failure is recorded");
        assert!(
            error.contains("does not support state streaming"),
            "{error}"
        );
        assert!(error.contains("v0.5.0"), "{error}");
    }

    #[tokio::test]
    async fn error_is_shown_until_connected() {
        let styles = crate::view::Styles::from_config(Default::default());
        let mut conn = Connection::new("http://127.0.0.1:1".parse().unwrap());
        assert!(conn.render_error(&styles).is_none());

        conn.disconnect("update stream closed by server".to_string());
        let line = conn.render_error(&styles).expect("an error to show");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "error: update stream closed by server");
    }
}
