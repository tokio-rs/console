/// InstrumentRequest requests the stream of updates
/// to observe the async runtime state over time.
///
/// TODO: In the future allow for the request to specify
/// only the data that the caller cares about (i.e. only
/// tasks but no resources)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InstrumentRequest {}
/// TaskDetailsRequest requests the stream of updates about
/// the specific task identified in the request.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskDetailsRequest {
    /// Identifies the task for which details were requested.
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<super::common::Id>,
}
/// PauseRequest requests the stream of updates to pause.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PauseRequest {}
/// ResumeRequest requests the stream of updates to resume after a pause.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResumeRequest {}
/// Update carries all information regarding tasks, resources, async operations
/// and resource operations in one message. There are a couple of reasons to combine all
/// of these into a single message:
///
/// - we can use one single timestamp for all the data
/// - we can have all the new_metadata in one place
/// - things such as async ops and resource ops do not make sense
///    on their own as they have relations to tasks and resources
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Update {
    /// The system time when this update was recorded.
    ///
    /// This is the timestamp any durations in the included `Stats` were
    /// calculated relative to.
    #[prost(message, optional, tag = "1")]
    pub now: ::core::option::Option<::prost_types::Timestamp>,
    /// Task state update.
    #[prost(message, optional, tag = "2")]
    pub task_update: ::core::option::Option<super::tasks::TaskUpdate>,
    /// Resource state update.
    #[prost(message, optional, tag = "3")]
    pub resource_update: ::core::option::Option<super::resources::ResourceUpdate>,
    /// Async operations state update
    #[prost(message, optional, tag = "4")]
    pub async_op_update: ::core::option::Option<super::async_ops::AsyncOpUpdate>,
    /// Any new span metadata that was registered since the last update.
    #[prost(message, optional, tag = "5")]
    pub new_metadata: ::core::option::Option<super::common::RegisterMetadata>,
}
/// `PauseResponse` is the value returned after a pause request.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PauseResponse {}
/// `ResumeResponse` is the value returned after a resume request.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResumeResponse {}
/// Generated client implementations.
pub mod instrument_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// `InstrumentServer<T>` implements `Instrument` as a service.
    #[derive(Debug, Clone)]
    pub struct InstrumentClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl InstrumentClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> InstrumentClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InstrumentClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            InstrumentClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        /// Produces a stream of updates representing the behavior of the instrumented async runtime.
        pub async fn watch_updates(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::Update>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rs.tokio.console.instrument.Instrument/WatchUpdates",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "rs.tokio.console.instrument.Instrument",
                        "WatchUpdates",
                    ),
                );
            self.inner.server_streaming(req, path, codec).await
        }
        /// Produces a stream of updates describing the activity of a specific task.
        pub async fn watch_task_details(
            &mut self,
            request: impl tonic::IntoRequest<super::TaskDetailsRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::super::tasks::TaskDetails>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rs.tokio.console.instrument.Instrument/WatchTaskDetails",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "rs.tokio.console.instrument.Instrument",
                        "WatchTaskDetails",
                    ),
                );
            self.inner.server_streaming(req, path, codec).await
        }
        /// Registers that the console observer wants to pause the stream.
        pub async fn pause(
            &mut self,
            request: impl tonic::IntoRequest<super::PauseRequest>,
        ) -> std::result::Result<tonic::Response<super::PauseResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rs.tokio.console.instrument.Instrument/Pause",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("rs.tokio.console.instrument.Instrument", "Pause"),
                );
            self.inner.unary(req, path, codec).await
        }
        /// Registers that the console observer wants to resume the stream.
        pub async fn resume(
            &mut self,
            request: impl tonic::IntoRequest<super::ResumeRequest>,
        ) -> std::result::Result<tonic::Response<super::ResumeResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/rs.tokio.console.instrument.Instrument/Resume",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("rs.tokio.console.instrument.Instrument", "Resume"),
                );
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod instrument_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with InstrumentServer.
    #[async_trait]
    pub trait Instrument: Send + Sync + 'static {
        /// Server streaming response type for the WatchUpdates method.
        type WatchUpdatesStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::Update, tonic::Status>,
            >
            + Send
            + 'static;
        /// Produces a stream of updates representing the behavior of the instrumented async runtime.
        async fn watch_updates(
            &self,
            request: tonic::Request<super::InstrumentRequest>,
        ) -> std::result::Result<
            tonic::Response<Self::WatchUpdatesStream>,
            tonic::Status,
        >;
        /// Server streaming response type for the WatchTaskDetails method.
        type WatchTaskDetailsStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<
                    super::super::tasks::TaskDetails,
                    tonic::Status,
                >,
            >
            + Send
            + 'static;
        /// Produces a stream of updates describing the activity of a specific task.
        async fn watch_task_details(
            &self,
            request: tonic::Request<super::TaskDetailsRequest>,
        ) -> std::result::Result<
            tonic::Response<Self::WatchTaskDetailsStream>,
            tonic::Status,
        >;
        /// Registers that the console observer wants to pause the stream.
        async fn pause(
            &self,
            request: tonic::Request<super::PauseRequest>,
        ) -> std::result::Result<tonic::Response<super::PauseResponse>, tonic::Status>;
        /// Registers that the console observer wants to resume the stream.
        async fn resume(
            &self,
            request: tonic::Request<super::ResumeRequest>,
        ) -> std::result::Result<tonic::Response<super::ResumeResponse>, tonic::Status>;
    }
    /// `InstrumentServer<T>` implements `Instrument` as a service.
    #[derive(Debug)]
    pub struct InstrumentServer<T: Instrument> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: Instrument> InstrumentServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for InstrumentServer<T>
    where
        T: Instrument,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/rs.tokio.console.instrument.Instrument/WatchUpdates" => {
                    #[allow(non_camel_case_types)]
                    struct WatchUpdatesSvc<T: Instrument>(pub Arc<T>);
                    impl<
                        T: Instrument,
                    > tonic::server::ServerStreamingService<super::InstrumentRequest>
                    for WatchUpdatesSvc<T> {
                        type Response = super::Update;
                        type ResponseStream = T::WatchUpdatesStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::InstrumentRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as Instrument>::watch_updates(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = WatchUpdatesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rs.tokio.console.instrument.Instrument/WatchTaskDetails" => {
                    #[allow(non_camel_case_types)]
                    struct WatchTaskDetailsSvc<T: Instrument>(pub Arc<T>);
                    impl<
                        T: Instrument,
                    > tonic::server::ServerStreamingService<super::TaskDetailsRequest>
                    for WatchTaskDetailsSvc<T> {
                        type Response = super::super::tasks::TaskDetails;
                        type ResponseStream = T::WatchTaskDetailsStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::TaskDetailsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as Instrument>::watch_task_details(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = WatchTaskDetailsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rs.tokio.console.instrument.Instrument/Pause" => {
                    #[allow(non_camel_case_types)]
                    struct PauseSvc<T: Instrument>(pub Arc<T>);
                    impl<T: Instrument> tonic::server::UnaryService<super::PauseRequest>
                    for PauseSvc<T> {
                        type Response = super::PauseResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::PauseRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as Instrument>::pause(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = PauseSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/rs.tokio.console.instrument.Instrument/Resume" => {
                    #[allow(non_camel_case_types)]
                    struct ResumeSvc<T: Instrument>(pub Arc<T>);
                    impl<T: Instrument> tonic::server::UnaryService<super::ResumeRequest>
                    for ResumeSvc<T> {
                        type Response = super::ResumeResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ResumeRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as Instrument>::resume(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ResumeSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: Instrument> Clone for InstrumentServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: Instrument> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Instrument> tonic::server::NamedService for InstrumentServer<T> {
        const NAME: &'static str = "rs.tokio.console.instrument.Instrument";
    }
}
