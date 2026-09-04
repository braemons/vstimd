//! There is nothing to tessellate.
//!
//! A dot field draws one shared unit quad, instanced once per dot, with every
//! per-dot value in the instance buffer and every per-field value in the push
//! constants. Nothing about the field produces vertices, so it has no tessellation
//! step and no entry in the mesh caches — only the instance buffer in
//! [`DotsInstanceCache`](crate::render::vk::cache::DotsInstanceCache).
//!
//! This file exists so that the absence is stated where the other stimulus bodies
//! put their tessellator, rather than being something you conclude from a missing
//! file.
