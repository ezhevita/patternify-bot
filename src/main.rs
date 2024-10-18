use teloxide::{prelude::*, utils::command::BotCommands};

mod handlers;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();

    let message_handler = Update::filter_message().branch(
        dptree::entry()
            .filter_command::<Command>()
            .branch(
                dptree::filter(|x: Message| {
                    x.reply_to_message().is_some_and(|reply_msg| {
                        reply_msg.text().is_some() || reply_msg.caption().is_some()
                    })
                })
                .endpoint(handlers::process_reply),
            )
            .branch(
                dptree::filter(|x: Message| x.text().is_some() || x.caption().is_some())
                    .endpoint(handlers::process_message),
            ),
    );

    let inline_handler = Update::filter_inline_query().branch(
        dptree::entry()
            .filter(|x: InlineQuery| !x.query.is_empty())
            .endpoint(handlers::process_inline_query),
    );

    let ignore_update = |_upd| Box::pin(async {});

    Dispatcher::builder(
        bot,
        dptree::entry()
            .branch(message_handler)
            .branch(inline_handler),
    )
    .enable_ctrlc_handler()
    .default_handler(ignore_update)
    .build()
    .dispatch()
    .await
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "<pattern> <input> - patternify text.")]
    Patternify(String),
}
