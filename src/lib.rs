//! Laboratorio: planetas y estrellas con shaders.
//!
//! Software renderer en CPU (el de los laboratorios 4 y 5, ampliado con vertex shader, profundidad
//! y fragment shaders). Una única esfera (`assets/sphere.obj`) se dibuja como tres cuerpos
//! celestes distintos cambiando solo el fragment shader.

pub mod framebuffer;
pub mod math;
pub mod mesh;
pub mod pipeline;
pub mod shaders;
