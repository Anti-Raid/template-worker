use axum::{http::Request, Router};
use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server,
};
use std::{convert::Infallible, path::PathBuf};
use tokio::net::UnixListener;
use tower_service::Service;

#[derive(Debug, Clone)]
pub enum CreateRpcServerBind {
    /// Bind to a specific address
    Address(String),
    /// Bind to a unix socket
    #[cfg(unix)]
    #[allow(dead_code)] // may be used in the future
    UnixSocket(String),
}

#[derive(Debug, Clone)]
pub struct CreateRpcServerOptions {
    /// The bind address for the RPC server
    pub bind: CreateRpcServerBind,
}

pub async fn start_rpc_server(
    opts: CreateRpcServerOptions,
    mut make_service: axum::routing::IntoMakeService<Router>,
) -> ! {
    match opts.bind {
        CreateRpcServerBind::Address(addr) => {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!("failed to bind to address: {err:#}");
                    std::process::exit(1);
                }
            };

            log::info!(
                "Listening on {}",
                match listener.local_addr() {
                    Ok(ok) => ok.to_string(),
                    Err(err) => {
                        log::error!("failed to get local address: {err:#}");
                        std::process::exit(1);
                    }
                }
            );

            loop {
                let (socket, _remote_addr) = match listener.accept().await {
                    Ok(ok) => ok,
                    Err(err) => {
                        log::error!("failed to accept connection: {err:#}");
                        continue;
                    }
                };

                let tower_service = unwrap_infallible(make_service.call(&socket).await);

                tokio::spawn(async move {
                    let socket = TokioIo::new(socket);

                    let hyper_service =
                        hyper::service::service_fn(move |request: Request<Incoming>| {
                            tower_service.clone().call(request)
                        });

                    if let Err(err) = server::conn::auto::Builder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(socket, hyper_service)
                        .await
                    {
                        log::error!("failed to serve connection: {err:#}");
                    }
                });
            }
        }
        #[cfg(unix)]
        CreateRpcServerBind::UnixSocket(path) => {
            let path = PathBuf::from(path);

            let _ = tokio::fs::remove_file(&path).await;

            match tokio::fs::create_dir_all(
                path.parent()
                    .expect("Failed to create parent unix socket path"),
            )
            .await
            {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!("failed to create parent directory: {err:#}");
                    std::process::exit(1);
                }
            }

            let uds = match UnixListener::bind(path.clone()) {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!("failed to bind to unix socket: {err:#}");
                    std::process::exit(1);
                }
            };

            loop {
                let (socket, _remote_addr) = match uds.accept().await {
                    Ok(ok) => ok,
                    Err(err) => {
                        log::error!("failed to accept connection: {err:#}");
                        continue;
                    }
                };

                let tower_service = unwrap_infallible(make_service.call(&socket).await);

                tokio::spawn(async move {
                    let socket = TokioIo::new(socket);

                    let hyper_service =
                        hyper::service::service_fn(move |request: Request<Incoming>| {
                            tower_service.clone().call(request)
                        });

                    if let Err(err) = server::conn::auto::Builder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(socket, hyper_service)
                        .await
                    {
                        log::error!("failed to serve connection: {err:#}");
                    }
                });
            }
        }
    }
}

fn unwrap_infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        #[allow(unreachable_patterns)]
        Err(never) => match never {},
    }
}
