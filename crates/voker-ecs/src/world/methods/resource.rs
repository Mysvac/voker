use core::alloc::Layout;
use core::any::TypeId;

use voker_ptr::{OwningPtr, PtrMut};

use crate::borrow::{NonSendMut, NonSendRef, ResMut, ResRef, UntypedMut};
use crate::resource::{Resource, ResourceId};
use crate::tick::TicksMut;
use crate::utils::{DebugCheckedUnwrap, DebugName};
use crate::world::{FromWorld, World};

#[inline(never)]
fn insert_internal<'a, 'b>(
    this: &'a mut World,
    value: OwningPtr<'b>,
    id: ResourceId,
) -> PtrMut<'a> {
    unsafe {
        this.prepare_resource(id);
        let tick = this.this_run_fast(); // we have `full_mut` world
        let data = this.storages.res_set.get_unchecked_mut(id);
        data.insert_untyped(value, tick);
        data.get_data_mut().debug_checked_unwrap()
    }
}

#[cold]
#[track_caller]
#[inline(never)]
fn uninitialized_resource(name: DebugName) -> ! {
    panic!(
        "Requested resource {name} does not exist in the `World`.
        Did you forget to add it using `app.insert_resource` / `app.init_resource`?
        Resources are also implicitly added via `app.add_message`,
        and can be added by plugins."
    )
}

impl World {
    /// Returns whether a resource of type `T` is present and active.
    ///
    /// This only checks registration + active storage state. It does not create
    /// the resource and does not borrow it.
    ///
    /// Unlike methods such as `get_resource`, this function does not require
    /// `T: Send` / `T: Sync`, because it never returns a typed reference.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource)]
    /// struct Counter(u32);
    ///
    /// assert!(!world.contains_resource::<Counter>());
    /// world.insert_resource(Counter(1));
    /// assert!(world.contains_resource::<Counter>());
    /// ```
    pub fn contains_resource<T: Resource>(&self) -> bool {
        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get(id)
        {
            data.is_active()
        } else {
            false
        }
    }

    /// Returns whether a main-thread resource of type `T` is present and active.
    ///
    /// This check is metadata-only and does not enforce the main-thread access
    /// assertion used by `get_non_send`/`non_send`.
    ///
    /// In this storage model, `contains_resource` and `contains_non_send`
    /// inspect the same underlying resource slot and therefore currently report
    /// the same presence result.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource)]
    /// struct LocalOnly;
    ///
    /// assert!(!world.contains_non_send::<LocalOnly>());
    /// world.insert_non_send(LocalOnly);
    /// assert!(world.contains_non_send::<LocalOnly>());
    /// ```
    pub fn contains_non_send<T: Resource>(&self) -> bool {
        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get(id)
        {
            data.is_active()
        } else {
            false
        }
    }

    /// Inserts or replaces a `Send` resource and returns a mutable reference to it.
    ///
    /// The resource is registered by type on first use. Once inserted, it can be
    /// accessed from systems through [`Res`], [`ResRef`], or [`ResMut`].
    ///
    /// If the resource is `!Send`, use [`World::insert_non_send`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Counter(u64);
    ///
    /// assert_eq!(*world.insert_resource(Counter(1)), Counter(1));
    /// assert_eq!(*world.insert_resource(Counter(2)), Counter(2));
    /// assert_eq!(world.get_resource::<Counter>(), Some(&Counter(2)));
    /// ```
    ///
    /// [`Res`]: crate::borrow::Res
    pub fn insert_resource<T: Resource + Send>(&mut self, value: T) -> &mut T {
        let id = self.resources.register::<T>();
        voker_ptr::into_owning!(value);
        unsafe { insert_internal(self, value, id).deref::<T>() }
    }

    /// Removes and returns a `Send` resource if it exists.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Foo;
    ///
    /// world.insert_resource(Foo);
    /// assert_eq!(world.remove_resource::<Foo>(), Some(Foo));
    /// assert_eq!(world.remove_resource::<Foo>(), None);
    /// ```
    pub fn remove_resource<T: Resource + Send>(&mut self) -> Option<T> {
        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get_mut(id)
        {
            unsafe { data.remove() }
        } else {
            None
        }
    }

