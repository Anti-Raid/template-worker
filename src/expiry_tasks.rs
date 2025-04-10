use antiraid_types::ar_event::AntiraidEvent;
use silverpelt::data::Data;
use std::time::Duration;

use crate::{
    dispatch::{dispatch_to_template, parse_event},
};
use crate::templatingrt::cache::{remove_scheduled_execution, get_all_expired_scheduled_executions};

const EXPIRY_TICK_TIME: Duration = Duration::from_secs(5);

pub async fn scheduled_executions_task(ctx: serenity::all::client::Context) -> ! {
    pub async fn event_listener(
        guild_id: serenity::all::GuildId,
        template_name: String,
        data: &Data,
        event: AntiraidEvent,
        serenity_context: &serenity::all::Context,
    ) -> Result<(), silverpelt::Error> {
        let tevent = parse_event(&event)?;

        dispatch_to_template(serenity_context, data, tevent, guild_id, template_name).await?;

        Ok(())
    }

    let data = ctx.data::<silverpelt::data::Data>();
    let mut set = tokio::task::JoinSet::new();
    loop {
        for (guild_id, expired_task) in get_all_expired_scheduled_executions() {
            let event = AntiraidEvent::ScheduledExecution(antiraid_types::ar_event::ScheduledExecutionEventData {
                id: expired_task.id.clone(),
                data: expired_task.data.clone(),
                run_at: expired_task.run_at,
            });

            let ctx = ctx.clone();
            let data = data.clone();

            set.spawn(async move {
                match event_listener(guild_id, expired_task.template_name.clone(), &data, event, &ctx).await {
                    Ok(_) => {
                        match remove_scheduled_execution(guild_id, &expired_task.id, &data.pool).await {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error removing scheduled execution: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error in scheduled_executions_task: {:?}", e);
                    }
                }
            });
        }

        tokio::time::sleep(EXPIRY_TICK_TIME).await;
    }
}