use std::sync::Arc;

use super::SessionTask;
use super::SessionTaskContext;
use crate::hook_runtime::run_post_compact_hooks;
use crate::hook_runtime::run_pre_compact_hooks;
use crate::session::turn_context::TurnContext;
use crate::state::TaskKind;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Default)]
pub(crate) struct CompactTask;

impl SessionTask for CompactTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Compact
    }

    fn span_name(&self) -> &'static str {
        "session_task.compact"
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        _cancellation_token: CancellationToken,
    ) -> Option<String> {
        let session = session.clone_session();

        // ecodex addition (goal f0004294): determine which compaction
        // implementation will run so PreCompact/PostCompact payloads
        // carry the right compact_type. Three paths share this entry.
        let compact_type: &'static str =
            if crate::compact::should_use_remote_compact_task(ctx.provider.info()) {
                if ctx
                    .features
                    .enabled(codex_features::Feature::RemoteCompactionV2)
                {
                    "remote_v2"
                } else {
                    "remote"
                }
            } else {
                "local"
            };

        // ecodex addition (goal f0004294): PreCompact fires synchronously
        // — the .await blocks compaction until plugin handlers finish
        // their snapshot work (~/.empirica/breadcrumbs writes).
        run_pre_compact_hooks(&session, &ctx, compact_type).await;

        let compact_result = if compact_type == "local" {
            session.services.session_telemetry.counter(
                "codex.task.compact",
                /*inc*/ 1,
                &[("type", "local")],
            );
            crate::compact::run_compact_task(session.clone(), ctx.clone(), input).await
        } else {
            session.services.session_telemetry.counter(
                "codex.task.compact",
                /*inc*/ 1,
                &[("type", "remote")],
            );
            if compact_type == "remote_v2" {
                crate::compact_remote_v2::run_remote_compact_task(session.clone(), ctx.clone())
                    .await
            } else {
                crate::compact_remote::run_remote_compact_task(session.clone(), ctx.clone()).await
            }
        };

        // ecodex addition (goal f0004294): PostCompact fires informationally
        // with success flag so plugin handlers know whether to restore
        // breadcrumbs (success=true) or surface a warning (success=false).
        run_post_compact_hooks(&session, &ctx, compact_type, compact_result.is_ok()).await;

        None
    }
}
