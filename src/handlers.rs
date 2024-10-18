use confusables::Confusable;
use teloxide::{
    payloads::SendMessage,
    prelude::*,
    requests::JsonRequest,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputMessageContentText,
        MessageEntity, ReplyParameters,
    },
};

use crate::Command;

pub async fn process_inline_query(bot: Bot, query: InlineQuery) -> ResponseResult<()> {
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

pub async fn process_reply(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    let response = _process_reply(bot, msg, cmd).send().await;

    if let Err(err) = response {
        log::error!("Error in handler: {:?}", err);
    }

    respond(())
}

fn _process_reply(bot: Bot, msg: Message, cmd: Command) -> JsonRequest<SendMessage> {
    let reply_message = msg.reply_to_message().unwrap();
    let text_to_process = reply_message.text().or(reply_message.caption()).unwrap();

    let response = bot
        .send_message(msg.chat.id, text_to_process)
        .reply_parameters(ReplyParameters::new(reply_message.id));

    let mut entities_to_send = reply_message
        .entities()
        .or(reply_message.caption_entities())
        .map_or_else(|| Vec::new(), |x| x.to_vec());

    let result = match cmd {
        Command::Patternify(pattern) if !pattern.is_empty() => {
            spoilerify(text_to_process, &pattern)
        }
        _ => return response.text("No pattern provided."),
    };

    assert!(!result.is_empty());

    entities_to_send.extend(result.into_iter());

    response.entities(entities_to_send)
}

pub async fn process_message(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    let response = _process_message(bot, msg, cmd).send().await;

    if let Err(err) = response {
        log::error!("Error in handler: {:?}", err);
    }

    respond(())
}

fn _process_message(bot: Bot, msg: Message, cmd: Command) -> JsonRequest<SendMessage> {
    let response = bot
        .send_message(msg.chat.id, "")
        .reply_parameters(ReplyParameters::new(msg.id));

    let (result, input) = match cmd {
        Command::Patternify(text) => match text.split_once(' ') {
            Some((pattern, input)) if !pattern.is_empty() && !input.is_empty() => {
                (spoilerify(&input, &pattern), input.to_string())
            }
            _ => {
                return response
                    .text("Invalid command usage, format: `/patternify <pattern without spaces> <input>`")
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2);
            }
        },
    };

    assert!(!result.is_empty());

    let mut entities = msg.entities().map_or_else(|| Vec::new(), |x| x.to_vec());
    entities.extend(result.into_iter());

    response.text(input).entities(entities)
}

fn spoilerify(input: &str, pattern: &str) -> Vec<MessageEntity> {
    const DEBUG_INPUT_STRIP_LENGTH: usize = 20;

    if input.is_empty() || pattern.is_empty() {
        return Vec::new();
    }

    log::debug!(
        "Processing input {} with pattern {}",
        prepare_input_for_logging(&input, DEBUG_INPUT_STRIP_LENGTH),
        prepare_input_for_logging(&pattern, DEBUG_INPUT_STRIP_LENGTH)
    );

    let mut pattern_indexes = Vec::with_capacity(pattern.chars().count());
    let mut entities = Vec::new();
    let mut input_iterator = input.chars();
    let mut u16_index = 0;

    let lowercased_pattern = pattern.to_lowercase();

    loop {
        let started_from_ind = u16_index;
        for pattern_char in lowercased_pattern.chars() {
            let pattern_char_str = pattern_char.to_string();
            let pattern_char_str_conf = pattern_char_str.detect_replace_confusable();
            let upper_pattern_char_str = pattern_char.to_uppercase().to_string();
            let upper_pattern_char_str_conf = upper_pattern_char_str.detect_replace_confusable();

            let mut value_found = false;

            loop {
                let input_char = match input_iterator.next() {
                    Some(x) => x,
                    None => break,
                };

                let input_char_str = input_char.to_string();
                let input_char_str_conf = input_char_str.detect_replace_confusable();
                let current_u16_index = u16_index;
                u16_index += input_char.len_utf16();

                if input_char_str_conf == pattern_char_str_conf
                    || input_char_str_conf == upper_pattern_char_str_conf
                {
                    pattern_indexes.push(current_u16_index);
                    value_found = true;
                    break;
                }
            }

            if !value_found {
                let last_ind = started_from_ind;
                entities.push(MessageEntity::spoiler(last_ind, u16_index - last_ind));

                return entities;
            }
        }

        let mut previous_offset = started_from_ind;
        for i in pattern_indexes.iter().map(|x| x.clone()) {
            let length = i - previous_offset;
            if length > 0 {
                entities.push(MessageEntity::spoiler(previous_offset, i - previous_offset));
            }

            previous_offset = i + 1;
        }

        pattern_indexes.clear();
    }
}

fn prepare_input_for_logging(input: &str, max_length: usize) -> String {
    let length = input.chars().count();
    let mut result = String::with_capacity(22);
    result.push('\'');
    result.push_str(String::from_iter(input.chars().take(max_length)).as_str());
    if length > max_length {
        result.push_str("...");
    }

    result.push('\'');
    if length > max_length {
        result.push_str(format!(" (length {length})").as_str());
    }

    result
}
