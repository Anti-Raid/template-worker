/// Defines a RPC layer
macro_rules! define_rpc_endpoints {
    (
        // name: VariantName(RequestType) -> ResponseType
        $( $method:ident : $variant:ident( $req:ty ) -> $res:ty ),* $(,)?
    ) => {
        use crate::Error;

        #[derive(serde::Serialize, serde::Deserialize)]
        /// Internal enum representing all possible RPC requests from client to server
        pub enum RpcRequest {
            $( $variant($req), )*
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        /// Internal enum representing all possible RPC responses from server to client
        pub enum RpcResponse {
            $( $variant($res), )*
            Error(String),
        }

        mod sealed {
            pub trait Sealed {}
        }

        pub trait RpcMessage: sealed::Sealed + Into<RpcRequest> {
            type Response;

            /// Extracts the expected response type from a generic RpcResponse, returning an error if the variant does not match
            fn extract_response(resp: RpcResponse) -> Result<Self::Response, Error>;
        }

        $(
            impl sealed::Sealed for $req {}

            impl From<$req> for RpcRequest {
                fn from(req: $req) -> Self {
                    RpcRequest::$variant(req)
                }
            }

            impl RpcMessage for $req {
                type Response = $res;

                fn extract_response(resp: RpcResponse) -> Result<Self::Response, Error> {
                    match resp {
                        RpcResponse::$variant(data) => Ok(data),
                        RpcResponse::Error(err) => Err(err.into()),
                        _ => Err("internal mesophyll error: server returned mismatched response variant!".into()),
                    }
                }
            }
        )*

        /// Trait to be implemented by the RPC handler, which executes the RPC calls based on the defined endpoints
        #[serenity::async_trait]
        pub trait RpcExecutor: Send + Sync {
            $(
                async fn $method(&self, req: $req) -> Result<$res, Error>;
            )*

            async fn execute_rpc(&self, request: RpcRequest) -> RpcResponse {
                match request {
                    $(
                        RpcRequest::$variant(req) => match self.$method(req).await {
                            Ok(res) => RpcResponse::$variant(res),
                            Err(e) => RpcResponse::Error(e.to_string()),
                        },
                    )*
                }
            }
        }

        pub trait RpcTransport: Send + Sync {
            async fn send(&self, req: RpcRequest) -> RpcResponse;
        }

        pub mod inmemory {
            use super::*;

            /// An in-memory transport for the In-Memory RPC implementation
            #[allow(dead_code)]
            pub struct InMemoryTransport<E: RpcExecutor> {
                executor: E,
            }

            impl<E: RpcExecutor> InMemoryTransport<E> {
                pub fn new(executor: E) -> Self {
                    Self { executor }
                }
            }

            impl<E: RpcExecutor> RpcTransport for InMemoryTransport<E> {
                async fn send(&self, req: RpcRequest) -> RpcResponse {
                    self.executor.execute_rpc(req).await
                }
            }
        }

        pub mod http2 {
            use super::*;
            /// A http2 transport for worker->master communications
            #[allow(dead_code)]
            pub struct Http2Transport {
                client: reqwest::Client,
                url: String,
            }

            impl Http2Transport {
                pub fn new(client: reqwest::Client, url: String) -> Self {
                    Self { client, url }
                }
            }

            impl RpcTransport for Http2Transport {
                // Encode with rmp_serde::to_vec
                async fn send(&self, req: RpcRequest) -> RpcResponse {
                    let req = match rmp_serde::to_vec(&req) {
                        Ok(r) => r,
                        Err(e) => return RpcResponse::Error(format!("Failed to serialize request: {e}")),
                    };
                    let resp = match self.client.post(&self.url).body(req).send().await {
                        Ok(r) => r,
                        Err(e) => return RpcResponse::Error(format!("HTTP request failed: {e}")),
                    };
                    let bytes = match resp.bytes().await {
                        Ok(b) => b,
                        Err(e) => return RpcResponse::Error(format!("Failed to read response bytes: {e}")),
                    };
                    match rmp_serde::from_slice(&bytes) {
                        Ok(r) => r,
                        Err(e) => RpcResponse::Error(format!("Failed to deserialize response: {e}")),
                    }
                }
            }
        }

        /// The high-level RPC client to be used
        pub struct RpcClient<T: RpcTransport> {
            transport: T,
        }

        impl<T: RpcTransport> RpcClient<T> {
            pub fn new(transport: T) -> Self {
                Self { transport }
            }

            pub async fn execute<M: RpcMessage>(&self, msg: M) -> Result<M::Response, crate::Error> {
                let req: RpcRequest = msg.into();
                let resp = self.transport.send(req).await;
                M::extract_response(resp)
            }
        }
    };
}

/// Master-worker communications
pub mod master_worker {
    use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
    use crate::worker::workervmmanager::Id;

    /// Heartbeating
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct HeartbeatReq {}

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct HeartbeatResp {
        pub time: chrono::DateTime<chrono::Utc>,
    }

    /// Dispatch a (custom) template event to a worker
    /// 
    /// Discord related events are dispatched via Sandwich
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct DispatchEventReq {
        pub id: Id,
        pub event: CreateEvent,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct DispatchEventResp {
        pub result: KhronosValue,
    }

    define_rpc_endpoints! {
        heartbeat: Heartbeat(HeartbeatReq) -> HeartbeatResp,
        dispatch_event: DispatchEvent(DispatchEventReq) -> DispatchEventResp,
    }
}