use crate::tg::{cmd, Bot};
use crate::util::prelude::*;
use crate::{db, Result, TgConfig};
use async_trait::async_trait;
use itertools::Itertools;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use teloxide::utils::command::BotCommands;
use teloxide::utils::markdown;

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename = "snake_case",
    description = "These commands are supported for the bot maintainer:"
)]
pub(crate) enum Cmd {
    #[command(description = "display this text.")]
    MaintainerHelp,

    #[command(description = "display version info")]
    Version,
}

pub(crate) struct HandleImp;

#[async_trait]
impl cmd::HandleImp<Cmd> for HandleImp {
    async fn handle_imp(&self, bot: &Bot, msg: &Message, _: &db::Repo, cmd: Cmd) -> Result {
        match cmd {
            Cmd::MaintainerHelp => {
                bot.reply_chunked(&msg, Cmd::descriptions().to_string())
                    .disable_web_page_preview(true)
                    .parse_mode(ParseMode::Html)
                    .await?;
            }
            Cmd::Version => {
                /// Generate the key-value pairs with vergen metadata
                macro_rules! vergen_meta {
                    ( $($meta_name:literal),* $(,)? ) => {
                        [$( ($meta_name, env!(concat!("VERGEN_", $meta_name))) ),*]
                    }
                }

                let meta = vergen_meta!(
                    "BUILD_SEMVER",
                    "BUILD_DATE",
                    "BUILD_TIME",
                    "GIT_BRANCH",
                    "GIT_COMMIT_DATE",
                    "GIT_COMMIT_TIME",
                    "GIT_SHA",
                    "RUSTC_CHANNEL",
                    "RUSTC_COMMIT_DATE",
                    "RUSTC_COMMIT_HASH",
                    "RUSTC_HOST_TRIPLE",
                    "RUSTC_LLVM_VERSION",
                    "RUSTC_SEMVER",
                    "CARGO_FEATURES",
                    "CARGO_PROFILE",
                    "CARGO_TARGET_TRIPLE",
                );

                let max_name_len = meta.iter().map(|(name, _)| name.len()).max().unwrap();

                let metadata = meta.iter().format_with("\n", |(name, val), f| {
                    let name = name.to_lowercase();
                    let kv = format!("{name:<0$} = {val}", max_name_len);
                    f(&markdown::escape(&kv))
                });

                let metadata = format!("```\n{metadata}\n```",);

                bot.reply_chunked(&msg, metadata).await?;
            }
        };

        Ok(())
    }
}

pub(crate) fn is_maintainer(cfg: TgConfig, msg: Message) -> bool {
    matches!(msg.from(), Some(sender) if sender.id == cfg.bot_maintainer)
}
