// Hand-written OpenGL 2.1 FFI. Core 1.1 functions are linked directly from
// opengl32.dll; anything newer is resolved via wglGetProcAddress at runtime.
#![allow(non_snake_case, dead_code)]

use std::ffi::CString;
use std::os::raw::c_void;
use std::cell::Cell;

pub const GL_FALSE: u8 = 0;
pub const GL_TRUE: u8 = 1;

pub const GL_POINTS: u32 = 0;
pub const GL_LINES: u32 = 0x0001;
pub const GL_LINE_LOOP: u32 = 0x0002;
pub const GL_TRIANGLES: u32 = 0x0004;

pub const GL_DEPTH_TEST: u32 = 0x0B71;
pub const GL_CULL_FACE: u32 = 0x0B44;
pub const GL_BLEND: u32 = 0x0BE2;

pub const GL_SRC_ALPHA: u32 = 0x0302;
pub const GL_ONE_MINUS_SRC_ALPHA: u32 = 0x0303;

pub const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
pub const GL_DEPTH_BUFFER_BIT: u32 = 0x0100;

pub const GL_FLOAT: u32 = 0x1406;
pub const GL_UNSIGNED_BYTE: u32 = 0x1401;
pub const GL_UNSIGNED_INT: u32 = 0x1405;

pub const GL_TEXTURE_2D: u32 = 0x0DE1;
pub const GL_RGBA: u32 = 0x1908;
pub const GL_RGBA8: i32 = 0x8058;
pub const GL_UNPACK_ALIGNMENT: u32 = 0x0CF5;
pub const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
pub const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
pub const GL_NEAREST: i32 = 0x2600;
pub const GL_TEXTURE_WRAP_S: u32 = 0x2802;
pub const GL_TEXTURE_WRAP_T: u32 = 0x2803;
pub const GL_CLAMP: i32 = 0x2900;
pub const GL_TEXTURE0: u32 = 0x84C0;

pub const GL_ARRAY_BUFFER: u32 = 0x8892;
pub const GL_ELEMENT_ARRAY_BUFFER: u32 = 0x8893;
pub const GL_STATIC_DRAW: u32 = 0x88E4;
pub const GL_DYNAMIC_DRAW: u32 = 0x88E8;

pub const GL_VERTEX_SHADER: u32 = 0x8B31;
pub const GL_FRAGMENT_SHADER: u32 = 0x8B30;
pub const GL_COMPILE_STATUS: u32 = 0x8B81;
pub const GL_LINK_STATUS: u32 = 0x8B82;

// ---------------------------------------------------------------- linked 1.1
#[cfg(windows)]
#[link(name = "opengl32")]
extern "system" {
    pub fn glEnable(cap: u32);
    pub fn glDisable(cap: u32);
    pub fn glClearColor(r: f32, g: f32, b: f32, a: f32);
    pub fn glClear(mask: u32);
    pub fn glViewport(x: i32, y: i32, w: i32, h: i32);
    pub fn glBlendFunc(sf: u32, df: u32);
    pub fn glCullFace(mode: u32);
    pub fn glDepthMask(flag: u8);
    pub fn glLineWidth(w: f32);
    pub fn glPixelStorei(pname: u32, param: i32);
    pub fn glGenTextures(n: i32, textures: *mut u32);
    pub fn glDeleteTextures(n: i32, textures: *const u32);
    pub fn glBindTexture(target: u32, texture: u32);
    pub fn glTexImage2D(
        target: u32,
        level: i32,
        internal: i32,
        w: i32,
        h: i32,
        border: i32,
        format: u32,
        ty: u32,
        data: *const c_void,
    );
    pub fn glTexParameteri(target: u32, pname: u32, param: i32);
    pub fn glDrawElements(mode: u32, count: i32, ty: u32, indices: *const c_void);
    pub fn glDrawArrays(mode: u32, first: i32, count: i32);
    pub fn glPointSize(size: f32);
    pub fn glGetError() -> u32;
    pub fn glGetString(name: u32) -> *const u8;
}

