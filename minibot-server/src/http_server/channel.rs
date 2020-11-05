use gotham::{
    handler::HandlerError,
    hyper::{Body, Response},
    state::{FromState, State},
};

use crate::http_server::middleware::authn::AuthIdentity;
use crate::net::ws;
use crate::services::base::account::AccountStoreHandle;

async fn handle_channel(state: &mut State) -> Result<Response<Body>, HandlerError> {
    let id = AuthIdentity::take_from(state);
    let account_store = AccountStoreHandle::take_from(state);

    let _acct = account_store
        .get_account(id.id())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Invalid User ID"))?;

    if ws::requested(state) {
        let (resp, fut) = ws::accept(state)?;

        tokio::spawn(async move {
            let _stream = fut.await;
        });

        Ok(resp)
    } else {
        Ok(ws::upgrade_required_response())
    }
}
