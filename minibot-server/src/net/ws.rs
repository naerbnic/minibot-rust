use futures::prelude::*;
use gotham::{
    hyper::{
        self,
        header::{HeaderValue, CONNECTION, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY, UPGRADE},
        upgrade::Upgraded,
        Body, HeaderMap, Response, StatusCode,
    },
    state::{FromState, State},
};
use sha1::Sha1;
use tokio_tungstenite::{tungstenite, WebSocketStream};

pub use tungstenite::protocol::{Message, Role};
pub use tungstenite::Error;

pub type WebSocket = WebSocketStream<Upgraded>;

const PROTO_WEBSOCKET: &str = "websocket";

pub fn requested(state: &State) -> bool {
    let headers = HeaderMap::borrow_from(state);
    headers.get(UPGRADE) == Some(&HeaderValue::from_static(PROTO_WEBSOCKET))
}

pub fn accept(
    state: &mut State,
) -> Result<
    (
        Response<Body>,
        impl Future<Output = Result<WebSocketStream<Upgraded>, hyper::Error>>,
    ),
    anyhow::Error,
> {
    let body = Body::take_from(state);
    let headers = HeaderMap::borrow_from(state);
    let res = response(headers)?;
    let ws = async move {
        let upgraded = body.on_upgrade().await?;
        Ok(WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await)
    };

    Ok((res, ws))
}

fn response(headers: &HeaderMap) -> Result<Response<Body>, anyhow::Error> {
    let key = headers.get(SEC_WEBSOCKET_KEY).ok_or(anyhow::anyhow!(
        "Websocket connection did not provide SEC_WEBSOCKET_KEY header."
    ))?;

    Ok(Response::builder()
        .header(UPGRADE, PROTO_WEBSOCKET)
        .header(CONNECTION, "upgrade")
        .header(SEC_WEBSOCKET_ACCEPT, accept_key(key.as_bytes()))
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .body(Body::empty())?)
}

fn accept_key(key: &[u8]) -> String {
    const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut sha1 = Sha1::default();
    sha1.update(key);
    sha1.update(WS_GUID);
    base64::encode(&sha1.digest().bytes())
}

pub fn upgrade_required_response() -> Response<Body> {
    Response::builder()
        .header(UPGRADE, PROTO_WEBSOCKET)
        .status(StatusCode::UPGRADE_REQUIRED)
        .body(Body::empty())
        .unwrap()
}
