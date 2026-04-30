//! Entity template system for procedural scene construction.
//!
//! Templates describe a reusable pattern of entities and relationships that can
//! be instantiated multiple times into a [`World`]. Scoped entity indices allow
//! templates to reference intra-template entities by position rather than by
//! live [`Entity`] id.
//!
//! The main entry points are [`Template`] (the trait) and [`FnTemplate`] (a
//! closure-based convenience wrapper).

use alloc::vec::Vec;

use crate::borrow::ResMut;
use crate::entity::Entity;
use crate::error::GameResult;
use crate::resource::Resource;
use crate::world::{EntityOwned, World};

// -----------------------------------------------------------------------------
// ScopedEntityIndex

#[derive(Copy, Clone, Debug)]
pub struct ScopedEntityIndex {
    /// The scope of the entity index. This must be defined ahead of time.
    pub scope: usize,
    /// The index that uniquely identifies the entity within the current scope.
    pub index: usize,
}

// -----------------------------------------------------------------------------
// ScopedEntities

#[derive(Debug)]
pub struct ScopedEntities(Vec<Option<Entity>>);

impl ScopedEntities {
    pub fn new(size: usize) -> Self {
        Self(alloc::vec![None; size])
    }

    pub fn get(
        &mut self,
        world: &mut World,
        entity_scopes: &EntityScopes,
        scoped_entity_index: ScopedEntityIndex,
    ) -> Entity {
        let index = entity_scopes.get(scoped_entity_index).unwrap();
        *self.0[index].get_or_insert_with(|| world.spawn_empty().entity())
    }

    pub fn set(
        &mut self,
        entity_scopes: &EntityScopes,
        scoped_entity_index: ScopedEntityIndex,
        entity: Entity,
    ) {
        let index = entity_scopes.get(scoped_entity_index).unwrap();
        self.0[index] = Some(entity);
    }
}

// -----------------------------------------------------------------------------
// EntityScopes

#[derive(Default, Debug)]
pub struct EntityScopes {
    scopes: Vec<Vec<Option<usize>>>,
    next_index: usize,
}

impl EntityScopes {
    fn get_mut(&mut self, scoped_entity_index: ScopedEntityIndex) -> &mut Option<usize> {
        // NOTE: this is ok because PatchContext::new_scope adds scopes as they are created.
        // this shouldn't panic unless internals are broken.
        let indices = &mut self.scopes[scoped_entity_index.scope];
        if scoped_entity_index.index >= indices.len() {
            indices.resize_with(scoped_entity_index.index + 1, || None);
        }
        #[expect(unsafe_code, reason = "just allocated above")]
        unsafe {
            indices.get_unchecked_mut(scoped_entity_index.index)
        }
    }

    #[inline]
    pub fn entity_len(&self) -> usize {
        self.next_index
    }

    pub fn alloc(&mut self, scoped_entity_index: ScopedEntityIndex) {
        *self.get_mut(scoped_entity_index) = Some(self.next_index);
        self.next_index += 1;
    }

    pub fn assign(&mut self, scoped_entity_index: ScopedEntityIndex, value: usize) {
        let option = self.get_mut(scoped_entity_index);
        *option = Some(value);
    }

    pub fn add_scope(&mut self) -> usize {
        let scope_index = self.scopes.len();
        self.scopes.push(Vec::new());
        scope_index
    }

    pub fn get(&self, scoped_entity_index: ScopedEntityIndex) -> Option<usize> {
        *self
            .scopes
            .get(scoped_entity_index.scope)?
            .get(scoped_entity_index.index)?
    }
}

// -----------------------------------------------------------------------------
// TemplateContext

pub struct TemplateContext<'a, 'w> {
    pub entity: &'a mut EntityOwned<'w>,
    pub scoped_entities: &'a mut ScopedEntities,
    pub entity_scopes: &'a EntityScopes,
}

