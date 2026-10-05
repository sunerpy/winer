//! The `lcu` protocol: champion, item and profile icons straight from the connected client.
//!
//! The window cannot fetch them itself: the LCU wants a token and presents a certificate only
//! winer trusts. On Windows the window addresses `http://lcu.localhost/<path>`, elsewhere
//! `lcu://localhost/<path>`; either way `<path>` is the asset path the catalog carries.

use tauri::{
    Manager, Runtime, UriSchemeContext, UriSchemeResponder,
    http::{Request, Response, StatusCode, header},
};
use winer_core::{CoreError, Service};

pub(crate) fn protocol<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let Some(service) = context
        .app_handle()
        .try_state::<Service>()
        .map(|service| service.inner().clone())
    else {
        return responder.respond(status(StatusCode::SERVICE_UNAVAILABLE));
    };
    let path = request.uri().path().to_owned();
    tauri::async_runtime::spawn(async move {
        let response = match service.asset(&path).await {
            Ok(asset) => Response::builder()
                .header(header::CONTENT_TYPE, asset.content_type.as_str())
                .header(header::CACHE_CONTROL, "max-age=86400, immutable")
                .body(asset.bytes.clone())
                .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
            Err(error) => status(match error {
                CoreError::Invalid(_) => StatusCode::BAD_REQUEST,
                CoreError::NotConnected => StatusCode::SERVICE_UNAVAILABLE,
                CoreError::Lcu(error) if error.is_not_found() => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_GATEWAY,
            }),
        };
        responder.respond(response);
    });
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    response
}
