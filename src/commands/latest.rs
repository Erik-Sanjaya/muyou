use std::sync::Arc;

use serenity::builder::CreateApplicationCommand;
use serenity::client::Context;

use crate::send_message;
use crate::Bot;

pub async fn run(ctx: &Context) -> String {
    let ctx_arc = Arc::new(ctx.to_owned());
    let lock = ctx.data.read().await;
    if let Some(state) = lock.get::<Bot>() {
        let state = state.read().await;
        let cache = state.element_cache.as_slice();
        if cache.is_empty() {
            return format!("Cache is empty (this is weird; probably means expired cookie)");
        }

        send_message(ctx_arc, state.channel_id, cache).await;

        format!("")
    } else {
        format!("Failed to get typemap")
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("latest")
        .description("Get the latest list of SOCS competitions")
}
