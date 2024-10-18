use confusables::Confusable;
use encode_unicode::StrExt;
use teloxide::{
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputMessageContentText,
        MessageEntity, ReplyParameters,
    },
    utils::command::BotCommands,
};

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
                .endpoint(process_reply),
            )
            .branch(
                dptree::filter(|x: Message| x.text().is_some() || x.caption().is_some())
                    .endpoint(process_message),
            ),
    );

    let inline_handler = Update::filter_inline_query().branch(
        dptree::entry()
            .filter(|x: InlineQuery| !x.query.is_empty())
            .endpoint(process_inline_query),
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

async fn process_inline_query(bot: Bot, query: InlineQuery) -> ResponseResult<()> {
    let pattern_result = match query.query.split_once(' ') {
        Some((pattern, input)) if !pattern.is_empty() && !input.is_empty() => {
            InlineQueryResultArticle::new(
                "patternify",
                format!("Patternify using '{pattern}'"),
                InputMessageContent::Text(
                    InputMessageContentText::new(input).entities(spoilerify(input, pattern)),
                ),
            )
        }
        _ => InlineQueryResultArticle::new(
            "invalid_query",
            "Invalid query, format: <pattern without spaces> <input>",
            InputMessageContent::Text(
                InputMessageContentText::new(
                    "Invalid query, format: `@PatternifyBot <pattern without spaces> <input>`",
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2),
            ),
        ),
    };

    let response = bot
        .answer_inline_query(query.id, vec![InlineQueryResult::Article(pattern_result)])
        .send()
        .await;

    if let Err(err) = response {
        log::error!("Error in handler: {:?}", err);
    }

    respond(())
}

async fn process_reply(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    let reply_message = match msg.reply_to_message() {
        Some(message) => message,
        None => return Ok(()),
    };

    let text_to_process = reply_message.text().or(reply_message.caption()).unwrap();

    let mut entities_to_send = match (reply_message.entities(), reply_message.caption_entities()) {
        (Some(entities), _) => entities,
        (None, Some(entities)) => entities,
        _ => return Ok(()),
    }
    .to_vec();

    let result = match cmd {
        Command::Patternify(pattern) => spoilerify(text_to_process, &pattern),
    };

    if result.is_empty() {
        return Ok(());
    }

    entities_to_send.extend(result.into_iter());

    let response = bot
        .send_message(msg.chat.id, text_to_process)
        .entities(entities_to_send)
        .reply_parameters(ReplyParameters::new(reply_message.id))
        .await;

    if let Err(err) = response {
        log::error!("Error in handler: {:?}", err);
    }

    respond(())
}

async fn process_message(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    let (result, input) = match cmd {
        Command::Patternify(text) => match text.split_once(' ') {
            Some((pattern, input)) => (spoilerify(&input, &pattern), input.to_string()),
            None => return Ok(()),
        },
    };

    if result.is_empty() {
        return Ok(());
    }

    let mut entities = msg.entities().map_or_else(|| Vec::new(), |x| x.to_vec());
    entities.extend(result.into_iter());

    let response = bot
        .send_message(msg.chat.id, input)
        .entities(entities)
        .reply_parameters(ReplyParameters::new(msg.id))
        .await;

    if let Err(err) = response {
        log::error!("Error in handler: {:?}", err);
    }

    Ok(())
}

fn spoilerify(input: &str, pattern: &str) -> Vec<MessageEntity> {
    if input.is_empty() || pattern.is_empty() {
        return Vec::new();
    }

    log::debug!(
        "Processing input '{}' (len {}) with pattern '{}' (len {})",
        String::from_iter(input.chars().take(20)),
        input.chars().count(),
        String::from_iter(pattern.chars().take(20)),
        pattern.chars().count()
    );

    let mut pattern_indexes = Vec::with_capacity(pattern.chars().count());

    let mut entities = Vec::new();
    let lowercased_pattern = pattern.to_lowercase();

    let mut previous_index = 0;
    loop {
        let started_from_ind = previous_index;
        for pattern_char in lowercased_pattern.chars() {
            let pattern_char_str = pattern_char.to_string();
            let pattern_char_conf = pattern_char_str.detect_replace_confusable();
            let upper_pattern_char_str = pattern_char.to_uppercase().to_string();
            let upper_pattern_char_str_conf = upper_pattern_char_str.detect_replace_confusable();
            let mut value_found = false;
            let mut u16_index = previous_index;
            for input_char in input.utf16chars().skip(previous_index) {
                let input_char_str = input_char.to_string();
                let input_char_str_conf = input_char_str.detect_replace_confusable();
                if input_char_str_conf == pattern_char_conf
                    || input_char_str_conf == upper_pattern_char_str_conf
                {
                    pattern_indexes.push(u16_index);
                    previous_index = u16_index + 1;
                    value_found = true;
                    break;
                }

                u16_index += 1;
            }

            if !value_found {
                let last_ind = started_from_ind;
                entities.push(MessageEntity::spoiler(
                    last_ind,
                    input.utf16chars().map(|x| x.len()).sum::<usize>() - last_ind,
                ));

                return entities;
            }
        }

        let mut previous_offset = started_from_ind;
        for i in pattern_indexes.iter().map(|x| *x) {
            let length = i - previous_offset;
            if length > 0 {
                entities.push(MessageEntity::spoiler(previous_offset, i - previous_offset));
            }

            previous_offset = i + 1;
        }

        pattern_indexes.clear();
    }
}
