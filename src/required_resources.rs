use shred::ResourceId;
use std::marker::PhantomData;

// This is a helper that determines the reads/writes required for a system. I would have preferred
// not to need this structure at all, but many of the shred types require lifetimes that just
// don't play nicely with tasks. This gets rid of those lifetimes.
#[derive(Debug)]
pub struct RequiredResources<T> {
    pub(super) reads: Vec<ResourceId>,
    pub(super) writes: Vec<ResourceId>,
    phantom_data: PhantomData<T>,
}

impl<T> RequiredResources<T> {
    pub fn new(reads: Vec<ResourceId>, writes: Vec<ResourceId>) -> Self {
        RequiredResources {
            reads,
            writes,
            phantom_data: PhantomData,
        }
    }

    pub fn from_system(system: &T) -> Self
    where
        T: for<'b> shred::System<'b> + Send + 'static,
    {
        use shred::Accessor;
        let reads = system.accessor().reads();
        let writes = system.accessor().writes();

        RequiredResources::new(reads, writes)
    }
}
