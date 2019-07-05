use hashbrown::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use shred::ResourceId;

// This allows the user to add all the resources that will be used during execution
pub struct DispatcherBuilder {
    world: shred::World,
    resource_locks: HashMap<ResourceId, tokio::sync::lock::Lock<()>>,
}

impl DispatcherBuilder {
    // Create an empty dispatcher builder
    pub fn new() -> Self {
        DispatcherBuilder {
            world: shred::World::empty(),
            resource_locks: HashMap::new(),
        }
    }

    // Insert a resource that will be available once the dispatcher is running. This will create
    // locks for each resource to be used during dispatch
    pub fn insert<R>(mut self, r: R) -> Self
    where
        R: shred::Resource,
    {
        let resource_id = ResourceId::new::<R>();
        // We could possibly do this just-in-time since we global lock to dispatch anyways, but
        // it would require wrapping in an RwLock so that we can get a mut ref
        self.resource_locks
            .insert(resource_id.clone(), tokio::sync::lock::Lock::new(()));

        self.world.insert_by_id(resource_id, r);
        self
    }

    // Create the dispatcher
    pub fn build(self) -> Dispatcher {
        return Dispatcher {
            next_task_id: std::sync::atomic::AtomicUsize::new(0),
            world: Arc::new(self.world),
            dispatch_lock: tokio::sync::lock::Lock::new(()),
            resource_locks: self.resource_locks,
            should_terminate: std::sync::atomic::AtomicBool::new(false),
        };
    }
}

// Create using DispatcherBuilder. This keeps track of which tasks are wanting to read/write to
// the shred world and provides locks to them in a way that does not deadlock. This is done
// by only allowing a single task to try to acquire locks at the same time. If a task fails to
// acquire a task, it drops any locks it has already acquired and awaits the lock it couldn't get.
// This way it's not blocking any other tasks that are able to proceed, and it's not spinning while
// it's waiting.
pub struct Dispatcher {
    next_task_id: std::sync::atomic::AtomicUsize,
    world: Arc<shred::World>,
    dispatch_lock: tokio::sync::lock::Lock<()>,
    //TODO: Change this to a RwLock, but waiting until I have something more "real" to test with
    resource_locks: HashMap<ResourceId, tokio::sync::lock::Lock<()>>,
    should_terminate: std::sync::atomic::AtomicBool,
}

impl Dispatcher {
    pub(super) fn dispatch_lock(&self) -> &tokio::sync::lock::Lock<()> {
        &self.dispatch_lock
    }

    pub(super) fn resource_locks(&self) -> &HashMap<ResourceId, tokio::sync::lock::Lock<()>> {
        &self.resource_locks
    }

    pub(super) fn take_task_id(&self) -> usize {
        // Relaxed because we only care that every call of this function returns a different value,
        // we don't care about the ordering
        self.next_task_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn end_game_loop(&self) {
        self.should_terminate.swap(true, Ordering::Release);
    }

    // Call this to kick off processing.
    pub fn enter_game_loop<F, FutureT>(self, f: F) -> shred::World
    where
        F: Fn(Arc<Dispatcher>) -> FutureT + Send + Sync + Copy + 'static,
        FutureT: futures::future::Future<Item = (), Error = ()> + Send + 'static,
    {
        // Put the dispatcher in an Arc so it can be shared among tasks
        let dispatcher = Arc::new(self);

        let dispatcher_clone = dispatcher.clone();

        let loop_future = futures::future::loop_fn((), move |_| {
            // This clone is so that we can pass it to the inner closure
            let dispatcher_clone2 = dispatcher_clone.clone();

            // Get a future that represents this frame's work
            (f.clone())(dispatcher_clone.clone()).map(move |_| {
                return if dispatcher_clone2.should_terminate.load(Ordering::Acquire) {
                    futures::future::Loop::Break(())
                } else {
                    futures::future::Loop::Continue(())
                };
            })
        });

        // Kick off the process
        debug!("Calling tokio run");
        tokio::run(loop_future);

        // After execution ends, unwrap the dispatcher arc
        let dispatcher = Arc::try_unwrap(dispatcher).unwrap_or_else(|_| {
            unreachable!();
        });

        // Then unwrap the world inside it
        let world = Arc::try_unwrap(dispatcher.world).unwrap_or_else(|_| {
            unreachable!();
        });

        // Return the world
        world
    }

    pub fn run_system<T>(&self, mut system: T) -> T
    where
        T: for<'b> shred::System<'b> + Send + 'static,
    {
        use shred::RunNow;
        system.run_now(&self.world);
        system
    }

    // Queues up a system to run. This code will acquire the appropriate resources first, then
    // run the given system
    pub fn create_future_with_result<T>(
        dispatcher: &Arc<Dispatcher>,
        system: T,
    ) -> Box<impl futures::Future<Item = T, Error = ()>>
    where
        T: for<'b> shred::System<'b> + Send + 'static,
    {
        let dispatcher = dispatcher.clone();
        let required_resources = super::RequiredResources::from_system(&system);
        use futures::Future;
        Box::new(
            super::AcquireResources::<T>::new(dispatcher.clone(), required_resources).and_then(
                move |_result| {
                    let system = dispatcher.run_system(system);
                    Ok(system)
                },
            ),
        )
    }

    // Queues up a system to run. This code will acquire the appropriate resources first, then
    // run the given system
    pub fn create_future<T>(
        dispatcher: &Arc<Dispatcher>,
        system: T,
    ) -> Box<impl futures::Future<Item = (), Error = ()>>
    where
        T: for<'b> shred::System<'b> + Send + 'static,
    {
        use futures::future::Future;
        Box::new(Dispatcher::create_future_with_result(dispatcher, system).map(|_| ()))
    }
}
