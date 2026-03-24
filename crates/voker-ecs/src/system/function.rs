use super::{AccessTable, System, SystemFlags, SystemMeta};
use crate::error::EcsError;
use crate::system::{IntoSystem, SystemName, UninitSystemError};
use crate::tick::Tick;
use crate::world::{World, WorldId};

use super::{SystemInput, SystemParam};

// -----------------------------------------------------------------------------
// SystemFunction

type SystemInputData<'a, P> = <P as SystemInput>::Data<'a>;
type SystemParamItem<'w, 's, P> = <P as SystemParam>::Item<'w, 's>;

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid system function",
    label = "invalid system function"
)]
pub trait SystemFunction<Marker>: Send + Sync + 'static {
    type Param: SystemParam;
    type Input: SystemInput;
    type Output;

    fn run(
        this: &mut Self,
        input: SystemInputData<Self::Input>,
        param: SystemParamItem<Self::Param>,
    ) -> Self::Output;
}

macro_rules! impl_tuple {
    (0: []) => {
        impl<O, Func> SystemFunction<fn() -> O> for Func
        where
            O: 'static,
            Func: Send + Sync + 'static,
            for<'a> &'a mut Func: FnMut() -> O
        {
            type Param = ();
            type Input = ();
            type Output = O;

            fn run(
                this: &mut Self,
                _input: (),
                _param: (),
            ) -> Self::Output {
                #[inline(always)]
                fn call<O>(mut func: impl FnMut() -> O) -> O {
                    func()
                }

                call(this)
            }
        }

        impl<I, O, Func> SystemFunction<(I, fn() -> O)> for Func
        where
            O: 'static,
            I: SystemInput + 'static,
            Func: Send + Sync + 'static,
            for<'a> &'a mut Func:
                FnMut(I) -> O +
                FnMut(I::Item<'_>) -> O +
        {
            type Param = ();
            type Input = I;
            type Output = O;

            fn run(
                this: &mut Self,
                input: I::Data<'_>,
                _param: (),
            ) -> Self::Output {
                #[inline(always)]
                fn call<I, O>(
                    mut func: impl FnMut(I) -> O,
                    input: I,
                ) -> O {
                    func(input)
                }

                call(this, I::wrap(input))
            }
        }
    };
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<O, $name, Func> SystemFunction<fn($name) -> O> for Func
        where
            O: 'static,
            $name: SystemParam + 'static,
            Func: Send + Sync + 'static,
            for<'a> &'a mut Func:
                FnMut($name) -> O +
                FnMut(<$name>::Item<'_, '_>) -> O
        {
            type Param = ( $name, );
            type Input = ();
            type Output = O;

            fn run(
                this: &mut Self,
                _input: (),
                param: ( <$name>::Item<'_,'_> ,),
            ) -> Self::Output {
                #[inline(always)]
                fn call<O, $name>(
                    mut func: impl FnMut($name) -> O,
                    param: ( $name , ),
                ) -> O {
                    func(param.0)
                }

                call(this, param)
            }
        }

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<I, O, $name, Func> SystemFunction<(I, fn($name) -> O)> for Func
        where
            O: 'static,
            I: SystemInput + 'static,
            $name: SystemParam + 'static,
            Func: Send + Sync + 'static,
            for<'a> &'a mut Func:
                FnMut(I, $name) -> O +
                FnMut(I::Item<'_>, <$name>::Item<'_, '_>) -> O
        {
            type Param = ( $name, );
            type Input = I;
            type Output = O;

            fn run(
                this: &mut Self,
                input: I::Data<'_>,
                param: ( <$name>::Item<'_,'_> ,),
            ) -> Self::Output {
                #[inline(always)]
                fn call<I, O, $name>(
                    mut func: impl FnMut(I, $name) -> O,
                    input: I,
                    param: ( $name , ),
                ) -> O {
                    func(input, param.0)
                }

                call(this, I::wrap(input), param)
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        impl<O, $($name,)* Func> SystemFunction<fn($($name),*) -> O> for Func
        where
            O: 'static,
            $($name: SystemParam + 'static,)*
            Func: Send + Sync + 'static,
            for<'a> &'a mut Func:
                FnMut($($name),*) -> O +
                FnMut($(<$name>::Item<'_, '_>),*) -> O
        {
            type Param = ( $($name),* );
            type Input = ();
            type Output = O;

            fn run(
                this: &mut Self,
                _input: (),
                param: ( $(<$name>::Item<'_,'_>, )* ),
            ) -> Self::Output {
                #[inline(always)]
                fn call<O, $($name),*>(
                    mut func: impl FnMut($($name),*) -> O,
                    param: ( $($name),* ),
                ) -> O {
                    func($(param.$index),*)
                }

                call(this, param)
            }
        }

        #[cfg_attr(docsrs, doc(hidden))]
        impl<I, O, $($name,)* Func> SystemFunction<(I, fn($($name),*) -> O)> for Func
        where
            O: 'static,
            I: SystemInput + 'static,
            $($name: SystemParam + 'static,)*
            Func: Send + Sync + 'static,
            for<'a> &'a mut Func:
                FnMut(I, $($name),*) -> O +
                FnMut(I::Item<'_>, $(<$name>::Item<'_, '_>),*) -> O
        {
            type Param = ( $($name),* );
            type Input = I;
            type Output = O;

            fn run(
                this: &mut Self,
                input: I::Data<'_>,
                param: ( $(<$name>::Item<'_,'_>, )* ),
            ) -> Self::Output {
                #[inline(always)]
                fn call<I, O, $($name),*>(
                    mut func: impl FnMut(I, $($name),*) -> O,
                    input: I,
                    param: ( $($name),* ),
                ) -> O {
                    func(input, $(param.$index),*)
                }

                call(this, I::wrap(input), param)
            }
        }
    }
}

voker_utils::range_invoke!(impl_tuple, 12);

// -----------------------------------------------------------------------------
// FunctionSystem

struct FunctionState<P: SystemParam> {
    param: P::State,
    world_id: WorldId,
}

pub struct FunctionSystem<M, F: SystemFunction<M>> {
    func: F,
    meta: SystemMeta,
    state: Option<FunctionState<F::Param>>,
}

impl<M, F: SystemFunction<M>> FunctionSystem<M, F> {
    pub fn new(func: F, name: SystemName) -> Self {
        let mut meta = SystemMeta::new(name);
        if <F::Param as SystemParam>::EXCLUSIVE {
            meta.set_exclusive();
        }
        if <F::Param as SystemParam>::NON_SEND {
            meta.set_non_send();
        }

        Self {
            func,
            meta,
            state: None,
        }
    }
}

impl<M: 'static, F: SystemFunction<M> + 'static> System for FunctionSystem<M, F> {
    type Input = F::Input;
    type Output = F::Output;

    fn name(&self) -> SystemName {
        self.meta.name()
    }

    fn flags(&self) -> SystemFlags {
        self.meta.flags()
    }

    fn get_last_run(&self) -> Tick {
        self.meta.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.meta.set_last_run(last_run)
    }

    fn initialize(&mut self, world: &mut World) -> AccessTable {
        let mut table = AccessTable::new();
        let state = self.state.get_or_insert_with(|| FunctionState {
            param: <F::Param as SystemParam>::init_state(world),
            world_id: world.id(),
        });
        if !<F::Param as SystemParam>::mark_access(&mut table, &state.param) {
            invalid_system_access(self.meta.name());
        }
        table
    }

    unsafe fn run(
        &mut self,
        input: <Self::Input as SystemInput>::Data<'_>,
        world: crate::world::UnsafeWorld<'_>,
    ) -> Result<Self::Output, EcsError> {
        let Some(state) = &mut self.state else {
            return Err(uninit_system_error(self.meta.name()));
        };
        let world_id = unsafe { world.read_only().id() };
        if state.world_id != world_id {
            mismatched_world(self.meta.name(), state.world_id, world_id);
        }

        let last_run = self.meta.get_last_run();
        let this_run = unsafe { world.read_only().advance_tick() };
        let param = unsafe {
            <F::Param as SystemParam>::build_param(world, &mut state.param, last_run, this_run)?
        };

        let output = <F as SystemFunction<M>>::run(&mut self.func, input, param);

        self.meta.set_last_run(this_run);

        Ok(output)
    }
}

#[cold]
#[inline(never)]
fn uninit_system_error(name: SystemName) -> EcsError {
    EcsError::from(UninitSystemError { name })
}

#[cold]
#[inline(never)]
fn invalid_system_access(name: SystemName) -> ! {
    panic!("System {name} params access conflict.")
}

#[cold]
#[inline(never)]
fn mismatched_world(name: SystemName, init: WorldId, run: WorldId) -> ! {
    panic!("System {name} is initialized in world {init}, but runs in world {run}.")
}

// -----------------------------------------------------------------------------
// FunctionSystem

impl<M: 'static, F: SystemFunction<M>> IntoSystem<F::Input, F::Output, M> for F {
    type System = FunctionSystem<M, F>;

    fn into_system(this: Self, name: SystemName) -> Self::System {
        FunctionSystem::new(this, name)
    }
}
