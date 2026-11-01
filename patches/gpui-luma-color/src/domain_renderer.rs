//! Shared RwLock holder for color slider / arc / ring domain-track renderers.

use std::sync::{Arc, RwLock};

use gpui_luma::infra::lock;

pub(crate) struct LockedDomain<D, C>
where
    D: ?Sized,
{
    delegate: Arc<RwLock<Arc<D>>>,
    context: Arc<RwLock<C>>,
}

impl<D, C> LockedDomain<D, C>
where
    D: ?Sized,
{
    pub(crate) fn new(delegate: Arc<D>, context: C) -> Self {
        Self { delegate: Arc::new(RwLock::new(delegate)), context: Arc::new(RwLock::new(context)) }
    }

    pub(crate) fn context(&self) -> C
    where
        C: Clone,
    {
        lock::read(&self.context).clone()
    }

    pub(crate) fn set_context(&self, context: C) {
        *lock::write(&self.context) = context;
    }

    pub(crate) fn set_delegate(&self, delegate: Arc<D>) {
        *lock::write(&self.delegate) = delegate;
    }

    pub(crate) fn shared_context(&self) -> Arc<RwLock<C>> {
        Arc::clone(&self.context)
    }

    pub(crate) fn with_context_mut<R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        f(&mut lock::write(&self.context))
    }

    pub(crate) fn read_delegate(&self) -> Arc<D> {
        lock::read(&self.delegate).clone()
    }

    #[cfg(test)]
    pub(crate) fn context_lock(&self) -> &RwLock<C> {
        &self.context
    }
}
