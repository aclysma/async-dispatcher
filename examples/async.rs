// This shows an example of kicking off an async task that, on completion, acquires locks to
// resources and uses them. This may block the main loop. (In a realistic scenario this block should
// be as short as possible, but for illustrative purposes, these tasks artificially block with
// std::thread::sleep calls.

#[macro_use]
extern crate log;

use std::sync::Arc;

use async_dispatcher::{Dispatcher, DispatcherBuilder, ExecuteSequential};

// A trivial resource that will be written to by the main loop via IncrementSystem and occasionally
// by HandleFileReadComplete which is an external task that reads a file
#[derive(Debug)]
struct ExampleResource {
    value: i32,
    last_time_started_file_read_task: std::time::Instant,
    last_data_read: Vec<u8>,
}

impl ExampleResource {
    fn new(value: i32) -> Self {
        ExampleResource {
            value,
            last_time_started_file_read_task: std::time::Instant::now(),
            last_data_read: vec![],
        }
    }
}

// This is queued to occur after reading a file, writes the data to ExampleResource
struct HandleFileReadComplete {
    data: Vec<u8>,
}

impl<'a> shred::System<'a> for HandleFileReadComplete {
    type SystemData = (shred::WriteExpect<'a, ExampleResource>);

    fn run(&mut self, data: Self::SystemData) {
        let mut a = data;

        info!("HandleFileReadComplete has the lock");

        info!("  Data saved to system, {} bytes", self.data.len());
        std::mem::swap(&mut a.last_data_read, &mut self.data);

        // make this last artificially long to make it more obvious that it's blocking other futures
        info!("  Sleep for 2000 ms");
        std::thread::sleep(std::time::Duration::from_millis(2000));

        info!("  HandleFileReadComplete is releasing the lock");
    }
}

// This is kicked off regularly by the main thread
struct IncrementSystem {
    dispatcher: Arc<Dispatcher>,
}

use futures::future::Future;
impl IncrementSystem {
    fn spawn_read_file_task(&mut self) {
        info!("  Going to kick off a read request");

        let dispatcher_clone = self.dispatcher.clone();
        tokio::spawn(
            tokio::fs::read("testfile.txt")
                .map_err(|err| warn!("File read failed: {}", err))
                .and_then(move |data| {
                    Dispatcher::create_future(&dispatcher_clone, HandleFileReadComplete { data })
                }),
        );
    }
}

impl<'a> shred::System<'a> for IncrementSystem {
    type SystemData = (shred::WriteExpect<'a, ExampleResource>);

    fn run(&mut self, data: Self::SystemData) {
        let mut a = data;

        info!("IncrementSystem has the lock");

        // Every second, consider kicking off a task to read data from disk, then apply the data
        // to the resource.
        let now = std::time::Instant::now();
        if now - a.last_time_started_file_read_task > std::time::Duration::from_millis(5000) {
            a.last_time_started_file_read_task = now;

            self.spawn_read_file_task();
        }

        a.value += 1;

        // make this last artificially long to make it more obvious that it's blocking other futures
        info!("  Sleep for 500 ms");
        std::thread::sleep(std::time::Duration::from_millis(500));

        info!("  IncrementSystem is releasing the lock");
    }
}

fn main() {
    // Set up logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        //.filter_module("tokio", log::LevelFilter::Trace)
        //.filter_module("async_dispatcher", log::LevelFilter::Trace)
        .init();

    // Populate resources
    let dispatcher = DispatcherBuilder::new()
        .insert(ExampleResource::new(1))
        .build();

    // Start a loop where we continuously increment ExampleResource
    let world = dispatcher.enter_game_loop(|dispatcher| {
        ExecuteSequential::new(vec![Dispatcher::create_future(
            &dispatcher,
            IncrementSystem {
                dispatcher: dispatcher.clone(),
            },
        )])
    });

    // At the end, print results
    info!(
        "ExampleResource: {}",
        world.fetch::<ExampleResource>().value
    );
}
