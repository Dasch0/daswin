use pollster::block_on;
use std::borrow::Cow;
use std::ffi::{c_void, CString};
use std::mem;
use std::os::raw;
use std::ptr;
use wgpu;

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, XlibDisplayHandle,
    XlibWindowHandle,
};
use x11::xlib;

/// Provides a basic framework for connecting to an X Display,
/// creating a window, displaying it and running the event loop
pub struct X11Window {
    pub display: *mut xlib::Display,
    pub window: xlib::Window,
    pub screen: i32,

    wm_protocols: xlib::Atom,
    wm_delete_window: xlib::Atom,
}

impl X11Window {
    /// Create a new window with a given title and size
    pub fn new(title: &str, width: u32, height: u32) -> X11Window {
        unsafe {
            // Open display connection.
            let display = xlib::XOpenDisplay(ptr::null());

            if display.is_null() {
                panic!("XOpenDisplay failed");
            }

            // Create window.
            let screen = xlib::XDefaultScreen(display);
            let root = xlib::XRootWindow(display, screen);

            let mut attributes: xlib::XSetWindowAttributes =
                mem::MaybeUninit::uninit().assume_init();
            attributes.background_pixel = xlib::XWhitePixel(display, screen);

            let window = xlib::XCreateWindow(
                display,
                root,
                0,
                0,
                width,
                height,
                0,
                0,
                xlib::InputOutput as raw::c_uint,
                ptr::null_mut(),
                xlib::CWBackPixel,
                &mut attributes,
            );

            // Set window title.
            let title_str = CString::new(title).unwrap();
            xlib::XStoreName(display, window, title_str.as_ptr() as *mut raw::c_char);

            // Hook close requests.
            let wm_protocols_str = CString::new("WM_PROTOCOLS").unwrap();
            let wm_delete_window_str = CString::new("WM_DELETE_WINDOW").unwrap();

            let wm_protocols = xlib::XInternAtom(display, wm_protocols_str.as_ptr(), xlib::False);
            let wm_delete_window =
                xlib::XInternAtom(display, wm_delete_window_str.as_ptr(), xlib::False);

            let mut protocols = [wm_delete_window];

            xlib::XSetWMProtocols(
                display,
                window,
                protocols.as_mut_ptr(),
                protocols.len() as raw::c_int,
            );

            X11Window {
                display,
                window,
                screen,
                wm_protocols,
                wm_delete_window,
            }
        }
    }

    /// Display the window
    pub fn show(&mut self) {
        unsafe {
            xlib::XMapWindow(self.display, self.window);
        }
    }

    /// Poll for events
    pub fn poll(&mut self, event: &mut xlib::XEvent) {
        unsafe {
            while xlib::XPending(self.display) != 0 {
                xlib::XNextEvent(self.display, event);
                // discard events to other windows
                if xlib::XFilterEvent(event, self.window) != 0 {
                    continue;
                }
                match event.get_type() {
                    xlib::ClientMessage => {}
                    xlib::KeyPress => {}
                    xlib::KeyRelease => {}
                    xlib::ButtonPress => {}
                    xlib::ButtonRelease => {}
                    xlib::MotionNotify => {}
                    _ => {}
                }
            }
        };
    }
}

unsafe impl HasRawWindowHandle for X11Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut xlib_handle = XlibWindowHandle::empty();
        xlib_handle.visual_id = 0;
        xlib_handle.window = self.window;
        RawWindowHandle::Xlib(xlib_handle)
    }
}

unsafe impl HasRawDisplayHandle for X11Window {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut xlib_handle = XlibDisplayHandle::empty();
        xlib_handle.display = self.display as *mut c_void;
        xlib_handle.screen = self.screen;
        RawDisplayHandle::Xlib(xlib_handle)
    }
}

impl Drop for X11Window {
    /// Destroys the window and disconnects from the display
    fn drop(&mut self) {
        unsafe {
            xlib::XDestroyWindow(self.display, self.window);
            xlib::XCloseDisplay(self.display);
        }
    }
}

fn main() {
    let width = 800;
    let height = 600;
    let mut window = X11Window::new("hello-sailor", width, height);
    window.show();

    // init wgpu
    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let surface = unsafe { instance.create_surface(&window) };
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        // Request an adapter which can render to our surface
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
        },
        None,
    ))
    .expect("Failed to create device");

    // Load the shaders from disk
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let swapchain_format = surface.get_supported_formats(&adapter)[0];

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(swapchain_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: surface.get_supported_alpha_modes(&adapter)[0],
    };

    surface.configure(&device, &config);
    // Main loop.
    let mut event: xlib::XEvent = unsafe { mem::MaybeUninit::uninit().assume_init() };

    loop {
        window.poll(&mut event);

        match event.get_type() {
            xlib::ClientMessage => {
                let xclient = xlib::XClientMessageEvent::from(event);

                if xclient.message_type == window.wm_protocols && xclient.format == 32 {
                    let protocol = xclient.data.get_long(0) as xlib::Atom;

                    if protocol == window.wm_delete_window {
                        break;
                    }
                }
            }

            _ => (),
        }

        let frame = surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&render_pipeline);
            rpass.draw(0..3, 0..1);
        }

        queue.submit(Some(encoder.finish()));
        drop(view);
        drop(frame);
    }
}
