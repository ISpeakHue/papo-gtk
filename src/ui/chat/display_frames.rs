//! Inspect displayed pixels in the private X11 GTK regression window.
//! Unlike snapshot_child, this does not force GTK to rebuild render nodes.
use gtk::prelude::*;
use gtk::glib::translate::ToGlibPtr;
use std::ffi::{c_char,c_int,c_uint,c_ulong,c_void};

#[link(name="X11")]
extern "C" {
    fn XOpenDisplay(name:*const c_char)->*mut c_void;
    fn XCloseDisplay(display:*mut c_void)->c_int;
    fn XGetImage(display:*mut c_void,window:c_ulong,x:c_int,y:c_int,width:c_uint,height:c_uint,planes:c_ulong,format:c_int)->*mut c_void;
    fn XGetPixel(image:*mut c_void,x:c_int,y:c_int)->c_ulong;
    fn XDestroyImage(image:*mut c_void)->c_int;
}
extern "C" { fn gdk_x11_surface_get_xid(surface:*mut gtk::gdk::ffi::GdkSurface)->c_ulong; }

pub(super) struct DisplayFrames { display:*mut c_void,window:c_ulong }
impl DisplayFrames {
    pub fn new(window:&adw::Window)->Option<Self>{
        let surface=window.surface()?;
        eprintln!("displayed-channel regression: backend={}, renderer={}",surface.type_().name(),window.renderer().unwrap().type_().name());
        if surface.type_().name()!="GdkX11Toplevel" { return None; }
        // Only read the test window's drawable, never the desktop/root window.
        let display=unsafe { XOpenDisplay(std::ptr::null()) };
        if display.is_null(){return None;}
        Some(Self{display,window:unsafe { gdk_x11_surface_get_xid(surface.to_glib_none().0) }})
    }
    pub fn contains(&self,window:&adw::Window,widget:&gtk::Widget,color:u32)->bool{
        let bounds=widget.compute_bounds(window).unwrap();let scale=window.scale_factor();
        gtk::prelude::WidgetExt::display(window).sync();
        let width=(bounds.width() as i32*scale).max(1);let height=(bounds.height() as i32*scale).max(1);
        let image=unsafe { XGetImage(self.display,self.window,bounds.x() as i32*scale,bounds.y() as i32*scale,width as u32,height as u32,!0,2) };
        assert!(!image.is_null(),"private window pixels must be readable");
        let mut found=false;
        'pixels:for y in (0..height).step_by(4){for x in (0..width).step_by(4){
            if unsafe { XGetPixel(image,x,y) }&0xffffff==color as c_ulong {found=true;break 'pixels;}
        }}
        unsafe { XDestroyImage(image); } found
    }
}
impl Drop for DisplayFrames { fn drop(&mut self){unsafe { XCloseDisplay(self.display); }} }
