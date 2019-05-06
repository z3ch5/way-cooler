//! A wrapper around a drawable. This controls all the other state about
//! the surface, such as the cursor used or where it on the screen.

// NOTE need to store the drawable in lua, because it's a reference to a
// drawable a lua object

use rlua::{self, prelude::LuaInteger, Context, Table, ToLua, UserData, UserDataMethods, Value};

use crate::{
    area::{Area, Margin, Origin, Size},
    common::{
        class::{self, Class, ClassBuilder},
        object::{self, Object, ObjectBuilder},
        property::Property
    },
    wayland_obj::{create_layer_surface, Buffer, LayerSurface}
};

use super::drawable::Drawable;

pub const DRAWINS_HANDLE: &'static str = "__drawins";

#[derive(Default)]
pub struct DrawinState {
    // Note that the drawable is stored in Lua.
    // TODO WINDOW_OBJECT_HEADER??
    visible: bool,
    geometry: Area,
    /// Do we have a pending geometry change that still needs to be applied?
    geometry_dirty: bool,

    surface: Option<LayerSurface>,
    struts: Margin
}

impl DrawinState {
    /// Update the geometry of this drawin and return the signals to be emitted.
    fn update_geometry<'lua>(
        &mut self,
        Area {
            size: Size {
                width: new_width,
                height: new_height
            },
            origin: Origin { x: new_x, y: new_y }
        }: Area
    ) -> Vec<(&'static str, rlua::Value<'lua>)> {
        let mut signals = vec![];
        let Area {
            size: Size {
                ref mut width,
                ref mut height
            },
            origin: Origin { ref mut x, ref mut y }
        } = self.geometry;

        if new_width > 0 && new_width != *width {
            *width = new_width;
            signals.push(("property::width", rlua::Value::Nil));
        }

        if new_height > 0 && new_height != *height {
            *height = new_height;
            signals.push(("property::height", rlua::Value::Nil));
        }

        if new_x != *x {
            *x = new_x;
            signals.push(("property::x", rlua::Value::Nil));
        }

        if new_y != *y {
            *y = new_y;
            signals.push(("property::y", rlua::Value::Nil));
        }

        if signals.len() > 0 {
            self.geometry_dirty = true;
            signals.push(("property::geometry", rlua::Value::Nil));
        }
        signals
    }
}

unsafe impl Send for DrawinState {}

pub type Drawin<'lua> = Object<'lua, DrawinState>;

impl UserData for DrawinState {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        object::default_add_methods(methods);
    }
}

