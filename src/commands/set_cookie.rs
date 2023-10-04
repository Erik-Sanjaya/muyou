use serenity::builder::CreateApplicationCommand;
use serenity::client::Context;
use serenity::model::prelude::command::CommandOptionType;
use serenity::model::prelude::interaction::application_command::{
    CommandDataOption, CommandDataOptionValue,
};

use crate::Bot;

pub async fn run(ctx: &Context, options: &[CommandDataOption]) -> String {
    let key = options
        .get(0)
        .expect("Expected cookie key")
        .resolved
        .as_ref()
        .expect("Expected cookie");

    let value = options
        .get(1)
        .expect("Expected cookie value")
        .resolved
        .as_ref()
        .expect("Expected cookie");

    if let (CommandDataOptionValue::String(key), CommandDataOptionValue::String(value)) =
        (key, value)
    {
        {
            let mut lock = ctx.data.write().await;
            let state = lock.get_mut::<Bot>();
            if let None = state {
                return format!("FAILED TO SET COOKIE: State lock get_mut returned None");
            }

            let cookie = format!("{}={}", key, value);

            state.unwrap().write().await.cookie = cookie;
        }
        format!("COOKIE: {}={}", key, value)
    } else {
        "Please provide a valid cookie string".to_string()
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("set_cookie")
        .description("Set the cookie for client")
        .create_option(|option| {
            option
                .name("key")
                .description("Cookie key")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_option(|option| {
            option
                .name("value")
                .description("Cookie value")
                .kind(CommandOptionType::String)
                .required(true)
        })
}
