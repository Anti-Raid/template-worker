use std::fmt::Debug;

/// A static reference to a type `T` that is guaranteed to be valid for the lifetime of the program.
/// 
/// This is useful in workers as workers should live until the program exits
#[derive(Clone)]
#[allow(dead_code)]
pub struct StaticRef<T: 'static> {
    data: &'static T,
    _phantom: std::marker::PhantomData<*const ()>,
}

#[allow(dead_code)]
impl<T: 'static> StaticRef<T> {
    /// Creates a new StaticRef from a static reference
    pub const fn from_static(data: &'static T) -> Self {
        Self {
            data,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Creates a new StaticRef by Box::leak
    pub fn new(data: T) -> Self {
        let boxed = Box::new(data);
        let static_ref: &'static T = Box::leak(boxed);
        Self::from_static(static_ref)
    }
}

impl<T: Debug + 'static> Debug for StaticRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("StaticRef").field(&self.data).finish()
    }
}

impl<T: 'static> std::ops::Deref for StaticRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

// While it *may* be safe, we still forbid it to allow distinguishing between
// thread safe and thread unsafe locations so we can then use a feature flag
// to switch between StaticRef/StaticRefSend and Rc/Arc


/// A static reference to a type `T` that is guaranteed to be valid for the lifetime of the program.
/// 
/// This is useful in workers as workers should live until the program exits
#[derive(Clone)]
#[allow(dead_code)]
pub struct SyncStaticRef<T: 'static> {
    data: &'static T,
}

#[allow(dead_code)]
impl<T: 'static> SyncStaticRef<T> {
    /// Creates a new SyncStaticRef from a static reference
    pub const fn from_static(data: &'static T) -> Self {
        Self {
            data,
        }
    }

    /// Creates a new SyncStaticRef by Box::leak
    pub fn new(data: T) -> Self {
        let boxed = Box::new(data);
        let static_ref: &'static T = Box::leak(boxed);
        Self::from_static(static_ref)
    }
}

impl<T: Debug + 'static> Debug for SyncStaticRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SyncStaticRef").field(&self.data).finish()
    }
}

impl<T: 'static> std::ops::Deref for SyncStaticRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}