impl<'lua> Drawin<'lua> {
    fn new(lua: rlua::Context<'lua>, args: Table<'lua>) -> rlua::Result<Drawin<'lua>> {
        let class = class::class_setup(lua, "drawin")?;
        let mut drawins = lua.named_registry_value::<str, Vec<Drawin>>(DRAWINS_HANDLE)?;
        let mut drawin = object_setup(lua, Drawin::allocate(lua, class)?)?
            .handle_constructor_argument(args)?
            .build();
        drawin.create_shell()?;
        drawin
            .drawable()?
            .set_associated_data::<Drawin>("drawin", drawin.clone())?;
        drawins.push(drawin.clone());
        lua.set_named_registry_value(DRAWINS_HANDLE, drawins.to_lua(lua)?)?;
        Ok(drawin)
    }

    pub(crate) fn refresh_pixmap(&mut self, buffer: &Buffer, geometry: Area) -> rlua::Result<()> {
        {
            let mut state = self.state_mut()?;
            let shell = state.surface.as_mut().unwrap();
            shell.set_size(geometry.size);

            shell.set_buffer(buffer, geometry.origin);

            shell.commit();
        }

        error!("refresh_pixmap done :)");
        Ok(())
    }

    /// Get the drawable associated with this drawin.
    ///
    /// It has the surface that is needed to render to the screen.
    pub fn drawable(&mut self) -> rlua::Result<Drawable<'lua>> {
        self.get_associated_data::<Drawable>("drawable")
    }

    fn create_shell(&mut self) -> rlua::Result<()> {
        let mut state = self.state_mut()?;
        if state.surface.is_some() {
            panic!("Surface already initialized.");
        }
        state.surface = Some(create_layer_surface().expect("Could not construct layer surface"));
        Ok(())
    }

    fn update_drawing(&mut self, lua: rlua::Context<'lua>) -> rlua::Result<()> {
        trace!("drawin::update_drawing");
        let mut state = self.state_mut()?;
        let DrawinState {
            ref mut surface,
            geometry,
            ..
        } = *state;
        if surface.is_none() {
            // NOTE: this happens because handle_constructor_argument() may trigger update_drawing
            // before the shell is created.....
            warn!("update_drawing ignored since surface is not initialized");
            return Ok(());
        }
        let surface = surface.as_mut().expect("Surface not initialized");
        surface.set_size(geometry.size);
        drop(surface);
        drop(state);

        let mut drawable = self.drawable()?;

        drawable.set_geometry(lua, geometry)?;
        self.set_associated_data("drawable", drawable)?;
        Ok(())
    }

    pub fn get_visible(&mut self) -> rlua::Result<bool> {
        let drawin = self.state()?;
        Ok(drawin.visible)
    }

    fn set_visible(&mut self, lua: rlua::Context<'lua>, val: bool) -> rlua::Result<()> {
        let mut state = self.state_mut()?;
        trace!("set_visible(): {}, old: {}", val, state.visible);
        if val == state.visible {
            return Ok(());
        }

        state.visible = val;

        drop(state);

        if val {
            self.map(lua)?
        } else {
            self.unmap(lua)?
        }

        trace!("will signal property::visible");

        Object::emit_signal(lua, self, "property::visible", Value::Nil)?;

        Ok(())
    }

    fn map(&mut self, lua: rlua::Context<'lua>) -> rlua::Result<()> {
        // TODO other things
        trace!("drawin::map");
        self.update_drawing(lua)?;
        Ok(())
    }

    fn unmap(&mut self, lua: rlua::Context<'lua>) -> rlua::Result<()> {
        // TODO?
        trace!("drawin::unmap");
        self.update_drawing(lua)?;
        Ok(())
    }

    pub fn get_geometry(&self) -> rlua::Result<Area> {
        Ok(self.state()?.geometry)
    }

    /// Move and/or resize a drawin
    fn resize(&mut self, lua: rlua::Context<'lua>, geometry: Area) -> rlua::Result<()> {
        trace!("drawin::resize");
        let signals = {
            let mut state = self.state_mut()?;
            state.update_geometry(geometry)
        };

        if signals.len() > 0 {
            self.update_drawing(lua)?;
        }

        // TODO update screen workareas like in awesome? Might not be necessary
        for (signal, args) in signals {
            trace!("drawin::resize: signaling {}", signal);
            Object::emit_signal(lua, self, &signal, args)?;
        }

        Ok(())
    }

    fn set_struts(&mut self, _lua: rlua::Context<'lua>, struts: Margin) -> rlua::Result<()> {
        trace!("set_struts({:?})", struts);
        if struts == self.state()?.struts {
            return Ok(());
        }

        self.state_mut()?.struts = struts;

        // TODO(ried): do something with the struts and emit screen change workarea event
        if struts != Margin::default() {
            warn!("Struts are not implemented, ignoring ({:?})", struts);
        }

        Ok(())
    }

    fn get_struts(&mut self) -> rlua::Result<Margin> {
        trace!("get_struts");
        Ok(self.state()?.struts)
    }
}

pub fn init(lua: rlua::Context) -> rlua::Result<Class<DrawinState>> {
    let drawins: Vec<Drawin> = Vec::new();
    lua.set_named_registry_value(DRAWINS_HANDLE, drawins.to_lua(lua)?)?;
    property_setup(lua, method_setup(lua, Class::builder(lua, "drawin", None)?)?)?
        .save_class("drawin")?
        .build()
}

fn method_setup<'lua>(
    lua: rlua::Context<'lua>,
    builder: ClassBuilder<'lua, DrawinState>
) -> rlua::Result<ClassBuilder<'lua, DrawinState>> {
    // TODO Do properly
    builder
           // TODO This should be adding properties, e.g like luaA_class_new
           .method("__call".into(), lua.create_function( Drawin::new)?)
}

fn property_setup<'lua>(
    lua: rlua::Context<'lua>,
    builder: ClassBuilder<'lua, DrawinState>
) -> rlua::Result<ClassBuilder<'lua, DrawinState>> {
    builder
        .property(Property::new(
            "x".into(),
            Some(lua.create_function(set_x)?),
            Some(lua.create_function(get_x)?),
            Some(lua.create_function(set_x)?)
        ))?
        .property(Property::new(
            "y".into(),
            Some(lua.create_function(set_y)?),
            Some(lua.create_function(get_y)?),
            Some(lua.create_function(set_y)?)
        ))?
        .property(Property::new(
            "width".into(),
            Some(lua.create_function(set_width)?),
            Some(lua.create_function(get_width)?),
            Some(lua.create_function(set_width)?)
        ))?
        .property(Property::new(
            "height".into(),
            Some(lua.create_function(set_height)?),
            Some(lua.create_function(get_height)?),
            Some(lua.create_function(set_height)?)
        ))?
        .property(Property::new(
            "visible".into(),
            Some(lua.create_function(set_visible)?),
            Some(lua.create_function(get_visible)?),
            Some(lua.create_function(set_visible)?)
        ))
}