// Linker stubs for non-Windows test hosts (never called).
#[cfg(not(windows))]
mod glstubs {
    use std::os::raw::c_void;
    macro_rules! gstub {
        ($($name:ident($($arg:ty),*) -> $ret:ty);* $(;)?) => {
            $(pub unsafe extern "system" fn $name($(_: $arg),*) -> $ret { unreachable!() })*
        };
    }
    gstub! {
        glEnable(u32) -> ();
        glDisable(u32) -> ();
        glClearColor(f32, f32, f32, f32) -> ();
        glClear(u32) -> ();
        glViewport(i32, i32, i32, i32) -> ();
        glBlendFunc(u32, u32) -> ();
        glCullFace(u32) -> ();
        glDepthMask(u8) -> ();
        glLineWidth(f32) -> ();
        glPixelStorei(u32, i32) -> ();
        glGenTextures(i32, *mut u32) -> ();
        glDeleteTextures(i32, *const u32) -> ();
        glBindTexture(u32, u32) -> ();
        glTexImage2D(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void) -> ();
        glTexParameteri(u32, u32, i32) -> ();
        glDrawElements(u32, i32, u32, *const c_void) -> ();
        glDrawArrays(u32, i32, i32) -> ();
        glPointSize(f32) -> ();
        glGetError() -> u32;
        glGetString(u32) -> *const u8
    }
}

#[cfg(not(windows))]
use glstubs::*;

// ------------------------------------------------------- loaded via wglGetProcAddress
#[derive(Clone, Copy)]
pub struct Gl {
    pub create_shader: unsafe extern "system" fn(u32) -> u32,
    pub shader_source: unsafe extern "system" fn(u32, i32, *const *const u8, *const i32),
    pub compile_shader: unsafe extern "system" fn(u32),
    pub get_shaderiv: unsafe extern "system" fn(u32, u32, *mut i32),
    pub get_shader_info_log: unsafe extern "system" fn(u32, i32, *mut i32, *mut u8),
    pub create_program: unsafe extern "system" fn() -> u32,
    pub attach_shader: unsafe extern "system" fn(u32, u32),
    pub bind_attrib_location: unsafe extern "system" fn(u32, u32, *const u8),
    pub link_program: unsafe extern "system" fn(u32),
    pub get_programiv: unsafe extern "system" fn(u32, u32, *mut i32),
    pub get_program_info_log: unsafe extern "system" fn(u32, i32, *mut i32, *mut u8),
    pub delete_shader: unsafe extern "system" fn(u32),
    pub use_program: unsafe extern "system" fn(u32),
    pub get_uniform_location: unsafe extern "system" fn(u32, *const u8) -> i32,
    pub uniform_matrix4fv: unsafe extern "system" fn(i32, i32, u8, *const f32),
    pub uniform1f: unsafe extern "system" fn(i32, f32),
    pub uniform1i: unsafe extern "system" fn(i32, i32),
    pub uniform3f: unsafe extern "system" fn(i32, f32, f32, f32),
    pub uniform4f: unsafe extern "system" fn(i32, f32, f32, f32, f32),
    pub gen_buffers: unsafe extern "system" fn(i32, *mut u32),
    pub delete_buffers: unsafe extern "system" fn(i32, *const u32),
    pub bind_buffer: unsafe extern "system" fn(u32, u32),
    pub buffer_data: unsafe extern "system" fn(u32, isize, *const c_void, u32),
    pub vertex_attrib_pointer: unsafe extern "system" fn(u32, i32, u32, u8, i32, isize),
    pub enable_vertex_attrib_array: unsafe extern "system" fn(u32),
    pub disable_vertex_attrib_array: unsafe extern "system" fn(u32),
    pub active_texture: unsafe extern "system" fn(u32),
    pub swap_interval: unsafe extern "system" fn(i32) -> i32, // wglSwapIntervalEXT
    // core 1.1, linked directly from opengl32.dll
    pub enable: unsafe extern "system" fn(u32),
    pub disable: unsafe extern "system" fn(u32),
    pub clear_color: unsafe extern "system" fn(f32, f32, f32, f32),
    pub clear: unsafe extern "system" fn(u32),
    pub viewport: unsafe extern "system" fn(i32, i32, i32, i32),
    pub blend_func: unsafe extern "system" fn(u32, u32),
    pub cull_face: unsafe extern "system" fn(u32),
    pub depth_mask: unsafe extern "system" fn(u8),
    pub line_width: unsafe extern "system" fn(f32),
    pub pixel_storei: unsafe extern "system" fn(u32, i32),
    pub gen_textures: unsafe extern "system" fn(i32, *mut u32),
    pub delete_textures: unsafe extern "system" fn(i32, *const u32),
    pub bind_texture: unsafe extern "system" fn(u32, u32),
    pub tex_image_2d: unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void),
    pub tex_parameteri: unsafe extern "system" fn(u32, u32, i32),
    pub draw_elements: unsafe extern "system" fn(u32, i32, u32, *const c_void),
    pub draw_arrays: unsafe extern "system" fn(u32, i32, i32),
    pub get_error: unsafe extern "system" fn() -> u32,
}

