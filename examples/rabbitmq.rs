use std::time::Duration;

use lapin::Connection;
use stream_consumer_task::StreamConsumerTask;
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
    let queue_name = "test";
    let conn = Connection::connect("amqp://127.0.0.1:5672/%2f", Default::default()).await?;
    let chan_csm = conn.create_channel().await?;
    chan_csm
        .queue_declare(queue_name, Default::default(), Default::default())
        .await?;
    let csm = chan_csm
        .basic_consume(queue_name, "", Default::default(), Default::default())
        .await?;
    let csm = StreamConsumerTask::start(csm, |delivery, mut stop_rx| async move {
        match delivery {
            Ok(delivery) => {
                let res: anyhow::Result<()> = async {
                    let payload = String::from_utf8(delivery.data.clone())?;
                    select! {
                        _ = sleep(Duration::from_secs(5)) => {
                            println!("{payload} - processed");
                            delivery.ack(Default::default()).await?;
                        }
                        _ = stop_rx.changed() => {
                            println!("{payload} - stopped");
                        }
                    }
                    Ok(())
                }
                .await;
                if let Err(err) = res {
                    eprintln!("failed to handle delivery: {err}");
                }
            }
            Err(err) => {
                eprintln!("failed to handle delivery: {err}");
            }
        }
    });
    let chan_prd = conn.create_channel().await?;
    let mut idx = 0;
    let _task = spawn(async move {
        loop {
            let payload = format!("msg_{idx:0>3}");
            let res = chan_prd
                .basic_publish(
                    "",
                    queue_name,
                    Default::default(),
                    payload.as_bytes(),
                    Default::default(),
                )
                .await;
            match res {
                Ok(_) => println!("{payload} - sent"),
                Err(err) => eprintln!("failed to send payload: {err}"),
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
