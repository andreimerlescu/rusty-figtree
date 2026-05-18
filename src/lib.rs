// src/lib.rs
pub mod error;
pub mod tree;
pub mod fig;
pub mod mutation;
pub mod priority;
pub mod rules;
pub mod validators;
pub mod callbacks;

pub mod sources;

// re-export the derive macro transparently
pub use figtree_derive::Figtree;

// prelude — what you get with `use figtree::prelude::*`
pub mod prelude {
    pub use crate::tree::Tree;
    pub use crate::fig::Fig;
    pub use crate::mutation::Mutation;
    pub use crate::rules::*;
    pub use crate::validators::*;
    pub use crate::callbacks::CallbackAfter;
    pub use figtree_derive::Figtree;
}
