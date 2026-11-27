// ecodex: the monitor tool's registry and argument parsing. Session shutdown
// relies on `abort_all` to stop every armed watcher (and, through the task,
// its subprocess); a fold that drops that call leaks background connections.

use std::time::Duration;

use pretty_assertions::assert_eq;

use super::ArmMonitorOptions;
use super::MonitorEntry;
use super::MonitorRegistry;
use super::MonitorStream;

fn pending_entry() -> MonitorEntry {
    MonitorEntry {
        handle: tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }),
        command: vec!["tail".to_string(), "-F".to_string(), "log".to_string()],
        pattern: "ERROR".to_string(),
        persistent: true,
    }
}

#[tokio::test]
async fn kill_aborts_only_the_named_monitor() {
    let registry = MonitorRegistry::new();
    registry
        .inner
        .lock()
        .await
        .insert("a".to_string(), pending_entry());
    let survivor = pending_entry();
    let survivor_aborted = survivor.handle.abort_handle();
    registry
        .inner
        .lock()
        .await
        .insert("b".to_string(), survivor);

    assert!(registry.kill("a").await);
    assert!(!registry.kill("a").await, "a killed id no longer matches");
    assert_eq!(registry.len().await, 1);
    assert!(!survivor_aborted.is_finished());
}

#[tokio::test]
async fn abort_all_empties_the_registry_and_stops_every_watcher() {
    let registry = MonitorRegistry::new();
    let first = pending_entry();
    let second = pending_entry();
    let handles = [first.handle.abort_handle(), second.handle.abort_handle()];
    registry.inner.lock().await.insert("a".to_string(), first);
    registry.inner.lock().await.insert("b".to_string(), second);

    registry.abort_all().await;
    assert_eq!(registry.len().await, 0);

    // An aborted task finishes once the runtime polls it; bound the wait.
    for _ in 0..100 {
        if handles.iter().all(tokio::task::AbortHandle::is_finished) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    for handle in handles {
        assert!(handle.is_finished(), "every watcher task is aborted");
    }
}

#[test]
fn arm_options_default_to_one_shot_stdout() {
    let options: ArmMonitorOptions =
        serde_json::from_str(r#"{"command":["tail","-F","log"],"pattern":"ERROR"}"#)
            .expect("minimal options parse");

    assert!(!options.persistent);
    assert!(matches!(options.stream, MonitorStream::Stdout));
    assert_eq!(options.cwd, None);
}
