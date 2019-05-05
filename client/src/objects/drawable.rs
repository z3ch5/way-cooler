//! A wrapper around a Cairo image surface.

use std::default::Default;

use cairo::{Format, ImageSurface};
use glib::translate::ToGlibPtr;
use rlua::{self, Context, LightUserData, Table, UserData, UserDataMethods, Value};

use crate::area::{Area, Origin, Size};
use crate::common::{
    class::{self, Class},
    object::{self, Object},
    property::Property
};
use crate::wayland_obj::{create_buffer, Buffer};

use super::drawin::Drawin;

pub struct DrawableState {
    // wayland_shell: Option<LayerSurface>,
    surface: Option<ImageSurface>,
    geo: Area,
    // TODO Use this to determine whether we draw this or not
    refreshed: bool,
    buffer: Option<Buffer>
}

pub type Drawable<'lua> = Object<'lua, DrawableState>;

impl Default for DrawableState {
    fn default() -> Self {
        DrawableState {
            surface: None,
            geo: Area::default(),
            refreshed: false,
            buffer: None
        }
    }
}

impl<'lua> Drawable<'lua> {
    pub fn new(lua: rlua::Context<'lua>) -> rlua::Result<Drawable> {
        let class = class::class_setup(lua, "drawable")?;
        let builder = Drawable::allocate(lua, class)?;
        // TODO Do properly
        let table = lua.create_table()?;
        table.set("geometry", lua.create_function(geometry)?)?;
        table.set("refresh", lua.create_function(refresh)?)?;
        Ok(builder.add_to_meta(table)?.build())
    }

    pub fn get_geometry(&self) -> rlua::Result<Area> {
        let drawable = self.state()?;
        Ok(drawable.geo)
    }

    pub fn get_surface(&self) -> rlua::Result<Value<'lua>> {
        let drawable = self.state()?;
        trace!("get_surface -> {}", drawable.surface.is_some());
        Ok(match drawable.surface {
            None => Value::Nil,
            Some(ref image) => {
                let stash = image.to_glib_none();
                let ptr = stash.0;
                // NOTE
                // We bump the reference count because now Lua has a reference which
                // it manages via LGI.
                //
                // If there's a bug, worst case scenario there's a memory leak.
                unsafe {
                    ::cairo_sys::cairo_surface_reference(ptr);
                }
                Value::LightUserData(LightUserData(ptr as _))
            }
        })
    }

    /// Sets the geometry, and allocates a new surface.
    pub fn set_geometry(&mut self, lua: rlua::Context<'lua>, geometry: Area) -> rlua::Result<()> {
        use rlua::Error::RuntimeError;
        let mut drawable = self.state_mut()?;
        let size_changed = drawable.geo != geometry;
        drawable.geo = geometry;
        if size_changed {
            drawable.surface = None;
            drawable.refreshed = false;
        }
        let size: Size = geometry.size;

        if size_changed && size.width > 0 && size.height > 0 {
            drawable.surface = Some(
                ImageSurface::create(Format::ARgb32, size.width as i32, size.height as i32)
                    .map_err(|err| RuntimeError(format!("Could not allocate {:?}", err)))?
            );

            drawable.buffer = Some(create_buffer(size).expect("Could not create buffer"));
        }
        drop(drawable);
        // TODO(ried): conditionally emit signals only for changed properties
        Object::emit_signal(lua, self, "property::surface", Value::Nil)?;
        Object::emit_signal(lua, self, "property::geometry", Value::Nil)?;
        Object::emit_signal(lua, self, "property::x", Value::Nil)?;
        Object::emit_signal(lua, self, "property::y", Value::Nil)?;
        Object::emit_signal(lua, self, "property::width", Value::Nil)?;
        Object::emit_signal(lua, self, "property::height", Value::Nil)?;
        Ok(())
    }

    /// Signals that the drawable's surface was updated.
    pub fn refresh(&mut self, _lua: Context) -> rlua::Result<()> {
        trace!("drawable::refresh");
        if self.state()?.refreshed {
            warn!("Drawable is already refreshed. Skipping work");
            return Ok(());
        }

        {
            let mut state = self.state_mut()?;
            let drawable = &mut *state;
            if let Some(data) = drawable.surface.as_ref().map(get_data) {
                drawable.buffer.as_mut().expect("No buffer available").write(data);
            }
            drawable.refreshed = true;
        }

        let geo = self.state()?.geo;

        error!("calling callback");
        let res = self
            .get_associated_data::<Drawin>("drawin")?
            .refresh_pixmap(self.state()?.buffer.as_ref().unwrap(), geo);

        info!("callback done");
        res
    }
}

impl UserData for DrawableState {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        object::default_add_methods(methods);
    }
}

pub fn init(lua: rlua::Context) -> rlua::Result<Class<DrawableState>> {
    Class::<DrawableState>::builder(lua, "drawable", None)?
        .property(Property::new(
            "surface".into(),
            None,
            Some(lua.create_function(get_surface)?),
            None
        ))?
        .save_class("drawable")?
        .build()
}

fn get_surface<'lua>(_: rlua::Context<'lua>, drawable: Drawable<'lua>) -> rlua::Result<Value<'lua>> {
    drawable.get_surface()
}

fn geometry<'lua>(lua: rlua::Context<'lua>, drawable: Drawable<'lua>) -> rlua::Result<Table<'lua>> {
    let geometry = drawable.get_geometry()?;
    let Origin { x, y } = geometry.origin;
    let Size { width, height } = geometry.size;
    let table = lua.create_table()?;
    table.set("x", x)?;
    table.set("y", y)?;
    table.set("width", width)?;
    table.set("height", height)?;
    Ok(table)
}

fn refresh<'lua>(lua: Context<'lua>, mut drawable: Drawable<'lua>) -> rlua::Result<()> {
    drawable.refresh(lua)
}

/// Get the data associated with the ImageSurface.
fn get_data(surface: &ImageSurface) -> &[u8] {
    // NOTE This is safe to do because there's one thread.
    //
    // We know Lua is not modifying it because it's not running.
    use cairo_sys;
    use std::slice;
    unsafe {
        let len = surface.get_stride() as usize * surface.get_height() as usize;
        let surface = surface.to_glib_none().0;
        slice::from_raw_parts(cairo_sys::cairo_image_surface_get_data(surface as _), len)
    }
}
