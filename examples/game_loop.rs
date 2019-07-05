// This is a trivial example that sets up a few resources and systems, and then dispatches them.
// It shows the general setup for a main loop with parallel and sequential stages

#[macro_use]
extern crate log;

use std::sync::Arc;

use async_dispatcher::{Dispatcher, DispatcherBuilder, ExecuteParallel, ExecuteSequential};

#[derive(Debug)]
struct MyResourceA {
    value: i32,
}

impl MyResourceA {
    fn new(value: i32) -> Self {
        MyResourceA { value }
    }
}

#[derive(Debug)]
struct MyResourceB {
    value: i32,
}

impl MyResourceB {
    fn new() -> Self {
        MyResourceB { value: 0 }
    }
}

#[derive(Debug)]
struct PrintSystems;
impl<'a> shred::System<'a> for PrintSystems {
    type SystemData = (
        shred::ReadExpect<'a, MyResourceA>,
        shred::WriteExpect<'a, MyResourceB>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (a, b) = data;

        info!("PrintSystem {:?} {:?}", &*a, &*b);
        //std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[derive(Debug)]
struct IncrementResourceBWithA;
impl<'a> shred::System<'a> for IncrementResourceBWithA {
    type SystemData = (
        shred::ReadExpect<'a, MyResourceA>,
        shred::WriteExpect<'a, MyResourceB>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (a, mut b) = data;
        b.value += a.value;

        if (b.value % 50000) == 0 {
            info!("IncrementSystem {:?} {:?}", *a, *b);
        }
        //std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[derive(Debug)]
struct IncrementResourceBWithValue {
    value: i32,
}
impl<'a> shred::System<'a> for IncrementResourceBWithValue {
    type SystemData = (
        shred::ReadExpect<'a, MyResourceA>,
        shred::WriteExpect<'a, MyResourceB>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (_a, mut b) = data;
        b.value += self.value;

        //info!("SystemWithData {:?} {:?}", &*a, &*b);
        //std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

struct TerminateIfIncrementResourceBHighEnough {
    value: i32,
    dispatcher: Arc<Dispatcher>,
}
impl<'a> shred::System<'a> for TerminateIfIncrementResourceBHighEnough {
    type SystemData = (shred::ReadExpect<'a, MyResourceB>);

    fn run(&mut self, data: Self::SystemData) {
        let b = data;

        if b.value > self.value {
            self.dispatcher.end_game_loop();
        }
    }
}

fn main() {
    // Set up logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        //.filter_module("tokio", log::LevelFilter::Trace)
        //.filter_module("async_dispatcher", log::LevelFilter::Trace)
        .init();

    // Populate resources
    let dispatcher = DispatcherBuilder::new()
        .insert(MyResourceA::new(1))
        .insert(MyResourceB::new())
        .build();

    let world = dispatcher.enter_game_loop(|dispatcher| {
        ExecuteSequential::new(vec![
            // These will happen in sequence
            Dispatcher::create_future(&dispatcher, PrintSystems),
            Dispatcher::create_future(&dispatcher, IncrementResourceBWithA),
            Dispatcher::create_future(&dispatcher, IncrementResourceBWithValue { value: 5 }),
            Dispatcher::create_future(&dispatcher, PrintSystems),
            // A few things in parallel
            Box::new(ExecuteParallel::new(vec![
                Dispatcher::create_future(&dispatcher, PrintSystems),
                Dispatcher::create_future(&dispatcher, PrintSystems),
                Dispatcher::create_future(&dispatcher, PrintSystems),
            ])),
            // Then finish the sequence
            Dispatcher::create_future(&dispatcher, PrintSystems),
            Dispatcher::create_future(
                &dispatcher,
                TerminateIfIncrementResourceBHighEnough {
                    value: 10000,
                    dispatcher: dispatcher.clone(),
                },
            ),
        ])
    });

    // At the end, print results
    info!(
        "MyResource1: {} MyResource2: {}",
        world.fetch::<MyResourceA>().value,
        world.fetch::<MyResourceB>().value
    );
}
