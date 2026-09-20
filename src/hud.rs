// HUD: crosshair, hotbar with block icons, hearts, air bubbles, selection wireframe.
use crate::gl;
use crate::math::Mat4;
use crate::raycast::RayHit;
use crate::renderer::{push_rect, Renderer};
use crate::world::tiles_of;

const SLOT: f32 = 22.0; // slot pitch in px
const ICON: f32 = 18.0;
const BAR_H: f32 = 20.0;

/// Draw one textured quad as 6 vertices (8 floats: x,y,u,v + biome-agnostic
/// icon tint so grayscale tiles like grass render green in the hotbar).
fn push_icon(v: &mut Vec<f32>, x0: f32, y0: f32, size: f32, tile: u32) {
    use crate::world::{icon_tint, ATLAS_COLS, ATLAS_ROWS};
    let tsu = 1.0 / ATLAS_COLS as f32;
    let tsv = 1.0 / ATLAS_ROWS as f32;
    let tu = (tile % ATLAS_COLS) as f32 * tsu;
    let tv = (tile / ATLAS_COLS) as f32 * tsv;
    let (x1, y1) = (x0 + size, y0 + size);
    let tint = icon_tint(tile);
    let quad = |ax: f32, ay: f32, au: f32, av: f32, out: &mut Vec<f32>| {
        out.extend_from_slice(&[ax, ay, 0.0, au, av, tint[0], tint[1], tint[2]]);
    };
    let (u0, v0) = (tu + 0.002, tv + 0.002);
    let (u1, v1) = (tu + tsu - 0.002, tv + tsv - 0.002);
    quad(x0, y0, u0, v0, v);
    quad(x0, y1, u0, v1, v);
    quad(x1, y1, u1, v1, v);
    quad(x0, y0, u0, v0, v);
    quad(x1, y1, u1, v1, v);
    quad(x1, y0, u1, v0, v);
}

/// 7x7 heart sprite, bit 6 = leftmost pixel.
const HEART: [u8; 7] = [
    0b0110110, 0b1111111, 0b1111111, 0b1111111, 0b0111110, 0b0011100, 0b0001000,
];
/// 7x7 air bubble sprite.
const BUBBLE: [u8; 7] = [
    0b0011100, 0b0111110, 0b1101111, 0b1011111, 0b1111111, 0b0111110, 0b0011100,
];

/// Draw a 7x7 bitmap sprite as horizontal pixel runs (color program must be active).
fn push_sprite(v: &mut Vec<f32>, bmp: &[u8; 7], x0: f32, y0: f32, s: f32, max_col: i32) {
    for (row, &bits) in bmp.iter().enumerate() {
        let mut x = 0i32;
        while x < 7 {
            if (bits >> (6 - x)) & 1 == 1 && x < max_col {
                let mut run = 1;
                while x + run < 7
                    && run + x < max_col
                    && (bits >> (6 - x - run)) & 1 == 1
                {
                    run += 1;
                }
                push_rect(
                    v,
                    x0 + x as f32 * s,
                    y0 + row as f32 * s,
                    x0 + (x + run) as f32 * s,
                    y0 + (row + 1) as f32 * s,
                );
                x += run;
            } else {
                x += 1;
            }
        }
    }
}

