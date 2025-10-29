use chrono::Utc;
use cubby_events::{
    disable_pipeline_tracing, emit_pipeline_trace, enable_pipeline_tracing,
    pipeline_tracing_enabled, subscribe_to_event, PipelineStage, PipelineTraceAggregator,
    PipelineTraceEvent, StageStatus, PIPELINE_TRACE_EVENT,
};
use futures::StreamExt;
use serial_test::serial;
use tokio::time::{sleep, Duration};

#[tokio::test]
#[serial]
async fn pipeline_trace_disabled_drops_events() {
    disable_pipeline_tracing();
    assert!(!pipeline_tracing_enabled());

    let mut subscription = subscribe_to_event::<PipelineTraceEvent>(PIPELINE_TRACE_EVENT);

    emit_pipeline_trace(PipelineTraceEvent {
        frame_number: Some(1),
        frame_id: Some(1),
        window: None,
        app: None,
        stage: PipelineStage::Ocr,
        status: StageStatus::Completed,
        started_at: Utc::now(),
        finished_at: Some(Utc::now()),
        duration_ms: Some(1),
        extra: serde_json::Value::Null,
    });

    let recv = tokio::time::timeout(Duration::from_millis(50), subscription.next()).await;
    assert!(recv.is_err(), "expected no event when tracing disabled");
}

#[tokio::test]
#[serial]
async fn pipeline_trace_aggregator_collects_events() {
    enable_pipeline_tracing();
    let (aggregator, handle) = PipelineTraceAggregator::start(Duration::from_secs(60));

    sleep(Duration::from_millis(10)).await;

    emit_pipeline_trace(PipelineTraceEvent {
        frame_number: Some(42),
        frame_id: Some(99),
        window: Some("Window".to_string()),
        app: Some("App".to_string()),
        stage: PipelineStage::Capture,
        status: StageStatus::Completed,
        started_at: Utc::now(),
        finished_at: Some(Utc::now()),
        duration_ms: Some(12),
        extra: serde_json::json!({ "test": true }),
    });

    let mut snapshot = None;
    for _ in 0..10 {
        let frames = aggregator.snapshot().await;
        if let Some(frame) = frames.into_iter().next() {
            snapshot = Some(frame);
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    let snapshot = snapshot.expect("aggregator collected event");
    assert_eq!(snapshot.frame_number, Some(42));
    assert_eq!(snapshot.frame_id, Some(99));

    let capture_events = snapshot
        .stages
        .get(&PipelineStage::Capture)
        .expect("capture stage present");
    assert_eq!(capture_events.len(), 1);
    assert_eq!(capture_events[0].status, StageStatus::Completed);

    handle.abort();
    disable_pipeline_tracing();
}
