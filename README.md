# stream-consumer-task

Start asynchronous task in background to consume stream.

It can be useful if your application consumes several RabbitMQ queues or Kafka Topics.

## Getting started

Add to your `Cargo.toml`:
```toml
[dependencies]
...
stream-consumer-task = "0"
...
```

You can look [examples](./examples/) to see how to use the library.

## Architecture

The task takes two arguments: a stream and an item handler.

Each time a new item is available in the stream, the consumer spawns a new task to handle it.

The item handler takes two arguments: the item from the stream and a [`Receiver<()>`](https://docs.rs/tokio/latest/tokio/sync/watch/struct.Receiver.html) to handle graceful shutdown.

If the consumer is stopped, a _stop signal_ is sent to all tasks and it will wait them termination.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).
