//! lgweb: skips GL calls that would not change GL state.
//!
//! On the web every GL call crosses from wasm into JavaScript, and macroquad re-applies the
//! framebuffer, viewport, scissor, program and depth state before each of its ~1000 draws a frame.
//! These shadow the raw functions inside `gl.rs`. They track only calls made through miniquad;
//! nothing else in macroquad or Looking Glass changes these states.
#![allow(non_snake_case)]

use crate::native::gl::{self, GLenum, GLint, GLsizei, GLuint};
use std::cell::Cell;

struct State {
    draw_framebuffer: Cell<Option<GLuint>>,
    read_framebuffer: Cell<Option<GLuint>>,
    viewport: Cell<Option<[GLint; 4]>>,
    scissor: Cell<Option<[GLint; 4]>>,
    program: Cell<Option<GLuint>>,
    scissor_test: Cell<Option<bool>>,
    depth_test: Cell<Option<bool>>,
    depth_func: Cell<Option<GLenum>>,
    front_face: Cell<Option<GLenum>>,
}

thread_local! {
    static STATE: State = const {
        State {
            draw_framebuffer: Cell::new(None),
            read_framebuffer: Cell::new(None),
            viewport: Cell::new(None),
            scissor: Cell::new(None),
            program: Cell::new(None),
            scissor_test: Cell::new(None),
            depth_test: Cell::new(None),
            depth_func: Cell::new(None),
            front_face: Cell::new(None),
        }
    };
}

/// Stores `value` and reports whether it differs from what GL last received.
fn changed<T: Copy + PartialEq>(cell: &Cell<Option<T>>, value: T) -> bool {
    let differs = cell.get() != Some(value);
    cell.set(Some(value));
    differs
}

pub unsafe fn glBindFramebuffer(target: GLenum, framebuffer: GLuint) {
    let send = STATE.with(|s| match target {
        gl::GL_FRAMEBUFFER => {
            let draw = changed(&s.draw_framebuffer, framebuffer);
            changed(&s.read_framebuffer, framebuffer) | draw
        }
        gl::GL_DRAW_FRAMEBUFFER => changed(&s.draw_framebuffer, framebuffer),
        gl::GL_READ_FRAMEBUFFER => changed(&s.read_framebuffer, framebuffer),
        _ => true,
    });
    if send {
        gl::glBindFramebuffer(target, framebuffer);
    }
}

/// Deleting a bound framebuffer rebinds 0: forget the bindings rather than mirror that rule.
pub unsafe fn glDeleteFramebuffers(n: GLsizei, framebuffers: *const GLuint) {
    STATE.with(|s| {
        s.draw_framebuffer.set(None);
        s.read_framebuffer.set(None);
    });
    gl::glDeleteFramebuffers(n, framebuffers);
}

pub unsafe fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
    if STATE.with(|s| changed(&s.viewport, [x, y, width, height])) {
        gl::glViewport(x, y, width, height);
    }
}

pub unsafe fn glScissor(x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
    if STATE.with(|s| changed(&s.scissor, [x, y, width, height])) {
        gl::glScissor(x, y, width, height);
    }
}

pub unsafe fn glUseProgram(program: GLuint) {
    if STATE.with(|s| changed(&s.program, program)) {
        gl::glUseProgram(program);
    }
}

pub unsafe fn glDeleteProgram(program: GLuint) {
    STATE.with(|s| s.program.set(None));
    gl::glDeleteProgram(program);
}

fn capability(s: &State, cap: GLenum) -> Option<&Cell<Option<bool>>> {
    match cap {
        gl::GL_SCISSOR_TEST => Some(&s.scissor_test),
        gl::GL_DEPTH_TEST => Some(&s.depth_test),
        _ => None,
    }
}

pub unsafe fn glEnable(cap: GLenum) {
    if STATE.with(|s| capability(s, cap).map_or(true, |c| changed(c, true))) {
        gl::glEnable(cap);
    }
}

pub unsafe fn glDisable(cap: GLenum) {
    if STATE.with(|s| capability(s, cap).map_or(true, |c| changed(c, false))) {
        gl::glDisable(cap);
    }
}

pub unsafe fn glDepthFunc(func: GLenum) {
    if STATE.with(|s| changed(&s.depth_func, func)) {
        gl::glDepthFunc(func);
    }
}

pub unsafe fn glFrontFace(mode: GLenum) {
    if STATE.with(|s| changed(&s.front_face, mode)) {
        gl::glFrontFace(mode);
    }
}