pub fn draw_hud(
    r: &Renderer,
    ortho: &Mat4,
    hotbar: &[u16; 9],
    slot: usize,
    w: i32,
    h: i32,
    hp: i32,
    air_bubbles: i32,
    dead: bool,
) {
    let gl = &r.gl;
    unsafe {
        let (w, h) = (w as f32, h as f32);

        // ---- crosshair (color program)
        (gl.use_program)(r.prog_c);
        (gl.enable)(gl::GL_BLEND);
        (gl.blend_func)(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
        (gl.uniform_matrix4fv)(r.cu_mvp, 1, gl::GL_FALSE, ortho.as_ptr());
        (gl.uniform3f)(r.cu_off, 0.0, 0.0, 0.0);
        (gl.uniform4f)(r.cu_color, 0.95, 0.95, 0.95, 0.85);
        (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, r.dyn_vbo);
        let mut cross = Vec::with_capacity(36);
        let (cx, cy) = (w * 0.5, h * 0.5);
        push_rect(&mut cross, cx - 6.0, cy - 1.0, cx + 6.0, cy + 1.0);
        push_rect(&mut cross, cx - 1.0, cy - 6.0, cx + 1.0, cy + 6.0);
        (gl.buffer_data)(
            gl::GL_ARRAY_BUFFER,
            (cross.len() * 4) as isize,
            cross.as_ptr() as *const _,
            gl::GL_DYNAMIC_DRAW,
        );
        r.set_pos_attrib_pub();
        (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (cross.len() / 3) as i32);

        // ---- hotbar background
        let n = hotbar.len() as f32;
        let bw = n * SLOT + 4.0;
        let bx0 = (w - bw) * 0.5;
        let by0 = h - BAR_H - 6.0;
        (gl.uniform4f)(r.cu_color, 0.05, 0.05, 0.08, 0.6);
        let mut bg = Vec::with_capacity(18 * 3);
        push_rect(&mut bg, bx0, by0, bx0 + bw, by0 + BAR_H + 2.0);
        (gl.buffer_data)(
            gl::GL_ARRAY_BUFFER,
            (bg.len() * 4) as isize,
            bg.as_ptr() as *const _,
            gl::GL_DYNAMIC_DRAW,
        );
        (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (bg.len() / 3) as i32);

        // ---- selected slot frame
        let sx0 = bx0 + 2.0 + slot as f32 * SLOT;
        (gl.uniform4f)(r.cu_color, 1.0, 1.0, 1.0, 0.9);
        let mut frame = Vec::with_capacity(4 * 18 * 3);
        let sy0 = by0;
        let sy1 = sy0 + BAR_H + 2.0;
        let fx1 = sx0 + SLOT;
        push_rect(&mut frame, sx0, sy0, fx1, sy0 + 2.0);
        push_rect(&mut frame, sx0, sy1 - 2.0, fx1, sy1);
        push_rect(&mut frame, sx0, sy0, sx0 + 2.0, sy1);
        push_rect(&mut frame, fx1 - 2.0, sy0, fx1, sy1);
        (gl.buffer_data)(
            gl::GL_ARRAY_BUFFER,
            (frame.len() * 4) as isize,
            frame.as_ptr() as *const _,
            gl::GL_DYNAMIC_DRAW,
        );
        (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (frame.len() / 3) as i32);

        // ---- hearts (10 = 20 hp), above hotbar left
        let hy0 = by0 - 20.0;
        for i in 0..10 {
            let hx0 = bx0 + 2.0 + i as f32 * 17.0;
            // container (dark)
            (gl.uniform4f)(r.cu_color, 0.08, 0.05, 0.06, 0.85);
            let mut sp = Vec::with_capacity(64 * 3);
            push_sprite(&mut sp, &HEART, hx0, hy0, 2.0, 7);
            (gl.buffer_data)(
                gl::GL_ARRAY_BUFFER,
                (sp.len() * 4) as isize,
                sp.as_ptr() as *const _,
                gl::GL_DYNAMIC_DRAW,
            );
            (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (sp.len() / 3) as i32);
            // filled part
            let fill = (hp - i * 2).clamp(0, 2);
            if fill > 0 && !dead {
                (gl.uniform4f)(r.cu_color, 0.9, 0.15, 0.18, 0.95);
                let mut sp = Vec::with_capacity(64 * 3);
                push_sprite(&mut sp, &HEART, hx0, hy0, 2.0, if fill == 2 { 7 } else { 4 });
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (sp.len() * 4) as isize,
                    sp.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (sp.len() / 3) as i32);
            }
        }

        // ---- air bubbles (right, only when submerged)
        if air_bubbles < 10 {
            for i in 0..10 {
                let bx = bx0 + bw - 2.0 - (10 - i) as f32 * 15.0;
                if i < air_bubbles {
                    (gl.uniform4f)(r.cu_color, 0.5, 0.75, 1.0, 0.95);
                    let mut sp = Vec::with_capacity(64 * 3);
                    push_sprite(&mut sp, &BUBBLE, bx, hy0, 2.0, 7);
                    (gl.buffer_data)(
                        gl::GL_ARRAY_BUFFER,
                        (sp.len() * 4) as isize,
                        sp.as_ptr() as *const _,
                        gl::GL_DYNAMIC_DRAW,
                    );
                    (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (sp.len() / 3) as i32);
                }
            }
        }
        (gl.disable)(gl::GL_BLEND);

        // ---- block icons (textured, main program)
        (gl.use_program)(r.prog);
        (gl.uniform_matrix4fv)(r.u_mvp, 1, gl::GL_FALSE, ortho.as_ptr());
        (gl.uniform1f)(r.u_fog_near, 1.0e9);
        (gl.uniform1f)(r.u_fog_far, 2.0e9);
        (gl.uniform1f)(r.u_alpha, 1.0);
        (gl.uniform1f)(r.u_light, 1.0);
        let mut icons = Vec::with_capacity(9 * 36);
        for (i, &b) in hotbar.iter().enumerate() {
            let ix = bx0 + 2.0 + i as f32 * SLOT + (SLOT - ICON) * 0.5;
            let iy = by0 + 1.0 + (BAR_H - ICON) * 0.5;
            push_icon(&mut icons, ix, iy, ICON, tiles_of(b)[1]);
        }
        (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, r.dyn_vbo);
        (gl.buffer_data)(
            gl::GL_ARRAY_BUFFER,
            (icons.len() * 4) as isize,
            icons.as_ptr() as *const _,
            gl::GL_DYNAMIC_DRAW,
        );
        r.set_full_attribs_pub();
        (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (icons.len() / 6) as i32);
    }
}

pub fn draw_selection(r: &Renderer, mvp: &Mat4, hit: &RayHit) {
    let gl = &r.gl;
    unsafe {
        (gl.use_program)(r.prog_c);
        (gl.uniform_matrix4fv)(r.cu_mvp, 1, gl::GL_FALSE, mvp.as_ptr());
        (gl.uniform3f)(r.cu_off, hit.x as f32, hit.y as f32, hit.z as f32);
        (gl.uniform4f)(r.cu_color, 0.0, 0.0, 0.0, 0.9);
        (gl.enable)(gl::GL_BLEND);
        (gl.blend_func)(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
        (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, r.cube_vbo);
        r.set_pos_attrib_pub();
        (gl.line_width)(2.0);
        (gl.draw_arrays)(gl::GL_LINES, 0, 24);
        (gl.line_width)(1.0);
        (gl.disable)(gl::GL_BLEND);
    }
}
