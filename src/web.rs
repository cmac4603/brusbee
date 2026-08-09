//! Webserver running on brusbee.
use esp_alloc as _;
use picoserve::{AppBuilder, AppRouter, routing::get_service};

pub struct AppProps;

impl AppBuilder for AppProps {
    type PathRouter = impl picoserve::routing::PathRouter;

    fn build_app(self) -> picoserve::Router<Self::PathRouter> {
        picoserve::Router::new()
            .route(
                "/",
                get_service(picoserve::response::File::html(include_str!(
                    "./assets/index.html"
                ))),
            )
            .route(
                "/style.css",
                get_service(picoserve::response::File::css(include_str!(
                    "./assets/output.css"
                ))),
            )
            .route(
                "/htmx.min.js",
                get_service(picoserve::response::File::javascript(include_str!(
                    "./assets/htmx.min.js"
                ))),
            )
    }
}

pub static CONFIG: picoserve::Config = picoserve::Config::const_default().keep_connection_alive();

// tied to how many sockets are created in the embassy-net network stack
// passed into [web_task]
pub const WEB_TASK_POOL_SIZE: usize = 2;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    task_id: usize,
    stack: embassy_net::Stack<'static>,
    app: &'static AppRouter<AppProps>,
) -> ! {
    let port = 8080;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::Server::new(app, &CONFIG, &mut http_buffer)
        .listen_and_serve(task_id, stack, port, &mut tcp_rx_buffer, &mut tcp_tx_buffer)
        .await
        .into_never()
}