fn object_setup<'lua>(
    lua: rlua::Context<'lua>,
    builder: ObjectBuilder<'lua, DrawinState>
) -> rlua::Result<ObjectBuilder<'lua, DrawinState>> {
    // TODO Do properly
    let table = lua.create_table()?;
    let drawable_table = Drawable::new(lua)?.to_lua(lua)?;
    table.set("drawable", drawable_table)?;
    table.set("geometry", lua.create_function(drawin_geometry)?)?;
    table.set("struts", lua.create_function(drawin_struts)?)?;
    table.set("buttons", lua.create_function(super::dummy)?)?;
    builder.add_to_meta(table)
}

fn set_visible<'lua>(
    lua: rlua::Context<'lua>,
    (mut drawin, visible): (Drawin<'lua>, bool)
) -> rlua::Result<()> {
    drawin.set_visible(lua, visible)
    // TODO signal
}

fn get_visible<'lua>(_: rlua::Context<'lua>, mut drawin: Drawin<'lua>) -> rlua::Result<bool> {
    drawin.get_visible()
    // TODO signal
}

fn drawin_geometry<'lua>(
    lua: rlua::Context<'lua>,
    (mut drawin, geometry): (Drawin<'lua>, Option<Table<'lua>>)
) -> rlua::Result<Table<'lua>> {
    trace!("drawin::geometry");
    if let Some(geometry) = geometry {
        let width = geometry.get::<_, u32>("width")?;
        let height = geometry.get::<_, u32>("height")?;
        let x = geometry.get::<_, i32>("x")?;
        let y = geometry.get::<_, i32>("y")?;
        if width > 0 && height > 0 {
            let geo = Area {
                origin: Origin { x, y },
                size: Size { width, height }
            };
            trace!("updating geometry to: {}x{}@{},{}", width, height, x, y);
            drawin.resize(lua, geo)?;
        }
    }
    let new_geo = drawin.get_geometry()?;
    let Size { width, height } = new_geo.size;
    let Origin { x, y } = new_geo.origin;
    let res = lua.create_table()?;
    res.set("x", x)?;
    res.set("y", y)?;
    res.set("height", height)?;
    res.set("width", width)?;
    Ok(res)
}

fn get_x<'lua>(_: rlua::Context<'lua>, drawin: Drawin<'lua>) -> rlua::Result<LuaInteger> {
    let Origin { x, .. } = drawin.get_geometry()?.origin;
    Ok(x as LuaInteger)
}

fn set_x<'lua>(lua: rlua::Context<'lua>, (mut drawin, x): (Drawin<'lua>, LuaInteger)) -> rlua::Result<()> {
    trace!("drawin::set_x");
    let mut geo = drawin.get_geometry()?.clone();
    geo.origin.x = x as i32;
    drawin.resize(lua, geo)?;
    Ok(())
}

fn get_y<'lua>(_: rlua::Context<'lua>, drawin: Drawin<'lua>) -> rlua::Result<LuaInteger> {
    let Origin { y, .. } = drawin.get_geometry()?.origin;
    Ok(y as LuaInteger)
}

fn set_y<'lua>(lua: rlua::Context<'lua>, (mut drawin, y): (Drawin<'lua>, LuaInteger)) -> rlua::Result<()> {
    trace!("drawin::set_y");
    let mut geo = drawin.get_geometry()?.clone();
    geo.origin.y = y as i32;
    drawin.resize(lua, geo)?;
    Ok(())
}

fn get_width<'lua>(_: rlua::Context<'lua>, drawin: Drawin<'lua>) -> rlua::Result<LuaInteger> {
    let Size { width, .. } = drawin.get_geometry()?.size;
    Ok(width as LuaInteger)
}

fn set_width<'lua>(
    lua: rlua::Context<'lua>,
    (mut drawin, width): (Drawin<'lua>, LuaInteger)
) -> rlua::Result<()> {
    trace!("drawin::set_width");
    let mut geo = drawin.get_geometry()?.clone();
    if width > 0 {
        geo.size.width = width as u32;
        drawin.resize(lua, geo)?;
    }
    Ok(())
}

fn get_height<'lua>(_lua: rlua::Context<'lua>, drawin: Drawin<'lua>) -> rlua::Result<LuaInteger> {
    let Size { height, .. } = drawin.get_geometry()?.size;
    Ok(height as LuaInteger)
}

fn set_height<'lua>(
    lua: rlua::Context<'lua>,
    (mut drawin, height): (Drawin<'lua>, LuaInteger)
) -> rlua::Result<()> {
    trace!("drawin::set_height");
    let mut geo = drawin.get_geometry()?;
    if height > 0 {
        geo.size.height = height as u32;
        drawin.resize(lua, geo)?;
    }
    Ok(())
}

fn drawin_struts<'lua>(
    lua: Context<'lua>,
    (mut drawin, struts): (Drawin<'lua>, Option<Margin>)
) -> rlua::Result<Value<'lua>> {
    if let Some(struts) = struts {
        drawin.set_struts(lua, struts)?;
    }

    drawin.get_struts()?.to_lua(lua)
}
