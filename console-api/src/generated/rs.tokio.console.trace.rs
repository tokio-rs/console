/// Start watching trace events with the provided filter.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchRequest {
    /// Specifies which trace events should be streamed.
    #[prost(string, tag="1")]
    pub filter: ::prost::alloc::string::String,
}
/// A trace event
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TraceEvent {
    /// A trace event
    #[prost(oneof="trace_event::Event", tags="1, 2, 3, 4, 5, 6")]
    pub event: ::core::option::Option<trace_event::Event>,
}
/// Nested message and enum types in `TraceEvent`.
pub mod trace_event {
    /// `RegisterThreads` signals that a new thread was registered.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RegisterThreads {
        /// `names` maps the registered thread id's to their associated name.
        #[prost(map="uint64, string", tag="1")]
        pub names: ::std::collections::HashMap<u64, ::prost::alloc::string::String>,
    }
    /// `Enter` signals that a span was entered.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Enter {
        /// `span_id` identifies the span that was entered.
        #[prost(message, optional, tag="1")]
        pub span_id: ::core::option::Option<super::super::common::SpanId>,
        /// `thread_id` identifies who entered the span.
        #[prost(uint64, tag="2")]
        pub thread_id: u64,
        /// `at` identifies when the span was entered.
        #[prost(message, optional, tag="3")]
        pub at: ::core::option::Option<::prost_types::Timestamp>,
    }
    /// `Exit` signals that a span was exited.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Exit {
        /// `span_id` identifies the span that was exited.
        #[prost(message, optional, tag="1")]
        pub span_id: ::core::option::Option<super::super::common::SpanId>,
        /// `thread_id` identifies who exited the span.
        #[prost(uint64, tag="2")]
        pub thread_id: u64,
        /// `at` identifies when the span was exited.
        #[prost(message, optional, tag="3")]
        pub at: ::core::option::Option<::prost_types::Timestamp>,
    }
    /// `Close` signals that a span was closed.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Close {
        /// `span_id` identifies the span that was closed.
        #[prost(message, optional, tag="1")]
        pub span_id: ::core::option::Option<super::super::common::SpanId>,
        /// `at` identifies when the span was closed.
        #[prost(message, optional, tag="2")]
        pub at: ::core::option::Option<::prost_types::Timestamp>,
    }
    /// A trace event
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Event {
        /// A new thread was registered.
        #[prost(message, tag="1")]
        RegisterThread(RegisterThreads),
        /// A new span metadata was registered.
        #[prost(message, tag="2")]
        RegisterMetadata(super::super::common::RegisterMetadata),
        /// A span was created.
        #[prost(message, tag="3")]
        NewSpan(super::super::common::Span),
        /// A span was entered.
        #[prost(message, tag="4")]
        EnterSpan(Enter),
        /// A span was exited.
        #[prost(message, tag="5")]
        ExitSpan(Exit),
        /// A span was closed.
        #[prost(message, tag="6")]
        CloseSpan(Close),
    }
}
/// Generated client implementations.
pub mod trace_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Allows observers to stream trace events for a given `WatchRequest` filter.
    #[derive(Debug, Clone)]
    pub struct TraceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl TraceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> TraceClient<T>
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
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> TraceClient<InterceptedService<T, F>>
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
            TraceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with `gzip`.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_gzip(mut self) -> Self {
            self.inner = self.inner.send_gzip();
            self
        }
        /// Enable decompressing responses with `gzip`.
        #[must_use]
        pub fn accept_gzip(mut self) -> Self {
            self.inner = self.inner.accept_gzip();
            self
        }
        /// Produces a stream of trace events for the given filter.
        pub async fn watch(
            &mut self,
            request: impl tonic::IntoRequest<super::WatchRequest>,
        ) -> Result<
            tonic::Response<tonic::codec::Streaming<super::TraceEvent>>,
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
                "/rs.tokio.console.trace.Trace/Watch",
            );
            self.inner.server_streaming(request.into_request(), path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod trace_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    ///Generated trait containing gRPC methods that should be implemented for use with TraceServer.
    #[async_trait]
    pub trait Trace: Send + Sync + 'static {
        ///Server streaming response type for the Watch method.
        type WatchStream: futures_core::Stream<
                Item = Result<super::TraceEvent, tonic::Status>,
            >
            + Send
            + 'static;
        /// Produces a stream of trace events for the given filter.
        async fn watch(
            &self,
            request: tonic::Request<super::WatchRequest>,
        ) -> Result<tonic::Response<Self::WatchStream>, tonic::Status>;
    }
    /// Allows observers to stream trace events for a given `WatchRequest` filter.
    #[derive(Debug)]
    pub struct TraceServer<T: Trace> {
        inner: _Inner<T>,
        accept_compression_encodings: (),
        send_compression_encodings: (),
    }
    struct _Inner<T>(Arc<T>);
    impl<T: Trace> TraceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
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
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for TraceServer<T>
    where
        T: Trace,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/rs.tokio.console.trace.Trace/Watch" => {
                    #[allow(non_camel_case_types)]
                    struct WatchSvc<T: Trace>(pub Arc<T>);
                    impl<
                        T: Trace,
                    > tonic::server::ServerStreamingService<super::WatchRequest>
                    for WatchSvc<T> {
                        type Response = super::TraceEvent;
                        type ResponseStream = T::WatchStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::WatchRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).watch(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = WatchSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.server_streaming(method, req).await;
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
    impl<T: Trace> Clone for TraceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: Trace> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Trace> tonic::transport::NamedService for TraceServer<T> {
        const NAME: &'static str = "rs.tokio.console.trace.Trace";
    }
}