    /// Drop a `Send` resource if it exists.
    ///
    /// This will be faster than removing, as there is no need to return data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug)]
    /// struct Temp;
    ///
    /// world.insert_resource(Temp);
    /// world.drop_resource::<Temp>();
    /// assert!(world.get_resource::<Temp>().is_none());
    /// ```
    pub fn drop_resource<T: Resource + Send>(&mut self) {
        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get_mut(id)
        {
            unsafe {
                data.clear();
            }
        }
    }

    /// Returns a shared reference to a resource without change detection.
    ///
    /// This mirrors the behavior of the [`Res`](crate::borrow::Res) system parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Bar(u64);
    ///
    /// world.insert_resource(Bar(20));
    /// assert_eq!(world.get_resource::<Bar>(), Some(&Bar(20)));
    /// ```
    pub fn get_resource<T: Resource + Sync>(&self) -> Option<&T> {
        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get(id)
            && let Some(ptr) = data.get_data()
        {
            ptr.debug_assert_aligned::<T>();
            Some(unsafe { ptr.deref::<T>() })
        } else {
            None
        }
    }

    /// Returns a shared reference to a resource without change detection.
    ///
    /// Similar to `get_resource().unwrap()`. Use [`World::get_resource`]
    /// instead if you want to handle this case.
    ///
    /// # Panics
    ///
    /// Panics if the resource does not exist.
    pub fn resource<T: Resource + Sync>(&self) -> &T {
        self.get_resource()
            .unwrap_or_else(|| uninitialized_resource(DebugName::type_name::<T>()))
    }