impl<'a, 'w> TemplateContext<'a, 'w> {
    pub fn new(
        entity: &'a mut EntityOwned<'w>,
        scoped_entities: &'a mut ScopedEntities,
        entity_scopes: &'a EntityScopes,
    ) -> Self {
        Self {
            entity,
            scoped_entities,
            entity_scopes,
        }
    }

    pub fn get_scoped_entity(&mut self, scoped_entity_index: ScopedEntityIndex) -> Entity {
        // SAFETY: this only uses the world to spawn an empty entity
        let world = unsafe { self.entity.world.full_mut() };
        self.scoped_entities
            .get(world, self.entity_scopes, scoped_entity_index)
    }

    #[inline]
    pub fn resource<R: Resource + Sync>(&self) -> &R {
        self.entity.resource()
    }

    #[inline]
    pub fn resource_mut<R: Resource + Send>(&mut self) -> ResMut<'_, R> {
        self.entity.resource_mut()
    }
}

// -----------------------------------------------------------------------------
// Template

pub trait Template {
    /// The type of value produced by this [`Template`].
    type Output;

    /// Uses this template and the given `entity` context to produce a [`Template::Output`].
    fn build_template(&self, context: &mut TemplateContext) -> GameResult<Self::Output>;

    /// Clones this template. See [`Clone`].
    fn clone_template(&self) -> Self;
}

// This includes `Unpin` to enable specialization for Templates that also implement Default.
// by using the ["auto trait specialization" trick](https://github.com/coolcatcoder/rust_techniques/issues/1)
impl<T: Clone + Default + Unpin> Template for T {
    type Output = T;

    fn build_template(&self, _context: &mut TemplateContext) -> GameResult<Self::Output> {
        Ok(self.clone())
    }

    fn clone_template(&self) -> Self {
        self.clone()
    }
}

// -----------------------------------------------------------------------------
// FromTemplate

pub trait FromTemplate: Sized {
    /// The [`Template`] for this type.
    type Template: Template;
}

impl<T: Clone + Default + Unpin> FromTemplate for T {
    type Template = T;
}

#[diagnostic::on_unimplemented(
    message = "\
    This type does **not** manually implement FromTemplate, and it must.\n\
    If you are deriving FromTemplate and you see this, it is likely because \
    a field does not have a FromTemplate impl. This can usually be fixed by \
    using a custom template for that field. \n \
    Ex: for an Option<Handle<Image>> field, annotate the field with \
    `#[template(OptionTemplate<HandleTemplate<Image>>)]`",
    note = "\
    FromTemplate currently uses pseudo-specialization to enable FromTemplate \
    to override Default. This error message is a consequence of t."
)]
pub trait SpecializeFromTemplate: Sized {}

// -----------------------------------------------------------------------------
// TemplateTuple

#[repr(transparent)]
pub struct TemplateTuple<T>(pub T);

