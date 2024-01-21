use std::time::Duration;

use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    producer::{FutureProducer, FutureRecord},
    ClientConfig, Message,
};
use stream_consumer_task::{KafkaStreamConsumerAdapter, StreamConsumerTask};
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
    spawn,
    task::JoinSet,
    time::sleep,
};

// main

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let brokers = "127.0.0.1:9092";
    let topic = "test";
    let csm: KafkaStreamConsumerAdapter = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", "test")
        .set("enable.auto.commit", "true")
        .create::<StreamConsumer>()?
        .into();
    csm.as_ref().subscribe(&[topic])?;
    let csm = StreamConsumerTask::start(csm, |msg, mut stop_rx| async move {
        match msg {
            Ok(msg) => {
                let res: anyhow::Result<()> = async {
                    let payload = msg.payload().map(Vec::from).unwrap_or_default();
                    let payload = String::from_utf8(payload)?;
                    select! {
                        _ = sleep(Duration::from_secs(5)) => {
                            println!("{payload} - processed");
                        }
                        _ = stop_rx.changed() => {
                            println!("{payload} - stopped");
                        }
                    }
                    Ok(())
                }
                .await;
                if let Err(err) = res {
                    eprintln!("failed to handle message: {err}");
                }
            }
            Err(err) => {
                eprintln!("failed to handle message: {err}");
            }
        }
    });
    let mut idx = 0;
    let prod: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .create()?;
    let _task = spawn(async move {
        loop {
            let payload = format!("msg_{idx:0>3}");
            let res = prod
                .send(
                    FutureRecord::to(topic)
                        .key(&format!("{idx}"))
                        .payload(&payload),
                    Duration::from_secs(0),
                )
                .await;
            match res {
                Ok(_) => println!("{payload} - sent"),
                Err((err, _)) => eprintln!("failed to send payload: {err}"),
            }
            sleep(Duration::from_secs(1)).await;
            idx += 1;
        }
    });
    let mut sig_int = signal(SignalKind::interrupt())?;
    let mut sig_term = signal(SignalKind::terminate())?;
    let mut sigs = JoinSet::new();
    sigs.spawn(async move {
        sig_int.recv().await;
    });
    sigs.spawn(async move {
        sig_term.recv().await;
    });
    sigs.join_next().await;
    csm.stop().await;
    Ok(())
}