    /// Returns a shared resource borrow with change detection.
    ///
    /// This mirrors the behavior of the [`ResRef`] system parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::tick::DetectChanges;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Bar(u64);
    ///
    /// world.insert_resource(Bar(20));
    /// let res = world.get_resource_ref::<Bar>().unwrap();
    /// assert!(res.is_added());
    /// assert!(res.is_changed());
    /// ```
    pub fn get_resource_ref<T: Resource + Sync>(&self) -> Option<ResRef<'_, T>> {
        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get(id)
        {
            let last_run = self.last_run();
            let this_run = self.this_run();
            let ptr = data.get_ref(last_run, this_run)?;
            Some(unsafe { ptr.into_resource::<T>() })
        } else {
            None
        }
    }

    /// Returns a shared resource borrow with change detection.
    ///
    /// Similar to `get_resource_ref().unwrap()`. Use [`World::get_resource_ref`]
    /// instead if you want to handle this case.
    ///
    /// # Panics
    ///
    /// Panics if the resource does not exist.
    pub fn resource_ref<T: Resource + Sync>(&self) -> ResRef<'_, T> {
        self.get_resource_ref()
            .unwrap_or_else(|| uninitialized_resource(DebugName::type_name::<T>()))
    }

    /// Returns an exclusive resource borrow with change detection.
    ///
    /// This mirrors the behavior of the [`ResMut`] system parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::tick::DetectChanges;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Bar(u64);
    ///
    /// world.insert_resource(Bar(20));
    /// let mut res = world.get_resource_mut::<Bar>().unwrap();
    /// *res = Bar(50);
    /// assert!(res.is_changed());
    /// ```
    pub fn get_resource_mut<T: Resource + Send>(&mut self) -> Option<ResMut<'_, T>> {
        // self is `data_mut`, instead of `full_mut`
        let this_run = self.this_run();
        let last_run = self.last_run();

        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get_mut(id)
        {
            let ptr = data.get_mut(last_run, this_run)?;
            Some(unsafe { ptr.into_resource::<T>() })
        } else {
            None
        }
    }

    /// Returns an exclusive resource borrow with change detection.
    ///
    /// Similar to `get_resource_mut().unwrap()`. Use [`World::get_resource_mut`]
    /// instead if you want to handle this case.
    ///
    /// # Panics
    ///
    /// Panics if the resource does not exist.
    pub fn resource_mut<T: Resource + Send>(&mut self) -> ResMut<'_, T> {
        self.get_resource_mut()
            .unwrap_or_else(|| uninitialized_resource(DebugName::type_name::<T>()))
    }

    /// Initialize resource if it does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Default, Debug)]
    /// struct Bar(bool);
    ///
    /// world.init_resource::<Bar>();
    ///
    /// assert_eq!(world.resource::<Bar>().0,  false);
    /// ```
    pub fn init_resource<T: Resource + Send + FromWorld>(&mut self) {
        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get(id)
            && data.is_active()
        {
            return;
        }
        let value = T::from_world(self);
        self.insert_resource::<T>(value);
    }

    /// Returns an exclusive resource borrow with change detection.
    ///
    /// If the resource does not exist, it will be automatically initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::tick::DetectChanges;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Default)]
    /// struct Bar(u64);
    ///
    /// let bar = world.resource_mut_or_init::<Bar>();
    /// assert_eq!(bar.0, 0);
    /// ```
    pub fn resource_mut_or_init<T: Resource + Send + FromWorld>(&mut self) -> ResMut<'_, T> {
        #[cold]
        #[inline(never)]
        fn get_or_init_cold<T: Resource + Send + FromWorld>(this: &mut World) -> ResMut<'_, T> {
            let id = this.register_resource::<T>();
            this.prepare_resource(id);

            let this_run = this.this_run_fast();
            let last_run = this.last_run();
            let value = T::from_world(this);

            unsafe {
                let data = this.storages.res_set.get_unchecked_mut(id);
                data.insert(value, this_run);
                data.get_mut(last_run, this_run)
                    .debug_checked_unwrap()
                    .into_resource()
            }
        }

        let last_run = self.last_run();
        let this_run = self.this_run_fast();
        let unsafe_world = self.unsafe_world();
        let world_mut = unsafe { unsafe_world.data_mut() };

        if let Some(id) = world_mut.get_resource_id::<T>()
            && let Some(data) = world_mut.storages.res_set.get_mut(id)
            && let Some(ptr) = data.get_mut(last_run, this_run)
        {
            unsafe { ptr.into_resource::<T>() }
        } else {
            let full_mut = unsafe { unsafe_world.full_mut() };
            get_or_init_cold::<T>(full_mut)
        }
    }

    /// Inserts or replaces a main-thread resource and returns a mutable reference to it.
    ///
    /// Unlike [`World::insert_resource`], this accepts `!Sync` values. Access to the
    /// resource is restricted to the thread that created the world.
    ///
    /// # Panics
    /// Panics if called from a thread other than the world's main thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct LocalCache(u64);
    ///
    /// world.insert_non_send(LocalCache(1));
    /// assert_eq!(world.get_non_send::<LocalCache>(), Some(&LocalCache(1)));
    /// ```
    pub fn insert_non_send<T: Resource>(&mut self, value: T) -> &mut T {
        assert! {
            self.thread_hash() == voker_os::thread::thread_hash(),
            "!Send Resource can only be inserted/removed on the main thread.",
        }

        // let id = self.register_resource::<T>();
        let id = self.resources.register::<T>();

        voker_ptr::into_owning!(value);
        unsafe { insert_internal(self, value, id).deref::<T>() }
    }

    /// Removes and returns a main-thread resource if it exists.
    ///
    /// # Panics
    /// Panics if called from a thread other than the world's main thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Foo;
    ///
    /// world.insert_non_send(Foo);
    /// assert_eq!(world.remove_non_send::<Foo>(), Some(Foo));
    /// assert_eq!(world.remove_non_send::<Foo>(), None);
    /// ```
    pub fn remove_non_send<T: Resource>(&mut self) -> Option<T> {
        assert! {
            self.thread_hash() == voker_os::thread::thread_hash(),
            "!Send Resource can only be inserted/removed on the main thread.",
        }

        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get_mut(id)
        {
            unsafe { data.remove() }
        } else {
            None
        }
    }

    /// Drop a resource if it exists.
    ///
    /// This will be faster than removing, as there is no need to return data.
    ///
    /// # Panics
    /// Panics if called from a thread other than the world's main thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug)]
    /// struct LocalTemp;
    ///
    /// world.insert_non_send(LocalTemp);
    /// world.drop_non_send::<LocalTemp>();
    /// assert!(world.get_non_send::<LocalTemp>().is_none());
    /// ```
    pub fn drop_non_send<T: Resource>(&mut self) {
        assert! {
            self.thread_hash() == voker_os::thread::thread_hash(),
            "!Send Resource can only be inserted/removed on the main thread.",
        }

        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get_mut(id)
        {
            unsafe { data.clear() }
        }
    }

    /// Returns a shared reference to a main-thread resource without change detection.
    ///
    /// # Panics
    /// Panics if called from a thread other than the world's main thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Bar(u64);
    ///
    /// world.insert_non_send(Bar(99));
    /// assert_eq!(world.get_non_send::<Bar>(), Some(&Bar(99)));
    /// ```
    pub fn get_non_send<T: Resource>(&mut self) -> Option<&T> {
        assert! {
            self.thread_hash() == voker_os::thread::thread_hash(),
            "!Sync Resource can only be accessed on the main thread.",
        }

        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get(id)
            && let Some(ptr) = data.get_data()
        {
            ptr.debug_assert_aligned::<T>();
            Some(unsafe { ptr.deref::<T>() })
        } else {
            None
        }
    }

    /// Returns a shared reference to a main-thread resource without change detection.
    ///
    /// Similar to `get_non_send().unwrap()`. Use [`World::get_non_send`]
    /// instead if you want to handle this case.
    ///
    /// # Panics
    /// - Panics if called from a thread other than the world's main thread.
    /// - Panics if the resource does not exist.
    pub fn non_send<T: Resource>(&mut self) -> &T {
        self.get_non_send::<T>()
            .unwrap_or_else(|| uninitialized_resource(DebugName::type_name::<T>()))
    }

    /// Returns a shared main-thread resource borrow with change detection.
    ///
    /// This mirrors the behavior of the [`NonSendRef`] system parameter.
    ///
    /// # Panics
    /// Panics if called from a thread other than the world's main thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::tick::DetectChanges;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Bar(u64);
    ///
    /// world.insert_non_send(Bar(7));
    /// let res = world.get_non_send_ref::<Bar>().unwrap();
    /// assert!(res.is_added());
    /// assert!(res.is_changed());
    /// ```
    pub fn get_non_send_ref<T: Resource>(&self) -> Option<NonSendRef<'_, T>> {
        assert! {
            self.thread_hash() == voker_os::thread::thread_hash(),
            "!Sync Resource can only be accessed on the main thread.",
        }

        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get(id)
        {
            let last_run = self.last_run();
            let this_run = self.this_run();
            let ptr = data.get_ref(last_run, this_run)?;
            Some(unsafe { ptr.into_non_send::<T>() })
        } else {
            None
        }
    }

    /// Returns a shared main-thread resource borrow with change detection.
    ///
    /// Similar to `get_non_send_ref().unwrap()`. Use [`World::get_non_send_ref`]
    /// instead if you want to handle this case.
    ///
    /// # Panics
    /// - Panics if called from a thread other than the world's main thread.
    /// - Panics if the resource does not exist.
    pub fn non_send_ref<T: Resource>(&mut self) -> NonSendRef<'_, T> {
        self.get_non_send_ref::<T>()
            .unwrap_or_else(|| uninitialized_resource(DebugName::type_name::<T>()))
    }

    /// Returns an exclusive main-thread resource borrow with change detection.
    ///
    /// This mirrors the behavior of the [`NonSendMut`] system parameter.
    ///
    /// # Panics
    /// Panics if called from a thread other than the world's main thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::tick::DetectChanges;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Debug, PartialEq, Eq)]
    /// struct Bar(u64);
    ///
    /// world.insert_non_send(Bar(7));
    /// let mut res = world.get_non_send_mut::<Bar>().unwrap();
    /// *res = Bar(8);
    /// assert!(res.is_changed());
    /// ```
    pub fn get_non_send_mut<T: Resource>(&mut self) -> Option<NonSendMut<'_, T>> {
        assert! {
            self.thread_hash() == voker_os::thread::thread_hash(),
            "!Send Resource can only be accessed mut on the main thread.",
        }
        // self is `data_mut`, instead of `full_mut`
        let this_run = self.this_run();
        let last_run = self.last_run();

        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get_mut(id)
        {
            let ptr = data.get_mut(last_run, this_run)?;
            Some(unsafe { ptr.into_non_send::<T>() })
        } else {
            None
        }
    }

    /// Returns an exclusive main-thread resource borrow with change detection.
    ///
    /// Similar to `get_non_send_mut().unwrap()`. Use [`World::get_non_send_mut`]
    /// instead if you want to handle this case.
    ///
    /// # Panics
    /// - Panics if called from a thread other than the world's main thread.
    /// - Panics if the resource does not exist.
    pub fn non_send_mut<T: Resource>(&mut self) -> NonSendMut<'_, T> {
        self.get_non_send_mut::<T>()
            .unwrap_or_else(|| uninitialized_resource(DebugName::type_name::<T>()))
    }

    /// Initialize resource if it does not exist.
    ///
    /// # Panics
    /// - Panics if called from a thread other than the world's main thread.
    pub fn init_non_send<T: Resource + FromWorld>(&mut self) {
        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get(id)
            && data.is_active()
        {
            return;
        }

        let value = T::from_world(self);
        self.insert_non_send::<T>(value);
    }

    /// Returns an exclusive resource borrow with change detection.
    ///
    /// If the resource does not exist, it will be automatically initialized.
    ///
    /// # Panics
    /// Panics if called from a thread other than the world's main thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::tick::DetectChanges;
    /// # use voker_ecs::world::World;
    /// # let mut world = World::alloc();
    /// #[derive(Resource, Default)]
    /// struct Bar(u64);
    ///
    /// let bar = world.non_send_mut_or_init::<Bar>();
    /// assert_eq!(bar.0, 0);
    /// ```
    pub fn non_send_mut_or_init<T: Resource + FromWorld>(&mut self) -> NonSendMut<'_, T> {
        #[cold]
        #[inline(never)]
        fn get_or_init_cold<T: Resource + FromWorld>(this: &mut World) -> NonSendMut<'_, T> {
            let id = this.register_resource::<T>();
            this.prepare_resource(id);
            let this_run = this.this_run_fast();
            let last_run = this.last_run();
            let value = T::from_world(this);

            unsafe {
                let data = this.storages.res_set.get_unchecked_mut(id);
                data.insert(value, this_run);
                data.get_mut(last_run, this_run)
                    .debug_checked_unwrap()
                    .into_non_send()
            }
        }

        assert! {
            self.thread_hash() == voker_os::thread::thread_hash(),
            "!Send Resource can only be accessed mut on the main thread.",
        }

        let last_run = self.last_run();
        let this_run = self.this_run_fast();
        let unsafe_world = self.unsafe_world();
        let world_mut = unsafe { unsafe_world.data_mut() };

        if let Some(id) = world_mut.get_resource_id::<T>()
            && let Some(data) = world_mut.storages.res_set.get_mut(id)
            && let Some(ptr) = data.get_mut(last_run, this_run)
        {
            unsafe { ptr.into_non_send::<T>() }
        } else {
            let full_mut = unsafe { unsafe_world.full_mut() };
            get_or_init_cold::<T>(full_mut)
        }
    }

    /// Executes a closure with exclusive access to a resource and the world.
    ///
    /// This method temporarily removes the resource from the world to satisfy
    /// Rust's borrowing rules, allowing the closure to mutably borrow both the
    /// resource and the world simultaneously.
    ///
    /// If the resource is not exist, return `None` directly.
    pub fn try_resource_scope<T: Resource + Send, R>(
        &mut self,
        func: impl FnOnce(&mut World, ResMut<T>) -> R,
    ) -> Option<R> {
        let last_run = self.last_run();
        let this_run = self.this_run_fast();

        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get_mut(id)
            && let Some((ptr, mut added, mut changed)) = unsafe { data.leak() }
        {
            unsafe {
                let res_mut = UntypedMut {
                    value: PtrMut::new(ptr),
                    ticks: TicksMut {
                        added: &mut added,
                        changed: &mut changed,
                        last_run,
                        this_run,
                    },
                }
                .into_resource();

                let ret = func(self, res_mut);

                if let Some(data) = self.storages.res_set.get_mut(id) {
                    data.from_raw(ptr, added, changed);
                } else {
                    core::ptr::drop_in_place::<T>(ptr.as_ptr() as *mut T);
                    let layout = Layout::new::<T>();
                    alloc::alloc::dealloc(ptr.as_ptr(), layout);
                }
                Some(ret)
            }
        } else {
            None
        }
    }

    /// Executes a closure with exclusive access to a resource and the world.
    ///
    /// This method temporarily removes the resource from the world to satisfy
    /// Rust's borrowing rules, allowing the closure to mutably borrow both the
    /// resource and the world simultaneously.
    ///
    /// # Panics
    ///
    /// Panics if the resource does not exist.
    pub fn resource_scope<T: Resource + Send, R>(
        &mut self,
        func: impl FnOnce(&mut World, ResMut<T>) -> R,
    ) -> R {
        self.try_resource_scope(func)
            .unwrap_or_else(|| uninitialized_resource(DebugName::type_name::<T>()))
    }

    /// Executes a closure with exclusive access to a resource and the world.
    ///
    /// This method temporarily removes the resource from the world to satisfy
    /// Rust's borrowing rules, allowing the closure to mutably borrow both the
    /// resource and the world simultaneously.
    ///
    /// If the resource is not exist, return `None` directly.
    ///
    /// # Panics
    /// Panics if called from a thread other than the world's main thread.
    pub fn try_non_send_scope<T: Resource, R>(
        &mut self,
        func: impl FnOnce(&mut World, NonSendMut<T>) -> R,
    ) -> Option<R> {
        assert! {
            self.thread_hash() == voker_os::thread::thread_hash(),
            "!Send Resource can only be accessed mut on the main thread.",
        }

        if let Some(id) = self.resources.get_id(TypeId::of::<T>())
            && let Some(data) = self.storages.res_set.get_mut(id)
            && let Some((ptr, mut added, mut changed)) = unsafe { data.leak() }
        {
            let last_run = self.last_run();
            let this_run = self.this_run_fast();
            unsafe {
                let res_mut = UntypedMut {
                    value: PtrMut::new(ptr),
                    ticks: TicksMut {
                        added: &mut added,
                        changed: &mut changed,
                        last_run,
                        this_run,
                    },
                }
                .into_non_send();

                let ret = func(self, res_mut);

                if let Some(data) = self.storages.res_set.get_mut(id) {
                    data.from_raw(ptr, added, changed);
                } else {
                    core::ptr::drop_in_place::<T>(ptr.as_ptr() as *mut T);
                    let layout = Layout::new::<T>();
                    alloc::alloc::dealloc(ptr.as_ptr(), layout);
                }
                Some(ret)
            }
        } else {
            None
        }
    }

    /// Executes a closure with exclusive access to a resource and the world.
    ///
    /// This method temporarily removes the resource from the world to satisfy
    /// Rust's borrowing rules, allowing the closure to mutably borrow both the
    /// resource and the world simultaneously.
    ///
    /// # Panics
    /// Panics if the resource does not exist or called from a thread other than
    /// the world's main thread.
    pub fn non_send_scope<T: Resource, R>(
        &mut self,
        func: impl FnOnce(&mut World, NonSendMut<T>) -> R,
    ) -> R {
        self.try_non_send_scope(func)
            .unwrap_or_else(|| uninitialized_resource(DebugName::type_name::<T>()))
    }
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::Ordering;
    use voker_os::sync::atomic::AtomicUsize;

    use crate::resource::Resource;
    use crate::tick::DetectChanges;
    use crate::world::World;

    #[derive(Resource, Debug, PartialEq, Eq)]
    struct Foo;

    #[derive(Resource, Debug, PartialEq, Eq)]
    struct Bar(u64);

    #[test]
    fn insert_basic() {
        let mut world = World::alloc();

        assert_eq!(*world.insert_resource(Foo), Foo);
        assert_eq!(*world.insert_resource(Bar(234)), Bar(234));

        assert_eq!(world.resource::<Foo>(), &Foo);
        assert_eq!(world.remove_resource::<Foo>(), Some(Foo));
        assert_eq!(world.get_resource::<Foo>(), None);
        assert_eq!(world.get_non_send::<Foo>(), None);
        assert_eq!(world.remove_non_send::<Foo>(), None);
        assert_eq!(world.remove_resource::<Foo>(), None);

        assert_eq!(world.resource::<Bar>(), &Bar(234));
        assert_eq!(world.remove_non_send::<Bar>(), Some(Bar(234)));
        assert_eq!(world.get_resource::<Bar>(), None);
        assert_eq!(world.get_non_send::<Bar>(), None);
        assert_eq!(world.remove_non_send::<Bar>(), None);
        assert_eq!(world.remove_resource::<Bar>(), None);
    }

    #[test]
    fn insert_replace() {
        let mut world = World::alloc();

        world.insert_resource(Bar(100));
        assert_eq!(world.get_resource::<Bar>(), Some(&Bar(100)));
        assert_eq!(world.get_non_send::<Bar>(), Some(&Bar(100)));

        world.insert_resource(Bar(200));
        assert_eq!(world.get_resource::<Bar>(), Some(&Bar(200)));
        assert_eq!(world.get_non_send::<Bar>(), Some(&Bar(200)));

        world.insert_non_send(Bar(800));
        assert_eq!(world.get_resource::<Bar>(), Some(&Bar(800)));
        assert_eq!(world.get_non_send::<Bar>(), Some(&Bar(800)));
    }

    #[test]
    fn remove_nonexistent() {
        let mut world = World::alloc();

        assert!(world.remove_resource::<Foo>().is_none());
        assert!(world.get_resource::<Foo>().is_none());
        assert!(world.get_resource_ref::<Foo>().is_none());
        assert!(world.get_resource_mut::<Foo>().is_none());

        assert!(world.remove_non_send::<Foo>().is_none());
        assert!(world.get_non_send::<Foo>().is_none());
        assert!(world.get_non_send_ref::<Foo>().is_none());
        assert!(world.get_non_send_mut::<Foo>().is_none());
    }

    #[test]
    fn get_ref() {
        let mut world = World::alloc();
        world.insert_resource(Bar(20));

        let res_ref = world.resource_ref::<Bar>();
        assert!(res_ref.is_changed());
        assert!(res_ref.is_added());

        world.reset_last_run();

        let res_ref = world.resource_ref::<Bar>();
        assert_eq!(*res_ref, Bar(20));
        assert!(!res_ref.is_changed());
        assert!(!res_ref.is_added());

        let res_ref = world.non_send_ref::<Bar>();
        assert_eq!(*res_ref, Bar(20));
        assert!(!res_ref.is_changed());
        assert!(!res_ref.is_added());
    }

    #[test]
    fn get_mut() {
        let mut world = World::alloc();
        world.insert_resource(Bar(20));

        let res_mut = world.resource_mut::<Bar>();
        assert!(res_mut.is_changed());
        assert!(res_mut.is_added());

        world.reset_last_run();
        let mut res_mut = world.resource_mut::<Bar>();
        assert_eq!(*res_mut, Bar(20));
        assert!(!res_mut.is_changed());
        assert!(!res_mut.is_added());

        *res_mut = Bar(100);
        assert!(res_mut.is_changed());
        assert!(!res_mut.is_added());

        world.reset_last_run();
        let mut res_mut = world.non_send_mut::<Bar>();
        assert_eq!(*res_mut, Bar(100));
        assert!(!res_mut.is_changed());
        assert!(!res_mut.is_added());

        *res_mut = Bar(50);
        assert!(res_mut.is_changed());
        assert!(!res_mut.is_added());

        assert_eq!(world.non_send::<Bar>(), &Bar(50));
        assert_eq!(world.resource::<Bar>(), &Bar(50));
    }

    #[test]
    fn drop_resource() {
        static DROP_COUNTER: AtomicUsize = AtomicUsize::new(0);

        #[derive(Debug, PartialEq, Eq)]
        struct DropTracker(usize);
        impl Resource for DropTracker {}

        impl Drop for DropTracker {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(self.0, Ordering::SeqCst);
            }
        }

        let mut world = World::alloc();

        // ------------------ Drop ----------------------
        DROP_COUNTER.store(0, Ordering::SeqCst);
        world.insert_resource(DropTracker(5));
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 0);
        world.drop_resource::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 5);
        world.insert_non_send(DropTracker(5));
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 5);
        world.drop_non_send::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 10);

        world.remove_non_send::<DropTracker>();
        world.remove_resource::<DropTracker>();
        world.drop_non_send::<DropTracker>();
        world.drop_resource::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 10);

        // ----------------- Remove  ----------------------
        DROP_COUNTER.store(0, Ordering::SeqCst);

        world.insert_non_send(DropTracker(5));
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 0);
        world.remove_non_send::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 5);

        world.insert_resource(DropTracker(5));
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 5);
        world.remove_resource::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 10);

        world.remove_non_send::<DropTracker>();
        world.remove_resource::<DropTracker>();
        world.drop_non_send::<DropTracker>();
        world.drop_resource::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 10);

        // ---------------- Overwrite ----------------------
        DROP_COUNTER.store(0, Ordering::SeqCst);

        world.insert_non_send(DropTracker(5));
        world.insert_resource(DropTracker(5));
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 5);
        world.drop_non_send::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 10);

        world.remove_non_send::<DropTracker>();
        world.remove_resource::<DropTracker>();
        world.drop_non_send::<DropTracker>();
        world.drop_resource::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 10);

        // ---------------- Overwrite ----------------------
        DROP_COUNTER.store(0, Ordering::SeqCst);

        for _ in 0..10 {
            world.insert_resource(DropTracker(1));
        }
        for _ in 0..10 {
            world.insert_non_send(DropTracker(1));
        }
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 19);

        world.remove_resource::<DropTracker>();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 20);
    }
}
