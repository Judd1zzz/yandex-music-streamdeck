use std::sync::{Arc, RwLock};
use std::time::Duration;

use sd_protocol::Outbound;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use ym_model::{ControlMode, PluginSettings, StateEvent};

use crate::action::{interest_of, Action, ActionCtx, Shared};

const SETTINGS_DEBOUNCE: Duration = Duration::from_millis(250);
const CMD_CHANNEL_CAP: usize = 64;

#[allow(clippy::large_enum_variant)]
pub enum ActorMsg {
    Appear,
    KeyDown,
    KeyUp,
    DialRotate(i32),
    DialDown,
    DialUp,
    PiAppear,
    Health,
    Tick,
    Settings(PluginSettings),
    ApplySettings(serde_json::Value),
}

pub struct ActorHandle {
    pub cmd_tx: mpsc::Sender<ActorMsg>,
    pub cancel: CancellationToken,
    pub control_mode: ControlMode,
    pub task: JoinHandle<()>,
}

pub fn spawn_actor(
    action: Box<dyn Action>,
    context: String,
    uuid: String,
    settings: PluginSettings,
    host: mpsc::Sender<Outbound>,
    shared: Arc<Shared>,
) -> ActorHandle {
    let (cmd_tx, cmd_rx) = mpsc::channel::<ActorMsg>(CMD_CHANNEL_CAP);
    let cancel = CancellationToken::new();
    let control_mode = settings.control_mode;
    let ctx = ActionCtx::new(
        context,
        uuid,
        host,
        Arc::new(RwLock::new(settings)),
        cancel.clone(),
        shared,
        cmd_tx.clone(),
    );
    let task = tokio::spawn(actor_loop(action, ctx, cmd_rx));
    ActorHandle { cmd_tx, cancel, control_mode, task }
}

async fn wait_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(t) => tokio::time::sleep_until(t).await,
        None => std::future::pending::<()>().await,
    }
}

async fn actor_loop(mut action: Box<dyn Action>, ctx: ActionCtx, mut cmd_rx: mpsc::Receiver<ActorMsg>) {
    let interests = action.interests();
    let mut state_rx = ctx.shared.subscribe();
    let mut state_closed = false;
    let mut save_deadline: Option<Instant> = None;

    loop {
        tokio::select! {
            biased;

            _ = ctx.cancel.cancelled() => {
                if save_deadline.take().is_some() {
                    let payload = ctx.settings().to_value();
                    ctx.send(Outbound::SetSettings { context: ctx.context.clone(), payload }).await;
                }
                action.on_disappear(&ctx).await;
                break;
            }

            msg = cmd_rx.recv() => {
                let Some(msg) = msg else { break };
                match msg {
                    ActorMsg::Appear => action.on_appear(&ctx).await,
                    ActorMsg::KeyDown => action.on_key_down(&ctx).await,
                    ActorMsg::KeyUp => action.on_key_up(&ctx).await,
                    ActorMsg::DialRotate(t) => action.on_dial_rotate(&ctx, t).await,
                    ActorMsg::DialDown => action.on_dial_down(&ctx).await,
                    ActorMsg::DialUp => action.on_dial_up(&ctx).await,
                    ActorMsg::PiAppear => action.on_pi_appear(&ctx).await,
                    ActorMsg::Health => action.on_health(&ctx).await,
                    ActorMsg::Tick => action.on_tick(&ctx).await,
                    ActorMsg::Settings(s) => {
                        ctx.set_settings_local(s);
                        action.on_settings(&ctx).await;
                    }
                    ActorMsg::ApplySettings(patch) => {
                        let mut merged = ctx.settings().to_value();
                        if let (Some(obj), Some(p)) = (merged.as_object_mut(), patch.as_object()) {
                            for (k, v) in p {
                                obj.insert(k.clone(), v.clone());
                            }
                        }
                        ctx.set_settings_local(PluginSettings::from_value(&merged));
                        action.on_settings(&ctx).await;
                        save_deadline = Some(Instant::now() + SETTINGS_DEBOUNCE);
                    }
                }
            }

            ev = state_rx.recv(), if !state_closed => match ev {
                Ok(ev) => {
                    if interests.contains(interest_of(ev.kind())) {
                        action.on_state(&ctx, &ev).await;
                    }
                    if matches!(ev, StateEvent::Connection(_)) {
                        ctx.report_status().await;
                    }
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => state_closed = true,
            },

            () = wait_deadline(save_deadline) => {
                let payload = ctx.settings().to_value();
                ctx.send(Outbound::SetSettings { context: ctx.context.clone(), payload }).await;
                save_deadline = None;
            }
        }
    }
}
