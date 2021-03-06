mod ftai;

use crate::tg;
use crate::util::prelude::*;
use crate::Result;
use async_trait::async_trait;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use teloxide::utils::markdown;

use self::ftai::FtaiCmd;

#[derive(BotCommands, Clone, Debug)]
#[command(rename = "snake_case", description = "Следующие команды доступны:")]
pub(crate) enum Cmd {
    #[command(description = "показать этот текст")]
    Help,

    #[command(description = "Сгенерировать аудио с помощью 15.ai: <персонаж>,<текст>")]
    Ftai(String),
}

#[async_trait]
impl tg::cmd::Command for Cmd {
    async fn handle(self, ctx: &tg::Ctx, msg: &Message) -> Result {
        match self {
            Cmd::Help => {
                ctx.bot
                    .reply_chunked(&msg, markdown::escape(&Cmd::descriptions().to_string()))
                    .disable_web_page_preview(false)
                    .await?;
            }
            Cmd::Ftai(cmd) => cmd.parse::<FtaiCmd>()?.handle(ctx, msg).await?,
        }
        Ok(())
    }
}
