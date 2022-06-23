use crate::tg::Bot;
use crate::util::prelude::*;
use crate::util::DynError;
use crate::Error;
use crate::Result;
use chrono::prelude::*;
use futures::prelude::*;
use once_cell::sync::Lazy as SyncLazy;
use parking_lot::Mutex as SyncMutex;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::ChatPermissions;
use teloxide::types::InlineKeyboardButton;
use teloxide::types::ReplyMarkup;
use teloxide::types::{InputFile, Message, MessageNewChatMembers};
use teloxide::utils::markdown;
use tokio::sync::oneshot;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::instrument;
use tracing::trace;
use tracing::warn;

/// Duration for the new users to solve the captcha. If they don't reply
/// in this time, they will be kicked.
const CAPTCHA_TIMEOUT: Duration = Duration::from_secs(60);
const CAPTCHA_DURATION_TEXT: &str = "1 минута";
const GREETING_ANIMATION_URL: &str = "https://derpicdn.net/img/2021/12/19/2767482/small.gif";

static UNVERIFIED_USERS: SyncLazy<SyncMutex<HashMap<UserId, oneshot::Sender<()>>>> =
    SyncLazy::new(Default::default);

#[derive(Serialize, Deserialize)]
struct CaptchaReplyPayload {
    expected_user_id: UserId,
    allowed: bool,
}

#[instrument(
    skip(bot, callback_query),
    fields(
        id = callback_query.id.as_str(),
        from = %callback_query.from.id,
        msg = callback_query.message.as_ref().and_then(|msg| msg.text()),
        data = callback_query.data.as_deref(),
    )
)]
pub(crate) async fn handle_callback_query(
    bot: Bot,
    callback_query: CallbackQuery,
) -> Result<(), Box<DynError>> {
    async {
        let callback_data = match callback_query.data {
            Some(data) => data,
            None => {
                warn!("Received empty callback data");
                return Ok(());
            }
        };

        let captcha_message = match callback_query.message {
            Some(message) => message,
            None => {
                warn!("Received empty callback message");
                return Ok(());
            }
        };

        let payload: CaptchaReplyPayload = serde_json::from_str(&callback_data).unwrap();

        if payload.expected_user_id != callback_query.from.id {
            info!(
                user_id = %callback_query.from.id,
                "User tried to reply to a capcha not meant for them",
            );
            return Ok(());
        }

        let user_id = callback_query.from.id;
        let chat_id = captcha_message.chat.id;

        if let Err(()) = UNVERIFIED_USERS.lock().remove(&user_id).unwrap().send(()) {
            warn!("Failed to cancel captcha time out (reciever dropped)");
            return Ok(());
        }

        bot.delete_message(chat_id, captcha_message.id).await?;

        if !payload.allowed {
            kick_user_due_to_captcha(&bot, chat_id, user_id).await?;
            return Ok(());
        }

        let default_perms = match bot.get_chat(chat_id).await?.permissions() {
            Some(perms) => perms,
            None => {
                warn!("Could not get default chat member permissions",);
                return Ok(());
            }
        };

        bot.restrict_chat_member(chat_id, user_id, default_perms)
            .await?;

        return Ok::<_, Error>(());
    }
    .await
    .map_err(Into::into)
}

pub(crate) async fn handle_new_chat_members(
    bot: Bot,
    msg: &Message,
    users: &MessageNewChatMembers,
) -> Result {
    async {
        let image_url: Url = GREETING_ANIMATION_URL.parse().unwrap();

        let futs = users.new_chat_members.iter().map(|user| async {
            let mention = user.md_link();
            let chat_id = msg.chat.id;
            let user_id = user.id;

            let caption = format!(
                "{}{}{}{}",
                mention,
                markdown::escape(
                    "\nHi, new friend! Привет, поняша :3\n\n\
                    Ответь на капчу: "
                ),
                "*Путин это кто?*",
                markdown::escape(&format!(
                    "\n\nУ тебя {CAPTCHA_DURATION_TEXT} на правильный ответ, иначе будешь кикнут.",
                ))
            );

            let payload_allow = CaptchaReplyPayload {
                expected_user_id: user_id,
                allowed: true,
            };

            let payload_deny = CaptchaReplyPayload {
                expected_user_id: user_id,
                allowed: false,
            };

            let payload_allow = serde_json::to_string(&payload_allow).unwrap();
            let payload_deny = serde_json::to_string(&payload_deny).unwrap();

            let buttons = [[
                InlineKeyboardButton::callback("Хуйло! 😉", payload_allow),
                InlineKeyboardButton::callback("Молодец (бан)! 🤨", payload_deny),
            ]];

            bot.restrict_chat_member(chat_id, user.id, ChatPermissions::empty())
                .await?;

            let captcha_message_id = bot
                .send_animation(chat_id, InputFile::url(image_url.clone()))
                .caption(caption)
                .reply_to_message_id(msg.id)
                .reply_markup(ReplyMarkup::inline_kb(buttons))
                .await?
                .id;

            let (send, recv) = oneshot::channel::<()>();

            let bot = bot.clone();
            tokio::spawn(async move {
                if let Ok(recv_result) = tokio::time::timeout(CAPTCHA_TIMEOUT, recv).await {
                    if let Err(err) = recv_result {
                        warn!("BUG: captcha confirmation timeout channel closed: {err:#?}");
                    } else {
                        trace!("Captcha confirmation timeout succefully cancelled");
                    }
                    return;
                }

                debug!(
                    captcha_timeout = format_args!("{CAPTCHA_TIMEOUT:.2?}"),
                    "Timed out waiting for captcha confirmation"
                );

                let (delete_message_result, kick_result) = futures::join!(
                    bot.delete_message(chat_id, captcha_message_id),
                    kick_user_due_to_captcha(&bot, chat_id, user_id)
                );

                if let Err(err) = delete_message_result {
                    error!("Failed to remove captcha message: {err:#?}");
                }

                if let Err(err) = kick_result {
                    error!("Failed to ban user due to captcha: {err:#?}");
                }
            });

            UNVERIFIED_USERS.lock().insert(user.id, send);

            Ok::<_, Error>(())
        });

        future::join_all(futs)
            .await
            .into_iter()
            .collect::<Result<Vec<()>>>()?;

        Ok::<_, Error>(())
    }
    .await
    .map_err(Into::into)
}

#[instrument(skip(bot))]
async fn kick_user_due_to_captcha(bot: &Bot, chat_id: ChatId, user_id: UserId) -> Result {
    let ban_timeout = Utc::now() + chrono::Duration::from_std(CAPTCHA_TIMEOUT).unwrap();

    debug!(until = ban_timeout.to_ymd_hms().as_str(), "Banning user");

    bot.kick_chat_member(chat_id, user_id)
        .until_date(ban_timeout)
        .await?;

    Ok(())
}