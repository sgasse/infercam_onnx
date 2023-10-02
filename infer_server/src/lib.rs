//! Inference server library.
//!

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

pub mod hour_glass;
pub mod meter;
pub mod msg_passing;
pub mod nn;
pub mod utils;

fn hashed<T>(data: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}