macro_rules! template_impl {
    (0: []) => {
        impl Template for TemplateTuple<()> {
            type Output = ();

            fn build_template(&self, _context: &mut TemplateContext) -> GameResult<Self::Output> {
                Ok(())
            }

            fn clone_template(&self) -> Self {
                TemplateTuple(())
            }
        }
    };
    (1: [0: P0]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<P0: Template> Template for TemplateTuple<(P0,)> {
            type Output = (P0::Output,);
            fn build_template(&self, context: &mut TemplateContext) -> GameResult<Self::Output> {
                Ok(( self.0.0.build_template(context)?, ))
            }

            fn clone_template(&self) -> Self {
                TemplateTuple(( self.0.0.clone_template(), ))
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: Template),*> Template for TemplateTuple<($($name),*)> {
            type Output = ( $($name::Output,)* );
            fn build_template(&self, context: &mut TemplateContext) -> GameResult<Self::Output> {
                Ok(( $(self.0.$index.build_template(context)?,)* ))
            }

            fn clone_template(&self) -> Self {
                TemplateTuple(( $(self.0.$index.clone_template(),)* ))
            }
        }
    };
}

voker_utils::range_invoke!(template_impl, 12);

// -----------------------------------------------------------------------------
// EntityTemplate

pub enum EntityTemplate {
    Entity(Entity),
    ScopedEntityIndex(ScopedEntityIndex),
}

impl Default for EntityTemplate {
    fn default() -> Self {
        Self::ScopedEntityIndex(ScopedEntityIndex { scope: 0, index: 0 })
    }
}

impl From<Entity> for EntityTemplate {
    fn from(entity: Entity) -> Self {
        Self::Entity(entity)
    }
}

impl Template for EntityTemplate {
    type Output = Entity;

    fn build_template(&self, context: &mut TemplateContext) -> GameResult<Self::Output> {
        Ok(match self {
            Self::Entity(entity) => *entity,
            Self::ScopedEntityIndex(scoped_entity_index) => {
                context.get_scoped_entity(*scoped_entity_index)
            }
        })
    }

    fn clone_template(&self) -> Self {
        match self {
            Self::Entity(entity) => Self::Entity(*entity),
            Self::ScopedEntityIndex(scoped_entity_index) => {
                Self::ScopedEntityIndex(*scoped_entity_index)
            }
        }
    }
}

impl FromTemplate for Entity {
    type Template = EntityTemplate;
}

// -----------------------------------------------------------------------------
// EntityTemplate

pub struct FnTemplate<F: Fn(&mut TemplateContext) -> GameResult<O>, O>(pub F);

impl<F, O> Template for FnTemplate<F, O>
where
    F: Fn(&mut TemplateContext) -> GameResult<O> + Clone,
{
    type Output = O;

    fn build_template(&self, context: &mut TemplateContext) -> GameResult<Self::Output> {
        (self.0)(context)
    }

    fn clone_template(&self) -> Self {
        Self(self.0.clone())
    }
}

pub fn template<F: Fn(&mut TemplateContext) -> GameResult<O>, O>(func: F) -> FnTemplate<F, O> {
    FnTemplate(func)
}

// -----------------------------------------------------------------------------
// OptionTemplate

#[derive(Default)]
pub enum OptionTemplate<T> {
    /// Template of [`Option::Some`].
    Some(T),
    /// Template of [`Option::None`].
    #[default]
    None,
}

impl<T> From<Option<T>> for OptionTemplate<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => OptionTemplate::Some(value),
            None => OptionTemplate::None,
        }
    }
}

impl<T> From<T> for OptionTemplate<T> {
    fn from(value: T) -> Self {
        OptionTemplate::Some(value)
    }
}

impl<T: Template> Template for OptionTemplate<T> {
    type Output = Option<T::Output>;

    fn build_template(&self, context: &mut TemplateContext) -> GameResult<Self::Output> {
        Ok(match &self {
            OptionTemplate::Some(template) => Some(template.build_template(context)?),
            OptionTemplate::None => None,
        })
    }

    fn clone_template(&self) -> Self {
        match self {
            OptionTemplate::Some(value) => OptionTemplate::Some(value.clone_template()),
            OptionTemplate::None => OptionTemplate::None,
        }
    }
}

// -----------------------------------------------------------------------------
// VecTemplate

pub struct VecTemplate<T>(pub Vec<T>);

impl<T> Default for VecTemplate<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Template> Template for VecTemplate<T> {
    type Output = Vec<T::Output>;

    fn build_template(&self, context: &mut TemplateContext) -> GameResult<Self::Output> {
        let mut output = Vec::with_capacity(self.0.len());
        for value in &self.0 {
            output.push(value.build_template(context)?);
        }
        Ok(output)
    }

    fn clone_template(&self) -> Self {
        VecTemplate(self.0.iter().map(Template::clone_template).collect())
    }
}

// -----------------------------------------------------------------------------
// BuiltInTemplate

pub trait BuiltInTemplate: Sized {
    /// The template to consider the "built in" template for this type.
    type Template: Template;
}

impl<T: FromTemplate> BuiltInTemplate for Option<T> {
    type Template = OptionTemplate<T::Template>;
}

impl<T: FromTemplate> BuiltInTemplate for Vec<T> {
    type Template = VecTemplate<T::Template>;
}
