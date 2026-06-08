use mlua::{UserDataFields, Value};

use crate::game::lua::types::{Color3, UDim, Vector3};

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
    // ========== BillboardGui Properties ==========

    fields.add_field_method_get("StudsOffset", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.billboard_gui_data.as_ref().map(|b| b.studs_offset))
    });
    fields.add_field_method_set("StudsOffset", |_, this, offset: Vector3| {
        let mut data = this.data.lock().unwrap();
        if let Some(billboard) = &mut data.billboard_gui_data {
            billboard.studs_offset = offset;
        }
        Ok(())
    });

    fields.add_field_method_get("AlwaysOnTop", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.billboard_gui_data.as_ref().map(|b| b.always_on_top))
    });
    fields.add_field_method_set("AlwaysOnTop", |_, this, value: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(billboard) = &mut data.billboard_gui_data {
            billboard.always_on_top = value;
        }
        Ok(())
    });

    fields.add_field_method_get("Adornee", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .billboard_gui_data
            .as_ref()
            .and_then(|b| b.adornee.as_ref())
            .and_then(|w| w.upgrade())
            .map(Instance::from_ref))
    });
    fields.add_field_method_set("Adornee", |_, this, part: Option<Instance>| {
        let mut data = this.data.lock().unwrap();
        if let Some(billboard) = &mut data.billboard_gui_data {
            billboard.adornee = part.map(|p| Arc::downgrade(&p.data));
        }
        Ok(())
    });

    // ========== GUI Properties ==========

    // AnchorPoint (GuiObject)
    fields.add_field_method_get("AnchorPoint", |lua, this| {
        let data = this.data.lock().unwrap();
        if let Some(gui) = &data.gui_data {
            let table = lua.create_table()?;
            table.set("X", gui.anchor_point.0)?;
            table.set("Y", gui.anchor_point.1)?;
            Ok(Value::Table(table))
        } else {
            Ok(Value::Nil)
        }
    });
    fields.add_field_method_set("AnchorPoint", |_, this, value: Value| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            if let Value::Table(t) = value {
                let x: f32 = t.get("X").unwrap_or(0.0);
                let y: f32 = t.get("Y").unwrap_or(0.0);
                gui.anchor_point = (x, y);
            }
        }
        Ok(())
    });

    // Rotation (GuiObject) - in degrees
    fields.add_field_method_get("Rotation", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.rotation))
    });
    fields.add_field_method_set("Rotation", |_, this, rotation: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.rotation = rotation;
        }
        Ok(())
    });

    // ZIndex (GuiObject)
    fields.add_field_method_get("ZIndex", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.z_index))
    });
    fields.add_field_method_set("ZIndex", |_, this, z_index: i32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.z_index = z_index;
        }
        Ok(())
    });

    // LayoutOrder (GuiObject)
    fields.add_field_method_get("LayoutOrder", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.layout_order))
    });
    fields.add_field_method_set("LayoutOrder", |_, this, layout_order: i32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.layout_order = layout_order;
        }
        Ok(())
    });

    // Visible (GuiObject)
    fields.add_field_method_get("Visible", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.visible))
    });
    fields.add_field_method_set("Visible", |_, this, visible: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.visible = visible;
        }
        Ok(())
    });

    // BackgroundColor3 (GuiObject)
    fields.add_field_method_get("BackgroundColor3", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.background_color))
    });
    fields.add_field_method_set("BackgroundColor3", |_, this, color: Color3| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.background_color = color;
        }
        Ok(())
    });

    // BackgroundTransparency (GuiObject)
    fields.add_field_method_get("BackgroundTransparency", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.background_transparency))
    });
    fields.add_field_method_set("BackgroundTransparency", |_, this, transparency: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.background_transparency = transparency.clamp(0.0, 1.0);
        }
        Ok(())
    });

    // BorderColor3 (GuiObject)
    fields.add_field_method_get("BorderColor3", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.border_color))
    });
    fields.add_field_method_set("BorderColor3", |_, this, color: Color3| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.border_color = color;
        }
        Ok(())
    });

    // BorderSizePixel (GuiObject)
    fields.add_field_method_get("BorderSizePixel", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.border_size_pixel))
    });
    fields.add_field_method_set("BorderSizePixel", |_, this, size: i32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.border_size_pixel = size.max(0);
        }
        Ok(())
    });

    // Text (TextLabel, TextButton)
    fields.add_field_method_get("Text", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.text.clone()))
    });
    fields.add_field_method_set("Text", |_, this, text: String| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.text = Some(text);
        }
        Ok(())
    });

    // TextColor3 (TextLabel, TextButton)
    fields.add_field_method_get("TextColor3", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.text_color))
    });
    fields.add_field_method_set("TextColor3", |_, this, color: Color3| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.text_color = Some(color);
        }
        Ok(())
    });

    // TextSize (TextLabel, TextButton)
    fields.add_field_method_get("TextSize", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.text_size))
    });
    fields.add_field_method_set("TextSize", |_, this, size: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.text_size = Some(size.max(1.0));
        }
        Ok(())
    });

    // TextTransparency (TextLabel, TextButton)
    fields.add_field_method_get("TextTransparency", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.text_transparency))
    });
    fields.add_field_method_set("TextTransparency", |_, this, transparency: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.text_transparency = Some(transparency.clamp(0.0, 1.0));
        }
        Ok(())
    });

    // TextStrokeTransparency (TextLabel, TextButton)
    fields.add_field_method_get("TextStrokeTransparency", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .gui_data
            .as_ref()
            .and_then(|g| g.text_stroke_transparency))
    });
    fields.add_field_method_set("TextStrokeTransparency", |_, this, transparency: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.text_stroke_transparency = Some(transparency.clamp(0.0, 1.0));
        }
        Ok(())
    });

    // Font (TextLabel, TextButton)
    fields.add_field_method_get("Font", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.font.clone()))
    });
    fields.add_field_method_set("Font", |_, this, font: Value| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            if let Value::String(s) = font {
                if let Ok(font_name) = s.to_str() {
                    gui.font = Some(font_name.to_string());
                }
            }
        }
        Ok(())
    });

    // TextScaled (TextLabel, TextButton)
    fields.add_field_method_get("TextScaled", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.text_scaled))
    });
    fields.add_field_method_set("TextScaled", |_, this, scaled: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.text_scaled = scaled;
        }
        Ok(())
    });

    // TextXAlignment (TextLabel, TextButton)
    fields.add_field_method_get("TextXAlignment", |lua, this| {
        let data = this.data.lock().unwrap();
        if let Some(gui) = &data.gui_data {
            let alignment = match gui.text_x_alignment {
                TextXAlignment::Left => "Left",
                TextXAlignment::Center => "Center",
                TextXAlignment::Right => "Right",
            };
            Ok(Value::String(lua.create_string(alignment)?))
        } else {
            Ok(Value::Nil)
        }
    });
    fields.add_field_method_set("TextXAlignment", |_, this, alignment: Value| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            if let Value::String(s) = alignment {
                if let Ok(str_val) = s.to_str() {
                    gui.text_x_alignment = match str_val.as_ref() {
                        "Left" => TextXAlignment::Left,
                        "Right" => TextXAlignment::Right,
                        _ => TextXAlignment::Center,
                    };
                }
            }
        }
        Ok(())
    });

    // TextYAlignment (TextLabel, TextButton)
    fields.add_field_method_get("TextYAlignment", |lua, this| {
        let data = this.data.lock().unwrap();
        if let Some(gui) = &data.gui_data {
            let alignment = match gui.text_y_alignment {
                TextYAlignment::Top => "Top",
                TextYAlignment::Center => "Center",
                TextYAlignment::Bottom => "Bottom",
            };
            Ok(Value::String(lua.create_string(alignment)?))
        } else {
            Ok(Value::Nil)
        }
    });
    fields.add_field_method_set("TextYAlignment", |_, this, alignment: Value| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            if let Value::String(s) = alignment {
                if let Ok(str_val) = s.to_str() {
                    gui.text_y_alignment = match str_val.as_ref() {
                        "Top" => TextYAlignment::Top,
                        "Bottom" => TextYAlignment::Bottom,
                        _ => TextYAlignment::Center,
                    };
                }
            }
        }
        Ok(())
    });

    // Image (ImageLabel, ImageButton)
    fields.add_field_method_get("Image", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.image.clone()))
    });
    fields.add_field_method_set("Image", |_, this, image: String| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.image = Some(image);
        }
        Ok(())
    });

    // ImageColor3 (ImageLabel, ImageButton)
    fields.add_field_method_get("ImageColor3", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.image_color))
    });
    fields.add_field_method_set("ImageColor3", |_, this, color: Color3| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.image_color = Some(color);
        }
        Ok(())
    });

    // ImageTransparency (ImageLabel, ImageButton)
    fields.add_field_method_get("ImageTransparency", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.image_transparency))
    });
    fields.add_field_method_set("ImageTransparency", |_, this, transparency: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.image_transparency = Some(transparency.clamp(0.0, 1.0));
        }
        Ok(())
    });

    // DisplayOrder (ScreenGui)
    fields.add_field_method_get("DisplayOrder", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.display_order))
    });
    fields.add_field_method_set("DisplayOrder", |_, this, order: i32| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.display_order = order;
        }
        Ok(())
    });

    // IgnoreGuiInset (ScreenGui)
    fields.add_field_method_get("IgnoreGuiInset", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.ignore_gui_inset))
    });
    fields.add_field_method_set("IgnoreGuiInset", |_, this, ignore: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.ignore_gui_inset = ignore;
        }
        Ok(())
    });

    // ResetOnSpawn (ScreenGui)
    fields.add_field_method_get("ResetOnSpawn", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().map(|g| g.reset_on_spawn))
    });
    fields.add_field_method_set("ResetOnSpawn", |_, this, reset_on_spawn: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(gui) = &mut data.gui_data {
            gui.reset_on_spawn = reset_on_spawn;
        }
        Ok(())
    });

    // CornerRadius (UICorner)
    fields.add_field_method_get("CornerRadius", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.ui_corner_data.as_ref().map(|c| c.corner_radius))
    });
    fields.add_field_method_set("CornerRadius", |_, this, corner_radius: UDim| {
        let mut data = this.data.lock().unwrap();
        if let Some(ui_corner) = &mut data.ui_corner_data {
            ui_corner.corner_radius = corner_radius;
        }
        Ok(())
    });

    // MouseButton1Click (GuiButton)
    fields.add_field_method_get("MouseButton1Click", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .gui_data
            .as_ref()
            .and_then(|g| g.mouse_button1_click.clone()))
    });

    // MouseButton1Down (GuiButton)
    fields.add_field_method_get("MouseButton1Down", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .gui_data
            .as_ref()
            .and_then(|g| g.mouse_button1_down.clone()))
    });

    // MouseButton1Up (GuiButton)
    fields.add_field_method_get("MouseButton1Up", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .gui_data
            .as_ref()
            .and_then(|g| g.mouse_button1_up.clone()))
    });

    // MouseEnter (GuiButton)
    fields.add_field_method_get("MouseEnter", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.mouse_enter.clone()))
    });

    // MouseLeave (GuiButton)
    fields.add_field_method_get("MouseLeave", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.gui_data.as_ref().and_then(|g| g.mouse_leave.clone()))
    });
}