thread_local! {
    static GL: Cell<Option<Gl>> = Cell::new(None);
}

/// Copy of the loaded GL function table.
pub fn g() -> Gl {
    GL.with(|c| c.get()).expect("OpenGL not initialized")
}

unsafe extern "system" fn no_swap_interval(_: i32) -> i32 {
    0
}

unsafe fn load<T>(name: &str) -> Result<T, String> {
    let c = CString::new(name).map_err(|_| format!("bad name {name}"))?;
    let p = super::win32::wgl_get_proc_address(c.as_ptr() as *const u8);
    if p.is_null() {
        return Err(format!("OpenGL function not found: {name}"));
    }
    Ok(std::mem::transmute_copy::<*mut c_void, T>(&p))
}

/// Must be called with a current GL context.
pub fn init() -> Result<(), String> {
    unsafe {
        let g = Gl {
            create_shader: load("glCreateShader")?,
            shader_source: load("glShaderSource")?,
            compile_shader: load("glCompileShader")?,
            get_shaderiv: load("glGetShaderiv")?,
            get_shader_info_log: load("glGetShaderInfoLog")?,
            create_program: load("glCreateProgram")?,
            attach_shader: load("glAttachShader")?,
            bind_attrib_location: load("glBindAttribLocation")?,
            link_program: load("glLinkProgram")?,
            get_programiv: load("glGetProgramiv")?,
            get_program_info_log: load("glGetProgramInfoLog")?,
            delete_shader: load("glDeleteShader")?,
            use_program: load("glUseProgram")?,
            get_uniform_location: load("glGetUniformLocation")?,
            uniform_matrix4fv: load("glUniformMatrix4fv")?,
            uniform1f: load("glUniform1f")?,
            uniform1i: load("glUniform1i")?,
            uniform3f: load("glUniform3f")?,
            uniform4f: load("glUniform4f")?,
            gen_buffers: load("glGenBuffers")?,
            delete_buffers: load("glDeleteBuffers")?,
            bind_buffer: load("glBindBuffer")?,
            buffer_data: load("glBufferData")?,
            vertex_attrib_pointer: load("glVertexAttribPointer")?,
            enable_vertex_attrib_array: load("glEnableVertexAttribArray")?,
            disable_vertex_attrib_array: load("glDisableVertexAttribArray")?,
            active_texture: load("glActiveTexture")?,
            swap_interval: load("wglSwapIntervalEXT").unwrap_or(no_swap_interval),
            enable: glEnable,
            disable: glDisable,
            clear_color: glClearColor,
            clear: glClear,
            viewport: glViewport,
            blend_func: glBlendFunc,
            cull_face: glCullFace,
            depth_mask: glDepthMask,
            line_width: glLineWidth,
            pixel_storei: glPixelStorei,
            gen_textures: glGenTextures,
            delete_textures: glDeleteTextures,
            bind_texture: glBindTexture,
            tex_image_2d: glTexImage2D,
            tex_parameteri: glTexParameteri,
            draw_elements: glDrawElements,
            draw_arrays: glDrawArrays,
            get_error: glGetError,
        };
        GL.with(|c| c.set(Some(g)));
        (g.swap_interval)(1); // vsync, best effort
    }
    Ok(())
}
