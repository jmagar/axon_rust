use super::super::*;
use lapin::options::{BasicAckOptions, BasicGetOptions};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// T-AMQP-1: open_amqp_connection_and_channel succeeds and declares a durable queue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn open_amqp_channel_connects_and_declares_durable_queue() -> Result<()> {
    let Some(amqp_url) = resolve_test_amqp_url() else {
        return Ok(());
    };

    let queue_name = format!("axon.test.amqp.{}", Uuid::new_v4().simple());
    let mut cfg = test_config("postgresql://dummy@127.0.0.1:1/dummy");
    cfg.amqp_url = amqp_url.clone();

    let result = open_amqp_connection_and_channel(&cfg, &queue_name).await;
    assert!(
        result.is_ok(),
        "open_amqp_connection_and_channel failed: {:?}",
        result.err()
    );

    let (conn, ch) = result.unwrap();
    let _ = ch.close(0, "").await;
    let _ = conn.close(200, "").await;

    Ok(())
}

// ---------------------------------------------------------------------------
// T-AMQP-2: batch_enqueue_jobs delivers messages to the queue, readable via basic_get
// ---------------------------------------------------------------------------

#[tokio::test]
async fn batch_enqueue_jobs_delivers_messages_to_queue() -> Result<()> {
    let Some(amqp_url) = resolve_test_amqp_url() else {
        return Ok(());
    };

    let queue_name = format!("axon.test.amqp.{}", Uuid::new_v4().simple());
    let mut cfg = test_config("postgresql://dummy@127.0.0.1:1/dummy");
    cfg.amqp_url = amqp_url.clone();

    // Open a connection+channel first so the queue is declared before publishing.
    // Keep conn alive — dropping it would close the channel mid-test.
    let (conn, ch) = open_amqp_connection_and_channel(&cfg, &queue_name).await?;

    // Publish 3 UUIDs. batch_enqueue_jobs opens its own connection, publishes,
    // and closes. The queue survives because it was declared durable above.
    let ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    batch_enqueue_jobs(&cfg, &queue_name, &ids).await?;

    // Consume the 3 messages using the channel we kept alive.
    let mut received_ids: Vec<String> = Vec::new();
    for i in 0..3 {
        let msg = ch
            .basic_get(&queue_name, BasicGetOptions::default())
            .await?;
        assert!(
            msg.is_some(),
            "expected message {i} but basic_get returned None"
        );
        let msg = msg.unwrap();
        let body = std::str::from_utf8(&msg.data)
            .expect("message body is not valid UTF-8")
            .to_string();
        msg.ack(BasicAckOptions::default()).await?;
        received_ids.push(body);
    }

    // 4th basic_get must return None — queue is now empty.
    let empty = ch
        .basic_get(&queue_name, BasicGetOptions::default())
        .await?;
    assert!(
        empty.is_none(),
        "expected empty queue after consuming all 3 messages, but got another message"
    );

    // Verify each received body is one of the published UUIDs.
    let published_strings: Vec<String> = ids.iter().map(|u| u.to_string()).collect();
    for received in &received_ids {
        assert!(
            published_strings.contains(received),
            "received unexpected UUID body: {received}"
        );
    }

    let _ = ch.close(0, "").await;
    let _ = conn.close(200, "").await;

    Ok(())
}

// ---------------------------------------------------------------------------
// T-AMQP-3: purge_queue_safe removes all enqueued messages
// ---------------------------------------------------------------------------

#[tokio::test]
async fn purge_queue_safe_removes_enqueued_messages() -> Result<()> {
    let Some(amqp_url) = resolve_test_amqp_url() else {
        return Ok(());
    };

    let queue_name = format!("axon.test.amqp.{}", Uuid::new_v4().simple());
    let mut cfg = test_config("postgresql://dummy@127.0.0.1:1/dummy");
    cfg.amqp_url = amqp_url.clone();

    // Publish 5 messages. batch_enqueue_jobs declares the queue and then closes
    // its own connection.
    let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    batch_enqueue_jobs(&cfg, &queue_name, &ids).await?;

    // Purge the queue via a fresh connection.
    purge_queue_safe(&cfg, &queue_name).await?;

    // Open yet another connection to verify the queue is empty.
    let (conn, ch) = open_amqp_connection_and_channel(&cfg, &queue_name).await?;
    let msg = ch
        .basic_get(&queue_name, BasicGetOptions::default())
        .await?;
    assert!(
        msg.is_none(),
        "expected empty queue after purge, but basic_get returned a message"
    );

    let _ = ch.close(0, "").await;
    let _ = conn.close(200, "").await;

    Ok(())
}

// ---------------------------------------------------------------------------
// T-AMQP-4: enqueue_job publishes a single message readable via basic_get
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enqueue_job_publishes_single_message() -> Result<()> {
    let Some(amqp_url) = resolve_test_amqp_url() else {
        return Ok(());
    };

    let queue_name = format!("axon.test.amqp.{}", Uuid::new_v4().simple());
    let mut cfg = test_config("postgresql://dummy@127.0.0.1:1/dummy");
    cfg.amqp_url = amqp_url.clone();

    let job_id = Uuid::new_v4();

    // enqueue_job declares the queue (via batch_enqueue_jobs → open_amqp_connection_and_channel),
    // publishes, waits for confirm, then closes.
    enqueue_job(&cfg, &queue_name, job_id).await?;

    // Open a fresh connection to read the message back.
    let (conn, ch) = open_amqp_connection_and_channel(&cfg, &queue_name).await?;
    let msg = ch
        .basic_get(&queue_name, BasicGetOptions::default())
        .await?;
    assert!(
        msg.is_some(),
        "expected one message from enqueue_job but basic_get returned None"
    );
    let msg = msg.unwrap();
    let body = std::str::from_utf8(&msg.data)
        .expect("message body is not valid UTF-8")
        .to_string();
    msg.ack(BasicAckOptions::default()).await?;

    assert_eq!(
        body,
        job_id.to_string(),
        "message body does not match the published job UUID"
    );

    let _ = ch.close(0, "").await;
    let _ = conn.close(200, "").await;

    Ok(())
}
