use crate::domain::common::value_object::ValueObject;

#[derive(Debug, Clone, PartialEq)]
pub struct Transform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    pub rotation: f32,
}

impl ValueObject for Transform {}
