use bytemuck::{cast_slice, Pod, Zeroable};
use std::iter::once;

pub struct FrameBuffer {
    width: usize,
    height: usize,
    pixels: Box<[Pixel]>,
}
impl FrameBuffer {
    pub(super) fn new(width: usize, height: usize) -> Self {
        Self {
            pixels: once(Pixel::black()).cycle().take(width * height).collect(),
            width,
            height,
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, pixel: Pixel) {
        let i = self.coord_to_index(x, y);

        if self.pixels.len() != 0 {
            self.pixels[i] = pixel;
        }
    }
    fn coord_to_index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn as_bytes(&self) -> &[u8] {
        cast_slice(&self.pixels)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Pixel {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}
impl Pixel {
    pub fn black() -> Pixel {
        Pixel {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 255,
        }
    }
}
unsafe impl Pod for Pixel {}
unsafe impl Zeroable for Pixel {}
