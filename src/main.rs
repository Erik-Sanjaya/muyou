mod commands;

use std::arch::x86_64::_mm_cvtsi64x_si128;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;

use chrono::{FixedOffset, Timelike, Utc};
use scraper::{Html, Selector};

use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::prelude::ChannelId;
use serenity::prelude::*;
use serenity::{async_trait, model::prelude::GuildId};
use shuttle_secrets::SecretStore;
use tokio::sync::mpsc;

use tracing::{error, info, warn};

#[derive(Clone)]
struct Bot {
    secrets: SecretStore,
    cookie: String,
    site: Arc<str>,
    channel_id: u64,
    is_loop_running: Arc<AtomicBool>,
    website_queried: Arc<AtomicBool>,
    element_cache: Vec<String>,
}

impl TypeMapKey for Bot {
    type Value = Arc<RwLock<Bot>>;
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!hello" {
            if let Err(e) = msg.channel_id.say(&ctx.http, "world!").await {
                error!("Error sending message: {:?}", e);
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id_str = if let Some(guild_id_str) = self.secrets.get("GUILD_ID") {
            guild_id_str
        } else {
            error!("'GUILD_ID' was not found");
            panic!("'GUILD_ID' was not found");
        };

        let guild_id = GuildId(guild_id_str.parse::<u64>().unwrap());

        let commands = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands
                .create_application_command(|command| commands::set_cookie::register(command))
                .create_application_command(|command| commands::get_cookie::register(command))
                .create_application_command(|command| commands::get_cache::register(command))
                .create_application_command(|command| commands::latest::register(command))
        })
        .await
        .unwrap();

        info!("{:#?}", commands);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let response_content = match command.data.name.as_str() {
                "set_cookie" => commands::set_cookie::run(&ctx, &command.data.options).await,
                "get_cookie" => commands::get_cookie::run(&ctx).await,
                "get_cache" => commands::get_cache::run(&ctx).await,
                "latest" => commands::latest::run(&ctx).await,

                command => unreachable!("Unknown command: {}", command),
            };

            let create_interaction_response =
                command.create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(response_content))
                });

            if let Err(why) = create_interaction_response.await {
                eprintln!("Cannot respond to slash command: {}", why);
            }
        }
    }

    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        println!("Cache built successfully!");

        let ctx = Arc::new(ctx);

        if self.is_loop_running.load(Ordering::Relaxed) {
            return;
        }

        let cookie = self.cookie.clone();
        let site = self.site.clone();
        let ctx1 = Arc::clone(&ctx);
        let queried = Arc::clone(&self.website_queried);
        let channel_id = self.channel_id.clone();

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let req = client
                .get(site.as_ref())
                .header(reqwest::header::COOKIE, cookie);

            let gmt7 = FixedOffset::east_opt(7 * 3600).unwrap();

            {
                // This is only for init, i hope it's fine to do this
                // can't really test it now, i'll just let the bot do it's thing
                let res = req.try_clone().unwrap().send().await.unwrap();
                if let Some(content) = extract_element(res).await {
                    info!("\n{:?}", content);

                    let mut lock = ctx1.data.write().await;
                    let state = lock.get_mut::<Bot>();
                    if let None = state {
                        return format!("Can't get the typemap: in cache_ready");
                    }

                    let mut data = state.unwrap().write().await;
                    if data.element_cache != content {
                        data.element_cache = content;
                        // send_message(ctx1.clone(), channel_id, content).await;
                    }
                };
            }

            loop {
                // Get the current time in GMT+7
                let now = Utc::now().with_timezone(&gmt7);
                // info!("tick");

                // TEST
                if now.hour() != 0
                    || (now.minute() != 0 && now.minute() != 2)
                    || queried.load(Ordering::Relaxed)
                {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    continue;
                }

                let res = req.try_clone().unwrap().send().await.unwrap();

                if let Some(content) = extract_element(res).await {
                    info!("\n{:?}", content);

                    let mut lock = ctx1.data.write().await;
                    let state = lock.get_mut::<Bot>();
                    if let None = state {
                        return format!("Can't get the typemap: in cache_ready");
                    }

                    let mut data = state.unwrap().write().await;
                    if data.element_cache != content {
                        data.element_cache = content.clone();
                        send_message(ctx1.clone(), channel_id, &content).await;
                    }
                };

                queried.store(true, Ordering::Relaxed);

                if queried.load(Ordering::Relaxed) && now.minute() != 0 && now.minute() != 2 {
                    queried.store(false, Ordering::Relaxed);
                }

                // TEST
                // tokio::time::sleep(Duration::from_secs(15)).await;
            }
        });

        // Now that the loop is running, we set the bool to true
        self.is_loop_running.swap(true, Ordering::Relaxed);
    }
}

// returns Option<String> because if it's a none it's 100% just doesn't exist
async fn extract_element(res: reqwest::Response) -> Option<Vec<String>> {
    let res = res.text().await.ok()?;
    info!("res to text");
    let document = Html::parse_document(&res);
    // can prob have the selector be a variable
    // i dont know why but i can't use `?` on this
    // for result, so i'm just returning an option
    let selector = Selector::parse(r#"select[name="cid"][id="cid"]"#).ok()?;
    info!("selector");

    let fragment = Html::parse_fragment(
        document
            .select(&selector)
            .next()
            .map(|e| e.inner_html())?
            .as_ref(),
    );

    let selector_option = Selector::parse(r#"option"#).ok()?;

    // I really don't think it can get out of order, but in that case, might wanna use HashSet instead
    // also i still cant figure out why this code is allowed to use the parser, but in the send_message it's not allowed
    Some(
        fragment
            .select(&selector_option)
            .map(|e| e.inner_html())
            .collect(),
    )
}

async fn send_message(ctx: Arc<Context>, channel_id: u64, html_string: &[String]) {
    let fields = html_string.into_iter().map(|e| ("", e, false));

    let message = ChannelId::from(channel_id)
        .send_message(&ctx, |m| {
            m.embed(|e| e.title("Latest SOCS List").fields(fields))
        })
        .await;

    if let Err(why) = message {
        warn!("Error sending message: {}", why);
    }
}

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
        token
    } else {
        return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
    };

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::DIRECT_MESSAGES;

    let cookie = if let Some(cookie) = secret_store.get("COOKIE") {
        cookie
    } else {
        "".into()
    };

    let site = if let Some(site) = secret_store.get("SITE") {
        site
    } else {
        return Err(anyhow!("'SITE' was not found").into());
    };

    let channel_id = if let Some(id) = secret_store.get("CHANNEL_ID") {
        id.parse::<u64>().unwrap()
    } else {
        return Err(anyhow!("'CHANNEL_ID' was not found").into());
    };

    let bot = Bot {
        secrets: secret_store.clone(),
        cookie,
        site: site.into(),
        channel_id,
        is_loop_running: Arc::new(AtomicBool::new(false)),
        website_queried: Arc::new(AtomicBool::new(false)),
        element_cache: vec![],
    };

    let client = Client::builder(&token, intents)
        .event_handler(bot.clone())
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<Bot>(Arc::new(RwLock::new(bot)));
    }

    Ok(client.into())
}

// TOOD:
// error handling for when the cookie expires
