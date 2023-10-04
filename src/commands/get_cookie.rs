use serenity::builder::CreateApplicationCommand;
use serenity::client::Context;

use crate::Bot;

pub async fn run(ctx: &Context) -> String {
    let lock = ctx.data.read().await;
    if let Some(state) = lock.get::<Bot>() {
        let state = state.read().await;
        let cookie: &str = state.cookie.as_ref();
        if cookie.is_empty() {
            return format!("Please set a cookie first using the `/set_cookie` command");
        }

        format!("COOKIE: {}", cookie)
    } else {
        format!("Failed to get lock")
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("get_cookie")
        .description("Get the cookie from client")
}
