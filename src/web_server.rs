use embassy_executor::Spawner;
use embassy_time::Duration;
use esp_println::println;
use picoserve::{
    extract::Json,
    make_static,
    routing::{get, get_service, post, PathRouter},
    AppBuilder, AppRouter,
    Router,
};

use crate::config;
use crate::display::{Settings, SETTINGS};

pub struct AppProps;

impl AppBuilder for AppProps {
    type PathRouter = impl picoserve::routing::PathRouter;

    fn build_app(self) -> picoserve::Router<Self::PathRouter> {
        picoserve::Router::new()
            .route(
                "/",
                get_service(picoserve::response::File::html(include_str!(
                    "web/index.html"
                ))),
            )
            .route(
                "/style.css",
                get_service(picoserve::response::File::css(include_str!(
                    "web/style.css"
                ))),
            )
            .route(
                "/main.js",
                get_service(picoserve::response::File::javascript(include_str!(
                    "web/main.js"
                ))),
            )
            .route(
                "/api/settings",
                get(|| async {
                    let settings = *SETTINGS.lock().await;
                    Json(settings)
                })
                .post(|Json(new_settings)| async move {
                    let mut lock = SETTINGS.lock().await;
                    *lock = new_settings;
                    Json(new_settings)
                }),
            )
    }
}

const WEB_TASK_POOL_SIZE: usize = config::WEB_TASK_POOL_SIZE;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
async fn web_task(
    id: usize,
    stack: embassy_net::Stack<'static>,
    app: &'static AppRouter<AppProps>,
    config: &'static picoserve::Config,
) -> ! {
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    println!("Web server listening on port {}", config::HTTP_SERVER_PORT);

    picoserve::Server::new(app, config, &mut http_buffer)
        .listen_and_serve(id, stack, config::HTTP_SERVER_PORT, &mut tcp_rx_buffer, &mut tcp_tx_buffer)
        .await
        .into_never()
}

pub async fn start_web_server(spawner: Spawner, stack: embassy_net::Stack<'static>) {
    println!("Starting web server with {WEB_TASK_POOL_SIZE} tasks...");

    let app = make_static!(AppRouter<AppProps>, AppProps.build_app());

    let config = make_static!(
        picoserve::Config,
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Duration::from_secs(5),
            persistent_start_read_request: Duration::from_secs(1),
            read_request: Duration::from_secs(1),
            write: Duration::from_secs(1),
        })
        .keep_connection_alive()
    );

    for id in 0..WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web_task(id, stack, app, config));
    }
}