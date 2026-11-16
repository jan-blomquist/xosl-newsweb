// @generated
/// Generated client implementations.
pub mod newsweb_service_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct NewswebServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl NewswebServiceClient<tonic::transport::Channel> {
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
    impl<T> NewswebServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::Body>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
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
        ) -> NewswebServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::Body>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::Body>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::Body>,
            >>::Error: Into<StdError> + std::marker::Send + std::marker::Sync,
        {
            NewswebServiceClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn list_disclosures(
            &mut self,
            request: impl tonic::IntoRequest<super::ListDisclosuresRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ListDisclosuresResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic_prost::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/com.vizroy.xosl.newsweb.v1.NewswebService/ListDisclosures",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "com.vizroy.xosl.newsweb.v1.NewswebService",
                        "ListDisclosures",
                    ),
                );
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn get_disclosure(
            &mut self,
            request: impl tonic::IntoRequest<super::GetDisclosureRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetDisclosureResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic_prost::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/com.vizroy.xosl.newsweb.v1.NewswebService/GetDisclosure",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "com.vizroy.xosl.newsweb.v1.NewswebService",
                        "GetDisclosure",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn read_attachment(
            &mut self,
            request: impl tonic::IntoRequest<super::ReadAttachmentRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ReadAttachmentResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic_prost::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/com.vizroy.xosl.newsweb.v1.NewswebService/ReadAttachment",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "com.vizroy.xosl.newsweb.v1.NewswebService",
                        "ReadAttachment",
                    ),
                );
            self.inner.server_streaming(req, path, codec).await
        }
        pub async fn watch_events(
            &mut self,
            request: impl tonic::IntoRequest<super::WatchEventsRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::WatchEventsResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic_prost::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/com.vizroy.xosl.newsweb.v1.NewswebService/WatchEvents",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "com.vizroy.xosl.newsweb.v1.NewswebService",
                        "WatchEvents",
                    ),
                );
            self.inner.server_streaming(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod newsweb_service_server {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with NewswebServiceServer.
    #[async_trait]
    pub trait NewswebService: std::marker::Send + std::marker::Sync + 'static {
        /// Server streaming response type for the ListDisclosures method.
        type ListDisclosuresStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::ListDisclosuresResponse, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        async fn list_disclosures(
            &self,
            request: tonic::Request<super::ListDisclosuresRequest>,
        ) -> std::result::Result<
            tonic::Response<Self::ListDisclosuresStream>,
            tonic::Status,
        >;
        async fn get_disclosure(
            &self,
            request: tonic::Request<super::GetDisclosureRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetDisclosureResponse>,
            tonic::Status,
        >;
        /// Server streaming response type for the ReadAttachment method.
        type ReadAttachmentStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::ReadAttachmentResponse, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        async fn read_attachment(
            &self,
            request: tonic::Request<super::ReadAttachmentRequest>,
        ) -> std::result::Result<
            tonic::Response<Self::ReadAttachmentStream>,
            tonic::Status,
        >;
        /// Server streaming response type for the WatchEvents method.
        type WatchEventsStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::WatchEventsResponse, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        async fn watch_events(
            &self,
            request: tonic::Request<super::WatchEventsRequest>,
        ) -> std::result::Result<
            tonic::Response<Self::WatchEventsStream>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct NewswebServiceServer<T> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T> NewswebServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for NewswebServiceServer<T>
    where
        T: NewswebService,
        B: Body + std::marker::Send + 'static,
        B::Error: Into<StdError> + std::marker::Send + 'static,
    {
        type Response = http::Response<tonic::body::Body>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            match req.uri().path() {
                "/com.vizroy.xosl.newsweb.v1.NewswebService/ListDisclosures" => {
                    #[allow(non_camel_case_types)]
                    struct ListDisclosuresSvc<T: NewswebService>(pub Arc<T>);
                    impl<
                        T: NewswebService,
                    > tonic::server::ServerStreamingService<
                        super::ListDisclosuresRequest,
                    > for ListDisclosuresSvc<T> {
                        type Response = super::ListDisclosuresResponse;
                        type ResponseStream = T::ListDisclosuresStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListDisclosuresRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as NewswebService>::list_disclosures(&inner, request)
                                    .await
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
                        let method = ListDisclosuresSvc(inner);
                        let codec = tonic_prost::ProstCodec::default();
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
                "/com.vizroy.xosl.newsweb.v1.NewswebService/GetDisclosure" => {
                    #[allow(non_camel_case_types)]
                    struct GetDisclosureSvc<T: NewswebService>(pub Arc<T>);
                    impl<
                        T: NewswebService,
                    > tonic::server::UnaryService<super::GetDisclosureRequest>
                    for GetDisclosureSvc<T> {
                        type Response = super::GetDisclosureResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetDisclosureRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as NewswebService>::get_disclosure(&inner, request).await
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
                        let method = GetDisclosureSvc(inner);
                        let codec = tonic_prost::ProstCodec::default();
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
                "/com.vizroy.xosl.newsweb.v1.NewswebService/ReadAttachment" => {
                    #[allow(non_camel_case_types)]
                    struct ReadAttachmentSvc<T: NewswebService>(pub Arc<T>);
                    impl<
                        T: NewswebService,
                    > tonic::server::ServerStreamingService<super::ReadAttachmentRequest>
                    for ReadAttachmentSvc<T> {
                        type Response = super::ReadAttachmentResponse;
                        type ResponseStream = T::ReadAttachmentStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ReadAttachmentRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as NewswebService>::read_attachment(&inner, request)
                                    .await
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
                        let method = ReadAttachmentSvc(inner);
                        let codec = tonic_prost::ProstCodec::default();
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
                "/com.vizroy.xosl.newsweb.v1.NewswebService/WatchEvents" => {
                    #[allow(non_camel_case_types)]
                    struct WatchEventsSvc<T: NewswebService>(pub Arc<T>);
                    impl<
                        T: NewswebService,
                    > tonic::server::ServerStreamingService<super::WatchEventsRequest>
                    for WatchEventsSvc<T> {
                        type Response = super::WatchEventsResponse;
                        type ResponseStream = T::WatchEventsStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::WatchEventsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as NewswebService>::watch_events(&inner, request).await
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
                        let method = WatchEventsSvc(inner);
                        let codec = tonic_prost::ProstCodec::default();
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
                _ => {
                    Box::pin(async move {
                        let mut response = http::Response::new(
                            tonic::body::Body::default(),
                        );
                        let headers = response.headers_mut();
                        headers
                            .insert(
                                tonic::Status::GRPC_STATUS,
                                (tonic::Code::Unimplemented as i32).into(),
                            );
                        headers
                            .insert(
                                http::header::CONTENT_TYPE,
                                tonic::metadata::GRPC_CONTENT_TYPE,
                            );
                        Ok(response)
                    })
                }
            }
        }
    }
    impl<T> Clone for NewswebServiceServer<T> {
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
    /// Generated gRPC service name
    pub const SERVICE_NAME: &str = "com.vizroy.xosl.newsweb.v1.NewswebService";
    impl<T> tonic::server::NamedService for NewswebServiceServer<T> {
        const NAME: &'static str = SERVICE_NAME;
    }
}
