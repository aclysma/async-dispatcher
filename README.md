# async-dispatcher
An experiment to adapt [shred](https://github.com/slide-rs/shred) for use with async code.

The goal of this experiment is to allow asynchronous tasks to acquire resources. For example, if you wanted to
load data from disk or network and pass it to a shred resource, with this crate you could await an IO task and then
continue it with acquiring and using the resource that is going to receive the data.

```rust
tokio::spawn(
    tokio::fs::read("file.txt")
        .map_err(|err| warn!("File read failed: {}", err) )
        .and_then(move |data| {
            Dispatcher::create_future(&dispatcher, HandleFileReadComplete { data })
        }
    )
);
```

As this code is written there are significant advantages and disadvantages for using this approach vs. shred. I think
many of the disadvantages could be removed with more work, but likely there would remain some trade-off between the two.

## Usage

Complete examples here:
* [hello_world](https://github.com/aclysma/async-dispatcher/blob/master/examples/hello_world.rs) - Near-minimal example usage 
* [game_loop](https://github.com/aclysma/async-dispatcher/blob/master/examples/game_loop.rs) - A more typical usage with several system executed in sequence and parallel
* [async](https://github.com/aclysma/async-dispatcher/blob/master/examples/async.rs) - A simple example showing how using async code might work
 

An empty shred resource

```rust
struct ExampleResource;
```

A shred system to use data to update the resource

```rust
struct HandleFileReadComplete {
    data: Vec<u8>
}

impl<'a> shred::System<'a> for HandleFileReadComplete {
    type SystemData = (shred::WriteExpect<'a, ExampleResource>);

    fn run(&mut self, data: Self::SystemData) {
        // Code for using the file data to update ExampleResource goes here!
    }
}
```

Code to kick off a task that reads from a file, then executes the system against the resource

```rust
tokio::spawn(
    tokio::fs::read("file.txt")
        .map_err(|err| warn!("File read failed: {}", err) )
        .and_then(move |data| {
            Dispatcher::create_future(&dispatcher, HandleFileReadComplete { data })
        }
    )
);
```

## Advantages:

* **Lower latency for completion of async tasks:** Normally with shred, you might need to set up a queue and have something
pull from that queue once every frame. Much of the time this is fine, but sometimes, especially if multiple
"sync points", the frame delay this incurs is undesirable. This approach permits writing to the resource safely with
only the delay required to acquire the locks.

* **Responsive Dispatching:** Shred sets up the order of execution at startup and does not adjust in realtime. If a game
suddenly requires more time for a particular system (for example if a bunch of physics objects spawn, or we need to
stream in a bunch of new data) shred does not adjust the execution path. An async approach could potentially be more
flexible since as resources become available, any pending tasks will always try to acquire them.

## Disadvantages:

* **Increased overhead:** This approach introduces a short critical path for determining if resources can be dispatched. If
the units of work being dispatched were extremely short, the overhead of this approach might be significant.

* **Non-deterministic:** A queue-draining approach mentioned above actually has a tremendous benefit - that since the queued
work will always be processed at the same time during a frame, this avoid unreliable timing-based bugs.

## Future Work:

* **Ergonomics:** I had difficulty with using shred as-is with this approach due to lifetimes being present on some of the
types (like SystemData). This might become easier when language-level support for async is ready to use. I would also
like to find a way to not have to pass the dispatcher Arc around (for example if there was some sort of "context" that
tokio could pass around between tasks that could hold a ref to the dispatcher)

* **Reduce Allocations:** The major performance overhead with this approach is memory allocation. I allocated a
RequiredResources struct to get around lifetime issues of using SystemData directly. If a method could be found that
avoids needing to allocate this struct, this could potentially reduce overhead.

* **Support adding/removing resources at runtime:** As it is now, it isn't possible to add a resource once the main loop,
begins, and you should only use ReadExpect and WriteExpect. Ideally this would either be supported, or it should be
more difficult to accidentally use Read/Write (possibly this crate could export shred types, but omit Read/Write)

* **Responsive Scheduling:** Ideally we could detect which tasks have consistently been part of the critical path for
completing a frame in the past and make sure those tasks get the first chance at acquiring resources

* **Instrumentation:** Would be great to have some way to track what tasks are running and how long they are taking

## Contribution

All contributions are assumed to be dual-licensed under MIT/Apache-2.

## License

Distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).