use crate::ascii_font::FONTS;

#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum PixelFormat {
    Rgb = 0,
    Bgr,
    Bitmask,
    BltOnly,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct PixelBitmask {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub reserved: u32,
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct ModeInfo {
    pub version: u32,
    pub hor_res: u32,
    pub ver_res: u32,
    pub format: PixelFormat,
    pub mask: PixelBitmask,
    pub stride: u32,
}

impl ModeInfo {
    pub fn resolution(&self) -> (usize, usize) {
        (self.hor_res as usize, self.ver_res as usize)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FrameBuffer {
    base: *mut u8,
    size: usize,
}

impl FrameBuffer {
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.base
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// Write to th index-th byte of the framebuffer
    ///
    /// # Safety
    /// This is unsafe : no bound check.
    pub unsafe fn write_byte(&mut self, index: usize, val: u8) {
        self.base.add(index).write_volatile(val);
    }

    /// Write to th index-th byte of the framebuffer
    ///
    /// # Safety
    /// This is unsafe : no bound check.
    pub unsafe fn write_value(&mut self, index: usize, value: [u8; 3]) {
        (self.base.add(index) as *mut [u8; 3]).write_volatile(value)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct PixelColor(pub u8, pub u8, pub u8); // RGB

// static singleton pointer
static mut RAW_GRAPHICS: [u8; core::mem::size_of::<Graphics>()] =
    [0; core::mem::size_of::<Graphics>()];
static mut GRAPHICS_INITIALIZED: bool = false;

#[derive(Copy, Clone)]
pub struct Graphics {
    fb: FrameBuffer,
    mi: ModeInfo,
    pixel_writer: unsafe fn(&mut FrameBuffer, usize, &PixelColor),
}

impl Graphics {
    pub fn new(fb: FrameBuffer, mi: ModeInfo) -> Self {
        unsafe fn write_pixel_rgb(fb: &mut FrameBuffer, index: usize, rgb: &PixelColor) {
            fb.write_value(index, [rgb.0, rgb.1, rgb.2]);
        }
        unsafe fn write_pixel_bgr(fb: &mut FrameBuffer, index: usize, rgb: &PixelColor) {
            fb.write_value(index, [rgb.2, rgb.1, rgb.0]);
        }
        let pixel_writer = match mi.format {
            PixelFormat::Rgb => write_pixel_rgb,
            PixelFormat::Bgr => write_pixel_bgr,
            _ => {
                panic!("This pixel format is not supported by the drawing demo");
            }
        };

        Graphics {
            fb,
            mi,
            pixel_writer,
        }
    }

    pub fn instance() -> &'static mut Self {
        if unsafe { !GRAPHICS_INITIALIZED } {
            panic!("Graphics is not initialized");
        }
        unsafe { &mut *(RAW_GRAPHICS.as_mut_ptr() as *mut Self) }
    }

    ///
    /// # Safety
    /// This is unsafe : handle raw pointers.
    pub unsafe fn initialize_instance(fb: *mut FrameBuffer, mi: *mut ModeInfo) {
        core::ptr::write(
            RAW_GRAPHICS.as_mut_ptr() as *mut Graphics,
            Graphics::new(*fb, *mi),
        );
        GRAPHICS_INITIALIZED = true;
    }

    /// Write to the pixel of the buffer
    ///
    pub fn write_pixel(&mut self, x: usize, y: usize, color: &PixelColor) {
        // TODO: don't panic.
        if x > self.mi.hor_res as usize {
            panic!("bad x coord");
        }
        if y > self.mi.ver_res as usize {
            panic!("bad x coord");
        }
        let pixel_index = y * (self.mi.stride as usize) + x;
        let base = 4 * pixel_index;
        unsafe {
            (self.pixel_writer)(&mut self.fb, base, color);
        }
    }

    pub fn write_ascii(&mut self, x: usize, y: usize, c: char, color: &PixelColor) {
        if (c as u32) < 0x20 || (c as u32) > 0x7f {
            return;
        }
        let font: [u8; 16] = FONTS[c as usize];
        for (dy, line) in font.iter().enumerate() {
            for dx in 0..8 {
                if (line << dx) & 0x80 != 0 {
                    self.write_pixel(x + dx, y + dy, &color);
                }
            }
        }
    }

    pub fn write_string(
        &mut self,
        mut x: usize,
        mut y: usize,
        str: &str,
        color: &PixelColor,
    ) -> (usize, usize) {
        let first_x = x;
        let (width, height) = self.resolution();
        for c in str.chars() {
            self.write_ascii(x, y, c, color);
            x += 8;
            if x > width {
                x = first_x;
                y += 20;
            }
            if y > height {
                // can not write
                return (x, y);
            }
        }
        (x, y)
    }

    pub fn resolution(&self) -> (usize, usize) {
        self.mi.resolution()
    }

    pub fn clear(&mut self, color: &PixelColor) {
        let (width, height) = self.resolution();
        for y in 0..height {
            for x in 0..width {
                self.write_pixel(x, y, color);
            }
        }
    }

    pub fn fb(&self) -> FrameBuffer {
        self.fb
    }

    pub fn mi(&self) -> ModeInfo {
        self.mi
    }
    pub fn text_writer(
        &mut self,
        first_x: usize,
        first_y: usize,
        color: &PixelColor,
    ) -> TextWriter {
        TextWriter::new(self, first_x, first_y, color)
    }
}

pub struct TextWriter<'a> {
    graphics: &'a mut Graphics,
    first_x: usize,
    first_y: usize,
    x: usize,
    y: usize,
    color: PixelColor,
}

impl<'a> TextWriter<'a> {
    pub fn new(
        graphics: &'a mut Graphics,
        first_x: usize,
        first_y: usize,
        color: &PixelColor,
    ) -> Self {
        TextWriter {
            graphics,
            first_x,
            first_y,
            x: first_x,
            y: first_y,
            color: *color,
        }
    }

    pub fn reset_coord(&mut self) {
        self.x = self.first_x;
        self.y = self.first_y;
    }

    pub fn change_color(&mut self, color: &PixelColor) {
        self.color = *color;
    }
}

impl<'a> core::fmt::Write for TextWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let (x, y) = self.graphics.write_string(self.x, self.y, s, &self.color);
        self.x = x;
        self.y = y;
        Ok(())
    }
}
