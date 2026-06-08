use mlua::UserDataFields;
use std::sync::Arc;

use crate::game::lua::types::CFrame;

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
    /// Compute and cache the relative transform for a WeldConstraint when both parts are set.
    fn compute_weld_constraint_relative_cframe(data: &mut InstanceData) {
        let wc = match &data.weld_constraint_data {
            Some(wc) => wc,
            None => return,
        };
        let part0_ref = wc.part0.as_ref().and_then(|w| w.upgrade());
        let part1_ref = wc.part1.as_ref().and_then(|w| w.upgrade());
        if let (Some(p0), Some(p1)) = (part0_ref, part1_ref) {
            let p0_data = p0.lock().unwrap();
            let p1_data = p1.lock().unwrap();
            if let (Some(p0_part), Some(p1_part)) = (&p0_data.part_data, &p1_data.part_data) {
                let relative_cframe = p0_part.cframe.inverse().multiply(&p1_part.cframe);
                if let Some(wc) = &mut data.weld_constraint_data {
                    wc.relative_cframe = Some(relative_cframe);
                }
            }
        }
    }

    // ========== Weld Properties ==========

    fields.add_field_method_get("Part0", |_, this| {
        let data = this.data.lock().unwrap();
        let from_weld = data
            .weld_data
            .as_ref()
            .and_then(|w| w.part0.as_ref())
            .and_then(|w| w.upgrade())
            .map(Instance::from_ref);
        if from_weld.is_some() {
            return Ok(from_weld);
        }
        Ok(data
            .weld_constraint_data
            .as_ref()
            .and_then(|w| w.part0.as_ref())
            .and_then(|w| w.upgrade())
            .map(Instance::from_ref))
    });
    fields.add_field_method_set("Part0", |_, this, part: Option<Instance>| {
        let mut data = this.data.lock().unwrap();
        let weak = part.map(|p| Arc::downgrade(&p.data));
        if let Some(weld) = &mut data.weld_data {
            weld.part0 = weak.clone();
        }
        if let Some(wc) = &mut data.weld_constraint_data {
            wc.part0 = weak;
            compute_weld_constraint_relative_cframe(&mut data);
        }
        Ok(())
    });

    fields.add_field_method_get("Part1", |_, this| {
        let data = this.data.lock().unwrap();
        let from_weld = data
            .weld_data
            .as_ref()
            .and_then(|w| w.part1.as_ref())
            .and_then(|w| w.upgrade())
            .map(Instance::from_ref);
        if from_weld.is_some() {
            return Ok(from_weld);
        }
        Ok(data
            .weld_constraint_data
            .as_ref()
            .and_then(|w| w.part1.as_ref())
            .and_then(|w| w.upgrade())
            .map(Instance::from_ref))
    });
    fields.add_field_method_set("Part1", |_, this, part: Option<Instance>| {
        let mut data = this.data.lock().unwrap();
        let weak = part.map(|p| Arc::downgrade(&p.data));
        if let Some(weld) = &mut data.weld_data {
            weld.part1 = weak.clone();
        }
        if let Some(wc) = &mut data.weld_constraint_data {
            wc.part1 = weak;
            compute_weld_constraint_relative_cframe(&mut data);
        }
        Ok(())
    });

    fields.add_field_method_get("C0", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.weld_data.as_ref().map(|w| w.c0))
    });
    fields.add_field_method_set("C0", |_, this, cframe: CFrame| {
        let mut data = this.data.lock().unwrap();
        if let Some(weld) = &mut data.weld_data {
            weld.c0 = cframe.orthonormalized();
        }
        Ok(())
    });

    fields.add_field_method_get("C1", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.weld_data.as_ref().map(|w| w.c1))
    });
    fields.add_field_method_set("C1", |_, this, cframe: CFrame| {
        let mut data = this.data.lock().unwrap();
        if let Some(weld) = &mut data.weld_data {
            weld.c1 = cframe.orthonormalized();
        }
        Ok(())
    });

    fields.add_field_method_get("Enabled", |_, this| {
        let data = this.data.lock().unwrap();
        // Return Enabled for Welds, WeldConstraints, BillboardGui, or existing gui_data.enabled
        if let Some(weld) = &data.weld_data {
            return Ok(Some(weld.enabled));
        }
        if let Some(wc) = &data.weld_constraint_data {
            return Ok(Some(wc.enabled));
        }
        if let Some(billboard) = &data.billboard_gui_data {
            return Ok(Some(billboard.enabled));
        }
        if let Some(gui) = &data.gui_data {
            return Ok(Some(gui.enabled));
        }
        Ok(None)
    });
    fields.add_field_method_set("Enabled", |_, this, enabled: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(weld) = &mut data.weld_data {
            weld.enabled = enabled;
        }
        if let Some(wc) = &mut data.weld_constraint_data {
            wc.enabled = enabled;
        }
        if let Some(billboard) = &mut data.billboard_gui_data {
            billboard.enabled = enabled;
        }
        if let Some(gui) = &mut data.gui_data {
            gui.enabled = enabled;
        }
        Ok(())
    });
}
