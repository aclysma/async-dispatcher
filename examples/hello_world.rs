// Near-minimal example of using this crate.

use std::sync::Arc;

use async_dispatcher::Dispatcher;
use async_dispatcher::DispatcherBuilder;

struct HelloWorldResourceA {
    value: i32,
}

struct HelloWorldResourceB {
    value: i32,
}

struct HelloWorldSystem {
    dispatcher: Arc<Dispatcher>,
}

impl<'a> shred::System<'a> for HelloWorldSystem {
    type SystemData = (
        shred::ReadExpect<'a, HelloWorldResourceA>,
        shred::WriteExpect<'a, HelloWorldResourceB>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (a, mut b) = data;

        println!("Hello World a: {:?} b: {:?}", a.value, b.value);
        b.value += 1;

        if b.value > 20 {
            self.dispatcher.end_game_loop();
        }
    }
}

fn main() {
    // Populate resources
    let dispatcher = DispatcherBuilder::new()
        .insert(HelloWorldResourceA { value: 5 })
        .insert(HelloWorldResourceB { value: 10 })
        .build();

    let _world = dispatcher.enter_game_loop(|dispatcher| {
        // These will happen in sequence
        Dispatcher::create_future(
            &dispatcher,
            HelloWorldSystem {
                dispatcher: dispatcher.clone(),
            },
        )
    });
}
