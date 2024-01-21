#![cfg_attr(docsrs, feature(doc_cfg))]

use futures::{Future, StreamExt};
use tokio::{
    select, spawn,
    sync::watch::{channel, Receiver, Sender},
    task::{JoinHandle, JoinSet},
};
use tracing::{trace, warn};

// Stream

/// A stream.
pub trait Stream<ITEM> {
    /// Returns next item.
    fn next(&mut self) -> impl Future<Output = Option<ITEM>> + Send;
}

// StreamExt

impl<ITEM, STREAM: StreamExt<Item = ITEM> + Send + Unpin> Stream<ITEM> for STREAM {
    async fn next(&mut self) -> Option<ITEM> {
        self.next().await
    }
}

// KafkaStreamConsumerAdapter

/// An adapter used to make easier [`Stream`](Stream) implementation of [`StreamConsumer`](https://docs.rs/rdkafka/latest/rdkafka/consumer/struct.StreamConsumer.html).
#[cfg(feature = "kafka")]
#[cfg_attr(docrs, doc(cfg(feature = "kafka")))]
pub struct KafkaStreamConsumerAdapter(rdkafka::consumer::StreamConsumer);

#[cfg(feature = "kafka")]
impl KafkaStreamConsumerAdapter {
    /// Creates new adapter.
    pub fn new(csm: rdkafka::consumer::StreamConsumer) -> Self {
        Self(csm)
    }
}

#[cfg(feature = "kafka")]
impl AsMut<rdkafka::consumer::StreamConsumer> for KafkaStreamConsumerAdapter {
    fn as_mut(&mut self) -> &mut rdkafka::consumer::StreamConsumer {
        &mut self.0
    }
}

#[cfg(feature = "kafka")]
impl AsRef<rdkafka::consumer::StreamConsumer> for KafkaStreamConsumerAdapter {
    fn as_ref(&self) -> &rdkafka::consumer::StreamConsumer {
        &self.0
    }
}

#[cfg(feature = "kafka")]
impl std::ops::Deref for KafkaStreamConsumerAdapter {
    type Target = rdkafka::consumer::StreamConsumer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "kafka")]
impl std::ops::DerefMut for KafkaStreamConsumerAdapter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "kafka")]
impl std::borrow::Borrow<rdkafka::consumer::StreamConsumer> for KafkaStreamConsumerAdapter {
    fn borrow(&self) -> &rdkafka::consumer::StreamConsumer {
        &self.0
    }
}

#[cfg(feature = "kafka")]
impl std::borrow::BorrowMut<rdkafka::consumer::StreamConsumer> for KafkaStreamConsumerAdapter {
    fn borrow_mut(&mut self) -> &mut rdkafka::consumer::StreamConsumer {
        &mut self.0
    }
}

#[cfg(feature = "kafka")]
impl From<rdkafka::consumer::StreamConsumer> for KafkaStreamConsumerAdapter {
    fn from(csm: rdkafka::consumer::StreamConsumer) -> Self {
        Self(csm)
    }
}

#[cfg(feature = "kafka")]
impl Stream<rdkafka::error::KafkaResult<rdkafka::message::OwnedMessage>>
    for KafkaStreamConsumerAdapter
{
    async fn next(
        &mut self,
    ) -> Option<rdkafka::error::KafkaResult<rdkafka::message::OwnedMessage>> {
        Some(self.0.recv().await.map(|msg| msg.detach()))
    }
}

// StreamConsumerTask

/// A task that consumes stream in backgroup.
pub struct StreamConsumerTask {
    stop_tx: Sender<()>,
    task: JoinHandle<()>,
}

impl<'a> StreamConsumerTask {
    /// Starts a new task.
    pub fn start<ITEM, FUT: Future<Output = ()> + Send + 'static>(
        mut stream: impl Stream<ITEM> + Send + 'static,
        handle: impl Fn(ITEM, Receiver<()>) -> FUT + Send + 'static,
    ) -> Self {
        let (stop_tx, mut stop_rx) = channel(());
        let task = spawn(async move {
            let mut tasks = JoinSet::new();
            loop {
                select! {
                    item = stream.next() => {
                        match item {
                            Some(item) => {
                                trace!("spawning task to handle stream item");
                                tasks.spawn(handle(item, stop_rx.clone()));
                            }
                            None => {
                                trace!("stream is closed");
                                break;
                            },
                        }
                    }
                    _ = stop_rx.changed() => {
                        trace!("stop signal received");
                        break;
                    }
                }
            }
            trace!("waiting for running tasks");
            while let Some(res) = tasks.join_next().await {
                if let Err(err) = res {
                    warn!("failed to wait task: {err}");
                }
            }
            trace!("consumer stopped");
        });
        Self { stop_tx, task }
    }

    /// Stops a task and wait for its termination.
    pub async fn stop(self) {
        trace!("sending stop signal");
        if let Err(err) = self.stop_tx.send(()) {
            warn!("failed to send stop signal: {err}");
        }
        trace!("waiting for running task");
        if let Err(err) = self.task.await {
            warn!("failed to wait task: {err}");
        }
    }
}
