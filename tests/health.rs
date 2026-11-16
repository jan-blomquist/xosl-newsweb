//! The health server and the `health` command's check, over a socket on the
//! loopback address.

#![allow(non_snake_case)]

use std::net::SocketAddr;
use std::time::Duration;
use tonic::service::Routes;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;
use xosl_newsweb::health::{Health, Outcome, ServeError, check, serve};

const EVALUATION_INTERVAL: Duration = Duration::from_secs(3600);
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// A server whose health is always `health`.
struct RunningServer {
    address: SocketAddr,
    shutdown: CancellationToken,
    task: JoinHandle<Result<(), ServeError>>,
}

impl RunningServer {
    async fn start(health: Health) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let shutdown = CancellationToken::new();
        let stop = shutdown.clone();
        let task = tokio::spawn(async move {
            let no_services = Routes::default();
            serve(
                listener,
                no_services,
                || health.clone(),
                EVALUATION_INTERVAL,
                &stop,
            )
            .await
        });
        Self {
            address,
            shutdown,
            task,
        }
    }

    fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    async fn stop(self) -> Result<(), ServeError> {
        self.shutdown.cancel();
        self.task.await.unwrap()
    }
}

fn unhealthy() -> Health {
    Health::Unhealthy {
        reason: "no listing".to_owned(),
    }
}

async fn status_of(server: &RunningServer, service: &str) -> ServingStatus {
    let channel = Channel::from_shared(server.url())
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut client = HealthClient::new(channel);
    let request = HealthCheckRequest {
        service: service.to_owned(),
    };
    client.check(request).await.unwrap().into_inner().status()
}

#[tokio::test]
async fn serve__healthy__reports_serving_for_both_service_names() {
    let server = RunningServer::start(Health::Healthy).await;
    let names = ["", "com.vizroy.xosl.newsweb.v1.NewswebService"];

    let mut statuses = Vec::new();
    for name in names {
        statuses.push(status_of(&server, name).await);
    }

    assert_eq!(statuses, [ServingStatus::Serving, ServingStatus::Serving]);
}

#[tokio::test]
async fn serve__unhealthy__reports_not_serving_for_both_service_names() {
    let server = RunningServer::start(unhealthy()).await;
    let names = ["", "com.vizroy.xosl.newsweb.v1.NewswebService"];

    let mut statuses = Vec::new();
    for name in names {
        statuses.push(status_of(&server, name).await);
    }

    assert_eq!(
        statuses,
        [ServingStatus::NotServing, ServingStatus::NotServing]
    );
}

#[tokio::test]
async fn serve__shutdown_cancelled__returns_ok() {
    let server = RunningServer::start(Health::Healthy).await;

    let result = server.stop().await;

    assert!(result.is_ok(), "result: {result:?}");
}

#[tokio::test]
async fn check__serving_server__is_healthy() {
    let server = RunningServer::start(Health::Healthy).await;

    let outcome = check(&server.url(), CHECK_TIMEOUT).await;

    assert_eq!(outcome, Outcome::Healthy);
}

#[tokio::test]
async fn check__not_serving_server__is_unhealthy() {
    let server = RunningServer::start(unhealthy()).await;

    let outcome = check(&server.url(), CHECK_TIMEOUT).await;

    assert_eq!(outcome, Outcome::Unhealthy);
}

#[tokio::test]
async fn check__nothing_listening__is_unreachable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);

    let outcome = check(&url, CHECK_TIMEOUT).await;

    assert!(
        matches!(outcome, Outcome::Unreachable { .. }),
        "outcome: {outcome:?}"
    );
}
