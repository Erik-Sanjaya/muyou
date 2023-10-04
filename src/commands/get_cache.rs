use serenity::builder::CreateApplicationCommand;
use serenity::client::Context;

use crate::Bot;

pub async fn run(ctx: &Context) -> String {
    let lock = ctx.data.read().await;
    if let Some(state) = lock.get::<Bot>() {
        let state = state.read().await;
        let cache = state.element_cache.as_slice();
        if cache.is_empty() {
            return format!("Cache is empty (this is weird; probably means expired cookie)");
        }

        format!("CACHE: {:?}", cache)
    } else {
        format!("Failed to get typemap")
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("get_cache")
        .description("Get the cache of the element")
}
