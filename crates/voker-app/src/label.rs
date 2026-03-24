use voker_ecs::label::Interned;

voker_ecs::define_label!(
    /// A strongly-typed class of labels used to identify an [`App`].
    #[diagnostic::on_unimplemented(
        note = "consider annotating `{Self}` with `#[derive(AppLabel)]`"
    )]
    AppLabel,
    APP_LABEL_INTERNER
);

/// A shorthand for `Interned<dyn AppLabel>`.
pub type InternedAppLabel = Interned<dyn AppLabel>;
